//! **A viewport's filter evaluated over its tiles' items alone answers exactly as one over the
//! viewer's whole visible set**: for keyword and number leaves, their combinations and a negation,
//! as the filter and as the highlight, for a viewer who sees everything and one who sees a third,
//! at several depths. The fixture is too small for narrowing to pay, so every request is narrowed;
//! the cost rule is tested beside it.

use crate::common;

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, Int32Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use common::*;
use mosaica_build::config::Config;
use mosaica_build::{build, BuildArgs};
use mosaica_engine::filter::{Endpoint, FilterExpr, FilterOperand, Scalar};
use mosaica_engine::{Engine, Session, ViewportRequest};

/// Enough items that a few tiles at depth 6 hold a sliver of them.
const N: u64 = 20_000;

const SCHEMA_TOML: &str = r#"
[[attribute]]
name  = "kw"
type  = "keyword"
index = true

[[attribute]]
name  = "n"
type  = "i32"
index = true
"#;

fn mix(e: u64) -> u64 {
    let mut x = e.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ (x >> 29)
}

/// Item `e`'s keyword: one of thirteen, none on every eleventh item.
fn kw_of(e: u64) -> Option<String> {
    (e % 11 != 4).then(|| format!("k{}", mix(e) % 13))
}

fn n_of(e: u64) -> i32 {
    (mix(e) >> 20) as i32 % 50
}

fn write_points(path: &Path) {
    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("entity_id", DataType::UInt64, false),
        Field::new("x", DataType::Float64, false),
        Field::new("y", DataType::Float64, false),
        Field::new("kw", DataType::Utf8, true),
        Field::new("n", DataType::Int32, false),
    ]));
    let ids: Vec<u64> = (0..N).collect();
    let at = |e: u64, shift: u32| ((mix(e) >> shift) % 1_000_000) as f64 / 1_000.0;
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from(ids.clone())),
            Arc::new(Float64Array::from(
                ids.iter().map(|&e| at(e, 0)).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                ids.iter().map(|&e| at(e, 24)).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                ids.iter().map(|&e| kw_of(e)).collect::<Vec<_>>(),
            )),
            Arc::new(Int32Array::from(
                ids.iter().map(|&e| n_of(e)).collect::<Vec<_>>(),
            )),
        ],
    )
    .unwrap();
    let mut w = ArrowWriter::try_new(File::create(path).unwrap(), schema, None).unwrap();
    w.write(&batch).unwrap();
    w.close().unwrap();
}

fn engine(dir: &Path) -> Engine {
    let points = dir.join("points.parquet");
    let pairs = dir.join("pairs.parquet");
    write_points(&points);
    write_pairs_n(&pairs, N);
    let schema_path = dir.join("schema.toml");
    std::fs::write(&schema_path, SCHEMA_TOML).unwrap();
    let schema = Config::parse(&schema_path, &HashMap::new()).unwrap().schema;
    let bundle = dir.join("bundle");
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
            points,
            &with_id(schema.clone()),
        ),
        out: bundle.clone(),
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
    .expect("the fixture builds");
    let engine = Engine::open(
        &bundle,
        &dir.join("cache"),
        &dir.join("wal.log"),
        config_uncapped(),
    )
    .expect("the engine opens");
    // Every filter is evaluated by the request that asks it, never read from an earlier one.
    engine.set_filter_cache_bytes(0);
    engine
}

fn leaf(column: &str, operand: FilterOperand) -> FilterExpr {
    FilterExpr::Leaf {
        column: column.to_string(),
        operand,
    }
}

fn n_between(lo: i32, hi: i32) -> FilterExpr {
    let at = |v: i32| {
        Some(Endpoint {
            value: Scalar::Int(i128::from(v)),
            inclusive: true,
        })
    };
    leaf(
        "n",
        FilterOperand::Range {
            lo: at(lo),
            hi: at(hi),
        },
    )
}

fn filters() -> Vec<FilterExpr> {
    let kw = |v: &str| leaf("kw", FilterOperand::TextEquals(v.to_string()));
    vec![
        kw("k3"),
        kw("absent"),
        leaf("kw", FilterOperand::TextPrefix("k1".to_string())),
        leaf(
            "kw",
            FilterOperand::TextIn(vec!["k0".into(), "k7".into(), "nothing".into()]),
        ),
        leaf("kw", FilterOperand::TextContains("2".to_string())),
        n_between(10, 30),
        FilterExpr::AllOf(vec![kw("k5"), n_between(0, 24)]),
        FilterExpr::AnyOf(vec![kw("k9"), n_between(45, 49)]),
        FilterExpr::NoneOf(vec![kw("k4")]),
    ]
}

/// Small boxes deep in, a wider one, and the whole map.
fn viewports() -> Vec<(u8, [f64; 4])> {
    vec![
        (6, [500.0, 500.0, 520.0, 515.0]),
        (6, [12.0, 940.0, 40.0, 960.0]),
        (7, [250.0, 250.0, 262.0, 263.0]),
        (7, [990.0, 0.0, 1000.0, 10.0]),
        (3, [100.0, 300.0, 400.0, 500.0]),
        (0, WHOLE_MAP),
    ]
}

/// The response's tiles and points, and how many items its tiles matched.
fn answer(
    engine: &Engine,
    session: &Session,
    req: ViewportRequest<'_>,
    narrow: bool,
) -> (String, u64) {
    engine.set_tile_filters_for_test(narrow);
    engine.set_tile_filters_always_for_test(narrow);
    let out = engine.viewport(session, req).expect("a filtered viewport answers");
    let matched = out.tiles.iter().map(|t| t.matched).sum();
    (format!("{:?}{:?}", out.tiles, out.points), matched)
}

#[test]
fn a_filter_over_the_tiles_items_answers_as_one_over_the_whole_visible_set() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(dir.path());
    let before = engine.tile_filters();
    let mut matched_in_small_boxes = 0;
    for credential in [full_coverage_credential(), subset_credential()] {
        let session = engine.authorise(&credential).unwrap();
        for (zoom, bbox) in viewports() {
            for filter in filters() {
                let request = || ViewportRequest::new("s0", zoom, bbox, 10_000);
                let filtered = |narrow| {
                    answer(&engine, &session, request().filter(filter.clone()), narrow)
                };
                let (narrowed, matched) = filtered(true);
                assert_eq!(
                    narrowed,
                    filtered(false).0,
                    "{filter:?} at depth {zoom} over {bbox:?}"
                );
                if zoom > 0 {
                    matched_in_small_boxes += matched;
                }
                let highlighted = |narrow| {
                    let req = request()
                        .filter(n_between(0, 40))
                        .highlight(filter.clone());
                    answer(&engine, &session, req, narrow)
                };
                assert_eq!(
                    highlighted(true).0,
                    highlighted(false).0,
                    "highlight {filter:?} at depth {zoom} over {bbox:?}"
                );
            }
        }
    }
    assert!(
        engine.tile_filters() > before,
        "the requests were answered from their tiles' items alone"
    );
    assert!(
        matched_in_small_boxes > 0,
        "the small boxes hold items their filters match"
    );
}
