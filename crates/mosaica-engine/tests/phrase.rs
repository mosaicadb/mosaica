//! **A phrase is answered from the text index's word positions, on every path a text layer comes
//! from**: the build's base, a flush's extent, a coalesce's merged extent, a fold's merged base, and
//! each again after a restart.
//!
//! The reference is the corpus's own prose run through the analyser and searched for the phrase's
//! token sequence, never anything read back from the bundle. The prose carries the shapes a
//! position reader can get wrong: a word repeated back to back, a word held more than a run's 128
//! times in one item, a word in more than a block's 128 items, a phrase at an item's start and end,
//! a phrase longer than any item, and items with no words at all.

mod common;

use std::collections::{BTreeMap, HashMap};
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
use mosaica_engine::filter::FilterOperand;
use mosaica_engine::Engine;
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

/// Every probe's answer from the live generation against the analyser's reading of `items`, the
/// live items' prose by entity.
fn assert_phrases(engine: &Engine, items: &BTreeMap<u32, String>, step: &str) {
    let analyser = mosaica_analyse::Analyser::new();
    let generation = engine.generation();
    let columns = &generation.only_shard().filter_columns;
    let mut candidate = croaring::Bitmap::new();
    candidate.add_range(0..1_000_000);
    for probe in probes() {
        let words = analyser.tokens(&probe);
        let want: Vec<u32> = items
            .iter()
            .filter(|(_, prose)| {
                analyser
                    .tokens(prose)
                    .windows(words.len())
                    .any(|w| w == words.as_slice())
            })
            .map(|(entity, _)| *entity)
            .collect();
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
