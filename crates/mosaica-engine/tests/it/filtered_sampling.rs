//! Which points a filtered map draws: a filtered tile thins by density as an unfiltered one does,
//! anchored on how many of the viewer's visible items the filter admits.
//!
//! The fixture puts three quarters of its items in one corner and spreads the rest over the whole
//! extent, so the tiles of one zoom hold very different numbers of matches. Each viewer's expected
//! sample is worked out from what its own responses report: the matched count summed over every
//! tile, the occupied tiles of its unfiltered map, and every match's `mosaica_id`, read from an
//! engine whose threshold admits everything.

use crate::common;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, StringArray, UInt32Array, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use tempfile::TempDir;

use common::*;
use mosaica_build::config::Config;
use mosaica_build::{build, BuildArgs};
use mosaica_engine::filter::{FilterExpr, FilterOperand};
use mosaica_engine::{Engine, EngineConfig, Session, ViewportOut, ViewportRequest};
use mosaica_lifecycle::wal::{ChangeOp, Wal, WalRecord};
use mosaica_types::AttrLocalId;

const ITEMS: u64 = 8_000;
/// θ's target marks per occupied tile in the live engine.
const TARGET: u64 = 32;
const K_MIN: usize = 2;
/// Above every tile's count, so only a request's own `k` caps a tile.
const CAP: usize = 10_000;

const SCHEMA_TOML: &str = r#"
[[vocabulary]]
name       = "kind"
width      = "u8"
value_set  = "closed"
visibility = "public"
  [vocabulary.values]
  common = 1
  other = 2
  rare = 3

[[attribute]]
name       = "kind"
type       = "category"
render     = true
index      = true
vocabulary = "kind"
"#;

/// Three items in four sit in the corner `[0, 400)²`; the rest spread over `[0, 1000)²`.
fn position(e: u64) -> (f64, f64) {
    if e.is_multiple_of(4) {
        (((e * 37) % 1000) as f64, ((e * 53) % 1000) as f64)
    } else {
        (((e * 37) % 400) as f64, ((e * 53) % 400) as f64)
    }
}

fn kind_of(e: u64) -> &'static str {
    if e % 997 == 5 {
        "rare"
    } else if e.is_multiple_of(3) {
        "common"
    } else {
        "other"
    }
}

/// Every item carries term 0; term 2 and term 3 split the items between two viewers.
fn viewer_term(e: u64) -> u32 {
    if (e / 5).is_multiple_of(2) {
        2
    } else {
        3
    }
}

fn credential(term: u32) -> Vec<u8> {
    format!("{{\"terms\": [\"{term}\"]}}").into_bytes()
}

fn write_parquet(path: &Path, batch: RecordBatch) {
    let mut w = ArrowWriter::try_new(File::create(path).unwrap(), batch.schema(), None).unwrap();
    w.write(&batch).unwrap();
    w.close().unwrap();
}

struct Fixture {
    tmp: TempDir,
    bundle: std::path::PathBuf,
    codes: HashMap<String, u32>,
}

fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let points = tmp.path().join("points.parquet");
    let pairs = tmp.path().join("pairs.parquet");
    let ids: Vec<u64> = (0..ITEMS).collect();
    write_parquet(
        &points,
        RecordBatch::try_new(
            Arc::new(ArrowSchema::new(vec![
                Field::new("entity_id", DataType::UInt64, false),
                Field::new("x", DataType::Float64, false),
                Field::new("y", DataType::Float64, false),
                Field::new("kind", DataType::Utf8, false),
            ])),
            vec![
                Arc::new(UInt64Array::from(ids.clone())),
                Arc::new(Float64Array::from_iter_values(
                    ids.iter().map(|&e| position(e).0),
                )),
                Arc::new(Float64Array::from_iter_values(
                    ids.iter().map(|&e| position(e).1),
                )),
                Arc::new(StringArray::from_iter_values(
                    ids.iter().map(|&e| kind_of(e)),
                )),
            ],
        )
        .unwrap(),
    );
    let (entities, terms): (Vec<u64>, Vec<u32>) = ids
        .iter()
        .flat_map(|&e| [(e, 0), (e, viewer_term(e))])
        .unzip();
    write_parquet(
        &pairs,
        RecordBatch::try_new(
            Arc::new(ArrowSchema::new(vec![
                Field::new("entity_id", DataType::UInt64, false),
                Field::new("term_id", DataType::UInt32, false),
            ])),
            vec![
                Arc::new(UInt64Array::from(entities)),
                Arc::new(UInt32Array::from(terms)),
            ],
        )
        .unwrap(),
    );
    let schema_path = tmp.path().join("schema.toml");
    std::fs::write(&schema_path, SCHEMA_TOML).unwrap();
    let schema = with_id(Config::parse(&schema_path, &HashMap::new()).unwrap().schema);
    let bundle = tmp.path().join("bundle");
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
        attribute_sources: mosaica_build::config::AttributeSource::over(points, &schema),
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
        schema,
    })
    .expect("the fixture builds");
    let codes = mosaica_store::read::open_bundle(&bundle)
        .unwrap()
        .manifest
        .vocabularies
        .iter()
        .find(|v| v.name == "kind")
        .expect("the manifest records the vocabulary")
        .values
        .iter()
        .map(|v| (v.key.clone(), v.code))
        .collect();
    Fixture { tmp, bundle, codes }
}

impl Fixture {
    fn kind(&self, key: &str) -> FilterExpr {
        FilterExpr::Leaf {
            column: "kind".to_string(),
            operand: FilterOperand::Equals(AttrLocalId::new(self.codes[key])),
        }
    }

    /// An engine whose threshold is live at `TARGET`, over `wal`.
    fn live(&self, wal: &str) -> Engine {
        self.open(wal, TARGET)
    }

    /// An engine whose threshold admits every item, so a tile serves all it matches.
    fn everything(&self) -> Engine {
        self.open("everything", ITEMS * 2)
    }

    fn open(&self, name: &str, target: u64) -> Engine {
        let config = EngineConfig {
            theta_target_marks: target,
            k_min: K_MIN,
            max_k: CAP,
            k_max_marks: CAP,
            ..common::config()
        };
        let dir = self.tmp.path();
        Engine::open(
            &self.bundle,
            &dir.join(format!("cache-{name}")),
            &dir.join(format!("wal-{name}.log")),
            config,
        )
        .expect("the engine opens")
    }
}

fn ask(
    engine: &Engine,
    session: &Session,
    zoom: u8,
    bbox: [f64; 4],
    k: usize,
    filter: Option<&FilterExpr>,
) -> ViewportOut {
    let mut req = ViewportRequest::new("s0", zoom, bbox, k);
    req.filter = filter.cloned();
    engine.viewport(session, req).expect("the viewport answers")
}

/// Each tile's served `mosaica_id`s: the points follow the tiles, `served` of them each.
fn by_tile(out: &ViewportOut) -> BTreeMap<u64, BTreeSet<u64>> {
    let mut ids = out.points.mosaica_ids.iter().copied();
    out.tiles
        .iter()
        .map(|t| (t.tile, ids.by_ref().take(t.served as usize).collect()))
        .collect()
}

/// The sample the definition draws from each tile's matches, `all` (every match's id by tile),
/// under the cut `m_target · n_occ · 2⁶⁴ / v` and the floor and cap `k`.
fn expected(
    all: &BTreeMap<u64, BTreeSet<u64>>,
    v: u64,
    n_occ: u64,
    k: usize,
) -> BTreeMap<u64, BTreeSet<u64>> {
    let numer = u128::from(TARGET) * u128::from(n_occ);
    let cut = (numer < u128::from(v)).then(|| ((numer << 64) / u128::from(v)) as u64);
    all.iter()
        .map(|(&tile, ids)| {
            let below = cut.map_or(ids.len(), |cut| ids.iter().filter(|&&id| id < cut).count());
            let m = k.min(K_MIN.min(k).max(below)).min(ids.len());
            (tile, ids.iter().copied().take(m).collect())
        })
        .collect()
}

/// The sample a viewer's own responses say its filtered map at `zoom` should hold.
fn expected_for(
    live: &Engine,
    everything: &Engine,
    session: &Session,
    zoom: u8,
    filter: &FilterExpr,
) -> BTreeMap<u64, BTreeSet<u64>> {
    let unfiltered = ask(live, session, zoom, WHOLE_MAP, CAP, None);
    let all = by_tile(&ask(
        everything,
        session,
        zoom,
        WHOLE_MAP,
        CAP,
        Some(filter),
    ));
    let matched = all.values().map(|ids| ids.len() as u64).sum();
    expected(&all, matched, unfiltered.tiles.len() as u64, CAP)
}

/// **A filtered tile serves in proportion to its matches, not every match up to the cap.** The
/// filtered map spends about the marks the unfiltered one does, and the corner tile, holding most
/// of the matches, draws far fewer than it matches.
#[test]
fn a_filtered_tile_thins_by_density_rather_than_filling_to_the_cap() {
    let fx = fixture();
    let (live, everything) = (fx.live("live"), fx.everything());
    let session = live.authorise(&full_coverage_credential()).unwrap();
    let common = fx.kind("common");
    for zoom in 0..=4 {
        let filtered = ask(&live, &session, zoom, WHOLE_MAP, CAP, Some(&common));
        assert_eq!(
            by_tile(&filtered),
            expected_for(&live, &everything, &session, zoom, &common),
            "zoom {zoom}: the filtered sample is the definition's over the matches"
        );
    }

    let filtered = ask(&live, &session, 1, WHOLE_MAP, CAP, Some(&common));
    let unfiltered = ask(&live, &session, 1, WHOLE_MAP, CAP, None);
    let corner = filtered
        .tiles
        .iter()
        .max_by_key(|t| t.matched)
        .expect("a tile matches");
    assert!(
        corner.served * 10 < corner.matched,
        "the corner tile is thinned: {corner:?}"
    );
    let spent = |out: &ViewportOut| out.tiles.iter().map(|t| t.served).sum::<u64>();
    let (filtered_marks, unfiltered_marks) = (spent(&filtered), spent(&unfiltered));
    assert!(
        filtered_marks * 2 > unfiltered_marks && filtered_marks < unfiltered_marks * 2,
        "about the same marks filtered ({filtered_marks}) as not ({unfiltered_marks})"
    );
}

/// **Adding a filter keeps every drawn point that matches it**, at every zoom, under a cap and at
/// the floor, and a pan does not move a filtered tile's sample.
#[test]
fn adding_a_filter_keeps_every_drawn_point_that_matches() {
    let fx = fixture();
    let (live, everything) = (fx.live("live"), fx.everything());
    let session = live.authorise(&full_coverage_credential()).unwrap();
    for key in ["common", "other", "rare"] {
        let filter = fx.kind(key);
        let matches: BTreeSet<u64> = ask(&everything, &session, 0, WHOLE_MAP, CAP, Some(&filter))
            .points
            .mosaica_ids
            .into_iter()
            .collect();
        for zoom in 0..=5 {
            for k in [CAP, 40, 3, 2] {
                let unfiltered = ask(&live, &session, zoom, WHOLE_MAP, k, None);
                let filtered: BTreeSet<u64> =
                    ask(&live, &session, zoom, WHOLE_MAP, k, Some(&filter))
                        .points
                        .mosaica_ids
                        .into_iter()
                        .collect();
                let kept: Vec<u64> = unfiltered
                    .points
                    .mosaica_ids
                    .iter()
                    .copied()
                    .filter(|id| matches.contains(id) && !filtered.contains(id))
                    .collect();
                assert!(
                    kept.is_empty(),
                    "{key}, zoom {zoom}, k {k}: {} drawn matching points were dropped",
                    kept.len()
                );
            }
        }

        // The part first, from a new session, so it counts the matches itself rather than taking
        // the whole map's count, and then again after the whole map.
        let pan = |session: &Session| {
            by_tile(&ask(&live, session, 3, [0.0, 0.0, 300.0, 300.0], CAP, Some(&filter)))
        };
        let fresh = live.authorise(&full_coverage_credential()).unwrap();
        let first = pan(&fresh);
        let whole = by_tile(&ask(&live, &fresh, 3, WHOLE_MAP, CAP, Some(&filter)));
        assert!(!first.is_empty(), "{key}: the pan holds a tile");
        assert_eq!(pan(&fresh), first, "{key}: the pan's sample moved");
        for (tile, ids) in &first {
            assert_eq!(&whole[tile], ids, "{key}: a pan moved tile {tile}'s sample");
        }
    }
}

/// **A filter matching few items draws every one of them**, and every tile holding a match draws at
/// least the floor's worth of them, however thinly a broad filter samples.
#[test]
fn a_sparse_filter_serves_every_match_and_the_floor_holds() {
    let fx = fixture();
    let live = fx.live("live");
    let session = live.authorise(&full_coverage_credential()).unwrap();
    let rare = (0..ITEMS).filter(|&e| kind_of(e) == "rare").count() as u64;
    assert!(rare > 1 && rare < TARGET, "a sparse filter: {rare}");
    for zoom in 0..=6 {
        let out = ask(
            &live,
            &session,
            zoom,
            WHOLE_MAP,
            CAP,
            Some(&fx.kind("rare")),
        );
        assert_eq!(
            out.points.len() as u64,
            rare,
            "zoom {zoom}: every rare item is drawn"
        );
        for t in &out.tiles {
            assert_eq!(t.served, t.matched, "zoom {zoom}: {t:?}");
        }
    }
    for zoom in [1, 2, 3] {
        let out = ask(
            &live,
            &session,
            zoom,
            WHOLE_MAP,
            CAP,
            Some(&fx.kind("common")),
        );
        for t in &out.tiles {
            assert!(
                t.served >= t.matched.min(K_MIN as u64),
                "zoom {zoom}: a tile with a match draws the floor: {t:?}"
            );
        }
        assert!(
            out.tiles.iter().any(|t| t.served < t.matched),
            "zoom {zoom}: the broad filter is thinned somewhere"
        );
    }
}

/// **A viewer's filtered sample is set by its own matches alone.** Each of two viewers who see
/// disjoint items draws the sample its own counts define, and suppressing the other's matches
/// changes nothing it is served.
#[test]
fn a_viewers_filtered_sample_is_set_by_its_own_matches() {
    let fx = fixture();
    let (live, everything) = (fx.live("live"), fx.everything());
    let common = fx.kind("common");
    let (a, b) = (credential(2), credential(3));
    for (who, cred) in [("a", &a), ("b", &b)] {
        let session = live.authorise(cred).unwrap();
        for zoom in [0, 2, 4] {
            assert_eq!(
                by_tile(&ask(&live, &session, zoom, WHOLE_MAP, CAP, Some(&common))),
                expected_for(&live, &everything, &session, zoom, &common),
                "viewer {who}, zoom {zoom}"
            );
        }
    }

    // Suppress a few hundred of b's matches, written before a second engine opens.
    let wal_path = fx.tmp.path().join("wal-suppressed.log");
    let suppressed: Vec<u64> = (0..ITEMS)
        .filter(|&e| viewer_term(e) == 3 && kind_of(e) == "common")
        .step_by(3)
        .collect();
    {
        let mut wal =
            Wal::open(&wal_path, mosaica_store::database_of(&fx.bundle).unwrap()).unwrap();
        let changes = suppressed
            .iter()
            .map(|&e| {
                let entity = item_of_id(&live, e)
                    .unwrap()
                    .expect("the item is in the bundle");
                (item(entity), ChangeOp::Suppress)
            })
            .collect();
        wal.append(&WalRecord::ChangeBatch { changes }).unwrap();
        wal.fsync().unwrap();
    }
    let after = fx.live("suppressed");
    for zoom in [0, 2, 4] {
        let before_a = ask(
            &live,
            &live.authorise(&a).unwrap(),
            zoom,
            WHOLE_MAP,
            CAP,
            Some(&common),
        );
        let after_a = ask(
            &after,
            &after.authorise(&a).unwrap(),
            zoom,
            WHOLE_MAP,
            CAP,
            Some(&common),
        );
        assert_eq!(
            after_a.tiles, before_a.tiles,
            "zoom {zoom}: a's counts moved"
        );
        assert_eq!(
            after_a.points, before_a.points,
            "zoom {zoom}: a's sample moved"
        );
        let matched = |out: &ViewportOut| out.tiles.iter().map(|t| t.matched).sum::<u64>();
        let before_b = ask(
            &live,
            &live.authorise(&b).unwrap(),
            zoom,
            WHOLE_MAP,
            CAP,
            Some(&common),
        );
        let after_b = ask(
            &after,
            &after.authorise(&b).unwrap(),
            zoom,
            WHOLE_MAP,
            CAP,
            Some(&common),
        );
        assert_eq!(
            matched(&after_b),
            matched(&before_b) - suppressed.len() as u64,
            "zoom {zoom}: the suppression reached b"
        );
    }
}

/// **A suppression reaches a filtered sample's anchor from the next request**, also over a viewport
/// that does not cover the map, where the session holds the count of matches its last request took.
#[test]
fn a_suppression_moves_the_filtered_anchor_on_the_next_request() {
    let fx = fixture();
    let mut live = fx.live("live");
    live.start_write_executor(8).expect("the executor starts");
    let everything = fx.everything();
    let session = live.authorise(&full_coverage_credential()).unwrap();
    let common = fx.kind("common");
    let part = [0.0, 0.0, 300.0, 300.0];
    ask(&live, &session, 3, WHOLE_MAP, CAP, Some(&common));
    let before = by_tile(&ask(&live, &session, 3, part, CAP, Some(&common)));

    // Only matches outside the part, so nothing but the anchor can move the part's sample.
    let changes = (0..ITEMS)
        .filter(|&e| kind_of(e) == "common" && (position(e).0 >= 500.0 || position(e).1 >= 500.0))
        .map(|e| {
            let entity = item_of_id(&live, e).unwrap().expect("the item is in the bundle");
            (item(entity), ChangeOp::Suppress)
        })
        .collect();
    live.accept_changes(changes).expect("the suppressions are accepted");

    let after = by_tile(&ask(&live, &session, 3, part, CAP, Some(&common)));
    let unfiltered = ask(&live, &session, 3, WHOLE_MAP, CAP, None);
    let matched = ask(&live, &session, 3, WHOLE_MAP, CAP, Some(&common))
        .tiles
        .iter()
        .map(|t| t.matched)
        .sum();
    let all = by_tile(&ask(&everything, &session_of(&everything), 3, part, CAP, Some(&common)));
    assert_eq!(
        after,
        expected(&all, matched, unfiltered.tiles.len() as u64, CAP),
        "the part's sample is the definition's over the matches left"
    );
    assert_ne!(after, before, "the suppression moved the anchor");
}

fn session_of(engine: &Engine) -> Session {
    engine.authorise(&full_coverage_credential()).unwrap()
}
