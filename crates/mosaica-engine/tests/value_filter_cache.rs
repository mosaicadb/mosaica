//! **A value filter's answer is kept across requests, and a kept answer is the one an evaluation
//! would give**: for each grant, for each leaf of a combined filter, after a suppression, a lift, a
//! deletion, a commit window and a flush, with the cache off, and after a restart.
//!
//! The reference is each item's values as the fixture wrote them, never anything read back from
//! the bundle. One column is empty on a third of the items, so its scan reads a presence bitmap.
//!
//! Every filter here is answered over the whole map, so beside its leaves it keeps its rows in the
//! view, one more entry built once per filter and grant and read by every repeat.

mod common;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, UInt32Array, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use common::*;
use mosaica_build::config::Config;
use mosaica_build::{build, BuildArgs};
use mosaica_engine::filter::{Endpoint, FilterExpr, FilterOperand, Scalar};
use mosaica_engine::{CacheStats, Engine, Session, ViewportRequest};
use mosaica_lifecycle::wal::{ChangeOp, WalScalar};
use mosaica_lifecycle::UnallocatedRow;
use mosaica_types::EntityId;

const N: u64 = 400;

const SCHEMA_TOML: &str = r#"
[[attribute]]
name  = "n"
type  = "u32"
index = true

[[attribute]]
name  = "m"
type  = "f64"
index = true
"#;

/// Item `source`'s `n`: none on every third item, a third the subset grant does not see.
fn n_of(source: u64) -> Option<u32> {
    (source % 3 != 1).then_some((source % 100) as u32)
}

/// Item `source`'s `m`.
fn m_of(source: u64) -> f64 {
    ((source * 7) % 50) as f64
}

/// The two values an item holds.
#[derive(Clone, Copy)]
struct Values {
    n: Option<u32>,
    m: f64,
}

fn values_of(source: u64) -> Values {
    Values {
        n: n_of(source),
        m: m_of(source),
    }
}

fn write_points(path: &Path) {
    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("entity_id", DataType::UInt64, false),
        Field::new("x", DataType::Float64, false),
        Field::new("y", DataType::Float64, false),
        Field::new("n", DataType::UInt32, true),
        Field::new("m", DataType::Float64, false),
    ]));
    let ids: Vec<u64> = (0..N).collect();
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from(ids.clone())),
            Arc::new(Float64Array::from(
                ids.iter()
                    .map(|e| ((e * 37) % 500) as f64)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                ids.iter()
                    .map(|e| ((e * 53) % 500) as f64)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt32Array::from(
                ids.iter().map(|e| n_of(*e)).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                ids.iter().map(|e| m_of(*e)).collect::<Vec<_>>(),
            )),
        ],
    )
    .unwrap();
    let mut w = ArrowWriter::try_new(File::create(path).unwrap(), schema, None).unwrap();
    w.write(&batch).unwrap();
    w.close().unwrap();
}

fn build_fixture(out: &Path, tmp: &Path) {
    let points = tmp.join("points.parquet");
    let pairs = tmp.join("pairs.parquet");
    write_points(&points);
    write_pairs_n(&pairs, N);
    let schema_path = tmp.join("schema.toml");
    std::fs::write(&schema_path, SCHEMA_TOML).unwrap();
    let schema = Config::parse(&schema_path, &HashMap::new())
        .expect("the numeric schema parses")
        .schema;
    build(&BuildArgs {
        views: vec![mosaica_build::ViewArgs {
            visibility: None,
            view_id: "s0".to_string(),
            title: None,
            projection: mosaica_spatial::Projection::None,
            extent: extent(),
            points: Some(points.clone()),
            point_fields: Default::default(),
            select: None,
            access: mosaica_build::config::AccessInput::relation(pairs),
        }],
        anchor: 0,
        groups: Vec::new(),
        scoped_attributes: Vec::new(),
        attribute_sources: mosaica_build::config::AttributeSource::over(
            points.clone(),
            &with_id(schema.clone()),
        ),
        out: out.to_path_buf(),
        limit: None,
        shard_size: None,
        strict: false,
        identity_key: test_key(),
        layers: Vec::new(),
        layer_inputs: Vec::new(),
        scoped_layers: Default::default(),
        emit_oracle_pairs: false,
        batch_items: None,
        memory_budget: None,
        band_rows: None,
        schema: with_id(schema),
    })
    .expect("a bundle with indexed numeric columns builds");
}

fn engine_over(tmp: &Path, root: &Path) -> Engine {
    let mut engine = Engine::open(
        root,
        &tmp.join("cache"),
        &tmp.join("wal.log"),
        config_uncapped(),
    )
    .expect("the engine opens against the numeric fixture");
    engine.start_write_executor(8).expect("the executor starts");
    engine.set_background_refresh_for_test(false);
    engine
}

/// Ingest the item of `source` and wait for the commit window that buffers it.
fn ingest_one(engine: &Engine, source: u64) -> EntityId {
    let buffered = engine.write_executor_stats().buffered_items;
    let values = values_of(source);
    let row = UnallocatedRow {
        view: "s0".to_string(),
        join: None,
        descriptors: vec![b"0".to_vec()],
        x: 10.0,
        y: 10.0,
        scalars: vec![
            values.n.map_or(WalScalar::Null, WalScalar::U32),
            WalScalar::F64(values.m),
        ],
        terms: engine.resolve_terms(&[b"0".to_vec()]),
        scoped: Vec::new(),
    };
    let entity = engine
        .ingest_rows(vec![row], format!("fresh-{source}"), [0u8; 32])
        .expect("an ingest carrying numbers is accepted")[0];
    wait_until(
        "the commit window's publication",
        std::time::Duration::from_secs(60),
        || engine.write_executor_stats().buffered_items > buffered,
    );
    entity
}

/// Wait for a flush of everything buffered.
fn flush(engine: &Engine) {
    let before = engine.write_executor_stats().flushes;
    engine.request_flush();
    wait_until(
        "the flush's publication",
        std::time::Duration::from_secs(60),
        || engine.write_executor_stats().flushes > before,
    );
}

/// `[lo, hi]` over `column`.
fn range(column: &str, lo: f64, hi: f64) -> FilterExpr {
    let at = |v| {
        Some(Endpoint {
            value: Scalar::Float(v),
            inclusive: true,
        })
    };
    FilterExpr::Leaf {
        column: column.to_string(),
        operand: FilterOperand::Range {
            lo: at(lo),
            hi: at(hi),
        },
    }
}

/// `n` in `[10, 59]`.
fn n_range() -> FilterExpr {
    range("n", 10.0, 59.0)
}

/// `m` in `[0, 24]`.
fn m_range() -> FilterExpr {
    range("m", 0.0, 24.0)
}

fn in_n(v: Values) -> bool {
    v.n.is_some_and(|n| (10..=59).contains(&n))
}

fn in_m(v: Values) -> bool {
    (0.0..=24.0).contains(&v.m)
}

/// How many of `items` satisfy `keep`.
fn count(items: &BTreeMap<u32, Values>, keep: impl Fn(Values) -> bool) -> u64 {
    items.values().filter(|v| keep(**v)).count() as u64
}

/// How many items a whole-map viewport filtered by `filter` matches for `session`.
fn matched(engine: &Engine, session: &Session, filter: FilterExpr) -> u64 {
    let request = ViewportRequest::new("s0", 0, WHOLE_MAP, 2 * N as usize).filter(filter);
    engine
        .viewport(session, request)
        .expect("a filtered viewport answers")
        .tiles
        .iter()
        .map(|t| t.matched)
        .sum()
}

/// The built items' values by entity, and the source each entity was built from.
fn built_items(root: &Path) -> (BTreeMap<u32, Values>, BTreeMap<u32, u64>) {
    let sources: BTreeMap<u32, u64> = source_to_new_map(root, "v00000")
        .into_iter()
        .map(|(source, entity)| (entity as u32, source))
        .collect();
    let items = sources
        .iter()
        .map(|(entity, source)| (*entity, values_of(*source)))
        .collect();
    (items, sources)
}

fn cache(engine: &Engine) -> CacheStats {
    engine.filter_cache_stats()
}

/// **A repeated range, alone and in a combined filter, is answered from the cache with the items
/// an evaluation gives**, each leaf of the combined filter is kept, and a restart answers alike
/// from an empty cache.
#[test]
fn a_repeated_value_filter_is_answered_from_the_cache_with_the_same_items() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (items, _) = built_items(&root);
    let both = FilterExpr::AllOf(vec![n_range(), m_range()]);
    let either = FilterExpr::AnyOf(vec![n_range(), m_range()]);
    let neither = FilterExpr::NoneOf(vec![n_range(), m_range()]);
    let filters = [
        (both.clone(), count(&items, |v| in_n(v) && in_m(v))),
        (either.clone(), count(&items, |v| in_n(v) || in_m(v))),
        (neither.clone(), count(&items, |v| !in_n(v) && !in_m(v))),
        (m_range(), count(&items, in_m)),
    ];
    assert!(filters.iter().all(|(_, want)| *want > 0 && *want < N));

    let engine = engine_over(tmp.path(), &root);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    assert_eq!(matched(&engine, &session, both.clone()), filters[0].1);
    let first = cache(&engine);
    assert_eq!(
        (first.misses, first.entries),
        (3, 3),
        "each leaf of a conjunction is evaluated once and kept, the narrowed one too, beside the \
         conjunction's rows"
    );
    for (filter, want) in &filters {
        let again = engine.authorise(&full_coverage_credential()).unwrap();
        assert_eq!(
            matched(&engine, &again, filter.clone()),
            *want,
            "{filter:?}"
        );
    }
    let after = cache(&engine);
    assert_eq!(
        after.misses,
        first.misses + 3,
        "no leaf is evaluated again, and each other filter keeps its rows"
    );
    assert!(after.hits > first.hits, "every leaf is read from the cache");

    drop(engine);
    let engine = engine_over(tmp.path(), &root);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    for (filter, want) in &filters {
        assert_eq!(
            matched(&engine, &session, filter.clone()),
            *want,
            "{filter:?}"
        );
    }
    let restarted = cache(&engine);
    assert_eq!(
        (restarted.misses, restarted.entries),
        (6, 6),
        "a restart keeps nothing and evaluates each leaf and keeps each filter's rows again"
    );
}

/// **Two grants never answer from each other's entries**, in either order and on a repeat.
#[test]
fn a_value_filter_answers_each_grant_its_own_items() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (items, sources) = built_items(&root);
    let all = count(&items, in_n);
    let subset: BTreeMap<u32, Values> = items
        .iter()
        .filter(|(entity, _)| subset_sees(sources[entity]))
        .map(|(entity, v)| (*entity, *v))
        .collect();
    let part_want = count(&subset, in_n);
    assert!(part_want > 0 && part_want < all);

    let engine = engine_over(tmp.path(), &root);
    let full = engine.authorise(&full_coverage_credential()).unwrap();
    let part = engine.authorise(&subset_credential()).unwrap();
    let none = engine.authorise(&zero_credential()).unwrap();
    for _ in 0..2 {
        assert_eq!(matched(&engine, &part, n_range()), part_want);
        assert_eq!(matched(&engine, &full, n_range()), all);
        assert_eq!(matched(&engine, &none, n_range()), 0);
    }
    let stats = cache(&engine);
    assert_eq!(stats.misses, 6, "each grant is evaluated once and keeps its rows");
    assert_eq!(
        stats.hits,
        6,
        "each grant builds its rows from its own leaf, and reads its own rows after"
    );
}

/// **A suppression, a lift, a deletion and a commit window each reach the next request from the
/// kept answer, and a flush evaluates it again** with the flushed item in it.
#[test]
fn a_change_reaches_the_next_value_filter_after_a_kept_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (mut items, _) = built_items(&root);
    let engine = engine_over(tmp.path(), &root);
    engine.set_merge_for_test(false);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    let answer = || matched(&engine, &session, n_range());
    let answer_kept = |step: &str| {
        let before = cache(&engine);
        let got = answer();
        let after = cache(&engine);
        assert_eq!(
            after.misses, before.misses,
            "{step} is answered from the kept entry"
        );
        assert!(
            after.hits > before.hits,
            "{step} is answered from the kept entry"
        );
        got
    };

    let held = count(&items, in_n);
    let mut in_range = items.iter().filter(|(_, v)| in_n(**v)).map(|(e, _)| *e);
    let (suppressed, deleted) = (in_range.next().unwrap(), in_range.next().unwrap());
    assert_eq!(answer(), held);

    engine
        .accept_change(EntityId::new(u64::from(suppressed)), ChangeOp::Suppress)
        .expect("a suppression is accepted");
    assert_eq!(answer_kept("a suppression"), held - 1);

    engine
        .accept_change(EntityId::new(u64::from(suppressed)), ChangeOp::Unsuppress)
        .expect("a lift is accepted");
    assert_eq!(answer_kept("a lift"), held);

    engine
        .accept_change(EntityId::new(u64::from(deleted)), ChangeOp::Delete)
        .expect("a deletion is accepted");
    items.remove(&deleted);
    assert_eq!(answer_kept("a deletion"), count(&items, in_n));

    let source = (N..N + 1_000)
        .find(|source| in_n(values_of(*source)))
        .expect("some source past the build is in the range");
    let entity = ingest_one(&engine, source);
    assert_eq!(
        answer_kept("a commit window"),
        count(&items, in_n),
        "a buffered item has no value to match"
    );

    flush(&engine);
    items.insert(entity.raw() as u32, values_of(source));
    // With the background refresh off, a session already served the view is served its previous
    // geometry, which has no row for the item; a session not yet served it sees the item.
    let fresh = engine.authorise(&full_coverage_credential()).unwrap();
    let before = cache(&engine);
    assert_eq!(
        matched(&engine, &fresh, n_range()),
        count(&items, in_n),
        "a flushed item is matched once it is visible"
    );
    assert_eq!(
        cache(&engine).misses,
        before.misses + 2,
        "a flush evaluates the leaf and keeps its rows again"
    );
    assert_eq!(matched(&engine, &fresh, n_range()), count(&items, in_n));
}

/// **A bound of zero keeps nothing and answers as the cache does.**
#[test]
fn a_zero_bound_keeps_no_value_filter_and_answers_alike() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (items, _) = built_items(&root);
    let engine = engine_over(tmp.path(), &root);
    engine.set_filter_cache_bytes(0);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    let both = FilterExpr::AllOf(vec![n_range(), m_range()]);
    for _ in 0..2 {
        assert_eq!(matched(&engine, &session, n_range()), count(&items, in_n));
        assert_eq!(
            matched(&engine, &session, both.clone()),
            count(&items, |v| in_n(v) && in_m(v))
        );
    }
    let stats = cache(&engine);
    assert_eq!((stats.entries, stats.hits, stats.misses), (0, 0, 0));
}
