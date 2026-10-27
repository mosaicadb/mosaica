//! **The `member_of` leaf: one artifact's membership, inside `M_auth`, as a filter clause**
//! (`highlight-and-hierarchy.md` §3).
//!
//! The oracle is the fixture's own generator: the members published for an artifact, intersected
//! with the terms the credential grants, counted directly — none of which touches the layer
//! registry, the existence criterion, the two membership layouts or the routed tree, which is
//! where a defect would live.
//!
//! What is asserted: the leaf's `matched` is the artifact's own masked count, under either
//! serving layout and for two principals; it composes with an ordinary leaf under `all_of`,
//! `any_of` and `none_of`; **an artifact this principal would not be served is an empty operand
//! and refuses nothing** — withheld by its criterion, suppressed, of another layer, and an
//! identifier that names nothing are one response; an **unknown layer is the caller's fault**,
//! where an unknown artifact never is; and the timing of the withheld case is not separable from
//! the unknown identifier's.

mod common;

use std::time::{Duration, Instant};

use common::*;
use mosaica_engine::filter::{FilterError, FilterExpr, MemberOfLeaf, RegionLeaf};
use mosaica_engine::{Engine, EngineError, LayerSelection, ViewportOut, ViewportRequest};
use mosaica_lifecycle::wal::ChangeOp;
use mosaica_lifecycle::IncomingArtifact;
use mosaica_types::layer::{
    ContentDeclaration, ExistenceCriterion, Hierarchy, HierarchyKind, LayerDeclaration,
    MembershipSource, ServingLayout,
};
use mosaica_types::{EntityId, MosaicaId, ShardId};

const LAYER: &str = "clusters/a";
/// The artifact's members: source ids `0..300`, which the broad credential sees whole and the
/// narrow one sees every third of.
const MEMBERS: std::ops::Range<u64> = 0..300;
/// A criterion low enough that both principals are served the artifact — so the leaf's answer is
/// each one's own masked membership rather than an empty operand.
const BOTH_SERVED: u64 = 50;
/// A criterion the broad principal clears and the narrow one does not.
const ONLY_BROAD_SERVED: u64 = 200;

fn declaration(layout: Option<ServingLayout>, bar: u64) -> LayerDeclaration {
    LayerDeclaration {
        scope: Default::default(),
        name: LAYER.into(),
        title: None,
        views: vec!["s0".into()],
        membership: MembershipSource::Enumerated,
        value_set: Default::default(),
        visibility: None,
        artifact_visibility: mosaica_types::layer::ArtifactVisibility::inherited(),
        require_member_visibility: Some(ExistenceCriterion::Count(bar)),
        hierarchy: Hierarchy {
            kind: HierarchyKind::Flat,
            prune_children: false,
        },
        content: ContentDeclaration {
            computed: vec!["centroid".into()],
            supplied: Vec::new(),
        },
        depends_on: Vec::new(),
        levels: Vec::new(),
        layout,
        shape: None,
    }
}

/// The oracle: how many of `members` this principal can see.
fn visible_members(members: std::ops::Range<u64>, sees: impl Fn(u64) -> bool) -> u64 {
    members.filter(|&e| sees(e)).count() as u64
}

fn member_of(artifact: MosaicaId) -> FilterExpr {
    FilterExpr::MemberOf(MemberOfLeaf {
        layer: LAYER.to_string(),
        artifact: mosaica_engine::filter::MemberOfArtifact::Id(artifact),
    })
}

fn member_key(key: &str, level: u32) -> FilterExpr {
    FilterExpr::MemberOf(MemberOfLeaf {
        layer: LAYER.to_string(),
        artifact: mosaica_engine::filter::MemberOfArtifact::Key {
            key: key.to_string(),
            level,
        },
    })
}

fn viewport(engine: &Engine, credential: &[u8], filter: Option<FilterExpr>) -> ViewportOut {
    let session = engine.authorise(credential).unwrap();
    let mut request = ViewportRequest::new("s0", 0, WHOLE_MAP, N_ITEMS as usize);
    if let Some(filter) = filter {
        request = request.filter(filter);
    }
    engine
        .viewport(&session, request)
        .expect("a viewport answers")
}

fn matched(out: &ViewportOut) -> u64 {
    out.tiles.iter().map(|t| t.matched).sum()
}

fn visible(out: &ViewportOut) -> u64 {
    out.tiles.iter().map(|t| t.visible).sum()
}

/// A bundle with one flat layer of one artifact over [`MEMBERS`], served under `layout`.
struct Fixture {
    _tmp: tempfile::TempDir,
    root: std::path::PathBuf,
    engine: Engine,
    id: MosaicaId,
}

/// `bar` is the layer's `Count` criterion: at [`BOTH_SERVED`] each principal is served the
/// artifact and the leaf answers their own masked membership; at [`ONLY_BROAD_SERVED`] the narrow
/// principal is below it and the leaf is their empty operand.
fn fixture(layout: Option<ServingLayout>, bar: u64) -> Fixture {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join("bundle");
    build_fixture(
        &root,
        &tmp.path().join("points.parquet"),
        &tmp.path().join("pairs.parquet"),
    );
    let engine = open_engine_publishing(&root, &tmp.path().join("cache"), &tmp.path().join("wal"));
    engine.register_layer(declaration(layout, bar)).unwrap();
    let map = source_to_new_map(&root, "v00000");
    let members: Vec<EntityId> = MEMBERS.map(|s| EntityId::new(map[&s])).collect();
    engine
        .publish_artifacts(
            LAYER.into(),
            0,
            vec![IncomingArtifact::from_entities(Some("c0".into()), members)],
        )
        .unwrap();
    let session = engine.authorise(&full_coverage_credential()).unwrap();
    let served = engine
        .viewport_artifacts(
            &session,
            mosaica_engine::ViewportArtifactsRequest::new("s0", 0, WHOLE_MAP, usize::MAX).layers(LayerSelection::All),
        )
        .unwrap()
        .artifacts();
    assert_eq!(served.len(), 1, "the broad principal is served the cluster");
    assert_eq!(served[0].masked_count, visible_members(MEMBERS, |_| true));
    Fixture {
        engine,
        root,
        id: served[0].mosaica_id,
        _tmp: tmp,
    }
}

impl Fixture {
    /// Publish a second artifact of the same layer over `members`, and return its identifier as
    /// the broad principal is served it.
    fn second(&self, members: std::ops::Range<u64>, key: &str) -> MosaicaId {
        let map = source_to_new_map(&self.root, &self.engine.generation().prefix);
        let entities: Vec<EntityId> = members.map(|s| EntityId::new(map[&s])).collect();
        self.engine
            .publish_artifacts(
                LAYER.into(),
                0,
                vec![IncomingArtifact::from_entities(Some(key.into()), entities)],
            )
            .unwrap();
        tick(&self.engine);
        let session = self.engine.authorise(&full_coverage_credential()).unwrap();
        let served = self
            .engine
            .viewport_artifacts(
                &session,
                mosaica_engine::ViewportArtifactsRequest::new("s0", 0, WHOLE_MAP, usize::MAX)
                    .layers(LayerSelection::All),
            )
            .unwrap()
            .artifacts();
        served
            .iter()
            .find(|a| a.key.as_deref() == Some(key))
            .expect("the second artifact is served")
            .mosaica_id
    }
}

/// **The leaf is the artifact's own masked count, and the layout decides only the route.**
///
/// The artifact-major route reads the held row bitmap and intersects it with the composed mask;
/// the row-major one scans the principal's visible rows comparing labels. Both must answer
/// `membership ∩ M_auth` — the same number the artifacts frame serves beside the artifact — for
/// every principal, which is what makes the layout a cost decision (decision 0093/0094).
///
/// **And the route here is the membership, never the column.** Both answer the same number, so
/// only a counter separates them: `Engine::member_of_column_walks` rises when the leaf reads the
/// column instead, which is what a level holding no artifact-major membership does — the case
/// `the_leaf_answers_from_the_column_where_the_form_holds_no_rows` covers. Nothing here has
/// folded, so no level has claimed a column and the counter stays at zero.
///
/// Mutations this kills: reading the membership without the mask (the broad principal's answer
/// would be right and the narrow one's wrong); scanning the row column over the request's tiles
/// rather than the whole view; taking the artifact's row set for the wrong ordinal; and taking
/// the column route where the membership would have answered.
#[test]
fn the_leaf_is_the_artifacts_masked_count_under_either_layout() {
    for layout in [
        None,
        Some(ServingLayout::ArtifactMajor),
        Some(ServingLayout::RowMajorLabel),
        Some(ServingLayout::RowMajorList),
    ] {
        let fx = fixture(layout, BOTH_SERVED);
        let what = format!("{layout:?}");
        // The declared layout is the served one here — a single artifact's memberships cannot
        // overlap, so nothing falls back and each arm of the loop exercises the route it names.
        // With no pin, a flat level's first publication of one artifact chooses the label form.
        assert_eq!(
            fx.engine.recorded_layout(LAYER, 0),
            Some(layout.unwrap_or(ServingLayout::RowMajorLabel)),
            "{what}: the level is served in the layout the loop asked for"
        );
        let broad = viewport(&fx.engine, &full_coverage_credential(), Some(member_of(fx.id)));
        let narrow = viewport(&fx.engine, &subset_credential(), Some(member_of(fx.id)));
        let want_broad = visible_members(MEMBERS, |_| true);
        let want_narrow = visible_members(MEMBERS, subset_sees);
        assert!(
            want_narrow > 0 && want_narrow < want_broad,
            "{what}: the two principals are a real split"
        );
        assert_eq!(matched(&broad), want_broad, "{what}: the broad principal");
        assert_eq!(matched(&narrow), want_narrow, "{what}: the narrow principal");
        // I12: the filter moves `matched`, never `visible`.
        assert_eq!(visible(&broad), N_ITEMS, "{what}: visible is unmoved");
        // A `member_of` leaf carries no region, so no verdict rides on the response.
        assert_eq!(broad.region, None, "{what}: no region verdict");
        assert_eq!(
            fx.engine.member_of_column_walks(),
            0,
            "{what}: the leaf read the artifact-major membership, not the row column"
        );
    }
}

/// **It composes like any other leaf**, in row space, and with a `region` beside it.
///
/// `all_of` is the intersection, `any_of` the union — checked against each other by inclusion–
/// exclusion rather than against a restated oracle — and `none_of` is the complement within the
/// visible set, which is the "everything outside this cluster" the artifact card offers.
#[test]
fn it_composes_under_the_three_combinators() {
    let fx = fixture(None, BOTH_SERVED);
    let other = fx.second(200..500, "c1");
    for credential in [full_coverage_credential(), subset_credential()] {
        let broad = credential == full_coverage_credential();
        let sees = |e: u64| broad || subset_sees(e);
        let a = viewport(&fx.engine, &credential, Some(member_of(fx.id)));
        let b = viewport(&fx.engine, &credential, Some(member_of(other)));
        let both = viewport(
            &fx.engine,
            &credential,
            Some(FilterExpr::AllOf(vec![member_of(fx.id), member_of(other)])),
        );
        let either = viewport(
            &fx.engine,
            &credential,
            Some(FilterExpr::AnyOf(vec![member_of(fx.id), member_of(other)])),
        );
        assert_eq!(
            matched(&both) + matched(&either),
            matched(&a) + matched(&b),
            "|A ∩ B| + |A ∪ B| = |A| + |B|"
        );
        assert_eq!(
            matched(&both),
            visible_members(200..300, sees),
            "the overlap is source ids 200..300, inside this principal's own mask"
        );

        let outside = viewport(
            &fx.engine,
            &credential,
            Some(FilterExpr::NoneOf(vec![member_of(fx.id)])),
        );
        assert_eq!(
            matched(&a) + matched(&outside),
            visible(&a),
            "inside and outside partition the visible set"
        );

        // A region beside it: the same row-space tree with two different leaf kinds in it, which
        // is the shape §3 says a date range or a drawn lasso takes beside this clause.
        let half = FilterExpr::Region(RegionLeaf::Shape(std::sync::Arc::new(
            mosaica_engine::shapes::ShapeF64::Bbox {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 1000.0,
                max_y: 500.0,
            }
            .canonical(mosaica_engine::shapes::Space::View, &extent())
            .expect("a well-formed box")
            .0,
        )));
        let in_half = viewport(&fx.engine, &credential, Some(half.clone()));
        let clipped = viewport(
            &fx.engine,
            &credential,
            Some(FilterExpr::AllOf(vec![member_of(fx.id), half.clone()])),
        );
        // One negation over both kinds is the visible set minus their union.
        let union = viewport(
            &fx.engine,
            &credential,
            Some(FilterExpr::AnyOf(vec![member_of(fx.id), half.clone()])),
        );
        let neither = viewport(
            &fx.engine,
            &credential,
            Some(FilterExpr::NoneOf(vec![member_of(fx.id), half])),
        );
        assert_eq!(matched(&union) + matched(&neither), visible(&union));
        assert!(matched(&neither) > 0 && matched(&union) > 0);
        assert!(
            matched(&clipped) <= matched(&a).min(matched(&in_half)),
            "a conjunction narrows both sides"
        );
        assert!(
            matched(&clipped) > 0 && matched(&clipped) < matched(&a),
            "and it is a proper, non-empty narrowing"
        );
    }
}

/// **An artifact this principal would not be served is an empty operand, never a refusal**, and
/// every reason it is not served gives the one answer.
///
/// Four routes: an identifier that names nothing; one below this principal's own existence
/// criterion; one that is suppressed; and one that names an artifact of *another* layer than the
/// leaf's. Each is a **value** that does not resolve within the layer named, and a `422` to any of
/// them would make the leaf an existence oracle over exactly what the criterion withholds
/// (`highlight-and-hierarchy.md` §3).
#[test]
fn an_artifact_that_is_not_served_is_an_empty_operand_and_refuses_nothing() {
    let fx = fixture(None, ONLY_BROAD_SERVED);
    let unknown = MosaicaId::new(0x7777_7777_7777_7777);

    // Withheld by the criterion: the narrow principal is below the bar of 200.
    let withheld = viewport(&fx.engine, &subset_credential(), Some(member_of(fx.id)));
    let never = viewport(&fx.engine, &subset_credential(), Some(member_of(unknown)));
    assert_eq!(matched(&never), 0, "an identifier that names nothing");
    assert_eq!(
        withheld,
        never.clone(),
        "withheld and unknown are one response"
    );
    // And so is its outside: `none_of` over an empty operand is every visible row.
    let outside = viewport(
        &fx.engine,
        &subset_credential(),
        Some(FilterExpr::NoneOf(vec![member_of(fx.id)])),
    );
    assert_eq!(matched(&outside), visible(&outside));

    // A `mosaica_id` that names a *point* rather than an artifact — the other never-an-artifact
    // shape, and the one a client is most likely to send by mistake.
    let point_id = viewport(&fx.engine, &full_coverage_credential(), None)
        .points
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("the whole map serves points");
    assert_eq!(
        matched(&viewport(
            &fx.engine,
            &full_coverage_credential(),
            Some(member_of(point_id))
        )),
        0,
        "an identifier naming a point is an empty operand"
    );

    // Suppressed: the broad principal, who was served it, now gets the empty operand too.
    let target = artifact_target(&fx.engine, fx.id);
    fx.engine.accept_changes(vec![(target, ChangeOp::Suppress)]).unwrap();
    let suppressed = viewport(&fx.engine, &full_coverage_credential(), Some(member_of(fx.id)));
    let never_broad = viewport(&fx.engine, &full_coverage_credential(), Some(member_of(unknown)));
    assert_eq!(matched(&suppressed), 0);
    assert_eq!(
        suppressed,
        never_broad,
        "suppressed and unknown are one response"
    );
    fx.engine
        .accept_changes(vec![(target, ChangeOp::Unsuppress)])
        .unwrap();
    assert_eq!(
        matched(&viewport(
            &fx.engine,
            &full_coverage_credential(),
            Some(member_of(fx.id))
        )),
        visible_members(MEMBERS, |_| true),
        "an unsuppressed artifact is its membership again"
    );
}

/// **A key names the artifact its identifier names**, for each principal, alone and under
/// `none_of`, for an artifact published live after the engine opened, and after a restart.
#[test]
fn a_key_answers_as_the_identifier_of_the_artifact_it_names() {
    let fx = fixture(None, BOTH_SERVED);
    let ids = [("c0", fx.id), ("c1", fx.second(200..500, "c1"))];
    let same = |engine: &Engine, what: &str| {
        for credential in [full_coverage_credential(), subset_credential()] {
            for (key, id) in ids {
                let by_key = viewport(engine, &credential, Some(member_key(key, 0)));
                assert!(matched(&by_key) > 0, "{what}: {key} matches its members");
                assert_eq!(
                    by_key,
                    viewport(engine, &credential, Some(member_of(id))),
                    "{what}: {key} by key and by identifier"
                );
                assert_eq!(
                    viewport(engine, &credential, Some(FilterExpr::NoneOf(vec![member_key(key, 0)]))),
                    viewport(engine, &credential, Some(FilterExpr::NoneOf(vec![member_of(id)]))),
                    "{what}: {key} under none_of"
                );
            }
        }
    };
    same(&fx.engine, "live");
    let Fixture { _tmp, root, engine, .. } = fx;
    drop(engine);
    let reopened =
        open_engine_publishing(&root, &_tmp.path().join("cache"), &_tmp.path().join("wal"));
    same(&reopened, "reopened");
}

/// **A key naming no artifact this principal would be served is the empty operand**: an unknown
/// key, the key of an artifact below this principal's criterion and the key of a suppressed
/// artifact are one response.
#[test]
fn a_key_naming_nothing_served_is_the_empty_operand() {
    let fx = fixture(None, ONLY_BROAD_SERVED);
    let unknown = MosaicaId::new(0x7777_7777_7777_7777);
    for credential in [full_coverage_credential(), subset_credential()] {
        let never = viewport(&fx.engine, &credential, Some(member_of(unknown)));
        assert_eq!(matched(&never), 0);
        assert_eq!(
            viewport(&fx.engine, &credential, Some(member_key("nowhere", 0))),
            never
        );
    }
    let narrow_never = viewport(&fx.engine, &subset_credential(), Some(member_of(unknown)));
    assert_eq!(
        viewport(&fx.engine, &subset_credential(), Some(member_key("c0", 0))),
        narrow_never,
        "a withheld artifact's key and an unknown key are one response"
    );

    let target = artifact_target(&fx.engine, fx.id);
    fx.engine.accept_changes(vec![(target, ChangeOp::Suppress)]).unwrap();
    assert_eq!(
        viewport(&fx.engine, &full_coverage_credential(), Some(member_key("c0", 0))),
        viewport(&fx.engine, &full_coverage_credential(), Some(member_key("nowhere", 0))),
        "a suppressed artifact's key and an unknown key are one response"
    );
    assert_eq!(
        matched(&viewport(
            &fx.engine,
            &full_coverage_credential(),
            Some(FilterExpr::NoneOf(vec![member_key("c0", 0)]))
        )),
        N_ITEMS,
        "and its outside is every visible item"
    );
}

/// **A layer this principal does not reach is `422`; an artifact they do not reach never is.**
///
/// A layer name is deployment schema — `/v1/meta` lists the layers this principal may name, and
/// the registry's probe answers alike for a gate-failed name and a never-registered one — so
/// refusing by name discloses nothing they were not already told. That is the same split
/// `contracts.md` §3.2 draws between an unknown column and an unknown value, and getting it the
/// other way round in either direction is the defect: refusing the artifact makes an oracle, and
/// answering the layer empty hides a caller's typo behind a blank map forever.
#[test]
fn an_unknown_layer_is_the_callers_fault_and_an_unknown_artifact_is_not() {
    let fx = fixture(None, BOTH_SERVED);
    let session = fx.engine.authorise(&full_coverage_credential()).unwrap();
    let request = |layer: &str| {
        fx.engine.viewport(
            &session,
            ViewportRequest::new("s0", 0, WHOLE_MAP, N_ITEMS as usize).filter(FilterExpr::MemberOf(
                MemberOfLeaf {
                    layer: layer.to_string(),
                    artifact: mosaica_engine::filter::MemberOfArtifact::Id(fx.id),
                },
            )),
        )
    };
    match request("clusters/nowhere") {
        Err(EngineError::FilterMalformed(detail)) => {
            assert!(
                detail.contains("clusters/nowhere") && detail.contains("member_of"),
                "the refusal names the layer and the leaf: {detail}"
            );
        }
        other => panic!("an unknown layer is the caller's fault, not {other:?}"),
    }
    assert!(
        FilterError::UnknownLayer("x".into()).is_callers_fault(),
        "and it is a 422 rather than a 500"
    );
    // The artifact named inside a layer it does not belong to is a *value*, so it is empty and not
    // a refusal — the registered layer, the wrong artifact.
    assert!(request(LAYER).is_ok(), "the layer itself answers");
}

/// **A level the layer does not hold is the caller's fault**, as an unknown layer is: levels are
/// schema `/v1/meta` publishes with the layer. A level on a layer this principal does not reach is
/// the unknown layer's refusal, so the level says nothing about a layer they may not see.
#[test]
fn a_level_the_layer_does_not_hold_is_the_callers_fault() {
    let fx = fixture(None, BOTH_SERVED);
    let session = fx.engine.authorise(&full_coverage_credential()).unwrap();
    let ask = |layer: &str, level: u32| {
        fx.engine.viewport(
            &session,
            ViewportRequest::new("s0", 0, WHOLE_MAP, N_ITEMS as usize).filter(FilterExpr::MemberOf(
                MemberOfLeaf {
                    layer: layer.to_string(),
                    artifact: mosaica_engine::filter::MemberOfArtifact::Key {
                        key: "c0".to_string(),
                        level,
                    },
                },
            )),
        )
    };
    assert!(ask(LAYER, 0).is_ok(), "the layer's one level answers");
    assert!(
        matches!(ask(LAYER, 1), Err(EngineError::FilterMalformed(_))),
        "a level past the layer's last is refused"
    );
    let unknown_layer = |level| match ask("clusters/nowhere", level) {
        Err(EngineError::FilterMalformed(detail)) => detail,
        other => panic!("an unknown layer is the caller's fault, not {other:?}"),
    };
    assert_eq!(
        unknown_layer(7),
        unknown_layer(0),
        "on a layer not reached, any level is the unknown layer's one refusal"
    );
}

/// **A withheld artifact's timing is not separable from a name naming nothing**, by identifier
/// or by key.
///
/// C33's registered residual is the C4/C24–C26 family's: a withheld artifact's criterion is
/// *evaluated* where an unknown identifier is a lookup miss, so the two are not bit-identical in
/// cost. What must not exist is a residual a caller could use — a withheld artifact taking a
/// membership read, a masked count over the whole level, or anything else proportional to what is
/// being withheld. For an identifier this asserts the coarse bound the register claims, the two
/// medians inside the same order of magnitude: a viewer needs an identifier to probe, and is never
/// served one for an artifact withheld from them.
///
/// A key is the sharper case, since keys are chosen by publishers and can be guessed, so a key
/// that misses the index takes the same count and verdict for its layer and level as a withheld
/// artifact's, whether below this principal's criterion or suppressed. Its bound is tight enough
/// to catch a key that skips that work, and read off each side's fastest ask, which load on the
/// box only slows.
#[test]
fn a_withheld_artifacts_timing_is_not_separable_from_a_name_naming_nothing() {
    let fx = fixture(None, ONLY_BROAD_SERVED);
    let ask = |session: &mosaica_engine::Session, filter: FilterExpr| {
        let started = Instant::now();
        fx.engine
            .viewport(
                session,
                ViewportRequest::new("s0", 0, WHOLE_MAP, N_ITEMS as usize).filter(filter),
            )
            .unwrap();
        started.elapsed()
    };
    let unknown = MosaicaId::new(0x7777_7777_7777_7777);
    let narrow = fx.engine.authorise(&subset_credential()).unwrap();
    let (withheld, missing) = paired(
        40,
        || ask(&narrow, member_of(fx.id)),
        || ask(&narrow, member_of(unknown)),
    );
    let ratio = median(&withheld) / median(&missing).max(f64::MIN_POSITIVE);
    assert!(
        (0.1..10.0).contains(&ratio),
        "an identifier below the criterion: the medians differ by {ratio:.2}×, which is a channel \
         rather than the register's stated residual"
    );

    // A case out of bounds is measured once more, and fails only if out of bounds again: noise
    // rarely lands twice, while a key that skips the gate's work is out of bounds every time.
    let same_cost = |what: &str, measure: &dyn Fn() -> (Vec<f64>, Vec<f64>)| {
        let ratio = || {
            let (withheld, missing) = measure();
            least(&withheld) / least(&missing).max(f64::MIN_POSITIVE)
        };
        let within = |ratio: f64| (0.8..1.25).contains(&ratio);
        let first = ratio();
        if within(first) {
            return;
        }
        let second = ratio();
        assert!(
            within(second),
            "{what}: the fastest asks differ by {first:.2}× and, measured again, {second:.2}×, \
             so a key naming nothing costs other than a withheld artifact's"
        );
    };
    same_cost("a key below the criterion", &|| {
        paired(
            200,
            || ask(&narrow, member_key("c0", 0)),
            || ask(&narrow, member_key("nowhere", 0)),
        )
    });
    let broad = fx.engine.authorise(&full_coverage_credential()).unwrap();
    let target = artifact_target(&fx.engine, fx.id);
    fx.engine.accept_changes(vec![(target, ChangeOp::Suppress)]).unwrap();
    same_cost("a suppressed artifact's key", &|| {
        paired(
            200,
            || ask(&broad, member_key("c0", 0)),
            || ask(&broad, member_key("nowhere", 0)),
        )
    });
}

/// `n` timed asks of each of `a` and `b` in seconds, after one warm pair, interleaved with the
/// order alternated so neither side always runs first.
fn paired(
    n: usize,
    a: impl Fn() -> Duration,
    b: impl Fn() -> Duration,
) -> (Vec<f64>, Vec<f64>) {
    a();
    b();
    let (mut a_s, mut b_s) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        if i % 2 == 0 {
            a_s.push(a().as_secs_f64());
            b_s.push(b().as_secs_f64());
        } else {
            b_s.push(b().as_secs_f64());
            a_s.push(a().as_secs_f64());
        }
    }
    (a_s, b_s)
}

fn median(times: &[f64]) -> f64 {
    let mut sorted = times.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

fn least(times: &[f64]) -> f64 {
    times.iter().copied().fold(f64::INFINITY, f64::min)
}

/// **The leaf answers the same set from the column alone**, on a level whose row column and whose
/// extents a fold wrote and a restart claimed.
///
/// Such a level builds no artifact-major membership at all
/// (`crate::artifacts::MembershipRows::rows_held`), so the route this exercises is the one the
/// counter above exists to distinguish: the walk of the principal's visible rows **inside the
/// artifact's extent**, reading each row's labels off the column. It must return the same
/// `membership ∩ M_auth` the bitmap returns, for a broad principal and a narrow one, or the leaf
/// answers a different question under a layout no client can see.
#[test]
fn the_leaf_answers_from_the_column_where_the_form_holds_no_rows() {
    let fx = fixture(Some(ServingLayout::RowMajorLabel), BOTH_SERVED);
    let want_broad = matched(&viewport(
        &fx.engine,
        &full_coverage_credential(),
        Some(member_of(fx.id)),
    ));
    let want_narrow = matched(&viewport(
        &fx.engine,
        &subset_credential(),
        Some(member_of(fx.id)),
    ));
    assert!(
        want_narrow > 0 && want_narrow < want_broad,
        "the two principals are a real split"
    );

    let before = fx.engine.write_executor_stats();
    fx.engine.request_fold().unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let now = fx.engine.write_executor_stats();
        assert_eq!(now.fold_failures, before.fold_failures);
        if now.folds > before.folds {
            break;
        }
        assert!(Instant::now() < deadline, "the fold never published");
        std::thread::sleep(Duration::from_millis(10));
    }
    let cache = fx._tmp.path().join("cache2");
    let wal = fx._tmp.path().join("wal2");
    drop(fx.engine);
    let reopened = open_engine_publishing(&fx.root, &cache, &wal);
    let id = fx.id;

    assert_eq!(
        matched(&viewport(
            &reopened,
            &full_coverage_credential(),
            Some(member_of(id))
        )),
        want_broad,
        "the broad principal's operand is the same set by the column route"
    );
    assert_eq!(
        matched(&viewport(&reopened, &subset_credential(), Some(member_of(id)))),
        want_narrow,
        "and the narrow principal's"
    );
    let form = reopened
        .held_artifact_form_for_test(ShardId::ZERO, "s0", LAYER, 0)
        .expect("the requests left the level's form held");
    assert!(
        !form.membership().rows_held(),
        "the level was served from its column and its extents, so it holds no per-artifact rows"
    );
    assert!(
        reopened.member_of_column_walks() >= 2,
        "and both leaves took the column route"
    );
}
