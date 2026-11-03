//! **A phrase is answered from the text index's word positions, on every path a text layer comes
//! from**: the build's base, a flush's extent, a coalesce's merged extent, a fold's merged base, and
//! each again after a restart.
//!
//! The reference is the corpus's own prose run through the analyser and searched for the phrase's
//! token sequence, never anything read back from the bundle. The prose carries the shapes a
//! position reader can get wrong: a word repeated back to back, a word held more than a run's 128
//! times in one item, a word in more than a block's 128 items, a phrase at an item's start and end,
//! a phrase longer than any item, and items with no words at all.
//!
//! A text leaf's answer is kept across requests, and the tests at the end show a kept answer is
//! the one an evaluation would give: for each grant, after a suppression, a lift, a deletion and an
//! ingest, with the cache off, and after a restart.

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use common::*;
use mosaica_build::config::Config;
use mosaica_build::{build, BuildArgs};
use mosaica_engine::filter::{FilterExpr, FilterOperand};
use mosaica_engine::{CacheStats, Engine, Session, ViewportRequest};
use mosaica_lifecycle::wal::{ChangeOp, WalScalar};
use mosaica_lifecycle::UnallocatedRow;
use mosaica_types::EntityId;

/// Built items: enough that the commonest words are held by several blocks' worth of items with a
/// partial block last.
const N: u64 = 400;

/// Ingested items, one flush each: the coalesce's default window, so the pass fires.
const FLUSHES: u64 = 8;

const SCHEMA_TOML: &str = r#"
[[attribute]]
name     = "prose"
type     = "text"
index    = true
analyser = "unicode"
"#;

const WORDS: [&str; 8] = [
    "the", "quick", "brown", "fox", "had", "of", "deep", "learning",
];

/// Item `source`'s prose.
fn prose_of(source: u64) -> String {
    match source {
        0 => "had had had had".to_string(),
        1 => format!("{}di", "la ".repeat(300)),
        2 => String::new(),
        3 => "?!".to_string(),
        // An ingested item holding a word more than a run's times, in an extent.
        n if n == N + 3 => format!("{}la di", "la di ".repeat(140)),
        _ => {
            let mut x = source.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let words = 3 + source % 11;
            (0..words)
                .map(|_| {
                    x = x
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    WORDS[(x >> 61) as usize]
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

/// The phrases asked after every step.
fn probes() -> Vec<String> {
    let mut probes: Vec<String> = [
        "the quick",
        "quick brown",
        "brown fox",
        "fox brown",
        "deep learning",
        "of the",
        "the the",
        "had had",
        "had had had",
        "had had had had had",
        "la la",
        "la di",
        "di la",
        "la la la la la la la la la la la la la la la la la la la la di",
        "the quick brown",
        "fox zzzz",
        "learning",
    ]
    .map(str::to_string)
    .to_vec();
    probes.push(vec!["the"; 20].join(" "));
    probes
}

fn write_points(path: &Path) {
    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("entity_id", DataType::UInt64, false),
        Field::new("x", DataType::Float64, false),
        Field::new("y", DataType::Float64, false),
        Field::new("prose", DataType::Utf8, true),
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
            Arc::new(StringArray::from(
                ids.iter().map(|e| Some(prose_of(*e))).collect::<Vec<_>>(),
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
        .expect("the text schema parses")
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
    .expect("a bundle with an indexed text column builds");
}

fn engine_over(tmp: &Path, root: &Path) -> Engine {
    let mut engine = Engine::open(
        root,
        &tmp.join("cache"),
        &tmp.join("wal.log"),
        config_uncapped(),
    )
    .expect("the engine opens against the text fixture");
    engine.start_write_executor(8).expect("the executor starts");
    engine.set_background_refresh_for_test(false);
    engine
}

/// Ingest the item of `source` and wait for the flush that publishes it.
fn ingest_and_flush(engine: &Engine, source: u64) -> EntityId {
    let row = UnallocatedRow {
        view: "s0".to_string(),
        join: None,
        descriptors: vec![b"0".to_vec()],
        x: 10.0,
        y: 10.0,
        scalars: vec![WalScalar::Utf8(prose_of(source))],
        terms: engine.resolve_terms(&[b"0".to_vec()]),
        scoped: Vec::new(),
    };
    let entity = engine
        .ingest_rows(vec![row], format!("fresh-{source}"), [0u8; 32])
        .expect("an ingest carrying prose is accepted")[0];
    let before = engine.write_executor_stats().flushes;
    engine.request_flush();
    wait_until(
        "the flush's publication",
        std::time::Duration::from_secs(60),
        || engine.write_executor_stats().flushes > before,
    );
    entity
}

fn live_text_layers(engine: &Engine) -> usize {
    engine
        .generation()
        .only_shard()
        .filter_columns
        .text_layer_count("prose")
        .expect("prose is a text column")
}

/// The entities of `items`, the live items' prose by entity, whose prose says `phrase`'s words
/// adjacent and in order, by the analyser's reading.
fn saying(items: &BTreeMap<u32, String>, phrase: &str) -> BTreeSet<u32> {
    let analyser = mosaica_analyse::Analyser::new();
    let words = analyser.tokens(phrase);
    items
        .iter()
        .filter(|(_, prose)| {
            analyser
                .tokens(prose)
                .windows(words.len())
                .any(|w| w == words.as_slice())
        })
        .map(|(entity, _)| *entity)
        .collect()
}

/// The entities of `items` whose prose holds at least `minimum` of `query`'s distinct words.
fn holding(items: &BTreeMap<u32, String>, query: &str, minimum: usize) -> BTreeSet<u32> {
    let analyser = mosaica_analyse::Analyser::new();
    let words: BTreeSet<String> = analyser.tokens(query).into_iter().collect();
    items
        .iter()
        .filter(|(_, prose)| {
            let held: BTreeSet<String> = analyser.tokens(prose).into_iter().collect();
            words.intersection(&held).count() >= minimum
        })
        .map(|(entity, _)| *entity)
        .collect()
}

/// Every probe's answer from the live generation against the analyser's reading of `items`, the
/// live items' prose by entity.
fn assert_phrases(engine: &Engine, items: &BTreeMap<u32, String>, step: &str) {
    let generation = engine.generation();
    let columns = &generation.only_shard().filter_columns;
    let mut candidate = croaring::Bitmap::new();
    candidate.add_range(0..1_000_000);
    for probe in probes() {
        let want: Vec<u32> = saying(items, &probe).into_iter().collect();
        let got: Vec<u32> = columns
            .resolve(
                "prose",
                &FilterOperand::Phrase {
                    query: probe.clone(),
                },
                &candidate,
            )
            .expect("a phrase answers")
            .iter()
            .collect();
        assert_eq!(got, want, "{step}: phrase {probe:?}");
    }
}

/// **The phrase answers what the prose says after the build, each flush, the coalesce, a fold that
/// deletes matching items, and a restart.**
#[test]
fn a_phrase_answers_what_the_prose_says_through_every_lifecycle_step() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let engine = engine_over(tmp.path(), &root);
    engine.set_merge_for_test(false);

    let mut items: BTreeMap<u32, String> = source_to_new_map(&root, "v00000")
        .into_iter()
        .map(|(source, entity)| (entity as u32, prose_of(source)))
        .collect();
    let analyser = mosaica_analyse::Analyser::new();
    let holding_the = items
        .values()
        .filter(|p| analyser.tokens(p).iter().any(|t| t == "the"))
        .count();
    assert!(
        holding_the > 2 * 128 && holding_the % 128 != 0,
        "`the` is held by several blocks of items and a partial one: {holding_the}"
    );
    assert_phrases(&engine, &items, "the build");

    let mut ingested = Vec::new();
    for source in N..N + FLUSHES {
        let entity = ingest_and_flush(&engine, source);
        items.insert(entity.raw() as u32, prose_of(source));
        ingested.push(entity);
        if source == N + 1 {
            assert!(
                live_text_layers(&engine) > 2,
                "each flush is a layer of its own"
            );
            assert_phrases(&engine, &items, "two flushes");
        }
    }
    tick_until(
        &engine,
        "the text layers to coalesce",
        std::time::Duration::from_secs(60),
        || live_text_layers(&engine) < 1 + FLUSHES as usize,
    );
    assert_phrases(&engine, &items, "the coalesce");

    drop(engine);
    let engine = engine_over(tmp.path(), &root);
    assert_phrases(&engine, &items, "a restart over the coalesced extent");

    // Items the probes match, from the base and from the extents.
    let matching: Vec<u32> = items
        .iter()
        .filter(|(_, prose)| {
            analyser
                .tokens(prose)
                .windows(2)
                .any(|w| w == ["the", "quick"])
        })
        .map(|(entity, _)| *entity)
        .step_by(2)
        .chain([ingested[3].raw() as u32])
        .collect();
    assert!(matching.len() > 3);
    for entity in &matching {
        engine
            .accept_change(EntityId::new(u64::from(*entity)), ChangeOp::Delete)
            .expect("a deletion is accepted");
        items.remove(entity);
    }
    fold(&engine);
    assert_eq!(
        live_text_layers(&engine),
        1,
        "the fold leaves the base alone"
    );
    assert_phrases(&engine, &items, "the fold");

    drop(engine);
    let engine = engine_over(tmp.path(), &root);
    assert_phrases(&engine, &items, "a restart over the folded base");
}

fn phrase(query: &str) -> FilterExpr {
    FilterExpr::Leaf {
        column: "prose".to_string(),
        operand: FilterOperand::Phrase {
            query: query.to_string(),
        },
    }
}

fn matching(query: &str, minimum: Option<u32>) -> FilterExpr {
    FilterExpr::Leaf {
        column: "prose".to_string(),
        operand: FilterOperand::Match {
            query: query.to_string(),
            minimum,
        },
    }
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

/// The built items' prose by entity, and the source each entity was built from.
fn built_items(root: &Path) -> (BTreeMap<u32, String>, BTreeMap<u32, u64>) {
    let sources: BTreeMap<u32, u64> = source_to_new_map(root, "v00000")
        .into_iter()
        .map(|(source, entity)| (entity as u32, source))
        .collect();
    let items = sources
        .iter()
        .map(|(entity, source)| (*entity, prose_of(*source)))
        .collect();
    (items, sources)
}

fn cache(engine: &Engine) -> CacheStats {
    engine.text_filter_cache_stats()
}

/// **A repeated `match` or `phrase` is answered from the cache with the items an evaluation
/// gives**, an m-of-n `match` apart from an all-words one, and a restart answers alike from an
/// empty cache.
#[test]
fn a_repeated_text_filter_is_answered_from_the_cache_with_the_same_items() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (items, _) = built_items(&root);
    let filters = [
        (phrase("the quick"), saying(&items, "the quick")),
        (
            matching("deep learning fox", None),
            holding(&items, "deep learning fox", 3),
        ),
        (
            matching("deep learning fox", Some(2)),
            holding(&items, "deep learning fox", 2),
        ),
    ];
    assert_ne!(
        filters[1].1, filters[2].1,
        "two of three words match items all three do not"
    );

    let engine = engine_over(tmp.path(), &root);
    for (filter, want) in &filters {
        let before = cache(&engine);
        let session = engine.authorise(&full_coverage_credential()).unwrap();
        assert_eq!(matched(&engine, &session, filter.clone()), want.len() as u64);
        let first = cache(&engine);
        assert_eq!(first.misses, before.misses + 1, "{filter:?} is evaluated once");
        assert_eq!(first.entries, before.entries + 1, "{filter:?} is kept apart");

        let again = engine.authorise(&full_coverage_credential()).unwrap();
        assert_eq!(matched(&engine, &again, filter.clone()), want.len() as u64);
        let second = cache(&engine);
        assert_eq!(second.misses, first.misses, "{filter:?} is not evaluated again");
        assert!(second.hits > first.hits, "{filter:?} is read from the cache");
    }

    drop(engine);
    let engine = engine_over(tmp.path(), &root);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    for (filter, want) in &filters {
        assert_eq!(matched(&engine, &session, filter.clone()), want.len() as u64);
    }
    let restarted = cache(&engine);
    assert_eq!(
        (restarted.hits, restarted.misses),
        (0, 3),
        "a restart keeps nothing"
    );
}

/// **Two grants never answer from each other's entries**, in either order and on a repeat.
#[test]
fn a_text_filter_answers_each_grant_its_own_items() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (items, sources) = built_items(&root);
    let all = saying(&items, "the quick");
    let subset: BTreeSet<u32> = all
        .iter()
        .copied()
        .filter(|entity| subset_sees(sources[entity]))
        .collect();
    assert!(!subset.is_empty() && subset.len() < all.len());

    let engine = engine_over(tmp.path(), &root);
    let full = engine.authorise(&full_coverage_credential()).unwrap();
    let part = engine.authorise(&subset_credential()).unwrap();
    let none = engine.authorise(&zero_credential()).unwrap();
    for _ in 0..2 {
        assert_eq!(
            matched(&engine, &part, phrase("the quick")),
            subset.len() as u64
        );
        assert_eq!(matched(&engine, &full, phrase("the quick")), all.len() as u64);
        assert_eq!(matched(&engine, &none, phrase("the quick")), 0);
    }
    let stats = cache(&engine);
    assert_eq!(stats.misses, 3, "each grant is evaluated once");
    assert_eq!(stats.hits, 3, "and read from its own entry after");
}

/// **A suppression, a lift, a deletion and an ingest each reach the next request** with a text
/// filter whose answer the cache held before it.
#[test]
fn a_change_reaches_the_next_text_filter_after_a_kept_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (mut items, _) = built_items(&root);
    let engine = engine_over(tmp.path(), &root);
    engine.set_merge_for_test(false);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    let answer = || matched(&engine, &session, phrase("the quick"));
    let want = |items: &BTreeMap<u32, String>| saying(items, "the quick").len() as u64;

    let held = want(&items);
    let mut saying_it = saying(&items, "the quick").into_iter();
    let (suppressed, deleted) = (saying_it.next().unwrap(), saying_it.next().unwrap());
    assert_eq!(answer(), held);
    assert_eq!(answer(), held);
    assert!(cache(&engine).hits > 0, "the answer is kept");

    engine
        .accept_change(EntityId::new(u64::from(suppressed)), ChangeOp::Suppress)
        .expect("a suppression is accepted");
    assert_eq!(answer(), held - 1, "a suppression reaches the next request");
    assert_eq!(answer(), held - 1);

    engine
        .accept_change(EntityId::new(u64::from(suppressed)), ChangeOp::Unsuppress)
        .expect("a lift is accepted");
    assert_eq!(answer(), held, "a lift reaches the next request");

    engine
        .accept_change(EntityId::new(u64::from(deleted)), ChangeOp::Delete)
        .expect("a deletion is accepted");
    items.remove(&deleted);
    assert_eq!(answer(), want(&items), "a deletion reaches the next request");

    let source = (N + FLUSHES..N + 1_000)
        .find(|source| !saying(&BTreeMap::from([(0, prose_of(*source))]), "the quick").is_empty())
        .expect("some source past the build says the phrase");
    let entity = ingest_and_flush(&engine, source);
    items.insert(entity.raw() as u32, prose_of(source));
    // With the background refresh off, a session already served the view is served its previous
    // geometry, which has no row for the item; a session not yet served it sees the item.
    let fresh = engine.authorise(&full_coverage_credential()).unwrap();
    let answer = || matched(&engine, &fresh, phrase("the quick"));
    assert_eq!(
        answer(),
        want(&items),
        "an ingested item is matched once it is visible"
    );
    assert_eq!(answer(), want(&items));
}

/// **A bound of zero keeps nothing and answers as the cache does.**
#[test]
fn a_zero_bound_keeps_no_text_filter_and_answers_alike() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(&root, tmp.path());
    let (items, _) = built_items(&root);
    let engine = engine_over(tmp.path(), &root);
    engine.set_text_filter_cache_bytes(0);
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    for _ in 0..2 {
        assert_eq!(
            matched(&engine, &session, phrase("the quick")),
            saying(&items, "the quick").len() as u64
        );
        assert_eq!(
            matched(&engine, &session, matching("deep fox", Some(1))),
            holding(&items, "deep fox", 1).len() as u64
        );
    }
    let stats = cache(&engine);
    assert_eq!((stats.entries, stats.hits, stats.misses), (0, 0, 0));
}
