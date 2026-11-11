//! **A layer's hierarchy by lineage, independent of the viewport** — `POST /v1/artifacts/browse`
//! (`highlight-and-hierarchy.md` §4).
//!
//! Rung 3 put a 30,217-descriptor DAG into a bundle and the viewport could show none of it: a
//! MeSH descriptor's members are spread over the whole layout, so the budget cut deepens uniformly
//! and the leaves arrive at a zoom nobody reaches. This verb serves the same artifacts **by
//! relation instead of by position** — the layer's roots, one artifact's children and parents, and
//! a name search — each row carrying the masked count the artifacts frame carries.
//!
//! # What makes it safe, in four sentences
//!
//! - **The gate is the viewport's own**, run through [`crate::artifacts::ArtifactView::verdict`]
//!   for every artifact of the form's candidate set, and run **before** `limit` and `cursor` are
//!   applied — so a page's fill and its `next` count only artifacts this principal may see, and a
//!   withheld artifact leaves no gap a caller could count.
//! - **Every count is C8's `and_cardinality` against `M_auth`**, computed per request: the masked
//!   count is the artifacts frame's, and the filtered one is `|membership ∩ M_auth ∩ filter|`
//!   against a verdict the filter contract's own routes produced inside `M_auth`.
//! - **Existence and `masked_count` never move with the filter** (**I3**, **I12**) — a row whose
//!   `matched_count` is zero is still served, exactly as an artifact whose `matched` bit is false
//!   is still served on the viewport.
//! - **A relation is named only where both ends are served** (C29, per entry). A served
//!   artifact hangs from the served artifacts nearest above it on every path ([`crate::lineage`]),
//!   so a withheld artifact is passed through and never named, and its served children are listed
//!   under its served parents exactly as the map's cut attaches them. A requested `parent` that
//!   fails its own criterion answers an empty page identically to a leaf.
//!
//! The register row is **C33**, and its argument is that enumeration was already available:
//! `artifact_budget` is a request bound and never a disclosure control, so a zoom-0 viewport at a
//! large budget with every level named serves the same set. This verb serves it paged and by
//! relation instead of by position.
//!
//! # What it costs
//!
//! The gate is asked one artifact at a time and keeps each answer. The roots and search forms ask
//! it of every artifact of the layer, one masked count each. The children form asks it of the
//! requested artifact, of the artifacts above it up to its nearest served ancestors, and of those
//! below it down to its nearest served descendants, so its cost follows the requested artifact's
//! neighbourhood and not the layer: a level of 3 × 10⁵ artifacts answers a children page in under
//! a tenth of a millisecond, where the roots form takes about 0.2 s. Each row returned also walks down to its own nearest served descendants for its `child_count`, and up to
//! its served roots for its rung. Keys are read for the rows returned, and for every candidate of
//! a search. Under `filters`, one more `and_cardinality` per artifact gated. **A filter
//! whose leaves route row space is the one whole-view scan this design adds** (§9 (d), owner
//! ruling): a leaf over a render-only column is answered by the viewport only inside its tiles,
//! and browse has none, so the row route's own predicate is run over every row of the view rather
//! than over a request's ranges. Served naively its `matched_count` would be silently zero, which
//! is the failure decision 0104 exists to prevent.
//!
//! A row without text of its own takes its name from an attached label. The label layers are read
//! on the first row that needs one, and a label is looked up only for the rows of a page, or for
//! the candidates of a search whose key does not match, through an index from target to label
//! kept with the label level's row form, so a page does not walk the label layer.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};

use mosaica_types::layer::{HierarchyKind, RegisteredLayer};
use mosaica_types::{EntityId, MosaicaId};

use crate::artifacts::{ArtifactVerdict, ArtifactView};
use crate::compose::{EffectiveMask, ShardedMask};
use crate::error::Result;
use crate::filter::FilterExpr;
use crate::layer_read::{check_level, LayerRefusal, ReadLevel};
use crate::viewport::{DependencyContext, ServedView};
use crate::Engine;
use crate::EngineError;
use mosaica_lifecycle::membership::Attachment;

/// Which of §4's three forms a request takes. One verb, three forms (§9 (a), owner ruling).
#[derive(Debug, Clone)]
pub enum BrowseForm {
    /// `parent` and `q` absent: the layer's artifacts with **no served ancestor**, at every level
    /// of a levelled layer unless the request names one.
    Roots,
    /// `parent` given: the artifacts it is a nearest served ancestor of, at any level, with the
    /// requested artifact's own nearest served ancestors beside them.
    Children(MosaicaId),
    /// `q` given: the artifacts whose key or [`BrowseRow::name`] contains `q` case-insensitively.
    Search(String),
}

/// One `POST /v1/artifacts/browse` request.
#[derive(Debug, Clone)]
pub struct BrowseRequest<'a> {
    pub view: &'a str,
    pub layer: &'a str,
    /// The one level the roots and search forms list on a **levelled** layer (`stacked`,
    /// `tiered`), which list every level without it. A `422` on the three one-level kinds, a
    /// kind's levels being deployment schema and a parameter accepted and ignored being a wrong
    /// answer that looks right (§4), and a `422` beside `parent`, since the children form lists
    /// children at every level.
    pub level: Option<u32>,
    pub form: BrowseForm,
    /// The viewport's own `filters` object. Its answer here is a **count per row** rather than the
    /// viewport's boolean, on §4's argument: browse has no tiles and no held payload, so 0104's
    /// two objections — a tile-scoped number beside a whole-view one, and a filter change having
    /// to move one bit of a held row — do not apply.
    pub filter: Option<FilterExpr>,
    /// Clamped to `selection.max_browse_rows`; `0` is a `422` on `/v1/categories`' argument (a
    /// zero-length page is a question with no answer).
    pub limit: usize,
    pub cursor: Option<BrowseCursor>,
    /// The palette size each row's slot is chosen for ([`crate::slots`]); `None` serves no slot.
    pub palette_size: Option<u8>,
    /// Flipped when the client goes away, which ends the request with
    /// [`EngineError::Cancelled`]. `None` is never cancelled.
    pub cancel: Option<crate::CancelToken>,
}

/// A position in the total order: the ordering count descending (`matched_count` under
/// `filters`, `masked_count` otherwise), then `masked_count` descending, then `mosaica_id`
/// ascending.
///
/// **The order is total**, which is what lets a cursor page over tied counts without duplicating
/// or dropping a row: two artifacts with the same counts are separated by their identifiers, and no
/// two artifacts share one.
///
/// It carries counts this principal was already served and an identifier they were already
/// handed, so it discloses nothing a page did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowseCursor {
    /// The ordering count: `matched_count` under `filters`, `masked_count` otherwise.
    pub count: u64,
    pub masked_count: u64,
    pub mosaica_id: u64,
}

impl BrowseCursor {
    /// The wire spelling: `<count>:<masked_count>:<mosaica_id>`, each decimal.
    pub fn parse(text: &str) -> Option<BrowseCursor> {
        let mut parts = text.split(':');
        let cursor = BrowseCursor {
            count: parts.next()?.parse().ok()?,
            masked_count: parts.next()?.parse().ok()?,
            mosaica_id: parts.next()?.parse().ok()?,
        };
        parts.next().is_none().then_some(cursor)
    }

    pub fn encode(&self) -> String {
        format!("{}:{}:{}", self.count, self.masked_count, self.mosaica_id)
    }

    /// Where this cursor sits in the total order, as the key rows are sorted by.
    fn key(&self) -> (std::cmp::Reverse<u64>, std::cmp::Reverse<u64>, u64) {
        (
            std::cmp::Reverse(self.count),
            std::cmp::Reverse(self.masked_count),
            self.mosaica_id,
        )
    }
}

/// One row of a browse page: the artifacts frame's identity row plus a name and the counts (§4).
///
/// **No geometry.** This verb serves a hierarchy; a layer that draws is drawn by the viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseRow {
    pub mosaica_id: MosaicaId,
    /// The publisher's own key, where they supplied one.
    pub key: Option<String>,
    /// The artifact's first supplied text content where it has one, and otherwise the first text
    /// of an artifact attached to it that `POST /v1/artifacts` would serve this principal. Label
    /// layers are taken in the order `/v1/meta` lists layers; within one, by level, then keyed
    /// artifacts by key, then keyless ones by publication, which is the order the viewport's
    /// frame serves a level in and the same for a built level and a published one. `None` where
    /// neither gives text.
    pub name: Option<String>,
    /// `|membership ∩ M_auth|`, computed per request and never precomputed (C8). **It never moves
    /// with the filter.**
    pub masked_count: u64,
    /// `|membership ∩ M_auth ∩ filter|`, or `None` where the request carried no `filters` — *there
    /// was no question*, exactly as the artifacts frame's `matched` is null then.
    pub matched_count: Option<u64>,
    /// The declared level on a levelled layer; the depth of this artifact in the **gated** forest
    /// on a treed one, which is the same definition the viewport applies over its own response
    /// read over the whole layer; `0` on a flat layer.
    pub rung: u32,
    /// The artifacts **this principal is served** nearest above this one on every path, ascending:
    /// its parents where they are served, and a withheld parent's own nearest served ancestors in
    /// its place, as the artifacts frame names them (C29, per entry).
    pub parent_ids: Vec<MosaicaId>,
    /// How many artifacts this principal is served name this one in their `parent_ids`: the size
    /// of this artifact's children form.
    pub child_count: u64,
    /// This viewer's slot for the artifact, where the request named a palette size: the slot
    /// the artifacts frame serves it with.
    pub slot: Option<u8>,
}

/// One browse page.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseOut {
    pub artifacts: Vec<BrowseRow>,
    /// The requested artifact's own nearest served ancestors, **present on the children form
    /// only** and `[]` on the others rather than absent, so a client reads one shape.
    pub parents: Vec<BrowseRow>,
    /// The cursor for the next page, or `None` where this page is the last.
    pub next: Option<BrowseCursor>,
}

/// Why a browse request could not be answered. Every one is the caller's fault and a `422` — each
/// names deployment schema the caller reads off `/v1/meta`, so refusing discloses nothing.
///
/// **What is deliberately absent is a refusal about an artifact.** A `parent` that names nothing,
/// one of another layer, one suppressed and one below this principal's own criterion all answer an
/// empty page, on the same rule §3's `member_of` leaf follows: refusing would make the verb an
/// existence oracle over exactly what the criterion withholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseRefused {
    /// A layer outside the list `/v1/meta` publishes to this principal. The registry's probe
    /// answers alike for a gate-failed name and a never-registered one, so this confirms nothing.
    UnknownLayer(String),
    /// A layer that hangs from another has no lineage of its own and its text is what its target
    /// shows as a name, so it is not browsed separately (§4).
    AttachedLayer(String),
    /// `level` on a kind with one level (`flat`, `nested`, `dag`), or a level this layer does not
    /// hold. A kind's levels are deployment schema, and a parameter accepted and ignored is a
    /// wrong answer that looks right.
    Level { layer: String, detail: String },
    /// `level` beside `parent`: the children form lists a parent's children at every level, so
    /// a level would be ignored.
    LevelWithParent,
    /// `limit = 0`: a zero-length page is a question with no answer.
    ZeroLimit,
}

impl std::fmt::Display for BrowseRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowseRefused::UnknownLayer(layer) => write!(
                f,
                "'{layer}' is not a layer this deployment publishes to you — /v1/meta lists the \
                 layers that can be browsed"
            ),
            BrowseRefused::AttachedLayer(layer) => write!(
                f,
                "layer '{layer}' attaches to another, so it has no lineage of its own and its text \
                 is what its target shows as a name. Browse the layer it hangs from"
            ),
            BrowseRefused::Level { layer, detail } => {
                write!(f, "layer '{layer}': {detail}")
            }
            BrowseRefused::LevelWithParent => write!(
                f,
                "`level` and `parent` were both given, but a parent's children are listed at \
                 every level. Omit `level` to list them, or omit `parent` to list the roots at \
                 that level"
            ),
            BrowseRefused::ZeroLimit => write!(
                f,
                "`limit` is 0, which asks for a page with no rows. Omit it for the deployment's \
                 default, or name a positive number up to `selection.max_browse_rows`"
            ),
        }
    }
}

impl crate::Engine {
    /// Serve one page of a layer's hierarchy — see this module's doc.
    pub fn browse(&self, session: &crate::Session, req: BrowseRequest<'_>) -> Result<BrowseOut> {
        if req.limit == 0 {
            return Err(EngineError::BrowseRefused(BrowseRefused::ZeroLimit));
        }
        if req.level.is_some() && matches!(req.form, BrowseForm::Children(_)) {
            return Err(EngineError::BrowseRefused(BrowseRefused::LevelWithParent));
        }
        let generation = self.generation.load_full();
        let view = req.view;
        crate::viewport::check_view(&generation, view)?;

        // **The layer, resolved through the principal's own reachable set** — one probe for a
        // gate-failed name and a never-registered one alike, so a `422` here confirms nothing this
        // principal was not already told by `/v1/meta`.
        let refused = |refusal| {
            EngineError::BrowseRefused(match refusal {
                LayerRefusal::Unknown => BrowseRefused::UnknownLayer(req.layer.to_string()),
                LayerRefusal::OneLevel(kind) => BrowseRefused::Level {
                    layer: req.layer.to_string(),
                    detail: format!(
                        "its kind is '{}', which has one level, so `level` names nothing. \
                         Omit it",
                        kind_name(kind)
                    ),
                },
                LayerRefusal::NoSuchLevel { held } => BrowseRefused::Level {
                    layer: req.layer.to_string(),
                    detail: format!(
                        "it holds {held} level(s), so level {} names nothing — /v1/meta \
                         publishes the level set",
                        req.level.unwrap_or(0)
                    ),
                },
            })
        };
        let layer = self
            .readable_layer(session, &generation, req.layer, view)
            .map_err(refused)?;
        if !layer.declaration.depends_on.is_empty() {
            return Err(EngineError::BrowseRefused(BrowseRefused::AttachedLayer(
                req.layer.to_string(),
            )));
        }
        check_level(&layer, req.level).map_err(refused)?;
        let levelled = matches!(
            layer.declaration.hierarchy.kind,
            HierarchyKind::Stacked | HierarchyKind::Tiered
        );
        let within = crate::lineage::within_level(layer.declaration.hierarchy.kind);

        // `open_view` laps into a probe; this verb publishes no per-stage timings, so it is given
        // one and its laps are dropped. Named `_probe` rather than silenced afterwards, so that a
        // stage field arriving here is a change to this line and not to a discard.
        let mut _probe = crate::timing::Probe::new();
        let crate::viewport::OpenView {
            served: served_view,
            mask,
            ..
        } = self.open_view(session, &generation, view, &req.cancel, &mut _probe)?;

        // **The filter, evaluated once for the request and over the whole view.** Every route it
        // may take is asked for its whole-view answer: an entity-space verdict is projected whole,
        // a region is whole-view by construction, and a leaf over a render-only column is scanned
        // over every row of the view — the owner's ruling of §9 (d), and the one whole-view pass
        // this design adds.
        let filter_rows = match &req.filter {
            None => None,
            Some(expr) => Some(
                self.whole_view_filter_rows(&served_view, &mask, expr, &req.cancel)?
                    .0,
            ),
        };

        let mut levels: Vec<ReadLevel> = Vec::with_capacity(layer.runs.len());
        for level in 0..layer.runs.len() as u32 {
            let mut read = self.read_level(&served_view, &mask, &layer, level, false)?;
            if let Some(filter_rows) = &filter_rows {
                read.filtered = read.filtered_counts(self, &mask, filter_rows, &req.cancel)?;
            }
            levels.push(read);
        }
        let no_dependency = |_: &Attachment| false;
        let gate = Gate {
            engine: self,
            layer: &layer,
            mask: &mask,
            filter_rows: filter_rows.as_deref(),
            views: levels
                .iter()
                .map(|read| read.view(self, &served_view, &mask, &layer, &no_dependency))
                .collect(),
            levels: &levels,
            below: match within {
                Some(true) => Below::Within(
                    levels
                        .iter()
                        .map(|read| {
                            self.level_lineage(
                                &layer,
                                read.level,
                                read.forms.lineage,
                                read.forms.first(),
                            )
                        })
                        .collect(),
                ),
                Some(false) => Below::Coarser(coarser_children(&levels)),
                None => Below::None,
            },
            within,
            at: RefCell::default(),
            gated: RefCell::default(),
            nearest: RefCell::default(),
            rungs: RefCell::default(),
        };

        // **The layers attached to this one that this principal reads**, each level under this
        // request's mask, and each artifact of them judged by the viewport's own verdict with the
        // viewport's dependency hook. They are read on the first row that has no text of its own,
        // and a name is looked up only for a row that needs one: the rows of a page, and the
        // candidates of a search whose key does not match.
        let reachable = self.reachable_layers(session);
        let ctx = DependencyContext::new(&served_view, &mask, &reachable);
        let dependency_served = self.dependency_gate(&ctx);
        // Each level's slots, worked out as the viewport works them out.
        let mut slots: Vec<Option<Arc<crate::slots::LevelSlots>>> = Vec::new();
        if let Some(palette) = req.palette_size {
            for level in 0..layer.runs.len() as u32 {
                slots.push(self.cluster_slots(
                    &served_view,
                    &mask,
                    &layer,
                    level,
                    palette,
                    &dependency_served,
                )?);
            }
        }
        let attached = AttachedLevels::default();
        let namer = Namer::new(
            self,
            &served_view,
            &mask,
            &layer,
            &reachable,
            &ctx,
            &dependency_served,
            &attached,
        );
        let resolved: RefCell<HashMap<usize, Option<String>>> = RefCell::default();
        let name_of = |at: usize| -> Option<String> {
            if let Some(name) = resolved.borrow().get(&at) {
                return name.clone();
            }
            let g = gate.get(at);
            let name = namer.name(g.name, g.level, g.ordinal);
            resolved.borrow_mut().insert(at, name.clone());
            name
        };

        // The form's own candidate set, taken over the gated artifacts and never over the level's
        // population.
        let (candidates, parents): (Vec<usize>, Vec<usize>) = match &req.form {
            BrowseForm::Roots => (
                gate.every(req.level)
                    .into_iter()
                    .filter(|&at| gate.nearest(at).is_empty())
                    .collect(),
                Vec::new(),
            ),
            BrowseForm::Children(id) => {
                // The requested artifact, under its own criterion. One that fails it — or names
                // nothing, or belongs to another layer — answers an empty page identically to a
                // leaf: this is §3's empty-operand rule, one verb over.
                let Some(at) = gate.of_id(*id) else {
                    return Ok(BrowseOut {
                        artifacts: Vec::new(),
                        parents: Vec::new(),
                        next: None,
                    });
                };
                (gate.below(at), gate.nearest(at).to_vec())
            }
            BrowseForm::Search(q) => {
                let needle = q.to_lowercase();
                let found = |text: Option<String>| {
                    text.is_some_and(|text| text.to_lowercase().contains(&needle))
                };
                let every = gate.every(req.level);
                let keys = gate.keys(&every);
                (
                    every
                        .into_iter()
                        .zip(keys)
                        .filter(|(at, key)| found(key.clone()) || found(name_of(*at)))
                        .map(|(at, _)| at)
                        .collect(),
                    Vec::new(),
                )
            }
        };

        // **The total order**, [`BrowseCursor`]'s: a row tied on its matched count is placed by
        // its masked count, so the larger artifact comes first, and then by `mosaica_id`, so a
        // cursor over tied counts neither duplicates nor drops a row.
        let cursor_of = |at: &usize| {
            let g = gate.get(*at);
            BrowseCursor {
                count: g.matched_count.unwrap_or(g.masked_count),
                masked_count: g.masked_count,
                mosaica_id: g.mosaica_id.raw(),
            }
        };
        let mut candidates: Vec<(usize, BrowseCursor)> =
            candidates.iter().map(|at| (*at, cursor_of(at))).collect();
        candidates.sort_by_key(|(_, cursor)| cursor.key());
        let after = match req.cursor {
            None => 0,
            Some(cursor) => candidates.partition_point(|(_, at)| at.key() <= cursor.key()),
        };
        let page: Vec<usize> = candidates
            .iter()
            .skip(after)
            .take(req.limit)
            .map(|&(at, _)| at)
            .collect();
        let next = (after + page.len() < candidates.len())
            .then(|| page.last().map(cursor_of))
            .flatten();

        let rows_of = |ats: &[usize]| -> Vec<BrowseRow> {
            let keys = gate.keys(ats);
            ats.iter()
                .zip(keys)
                .map(|(&at, key)| {
                    let g = gate.get(at);
                    BrowseRow {
                        mosaica_id: g.mosaica_id,
                        key,
                        name: name_of(at),
                        masked_count: g.masked_count,
                        matched_count: g.matched_count,
                        rung: if levelled { g.level } else { gate.rung(at) },
                        parent_ids: {
                            let mut ids: Vec<MosaicaId> = gate
                                .nearest(at)
                                .iter()
                                .map(|&up| gate.get(up).mosaica_id)
                                .collect();
                            ids.sort_unstable_by_key(|id| id.raw());
                            ids
                        },
                        // The served artifacts naming this one among their nearest served
                        // parents, which is the children form's own candidate set. A `flat` or
                        // `stacked` layer has no links to count.
                        child_count: match within {
                            Some(_) => gate.below(at).len() as u64,
                            None => 0,
                        },
                        slot: slots
                            .get(g.level as usize)
                            .and_then(Option::as_ref)
                            .and_then(|slots| slots.get(g.ordinal)),
                    }
                })
                .collect()
        };
        let mut parent_rows = rows_of(&parents);
        parent_rows.sort_by_key(|row| row.mosaica_id.raw());
        parent_rows.dedup_by_key(|row| row.mosaica_id.raw());
        let artifacts = rows_of(&page);
        ctx.finish()?;
        Ok(BrowseOut {
            artifacts,
            parents: parent_rows,
            next,
        })
    }
}

/// One served artifact of the layer, as the gate left it: what its row needs.
#[derive(Clone)]
struct Gated {
    level: u32,
    ordinal: u32,
    masked_count: u64,
    /// `|membership ∩ M_auth ∩ filter|` under `filters`, `None` without.
    matched_count: Option<u64>,
    mosaica_id: MosaicaId,
    /// The artifact's own first supplied text, from the content this viewer is served.
    name: Option<String>,
}

/// Where a layer's edges lead downward: the children each artifact's parent edges make it, by the
/// edges [`crate::lineage::nearest_served`] climbs.
enum Below {
    /// A `nested` or `dag` layer's in-level edges, through each level's held lineage.
    Within(Vec<Arc<crate::cut::Lineage>>),
    /// A `tiered` layer's edges to coarser levels, by parent.
    Coarser(FxHashMap<(u32, u32), Vec<(u32, u32)>>),
    /// A kind with no edges.
    None,
}

/// A `tiered` layer's children by parent, read from every level's edges to coarser levels.
fn coarser_children(levels: &[ReadLevel]) -> FxHashMap<(u32, u32), Vec<(u32, u32)>> {
    let mut children: FxHashMap<(u32, u32), Vec<(u32, u32)>> = FxHashMap::default();
    for read in levels {
        let rows = read.forms.first();
        for ordinal in 0..rows.len() as u32 {
            for parent in rows.parents(ordinal) {
                if parent.level < read.level {
                    children
                        .entry((parent.level, parent.ordinal))
                        .or_default()
                        .push((read.level, ordinal));
                }
            }
        }
    }
    children
}

/// The viewport's gate over one layer for one request, asked one artifact at a time and holding
/// each answer, so a form reads the artifacts its answer depends on and no others: the children
/// form the artifacts below the one requested down to the nearest served ones, and its parents the
/// ones above it up to the nearest served ones.
struct Gate<'a> {
    engine: &'a Engine,
    layer: &'a RegisteredLayer,
    mask: &'a ShardedMask,
    filter_rows: Option<&'a [croaring::Bitmap]>,
    levels: &'a [ReadLevel],
    views: Vec<ArtifactView<'a, EffectiveMask>>,
    below: Below,
    within: Option<bool>,
    /// Each artifact asked about, by `(level, ordinal)`, and its index in `gated` where served.
    at: RefCell<FxHashMap<(u32, u32), Option<usize>>>,
    gated: RefCell<Vec<Gated>>,
    /// The nearest served ancestors of each served artifact asked about, by index in `gated`.
    nearest: RefCell<FxHashMap<usize, Rc<[usize]>>>,
    rungs: RefCell<FxHashMap<usize, u32>>,
}

impl Gate<'_> {
    /// The artifact at `ordinal` of `level` where this viewer is served it, by its index.
    fn served(&self, level: u32, ordinal: u32) -> Option<usize> {
        if let Some(&at) = self.at.borrow().get(&(level, ordinal)) {
            return at;
        }
        let at = self.verdict(level, ordinal).map(|gated| {
            let mut held = self.gated.borrow_mut();
            held.push(gated);
            held.len() - 1
        });
        self.at.borrow_mut().insert((level, ordinal), at);
        at
    }

    fn verdict(&self, level: u32, ordinal: u32) -> Option<Gated> {
        let read = self.levels.get(level as usize)?;
        if ordinal as usize >= read.forms.first().len() {
            return None;
        }
        let entity = self.layer.runs[level as usize]
            .entity_of(u64::from(ordinal))
            .map(EntityId::new)?;
        let ArtifactVerdict::Serve {
            masked_count,
            position,
        } = self.views[level as usize].verdict(entity, ordinal)
        else {
            return None;
        };
        let mosaica_id = self.engine.artifact_id(self.layer.space, entity)?;
        // Content decides servability as well as text: a viewer containing no content's
        // generating set receives no artifact at all rather than one with its description
        // missing (decision 0076), which is why this runs inside the gate and not beside the row
        // build.
        let content = read.content(self.engine, self.layer, ordinal, entity, position)?;
        Some(Gated {
            level,
            ordinal,
            masked_count,
            matched_count: self
                .filter_rows
                .map(|rows| read.matched_count(ordinal, self.mask, rows)),
            mosaica_id,
            name: content.name(),
        })
    }

    fn get(&self, at: usize) -> Gated {
        self.gated.borrow()[at].clone()
    }

    /// Every served artifact of the layer, or of `level` where one is named.
    fn every(&self, level: Option<u32>) -> Vec<usize> {
        let mut every = Vec::new();
        for (walked, read) in self.levels.iter().enumerate() {
            let walked = walked as u32;
            if level.is_some_and(|level| level != walked) {
                continue;
            }
            every.extend(
                (0..read.forms.first().len() as u32).filter_map(|o| self.served(walked, o)),
            );
        }
        every
    }

    /// The served artifact `id` names, where it is one of this layer's.
    fn of_id(&self, id: MosaicaId) -> Option<usize> {
        let (space, entity) = self.engine.identity_key.invert_artifact(id)?;
        if space != self.layer.space {
            return None;
        }
        self.layer
            .runs
            .iter()
            .enumerate()
            .find_map(|(level, runs)| {
                let ordinal = u32::try_from(runs.ordinal_of(entity.raw())?).ok()?;
                self.served(level as u32, ordinal)
            })
    }

    /// The served artifacts nearest above `at` on every path ([`crate::lineage`]), by index,
    /// ascending.
    fn nearest(&self, at: usize) -> Rc<[usize]> {
        if let Some(held) = self.nearest.borrow().get(&at) {
            return Rc::clone(held);
        }
        let nearest: Rc<[usize]> = match self.within {
            None => Rc::from([]),
            Some(within) => {
                let g = self.get(at);
                crate::lineage::nearest_served(
                    |level, ordinal| {
                        self.levels
                            .get(level as usize)
                            .map_or(&[], |read| read.forms.first().parents(ordinal))
                    },
                    |level, ordinal| self.served(level, ordinal),
                    (g.level, g.ordinal),
                    within,
                )
                .into()
            }
        };
        self.nearest.borrow_mut().insert(at, Rc::clone(&nearest));
        nearest
    }

    /// The served artifacts that name `at` among their nearest served ancestors, by index,
    /// ascending: its children where they are served, and a withheld child's own nearest served
    /// descendants in its place. Walks down from `at` through the withheld artifacts below it, by
    /// the edges [`Self::nearest`] climbs, so an artifact is listed here exactly where `at` is in
    /// its [`Self::nearest`].
    fn below(&self, at: usize) -> Vec<usize> {
        let g = self.get(at);
        let start = (g.level, g.ordinal);
        let children = |(level, ordinal): (u32, u32)| -> Vec<(u32, u32)> {
            match &self.below {
                Below::Within(lineages) => lineages
                    .get(level as usize)
                    .map_or(&[][..], |lineage| lineage.children_of_node(ordinal))
                    .iter()
                    .map(|&child| (level, child))
                    .collect(),
                Below::Coarser(children) => {
                    children.get(&(level, ordinal)).cloned().unwrap_or_default()
                }
                Below::None => Vec::new(),
            }
        };
        let mut found = Vec::new();
        let mut seen: FxHashSet<(u32, u32)> = FxHashSet::default();
        seen.insert(start);
        let mut walk = children(start);
        while let Some(next) = walk.pop() {
            if !seen.insert(next) {
                continue;
            }
            match self.served(next.0, next.1) {
                Some(child) => found.push(child),
                None => walk.extend(children(next)),
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }

    /// The depth of `at` in the forest its nearest served ancestors form: the longest path from a
    /// served artifact with none. An ancestor met again on its own path adds nothing.
    fn rung(&self, at: usize) -> u32 {
        const PENDING: u32 = u32::MAX;
        let mut stack = vec![at];
        while let Some(&top) = stack.last() {
            let known = self.rungs.borrow().get(&top).copied();
            match known {
                None => {
                    self.rungs.borrow_mut().insert(top, PENDING);
                    let ups = self.nearest(top);
                    let rungs = self.rungs.borrow();
                    stack.extend(ups.iter().filter(|up| !rungs.contains_key(up)));
                }
                Some(PENDING) => {
                    let ups = self.nearest(top);
                    let rung = {
                        let rungs = self.rungs.borrow();
                        ups.iter()
                            .filter_map(|up| rungs.get(up).copied())
                            .filter(|&k| k != PENDING)
                            .map(|k| k + 1)
                            .max()
                            .unwrap_or(0)
                    };
                    self.rungs.borrow_mut().insert(top, rung);
                    stack.pop();
                }
                Some(_) => {
                    stack.pop();
                }
            }
        }
        self.rungs.borrow()[&at]
    }

    /// The publisher's key of each artifact of `ats`, read under one hold of the store.
    fn keys(&self, ats: &[usize]) -> Vec<Option<String>> {
        let name = self.layer.declaration.name.as_str();
        let gated = self.gated.borrow();
        self.engine.write.live().with_artifacts(|store| {
            ats.iter()
                .map(|&at| {
                    let g = &gated[at];
                    store.get(name, g.level, g.ordinal)?.key.clone()
                })
                .collect()
        })
    }
}

/// The attached layers a [`Namer`] reads, each with its levels read under the request's mask:
/// read on the first name that needs one.
pub(crate) type AttachedLevels = OnceCell<Vec<(RegisteredLayer, Vec<ReadLevel>)>>;

/// An artifact's name as this viewer is shown it: its own text where it has some, otherwise the
/// text of the first artifact attached to it that this viewer is served, the attached layers taken
/// in the principal's reachable order and each level's attached artifacts by key, then the keyless
/// by ordinal, as the viewport orders them.
pub(crate) struct Namer<'a, 'c> {
    engine: &'a Engine,
    served: &'a ServedView<'a>,
    mask: &'a ShardedMask,
    layer: &'a RegisteredLayer,
    reachable: &'a mosaica_lifecycle::ResolvedLayers,
    ctx: &'a DependencyContext<'c>,
    dependency_served: &'a dyn Fn(&Attachment) -> bool,
    attached: &'a AttachedLevels,
    views: OnceCell<Vec<Vec<ArtifactView<'a, EffectiveMask>>>>,
}

impl<'a, 'c> Namer<'a, 'c> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        engine: &'a Engine,
        served: &'a ServedView<'a>,
        mask: &'a ShardedMask,
        layer: &'a RegisteredLayer,
        reachable: &'a mosaica_lifecycle::ResolvedLayers,
        ctx: &'a DependencyContext<'c>,
        dependency_served: &'a dyn Fn(&Attachment) -> bool,
        attached: &'a AttachedLevels,
    ) -> Self {
        Namer {
            engine,
            served,
            mask,
            layer,
            reachable,
            ctx,
            dependency_served,
            attached,
            views: OnceCell::new(),
        }
    }

    /// The name of the artifact at `ordinal` of `level`, whose own text, where this viewer is
    /// served its content, is `own`.
    pub(crate) fn name(&self, own: Option<String>, level: u32, ordinal: u32) -> Option<String> {
        own.or_else(|| self.attached_name(level, ordinal))
    }

    fn attached_name(&self, level: u32, ordinal: u32) -> Option<String> {
        let (engine, served, mask) = (self.engine, self.served, self.mask);
        let target_layer = self.layer.declaration.name.as_str();
        let target = self.layer.runs[level as usize]
            .entity_of(u64::from(ordinal))
            .map(EntityId::new)?;
        let layers = self.attached.get_or_init(|| {
            self.reachable
                .names()
                .filter_map(|name| {
                    engine
                        .readable_layer(served.session, served.generation, name, served.name)
                        .ok()
                })
                .filter(|label| {
                    label
                        .declaration
                        .depends_on
                        .iter()
                        .any(|d| d == target_layer)
                })
                .map(|label| {
                    let levels = (0..label.runs.len() as u32)
                        .filter_map(|level| {
                            self.ctx
                                .held(engine.read_level(served, mask, &label, level, false))
                        })
                        .collect();
                    (label, levels)
                })
                .collect()
        });
        let views = self.views.get_or_init(|| {
            layers
                .iter()
                .map(|(label, levels)| {
                    levels
                        .iter()
                        .map(|read| read.view(engine, served, mask, label, self.dependency_served))
                        .collect()
                })
                .collect()
        });
        layers
            .iter()
            .zip(views)
            .find_map(|((label, levels), views)| {
                levels.iter().zip(views).find_map(|(read, view)| {
                    // An edge naming an entity the target slot no longer holds is into an
                    // artifact since republished over.
                    let rows = read.forms.first();
                    let mut attached: Vec<u32> = rows
                        .records()
                        .attached_to(self.layer.space, level, ordinal)
                        .filter(|&at| rows.attachment(at).is_some_and(|a| a.entity == target))
                        .collect();
                    if attached.len() > 1 {
                        let name = label.declaration.name.as_str();
                        let mut keyed: Vec<(Option<String>, u32)> =
                            engine.write.live().with_artifacts(|store| {
                                attached
                                    .iter()
                                    .map(|&at| {
                                        (
                                            store
                                                .get(name, read.level, at)
                                                .and_then(|r| r.key.clone()),
                                            at,
                                        )
                                    })
                                    .collect()
                            });
                        keyed.sort_unstable_by(|(a, at), (b, bt)| {
                            (a.is_none(), a, at).cmp(&(b.is_none(), b, bt))
                        });
                        attached = keyed.into_iter().map(|(_, at)| at).collect();
                    }
                    attached.into_iter().find_map(|at| {
                        let entity = label.runs[read.level as usize]
                            .entity_of(u64::from(at))
                            .map(EntityId::new)?;
                        let ArtifactVerdict::Serve { position, .. } = view.verdict(entity, at)
                        else {
                            return None;
                        };
                        read.content(engine, label, at, entity, position)?.name()
                    })
                })
            })
    }
}

fn kind_name(kind: HierarchyKind) -> &'static str {
    match kind {
        HierarchyKind::Flat => "flat",
        HierarchyKind::Nested => "nested",
        HierarchyKind::Dag => "dag",
        HierarchyKind::Stacked => "stacked",
        HierarchyKind::Tiered => "tiered",
    }
}
