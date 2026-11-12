//! A filter evaluated in row space: the hot-column scan, the crossing, and the leaf resolvers.

use super::*;
use crate::filter::{as_f64, narrow_hi, narrow_lo, NativeBound, Narrowed};

impl Engine {
    /// `f`, the per-tile crossing of `rows` rows, on the engine's pool; or on the calling thread
    /// where the rows are no more than one chunk, which the crossing walks there without the pool
    /// and for which the handoff to the pool would cost more than the walk.
    fn on_pool<R: Send>(&self, rows: u64, f: impl FnOnce() -> R + Send) -> R {
        if rows <= u64::from(CROSSING_CHUNK_MIN_ROWS) {
            f()
        } else {
            self.pool.install(f)
        }
    }

    /// Cross a filter's entity-space result into one shard's row space for the view, by whichever
    /// of two routes is cheaper ([`crossing_projects`]): projecting the whole result, or walking the
    /// request's own tiles and testing each row's entity. The two agree exactly over every range
    /// the request can ask about. Either route ends with [`EngineError::Cancelled`] once the
    /// request holding `cancel` has gone.
    fn cross_filter_into_row_space(
        &self,
        side: &ServedSide<'_>,
        entities: &croaring::Bitmap,
        domain: &[Range<u32>],
        rows_in_ranges: u64,
        per_tile_only: bool,
        cancel: &Option<CancelToken>,
    ) -> Result<FilterRows> {
        let row_space = &side.data.row_space;
        if !crossing_projects(row_space, entities.cardinality(), rows_in_ranges, per_tile_only) {
            // `None` is the row space declining to invert a row; falling through to the exact
            // route costs latency only, where trusting a partial answer would drop rows.
            if let Some(rows) = self.on_pool(rows_in_ranges, || {
                per_tile_crossing(row_space, entities, domain, rows_in_ranges, cancel)
            })? {
                self.counters
                    .filter_crossings_per_tile
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(FilterRows::Viewport {
                    rows,
                    domain: domain.to_vec(),
                });
            }
        }
        let rows = project_until(row_space, entities, cancel)?;
        self.counters
            .filter_crossings_projected
            .fetch_add(1, Ordering::Relaxed);
        Ok(FilterRows::Complete(rows))
    }

    /// A routed filter's answer as rows over `domain`, ascending, disjoint and merged ranges of
    /// `side`'s row space holding `rows_in_ranges` rows: an entity-space verdict crossed into row
    /// space, or a tree with row-space leaves evaluated there. `per_tile_only` holds either to the
    /// domain rather than letting a whole-view projection be chosen where it is cheaper. It ends
    /// with [`EngineError::Cancelled`] once the request holding `cancel` has gone.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rows_of_routed(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        routed: crate::filter::RoutedFilter,
        domain: &[Range<u32>],
        rows_in_ranges: u64,
        per_tile_only: bool,
        cancel: &Option<CancelToken>,
    ) -> Result<RoutedRows> {
        Ok(match routed {
            crate::filter::RoutedFilter::Entity(entities) => RoutedRows {
                unmasked: Some(entities.cardinality()),
                rows: self.cross_filter_into_row_space(
                    side,
                    &entities,
                    domain,
                    rows_in_ranges,
                    per_tile_only,
                    cancel,
                )?,
                region: None,
            },
            crate::filter::RoutedFilter::Row(tree) => {
                let rows = self.evaluate_row_route(
                    &tree,
                    served,
                    side,
                    domain,
                    rows_in_ranges,
                    per_tile_only,
                    cancel,
                )?;
                self.counters.filter_row_routed.fetch_add(1, Ordering::Relaxed);
                RoutedRows {
                    // A region's interior rows have not met the mask, so their count is a
                    // pre-mask quantity about the region, and is not taken.
                    unmasked: (!tree.has_region()).then(|| rows.rows().cardinality()),
                    region: tree.region_verdict(),
                    rows,
                }
            }
        })
    }

    /// Evaluate a routed filter tree with row-space leaves over the request's own rows, exact
    /// over `domain` and silent outside it. A leaf reads every row of the domain whatever the
    /// principal may see, so the bitmap returned here still contains suppressed rows; a caller
    /// narrows the answer only through `EffectiveMask::with_filter`. Code 0 is the vocabulary's
    /// absent sentinel and matches nothing. Every entity-space verdict is crossed in one joint
    /// walk, or projected where cheaper, and the tree combines entirely in row space. A tree
    /// bounded by nothing but region leaves and projected verdicts comes back
    /// [`FilterRows::Complete`]; a render leaf or a per-tile crossing bounds the answer and comes
    /// back [`FilterRows::Viewport`]. A render leaf's rows are read from the filter cache over the
    /// rows it has scanned for the leaf, and scanned over the rest ([`crate::filter_cache`]). The
    /// crossing and each leaf's scan end with [`EngineError::Cancelled`] once the request holding
    /// `cancel` has gone, and a scan that ends so is not kept.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_row_route(
        &self,
        tree: &crate::filter::RowExpr,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        domain: &[Range<u32>],
        rows_in_ranges: u64,
        per_tile_only: bool,
        cancel: &Option<CancelToken>,
    ) -> Result<FilterRows> {
        let row_space = &side.data.row_space;
        let segments = &side.segments[..];
        let total_rows = row_space.total_rows();
        let domain_rows = crate::compose::rows_of(domain);
        // Every entity-space verdict's row image, crossed together in one walk.
        let verdicts = tree.entity_verdicts();
        let mut whole_view = tree.is_whole_view();
        let images: Vec<croaring::Bitmap> = if verdicts.is_empty() {
            Vec::new()
        } else {
            let total_matched: u64 = verdicts.iter().map(|v| v.cardinality()).sum();
            let walks = !crossing_projects(row_space, total_matched, rows_in_ranges, per_tile_only);
            let walked = match walks {
                true => self.on_pool(rows_in_ranges, || {
                    per_tile_crossing_multi(row_space, &verdicts, domain, rows_in_ranges, cancel)
                })?,
                false => None,
            };
            match walked {
                Some(images) => {
                    self.counters
                        .filter_crossings_per_tile
                        .fetch_add(1, Ordering::Relaxed);
                    whole_view = false;
                    images
                }
                None => {
                    self.counters
                        .filter_crossings_projected
                        .fetch_add(1, Ordering::Relaxed);
                    if whole_view {
                        verdicts
                            .iter()
                            .map(|v| project_until(row_space, v, cancel))
                            .collect::<Result<_>>()?
                    } else {
                        // Clamped so the combined answer never claims a row outside what
                        // `FilterRows::Viewport` says was tested.
                        verdicts
                            .iter()
                            .map(|v| Ok(project_until(row_space, v, cancel)?.and(&domain_rows)))
                            .collect::<Result<_>>()?
                    }
                }
            }
        };
        let scope = if whole_view {
            RowScope::WholeView {
                total_rows: u32::try_from(total_rows).unwrap_or(u32::MAX),
            }
        } else {
            RowScope::Domain(&domain_rows)
        };
        let grant = served.session.terms_digest();
        let scan = |column: &str, family: crate::filter::Family, operand: &FilterOperand| {
            let key = crate::filter_cache::row_key_of(
                side.shard,
                grant,
                served.name,
                side.data.incarnation,
                column,
                operand,
            );
            let values = LeafValues::of(family, operand);
            crate::filter_cache::rows_answer(
                &self.filter_cache,
                side.shard,
                key,
                domain,
                &domain_rows,
                |ranges| scan_rows(segments, ranges, column, values.predicate(), cancel),
            )
        };
        let mut next_image = 0usize;
        let rows = self
            .pool
            .install(|| eval_row_expr(tree, &images, &mut next_image, &scan, &scope))?;
        Ok(if whole_view {
            FilterRows::Complete(rows)
        } else {
            FilterRows::Viewport {
                rows,
                domain: domain.to_vec(),
            }
        })
    }

    /// `expr` routed in `side` and, where it routes to row space, evaluated over every row of the
    /// view it can match: the rows of the drawn region that bounds it ([`bounding_region`]), and
    /// the whole view where none does. `choice` decides a column with both routes, the same in every
    /// side of the request ([`Engine::region_costs`]).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn route_over_view(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        route: &RouteFilter<'_>,
        expr: &crate::filter::FilterExpr,
        choice: crate::filter::RouteChoice<'_>,
    ) -> Result<OverView> {
        let domain = match bounding_region(expr) {
            Some(leaf) => self
                .region_rows(leaf, served, side, resolved, cancel)
                .map_err(filter_refusal)?
                .domain,
            None => None,
        };
        let total = side.data.row_space.total_rows();
        let whole = 0..u32::try_from(total).unwrap_or(u32::MAX);
        let (domain, rows): (&[Range<u32>], u64) = match &domain {
            Some(domain) => (domain, domain.iter().map(|r| u64::from(r.end - r.start)).sum()),
            None => (std::slice::from_ref(&whole), total),
        };
        Ok(match route(expr, choice)? {
            crate::filter::RoutedFilter::Entity(entities) => OverView::Entities(entities),
            crate::filter::RoutedFilter::Row(tree) => {
                let region = tree.region_verdict();
                // Exact over the whole view: every match lies inside the bounding region, whose
                // rows the domain holds.
                let rows = match self
                    .evaluate_row_route(&tree, served, side, domain, rows, false, cancel)?
                {
                    FilterRows::Complete(rows) | FilterRows::Viewport { rows, .. } => rows,
                };
                OverView::Rows { rows, region }
            }
        })
    }

    /// What a column with both routes costs in `expr` over the whole view, decided once for every
    /// side `mask` reads as the viewport decides it for a request's tiles: over the rows of the
    /// drawn region that bounds `expr` in every side. `None` where no region bounds it, and such a
    /// column then takes the entity route.
    pub(crate) fn region_costs(
        &self,
        served: &ServedView<'_>,
        mask: &ShardedMask,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        expr: &crate::filter::FilterExpr,
    ) -> Result<Option<ViewLeafCosts>> {
        let Some(leaf) = bounding_region(expr) else {
            return Ok(None);
        };
        let mut domains = Vec::new();
        for (_, side, _) in served.filtered_sides(mask) {
            let held = self
                .region_rows(leaf, served, side, resolved, cancel)
                .map_err(filter_refusal)?;
            let Some(domain) = held.domain else {
                return Ok(None);
            };
            domains.push((side, domain));
        }
        let sides: Vec<_> = domains.iter().map(|(side, domain)| (*side, &domain[..])).collect();
        Ok(Some(self.leaf_costs(served, &sides, mask.visible_total(), [expr])))
    }

    /// [`ViewLeafCosts`] for every leaf of `exprs` over `sides`, each with the rows the request
    /// reads there.
    pub(crate) fn leaf_costs<'e>(
        &self,
        served: &ServedView<'_>,
        sides: &[(&ServedSide<'_>, &[Range<u32>])],
        visible: u64,
        exprs: impl IntoIterator<Item = &'e crate::filter::FilterExpr>,
    ) -> ViewLeafCosts {
        fn leaves<'e>(
            expr: &'e crate::filter::FilterExpr,
            out: &mut Vec<(&'e str, &'e FilterOperand)>,
        ) {
            use crate::filter::FilterExpr;
            match expr {
                FilterExpr::Leaf { column, operand } => out.push((column, operand)),
                FilterExpr::AllOf(kids) | FilterExpr::AnyOf(kids) | FilterExpr::NoneOf(kids) => {
                    kids.iter().for_each(|kid| leaves(kid, out))
                }
                FilterExpr::Region(_) | FilterExpr::MemberOf(_) => {}
            }
        }
        let mut named = Vec::new();
        exprs.into_iter().for_each(|expr| leaves(expr, &mut named));
        let grant = served.session.terms_digest();
        let priced = named
            .into_iter()
            .map(|(column, operand)| {
                let mut cost = crate::filter::LeafCost {
                    posting: Some(0),
                    kept: 0,
                };
                for (side, domain) in sides {
                    let posting = side.shard.filter_columns.posting_entities(column, operand);
                    cost.posting = cost.posting.zip(posting).map(|(sum, held)| sum + held);
                    let key = crate::filter_cache::row_key_of(
                        side.shard,
                        grant,
                        served.name,
                        side.data.incarnation,
                        column,
                        operand,
                    );
                    cost.kept += crate::filter_cache::kept_rows(&self.filter_cache, key, domain);
                }
                (column.to_string(), operand.clone(), cost)
            })
            .collect();
        ViewLeafCosts {
            rows: sides
                .iter()
                .flat_map(|(_, domain)| domain.iter())
                .map(|r| u64::from(r.end - r.start))
                .sum(),
            visible,
            priced,
        }
    }
}

/// What each leaf over a column with both routes costs over the rows one request reads in each
/// side: the postings it holds and the rows the filter cache already holds for it, summed over the
/// sides.
pub(crate) struct ViewLeafCosts {
    rows: u64,
    visible: u64,
    priced: Vec<(String, FilterOperand, crate::filter::LeafCost)>,
}

impl ViewLeafCosts {
    /// The route each such leaf takes, chosen by these costs.
    pub(crate) fn routes(&self) -> crate::filter::RouteChoice<'_> {
        crate::filter::RouteChoice::Cheaper {
            rows: self.rows,
            visible: self.visible,
            costs: self,
        }
    }
}

impl crate::filter::LeafCosts for ViewLeafCosts {
    fn cost(&self, column: &str, operand: &FilterOperand) -> crate::filter::LeafCost {
        self.priced
            .iter()
            .find(|(c, o, _)| c == column && o == operand)
            .map_or_else(Default::default, |(_, _, cost)| *cost)
    }
}

/// One expression routed in one side by [`Engine::route_filters_under`].
pub(crate) type RouteFilter<'a> = dyn Fn(
        &crate::filter::FilterExpr,
        crate::filter::RouteChoice<'_>,
    ) -> Result<crate::filter::RoutedFilter>
    + 'a;

/// [`Engine::route_over_view`]'s answer.
pub(crate) enum OverView {
    /// The filter routed entirely in entity space, under the candidate.
    Entities(croaring::Bitmap),
    /// The filter's rows over the whole view, before the mask, and the coarsest verdict its region
    /// leaves reached.
    Rows {
        rows: croaring::Bitmap,
        region: Option<crate::region::RegionVerdict>,
    },
}

/// The region leaf every match of `expr` lies inside: `expr` itself, or a kid of an `all_of` at
/// its top, through any number of `all_of`s.
fn bounding_region(expr: &crate::filter::FilterExpr) -> Option<&crate::filter::RegionLeaf> {
    use crate::filter::FilterExpr;
    match expr {
        FilterExpr::Region(leaf) => Some(leaf),
        FilterExpr::AllOf(kids) => kids.iter().find_map(bounding_region),
        _ => None,
    }
}

/// How many times fewer entities a request's tiles must hold than the viewer's visible set before
/// its filter is evaluated over the tiles' entities alone: reaching one entity's value from a row
/// is a random read where scanning the set is a sequential one.
const NARROW_ENTITY_RATIO: u64 = 16;

/// How many times fewer 2¹⁶-entity blocks of a value column the tiles' entities must fall in than
/// the viewer's visible set does. Where the column is not resident, reading one entity's value
/// faults in the kernel's read-ahead around it, which is about a block of ordinals, so a block
/// costs about the same however few of its entities are read; and a read at random costs more
/// than one in sequence.
const NARROW_BLOCK_RATIO: u64 = 4;

/// Whether a request whose tiles span `rows` rows could hold few enough entities, against `visible`
/// in the view, for [`narrowing_pays`] to hold: a row holds one entity, so this is checked before
/// the rows are mapped to theirs.
fn narrowing_may_pay(rows: u64, visible: u64) -> bool {
    rows.saturating_mul(NARROW_ENTITY_RATIO) <= visible
}

/// Whether evaluating a filter over `within`, the visible entities of a request's tiles, costs less
/// than over the viewer's `visible` entities in the side, spread over `visible_blocks` blocks of
/// 2¹⁶ entities: by entities compared, and by blocks read.
///
/// Every count is the viewer's own, and none depends on a filter's values.
fn narrowing_pays(within: &croaring::Bitmap, visible: u64, visible_blocks: u64) -> bool {
    let blocks = u64::from(within.statistics().n_containers);
    within.cardinality().saturating_mul(NARROW_ENTITY_RATIO) <= visible
        && blocks.saturating_mul(NARROW_BLOCK_RATIO) <= visible_blocks
}

/// Rows of a viewport's tiles [`sampled_blocks`] maps to their entities.
const BLOCK_SAMPLE_ROWS: u64 = 4096;

/// How many distinct 2¹⁶-entity blocks hold the visible entities of up to [`BLOCK_SAMPLE_ROWS`]
/// rows spread evenly over `domain`, `rows` rows in all: no more than the blocks all its visible
/// entities fall in. Zero where the row space cannot map a row to its entity.
fn sampled_blocks(
    row_space: &mosaica_store::permutation::RowSpace,
    domain: &[Range<u32>],
    rows: u64,
    visible: &croaring::Bitmap,
) -> u64 {
    let step = rows.div_ceil(BLOCK_SAMPLE_ROWS).max(1);
    let mut blocks = Vec::new();
    let mut next = 0u64;
    let mut before = 0u64;
    for r in domain {
        let len = u64::from(r.end - r.start);
        while next < before + len {
            let row = r.start + (next - before) as u32;
            if let Some(entity) = row_space.entity_of(RowId::new(row)) {
                let entity = entity.raw() as u32;
                if visible.contains(entity) {
                    blocks.push(entity >> 16);
                }
            }
            next += step;
        }
        before += len;
    }
    blocks.sort_unstable();
    blocks.dedup();
    blocks.len() as u64
}

/// [`Engine::rows_of_routed`]'s answer.
pub(crate) struct RoutedRows {
    pub(crate) rows: FilterRows,
    /// The coarsest verdict a region leaf reached.
    pub(crate) region: Option<crate::region::RegionVerdict>,
    /// How many rows or entities matched before the mask, where that is a quantity about the
    /// filter alone: `None` for a tree holding a region.
    pub(crate) unmasked: Option<u64>,
}

/// A render leaf's rows over a row-space evaluation's domain, by its column, family and operand.
type RowLeafScan<'a> =
    dyn Fn(&str, crate::filter::Family, &FilterOperand) -> Result<croaring::Bitmap> + Sync + 'a;

/// The rows a row-space evaluation answers over — see [`Engine::evaluate_row_route`].
enum RowScope<'d> {
    /// Every row of the view: the tree holds nothing the request's domain bounds.
    WholeView { total_rows: u32 },
    /// The request's own rows, as one bitmap, because a sibling leaf is bounded by them.
    Domain(&'d croaring::Bitmap),
}

impl RowScope<'_> {
    /// Every row in scope, which a negation subtracts its matches from.
    fn all_rows(&self) -> croaring::Bitmap {
        match self {
            RowScope::WholeView { total_rows } => croaring::Bitmap::from_range(0..*total_rows),
            RowScope::Domain(rows) => (*rows).clone(),
        }
    }

    /// A whole-view row set, narrowed to the scope where the scope is narrower.
    fn clamp(&self, rows: &croaring::Bitmap) -> croaring::Bitmap {
        match self {
            RowScope::WholeView { .. } => rows.clone(),
            RowScope::Domain(domain) => rows.and(domain),
        }
    }
}

/// Evaluate one routed node over `domain`, in row space. `images` are the pre-crossed row images
/// of the tree's entity-space verdicts, consumed in the order
/// [`crate::filter::RowExpr::entity_verdicts`] collects them; `next_image` is that cursor. `scan`
/// answers a render leaf over the domain.
fn eval_row_expr(
    expr: &crate::filter::RowExpr,
    images: &[croaring::Bitmap],
    next_image: &mut usize,
    scan: &RowLeafScan<'_>,
    scope: &RowScope<'_>,
) -> Result<croaring::Bitmap> {
    use crate::filter::RowExpr;
    match expr {
        RowExpr::Entity(_) => {
            let image = images[*next_image].clone();
            *next_image += 1;
            Ok(image)
        }
        RowExpr::Leaf {
            column,
            family,
            operand,
        } => scan(column, *family, operand),
        RowExpr::Region(region) => Ok(scope.clamp(&region.rows)),
        // Already `membership ∩ M_auth` over the whole view, clamped where a sibling leaf bounds
        // the tree to the request's rows.
        RowExpr::MemberOf(rows) => Ok(scope.clamp(rows)),
        RowExpr::NotInRows(kids) => {
            // Every kid is evaluated, even once the difference is empty: `images` is positional,
            // so a skipped kid would leave the cursor short of the verdicts after it.
            let mut out = scope.all_rows();
            for kid in kids {
                out.andnot_inplace(&eval_row_expr(
                    kid, images, next_image, scan, scope,
                )?);
            }
            Ok(out)
        }
        RowExpr::AllOf(kids) => {
            let mut out: Option<croaring::Bitmap> = None;
            for kid in kids {
                let kid_rows = eval_row_expr(kid, images, next_image, scan, scope)?;
                out = Some(match out {
                    None => kid_rows,
                    Some(mut acc) => {
                        acc.and_inplace(&kid_rows);
                        acc
                    }
                });
            }
            // Unreachable empty: an empty `all_of` is entity-pure and never routes here.
            Ok(out.unwrap_or_default())
        }
        RowExpr::AnyOf(kids) => {
            let mut out = croaring::Bitmap::new();
            for kid in kids {
                out |= eval_row_expr(kid, images, next_image, scan, scope)?;
            }
            Ok(out)
        }
    }
}

/// A row-space leaf's test against one row of the hot column. Absence is a per-family rule: a
/// category's is its vocabulary's reserved code 0; every other family's is a presence bitmap
/// beside the column, since the hot column is non-nullable and an absent number is written as
/// the type's zero, a value a range containing zero must not match.
enum RowPredicate<'a> {
    /// A category's code is non-sentinel and in this set. An empty set matches nothing.
    CodeIn(&'a [u32]),
    /// A number's value equals one of these. An empty set matches nothing.
    NumberIn(&'a [Scalar]),
    /// A number's value lies between these bounds. Either may be absent, an open side.
    Range {
        lo: Option<Endpoint>,
        hi: Option<Endpoint>,
    },
}

impl RowPredicate<'_> {
    /// Does this family read absence from the presence bitmap? A category does not.
    fn reads_presence(&self) -> bool {
        match self {
            RowPredicate::CodeIn(_) => false,
            RowPredicate::NumberIn(_) | RowPredicate::Range { .. } => true,
        }
    }
}

/// One row-space leaf's comparands, owned for as long as the scan borrows them. A family/operand
/// pair the parse would have refused becomes an empty set, which matches nothing, never a panic
/// and never a number compared against a code.
enum LeafValues {
    Codes(Vec<u32>),
    Numbers(Vec<Scalar>),
    Range {
        lo: Option<Endpoint>,
        hi: Option<Endpoint>,
    },
}

impl LeafValues {
    fn of(family: Family, operand: &FilterOperand) -> LeafValues {
        match (family, operand) {
            (Family::Category, FilterOperand::Equals(v)) => LeafValues::Codes(vec![v.raw()]),
            (Family::Category, FilterOperand::In(vs)) => {
                LeafValues::Codes(vs.iter().map(|v| v.raw()).collect())
            }
            (Family::Numeric, FilterOperand::NumEquals(n)) => LeafValues::Numbers(vec![*n]),
            (Family::Numeric, FilterOperand::NumIn(ns)) => LeafValues::Numbers(ns.clone()),
            (Family::Numeric, FilterOperand::Range { lo, hi }) => {
                LeafValues::Range { lo: *lo, hi: *hi }
            }
            _ => LeafValues::Codes(Vec::new()),
        }
    }

    fn predicate(&self) -> RowPredicate<'_> {
        match self {
            LeafValues::Codes(codes) => RowPredicate::CodeIn(codes),
            LeafValues::Numbers(numbers) => RowPredicate::NumberIn(numbers),
            LeafValues::Range { lo, hi } => RowPredicate::Range { lo: *lo, hi: *hi },
        }
    }
}

/// A row-space leaf's predicate resolved against one segment's rendered column, settled once per
/// segment rather than once per row: a range's endpoints narrowed to the column's own type, a
/// needle the type cannot hold gone, a needle set sorted for a binary search. A predicate that
/// can match no row of the column has no representation: [`Prepared::of`] answers `None`.
enum Prepared<'a> {
    Bool(&'a arrow::array::BooleanArray, IntTest<u8>),
    U8(&'a [u8], IntTest<u8>),
    U16(&'a [u16], IntTest<u16>),
    U32(&'a [u32], IntTest<u32>),
    U64(&'a [u64], IntTest<u64>),
    I8(&'a [i8], IntTest<i8>),
    I16(&'a [i16], IntTest<i16>),
    I32(&'a [i32], IntTest<i32>),
    I64(&'a [i64], IntTest<i64>),
    F32(&'a [f32], FloatTest),
    F64(&'a [f64], FloatTest),
}

/// What one stored integer is tested by, at the column's own width. `Eq` is kept apart from a
/// one-value `In` because it is the common shape and one comparison against a constant.
enum IntTest<T> {
    Eq(T),
    /// Sorted and deduplicated: a linear `contains` costs O(needles) per row.
    In(Vec<T>),
    /// Inclusive on both sides, exclusivity folded into the value; `None` is an open side.
    Range { lo: Option<T>, hi: Option<T> },
}

/// What one stored float is tested by. NaN must stay unordered, which narrowing through an
/// integer would destroy, so floats keep the `f64` comparison.
enum FloatTest {
    /// Unsorted: NaN equals nothing, itself included, so a NaN needle matches no row without a
    /// special case.
    In(Vec<f64>),
    /// Each side carries its own inclusivity, there being no next float to fold an exclusive
    /// bound into.
    Range {
        lo: Option<(f64, bool)>,
        hi: Option<(f64, bool)>,
    },
}

impl<'a> Prepared<'a> {
    /// The predicate against one segment's column, or `None` where no row of that segment can
    /// match: a range narrowed to nothing, or a needle set the column's type cannot hold.
    fn of(slice: &ScalarSlice<'a>, predicate: &RowPredicate<'_>) -> Option<Prepared<'a>> {
        match predicate {
            RowPredicate::CodeIn(codes) => match slice {
                ScalarSlice::U8(v) => Some(Prepared::U8(v, code_test(codes)?)),
                ScalarSlice::U16(v) => Some(Prepared::U16(v, code_test(codes)?)),
                ScalarSlice::U32(v) => Some(Prepared::U32(v, code_test(codes)?)),
                _ => None,
            },
            RowPredicate::NumberIn(needles) => match slice {
                ScalarSlice::Bool(a) => Some(Prepared::Bool(a, int_in(needles)?)),
                ScalarSlice::U8(v) => Some(Prepared::U8(v, int_in(needles)?)),
                ScalarSlice::U16(v) => Some(Prepared::U16(v, int_in(needles)?)),
                ScalarSlice::U32(v) => Some(Prepared::U32(v, int_in(needles)?)),
                ScalarSlice::U64(v) => Some(Prepared::U64(v, int_in(needles)?)),
                ScalarSlice::I8(v) => Some(Prepared::I8(v, int_in(needles)?)),
                ScalarSlice::I16(v) => Some(Prepared::I16(v, int_in(needles)?)),
                ScalarSlice::I32(v) => Some(Prepared::I32(v, int_in(needles)?)),
                ScalarSlice::I64(v) | ScalarSlice::TimestampUs(v) => {
                    Some(Prepared::I64(v, int_in(needles)?))
                }
                ScalarSlice::F32(v) => Some(Prepared::F32(v, float_in(needles)?)),
                ScalarSlice::F64(v) => Some(Prepared::F64(v, float_in(needles)?)),
                ScalarSlice::Utf8(_) => None,
            },
            RowPredicate::Range { lo, hi } => match slice {
                // A bool is compared as the 0/1 the entity route stores it as.
                ScalarSlice::Bool(a) => Some(Prepared::Bool(a, int_range(*lo, *hi)?)),
                ScalarSlice::U8(v) => Some(Prepared::U8(v, int_range(*lo, *hi)?)),
                ScalarSlice::U16(v) => Some(Prepared::U16(v, int_range(*lo, *hi)?)),
                ScalarSlice::U32(v) => Some(Prepared::U32(v, int_range(*lo, *hi)?)),
                ScalarSlice::U64(v) => Some(Prepared::U64(v, int_range(*lo, *hi)?)),
                ScalarSlice::I8(v) => Some(Prepared::I8(v, int_range(*lo, *hi)?)),
                ScalarSlice::I16(v) => Some(Prepared::I16(v, int_range(*lo, *hi)?)),
                ScalarSlice::I32(v) => Some(Prepared::I32(v, int_range(*lo, *hi)?)),
                ScalarSlice::I64(v) | ScalarSlice::TimestampUs(v) => {
                    Some(Prepared::I64(v, int_range(*lo, *hi)?))
                }
                ScalarSlice::F32(v) => Some(Prepared::F32(v, float_range(*lo, *hi)?)),
                ScalarSlice::F64(v) => Some(Prepared::F64(v, float_range(*lo, *hi)?)),
                ScalarSlice::Utf8(_) => None,
            },
        }
    }
}

/// The codes a category's value list names, at the column's width. A code the width cannot hold,
/// and code 0, name no row and are dropped here rather than costing a comparison per row.
fn code_test<T: Copy + Ord + TryFrom<u32>>(codes: &[u32]) -> Option<IntTest<T>> {
    let mut w: Vec<T> = codes
        .iter()
        .filter(|code| **code != 0)
        .filter_map(|code| T::try_from(*code).ok())
        .collect();
    w.sort_unstable();
    w.dedup();
    set_test(w)
}

/// The needles a numeric `in` names, at the column's width: one the type cannot hold, or a
/// fractional needle against an integer column, is dropped here.
fn int_in<T: Copy + Ord + TryFrom<i128>>(needles: &[Scalar]) -> Option<IntTest<T>> {
    let mut w: Vec<T> = needles
        .iter()
        .filter_map(|n| match n {
            Scalar::Int(i) => T::try_from(*i).ok(),
            Scalar::Float(_) => None,
        })
        .collect();
    w.sort_unstable();
    w.dedup();
    set_test(w)
}

/// A sorted set as the cheapest test that answers it.
fn set_test<T: Copy>(w: Vec<T>) -> Option<IntTest<T>> {
    match w.len() {
        0 => None,
        1 => Some(IntTest::Eq(w[0])),
        _ => Some(IntTest::In(w)),
    }
}

fn float_in(needles: &[Scalar]) -> Option<FloatTest> {
    let w: Vec<f64> = needles.iter().map(|n| as_f64(*n)).collect();
    (!w.is_empty()).then_some(FloatTest::In(w))
}

/// Both bounds as inclusive native values. `None` is the unsatisfiable range, so the segment is
/// not scanned.
fn int_range<T: TryFrom<i128> + NativeBound>(
    lo: Option<Endpoint>,
    hi: Option<Endpoint>,
) -> Option<IntTest<T>> {
    let narrow = |n| match n {
        Narrowed::Unsatisfiable => None,
        Narrowed::Unbounded => Some(None),
        Narrowed::At(x) => Some(Some(x)),
    };
    Some(IntTest::Range {
        lo: narrow(narrow_lo::<T>(lo))?,
        hi: narrow(narrow_hi::<T>(hi))?,
    })
}

fn float_range(lo: Option<Endpoint>, hi: Option<Endpoint>) -> Option<FloatTest> {
    let bound = |e: Option<Endpoint>| e.map(|e| (as_f64(e.value), e.inclusive));
    let (lo, hi) = (bound(lo), bound(hi));
    // A NaN bound is satisfied by nothing, which is what `narrow_*` answers for the integer widths.
    if [lo, hi].iter().flatten().any(|(b, _)| b.is_nan()) {
        return None;
    }
    Some(FloatTest::Range { lo, hi })
}

/// One contiguous run of rows inside one segment: the rows that match the predicate and carry a
/// value. The presence bitmap (`present`, shifted into view row space by [`scan_rows`]) is
/// intersected once per run, outside the row loop.
#[inline]
fn scan_run(
    prepared: &Prepared<'_>,
    base: u32,
    run: Range<u32>,
    present: Option<&croaring::Bitmap>,
    rows: &mut croaring::Bitmap,
    buf: &mut Vec<u32>,
) {
    match present {
        None => match_run(prepared, base, run, rows, buf),
        Some(present) => {
            let mut matched = croaring::Bitmap::new();
            match_run(prepared, base, run, &mut matched, buf);
            matched.and_inplace(present);
            rows.or_inplace(&matched);
        }
    }
}

/// One contiguous run of rows, tested against the prepared matcher alone — presence is
/// [`scan_run`]'s. Deciding the stored width once per segment and the test once per run, rather
/// than redeciding both per row, measured 0.48–0.73 ns per row against 2.5–3.4 ns for a loop that
/// redecides them. `buf` is empty on entry and on return; its allocation is reused across the
/// runs of a chunk, never to carry rows between them.
fn match_run(
    prepared: &Prepared<'_>,
    base: u32,
    run: Range<u32>,
    rows: &mut croaring::Bitmap,
    buf: &mut Vec<u32>,
) {
    let span = (run.start - base) as usize..(run.end - base) as usize;
    let first_row = run.start;
    match prepared {
        // Arrow does not store a bool as a flat slice; the run's bits are taken once and read
        // forward, then tested as the integer widths are.
        Prepared::Bool(a, test) => {
            let bits = a.values().slice(span.start, span.len());
            int_run(bits.iter().map(u8::from), first_row, test, rows, buf)
        }
        Prepared::U8(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::U16(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::U32(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::U64(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::I8(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::I16(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::I32(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::I64(v, test) => int_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::F32(v, test) => float_run(values_of(v, span), first_row, test, rows, buf),
        Prepared::F64(v, test) => float_run(values_of(v, span), first_row, test, rows, buf),
    }
    rows.add_many(buf);
    buf.clear();
}

/// One run of a flat slice, as the values themselves.
#[inline]
fn values_of<T: Copy>(values: &[T], span: Range<usize>) -> impl Iterator<Item = T> + '_ {
    values[span].iter().copied()
}

/// One integer run: the test decided once, then a native compare per value.
#[inline]
fn int_run<T: Copy + Ord>(
    values: impl Iterator<Item = T>,
    first_row: u32,
    test: &IntTest<T>,
    rows: &mut croaring::Bitmap,
    buf: &mut Vec<u32>,
) {
    match test {
        IntTest::Eq(needle) => run_matching(values, first_row, rows, buf, |x| x == *needle),
        IntTest::In(w) => {
            run_matching(values, first_row, rows, buf, |x| w.binary_search(&x).is_ok())
        }
        IntTest::Range { lo, hi } => {
            let (lo, hi) = (*lo, *hi);
            run_matching(values, first_row, rows, buf, move |x| {
                lo.is_none_or(|b| x >= b) && hi.is_none_or(|b| x <= b)
            })
        }
    }
}

/// One float run, widened to `f64` for the comparison, as the entity route does.
#[inline]
fn float_run<T: Copy + Into<f64>>(
    values: impl Iterator<Item = T>,
    first_row: u32,
    test: &FloatTest,
    rows: &mut croaring::Bitmap,
    buf: &mut Vec<u32>,
) {
    match test {
        FloatTest::In(w) => run_matching(values, first_row, rows, buf, |x| w.contains(&x.into())),
        FloatTest::Range { lo, hi } => {
            let (lo, hi) = (*lo, *hi);
            run_matching(values, first_row, rows, buf, move |x| {
                let x: f64 = x.into();
                lo.is_none_or(|(b, inc)| if inc { x >= b } else { x > b })
                    && hi.is_none_or(|(b, inc)| if inc { x <= b } else { x < b })
            })
        }
    }
}

/// The monomorphic inner loop every arm above resolves to: one run of values, one test, one
/// buffered flush.
#[inline]
fn run_matching<T: Copy>(
    values: impl Iterator<Item = T>,
    first_row: u32,
    rows: &mut croaring::Bitmap,
    buf: &mut Vec<u32>,
    matches: impl Fn(T) -> bool,
) {
    for (offset, value) in values.enumerate() {
        if matches(value) {
            buf.push(first_row + offset as u32);
            if buf.len() == 1024 {
                rows.add_many(buf);
                buf.clear();
            }
        }
    }
}

/// One segment's share of a row-space leaf: where its rows begin, the prepared matcher over the
/// column's values, and which of those rows carry one.
struct ScannedSegment<'a> {
    row_base: u32,
    /// `None` where no row of this segment can match. The scan skips the segment rather than
    /// rediscovering that once per run.
    values: Option<Prepared<'a>>,
    /// The rows that carry a value, in view row space. `None` where every row does.
    present: Option<croaring::Bitmap>,
}

/// Test every row of `domain` against `column`'s hot values — the render-column scan, parallel
/// over the domain, chunked exactly as the per-tile crossing is, and on the calling thread where
/// that is one chunk. `domain` ascends, which the walk through each chunk's segments relies on.
/// A segment that holds the column
/// at a type other than a fixed width is a malformed bundle, refused. A segment whose schema does
/// not hold the column matches nothing, answered from the schema, never from a blob read. The scan
/// ends with [`EngineError::Cancelled`] at the next chunk once the request holding `cancel` has
/// gone.
fn scan_rows(
    segments: &[(&SegmentData, u32)],
    domain: &[Range<u32>],
    column: &str,
    predicate: RowPredicate<'_>,
    cancel: &Option<CancelToken>,
) -> Result<croaring::Bitmap> {
    // Per-segment slices and presence, resolved once. `segments` is ascending by `row_base`,
    // which the per-row resolution below relies on.
    let slices: Vec<ScannedSegment<'_>> = segments
        .iter()
        .map(|&(segment, row_base)| {
            let values = match segment.columns.scalar(column) {
                // The segment predates the column's declaration: no row of it carries a value.
                None => None,
                // `utf8`: the schema refuses `render` on a string, so the segment and the
                // manifest disagree about the tail.
                Some(ScalarSlice::Utf8(_)) => {
                    return Err(EngineError::Malformed(format!(
                        "a segment of this view holds rendered column '{column}' at a type other \
                         than a fixed width, which the routed filter requires; the manifest and \
                         the segment disagree about the tail"
                    )))
                }
                Some(slice) => Prepared::of(&slice, &predicate),
            };
            // Only for a family that stores absence beside the column, and only where there is a
            // scan to narrow. A category's absence is a code in the column itself and it has no
            // bitmap at all.
            let present = (values.is_some() && predicate.reads_presence())
                .then(|| present_rows(segment, column, row_base))
                .flatten();
            Ok(ScannedSegment {
                row_base,
                values,
                present,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let chunks = domain_chunks(domain, domain.iter().map(|r| r.len() as u64).sum());
    let parts: Vec<croaring::Bitmap> = over_chunks(&chunks, cancel, |chunk| {
        let mut rows = croaring::Bitmap::new();
        let mut buf: Vec<u32> = Vec::with_capacity(1024);
        // The segment owning the chunk's first row, advanced as the walk crosses a boundary: a
        // merged range can span two adjacent segments even though no domain range spans one, and
        // the chunk's ranges ascend.
        let (mut seg, _) = segment_holding(segments, chunk[0].start)
            .expect("the first segment's rows begin at 0");
        for range in chunk {
            let mut row = range.start;
            while row < range.end {
                while seg + 1 < slices.len() && slices[seg + 1].row_base <= row {
                    seg += 1;
                }
                let seg_end = slices
                    .get(seg + 1)
                    .map_or(range.end, |next| next.row_base.min(range.end));
                let segment = &slices[seg];
                if let Some(values) = &segment.values {
                    scan_run(
                        values,
                        segment.row_base,
                        row..seg_end,
                        segment.present.as_ref(),
                        &mut rows,
                        &mut buf,
                    );
                }
                row = seg_end;
            }
        }
        rows
    })?;
    let refs: Vec<&croaring::Bitmap> = parts.iter().collect();
    Ok(croaring::Bitmap::fast_or(&refs))
}

/// The rows of one segment that carry a value for `column`, in view row space — `None` where
/// every row does. `ColumnsRef::presence` answers a column with no file, or an unknown name, with
/// an all-present bitmap, so there is no way to read a missing artefact as an absence.
fn present_rows(segment: &SegmentData, column: &str, row_base: u32) -> Option<croaring::Bitmap> {
    segment
        .columns
        .presence(column)
        .bitmap()
        .map(|rows| rows.add_offset(i64::from(row_base)))
}

/// Whether a filter's entity-space result of `matched` entities is crossed into `row_space` by
/// projecting it whole rather than by walking the request's `rows_in_ranges` rows: where it is no
/// more than [`PER_TILE_CROSSING_RATIO`] times as large, or the row space cannot map a row to its
/// entity. `per_tile_only` is the highlight's walk, whose answers all lie in the request's tiles.
fn crossing_projects(
    row_space: &mosaica_store::permutation::RowSpace,
    matched: u64,
    rows_in_ranges: u64,
    per_tile_only: bool,
) -> bool {
    let walk = per_tile_only || matched > rows_in_ranges.saturating_mul(PER_TILE_CROSSING_RATIO);
    !(walk && row_space.can_invert())
}

/// How many times larger than the viewport a filter result must be before the per-tile crossing is
/// taken instead of projecting. Measured crossover is 1× the viewport's rows for a result
/// contiguous in entity space and 3–5× for a scattered one, and a real result is scattered:
/// entity ids are uncorrelated with any attribute. `Engine::filter_crossing_routes` is the
/// observable that would catch this ratio drifting wrong.
///
/// ⊘ **Measured again** once the crossing tested rows through a probe and the projection ran on
/// the pool's threads, on a 4-core Xeon over 10⁹ entities with a result scattered across them: over
/// a 4.2-million-row domain the projection was faster up to 2× and the per-tile crossing from 4×,
/// on one thread and on four. Over a 1-million-row domain the per-tile crossing was faster at
/// every ratio from 0.25× up, so a projection taken there costs a little more, once where the
/// filter cache keeps the rows it projects ([`crate::filter_cache`]).
const PER_TILE_CROSSING_RATIO: u64 = 3;

/// Split a chunk of the crossing domain no smaller than this, so a viewport small enough that the
/// fan-out costs more than the walk does not pay for one. 4,096 rows is comfortably above rayon's
/// own per-task cost, and small enough that a realistic viewport still splits hundreds of ways.
const CROSSING_CHUNK_MIN_ROWS: u32 = 4096;

/// The view-space rows a request's tiles span: every tile part shifted into view row space by its
/// segment's `row_base`, sorted, and merged where adjacent, so the domain is never widened.
pub(crate) fn crossing_domain(
    ranges: &[Vec<(usize, Range<u32>)>],
    row_bases: &[u32],
) -> Vec<Range<u32>> {
    merged_spans(
        ranges
            .iter()
            .flat_map(|parts| parts.iter())
            .map(|(s, r)| row_bases[*s] + r.start..row_bases[*s] + r.end)
            .collect(),
    )
}

/// `spans` sorted, and merged where adjacent or overlapping.
pub(crate) fn merged_spans(mut spans: Vec<Range<u32>>) -> Vec<Range<u32>> {
    spans.sort_unstable_by_key(|r| r.start);
    let mut merged: Vec<Range<u32>> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

/// The vocabulary a predicate column's values are named by, or `None` where the column has none,
/// in which case an artifact's key is the value's own canonical decimal spelling — also `None`
/// for a layer whose membership is not an attribute predicate at all.
pub(crate) fn predicate_vocabulary<'a>(
    generation: &'a crate::Generation,
    declaration: &mosaica_types::layer::LayerDeclaration,
) -> Option<&'a mosaica_store::vocabulary::VocabularyMinter> {
    let mosaica_types::layer::MembershipSource::Attribute(field) = &declaration.membership else {
        return None;
    };
    let name = generation
        .bundle
        .manifest
        .declared_scalars
        .iter()
        .find(|scalar| &scalar.name == field)?
        .vocabulary
        .as_deref()?;
    generation.vocabularies.get(name)
}

/// Where a predicate layer's membership comes from, for one request against one generation.
/// `None` for an enumerated layer, and for a predicate layer whose rule cannot be evaluated at
/// all — a column this generation does not hold, or a spatial layer that declares no shape.
/// Fail-closed: such a level is served with no membership, rather than every artifact being one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn predicate_source<'a>(
    declaration: &mosaica_types::layer::LayerDeclaration,
    shard: &'a crate::Shard,
    view: &str,
    view_data: &mosaica_store::read::ViewData,
    segments: &'a [(&'a mosaica_store::read::SegmentData, u32)],
    code_of_key: &'a dyn Fn(&str) -> Option<u32>,
    shapes: &crate::shapes::ShapeStore,
    store: &mosaica_lifecycle::membership::ArtifactStore,
    level: u32,
) -> Option<crate::artifacts::PredicateSource<'a>> {
    match &declaration.membership {
        mosaica_types::layer::MembershipSource::Enumerated => None,
        mosaica_types::layer::MembershipSource::Attribute(field) => {
            let values = shard.filter_columns.value_layers(field)?;
            Some(crate::artifacts::PredicateSource::Attribute(
                crate::artifacts::AttributeSource {
                    values,
                    code_of_key,
                },
            ))
        }
        // A spatial layer with no `shape` holds no artifacts and has nothing to resolve.
        mosaica_types::layer::MembershipSource::Spatial => {
            declaration.shape?;
            let held = shapes.level(
                view,
                view_data.incarnation,
                &declaration.name,
                level,
                store,
                &crate::shapes::PersistedPieces::none(),
            );
            Some(crate::artifacts::PredicateSource::Spatial(
                crate::artifacts::SpatialSource {
                    level: held,
                    segments,
                    total_rows: u32::try_from(view_data.row_space.total_rows()).unwrap_or(u32::MAX),
                },
            ))
        }
    }
}

impl Engine {
    /// The request's filter and highlight, evaluated and crossed into the mask — after the tile
    /// ranges, before the sweep. The masked counts already computed ignore the filter, or an
    /// artifact would appear and vanish as a viewer typed. `filters` and `highlight` are evaluated
    /// together against the same candidate and the same pre-filter mask — a highlight is a
    /// conjunction with the filter's candidate by construction, and evaluating its leaves against
    /// an already-filtered mask would make the two positions of one clause mean different things.
    /// Only the filter's rows narrow what is drawn.
    ///
    /// Evaluated in each of [`ShardedMask::filtered_leaves`], over the rows the request's tiles
    /// span in that shard: a filter only narrows a shard's visible rows, so no shard's answer
    /// depends on another's.
    pub(super) fn narrow_to_filters(
        &self,
        served: &ServedView<'_>,
        mask: ShardedMask,
        tiling: &Tiling<'_>,
        v_total: u64,
        req: &ViewportRequest<'_>,
        probe: &mut Probe,
    ) -> Result<(ShardedMask, Option<crate::region::RegionVerdict>)> {
        if req.filter.is_none() && req.highlight.is_none() {
            return Ok((mask, None));
        }
        check_cancelled(&req.cancel)?;
        // The route rule reads the request's whole extent and visible total, and every shard's
        // postings and kept rows, so a leaf takes the same space in every shard.
        let placed: Vec<_> = served
            .filtered_sides(&mask)
            .map(|(place, side, _)| (side, tiling.domain_in(place)))
            .collect();
        let sides: Vec<_> = placed
            .iter()
            .map(|(side, (domain, _))| (*side, &domain[..]))
            .collect();
        let exprs = req.filter.iter().chain(req.highlight.iter());
        let costs = self.leaf_costs(served, &sides, v_total, exprs);
        let choice = costs.routes();
        let mut region_verdict: Option<crate::region::RegionVerdict> = None;
        let mut filters = Vec::new();
        let mut highlights = Vec::new();
        let resolved = ResolvedLeaves::new(&mask);
        let always = self.switches.tile_filters_always.load(Ordering::Relaxed);
        let may_narrow = self.switches.tile_filters_enabled.load(Ordering::Relaxed)
            && (always || narrowing_may_pay(tiling.rows_in_ranges, v_total));
        let grant = served.session.terms_digest();
        let keeps = self.filter_cache.bound_bytes() > 0;
        for (side, (domain, rows_in_ranges)) in &placed {
            let (side, rows_in_ranges) = (*side, *rows_in_ranges);
            let domain_rows = crate::compose::rows_of(domain);
            let tiles = may_narrow.then_some(&domain_rows);
            let within = match tiles {
                Some(tiles) => self.tiles_candidate(served, side, domain, tiles, rows_in_ranges)?,
                None => None,
            };
            let candidate = match &within {
                Some(within) => FilterCandidate::Within(within),
                None => FilterCandidate::Compose,
            };
            let (filter_rows, highlight_rows) =
                self.route_filters_under(served, side, candidate, &resolved, &req.cancel, |route| {
                    // `count_matched` excludes the highlight from the `filter_matched` probe: its
                    // cardinality is not the filter's.
                    let mut evaluate = |expr: &crate::filter::FilterExpr,
                                        per_tile_only: bool,
                                        count_matched: bool|
                     -> Result<(FilterRows, Option<crate::region::RegionVerdict>)> {
                        let projection = keeps
                            .then(|| {
                                crate::filter_cache::projection_key_of(
                                    side.shard,
                                    grant,
                                    served.name,
                                    side.data.incarnation,
                                    expr,
                                )
                            })
                            .flatten();
                        let kept = projection.as_ref().and_then(|key| {
                            crate::filter_cache::projected_rows(&self.filter_cache, key, &domain_rows)
                        });
                        // Built where the request evaluates every item it may see, never only those
                        // of its tiles.
                        let builds = within.is_none()
                            && side.shard.filter_columns.routes_to_items(expr, choice);
                        let found = match (kept, projection) {
                            (Some(rows), _) => Some(RoutedRows {
                                rows: FilterRows::Viewport {
                                    rows,
                                    domain: domain.to_vec(),
                                },
                                region: None,
                                unmasked: None,
                            }),
                            (None, Some(key)) if builds => Some(self.rows_over_fragment(
                                served,
                                side,
                                &resolved,
                                &req.cancel,
                                expr,
                                key,
                                (domain, &domain_rows, rows_in_ranges),
                                per_tile_only,
                            )?),
                            _ => None,
                        };
                        let mut out = match found {
                            Some(found) => {
                                probe.lap(|t| &mut t.filter_eval_ns);
                                self.with_buffered(
                                    served,
                                    side,
                                    &resolved,
                                    &req.cancel,
                                    expr,
                                    &domain_rows,
                                    found,
                                )?
                            }
                            None => {
                                let routed = route(expr, choice)?;
                                probe.lap(|t| &mut t.filter_eval_ns);
                                self.rows_of_routed(
                                    served,
                                    side,
                                    routed,
                                    domain,
                                    rows_in_ranges,
                                    per_tile_only,
                                    &req.cancel,
                                )?
                            }
                        };
                        if let (Some(tiles), Some(_)) = (tiles, &within) {
                            out.rows = out.rows.held_to(tiles, domain.to_vec());
                        }
                        if let (true, Some(matched)) = (count_matched, out.unmasked) {
                            probe.count(|t| &mut t.filter_matched, matched);
                        }
                        probe.lap(|t| &mut t.filter_cross_ns);
                        Ok((out.rows, out.region))
                    };
                    let filter_rows = match &req.filter {
                        None => None,
                        Some(expr) => {
                            let (rows, verdict) = evaluate(expr, false, true)?;
                            region_verdict =
                                crate::region::RegionVerdict::coarsest(region_verdict, verdict);
                            Some(rows)
                        }
                    };
                    // The highlight always takes the per-tile walk: its three answers are all
                    // inside the request's tiles, so the whole-view projection would be paid for
                    // nothing.
                    let highlight_rows = match &req.highlight {
                        None => None,
                        Some(expr) => {
                            let (rows, verdict) = evaluate(expr, true, false)?;
                            // The coarsest of the two: a cover anywhere makes the response's
                            // verdict a cover.
                            region_verdict =
                                crate::region::RegionVerdict::coarsest(region_verdict, verdict);
                            Some(rows)
                        }
                    };
                    Ok((filter_rows, highlight_rows))
                })?;
            filters.extend(filter_rows.map(|rows| (side.shard.id(), rows)));
            highlights.extend(highlight_rows.map(|rows| (side.shard.id(), rows)));
        }
        Ok((
            mask.with_filters(filters).with_highlights(highlights),
            region_verdict,
        ))
    }

    /// `expr`'s rows over a request's domain in `side`, given as its ranges, its rows and how many
    /// rows it holds, before the mask and without the request's buffered items: `expr` evaluated
    /// once, over the grant's fragment ([`Self::items_over_fragment`]), and crossed into the view.
    /// Where it is projected whole, its rows are kept in the filter cache under `key`.
    #[allow(clippy::too_many_arguments)]
    fn rows_over_fragment(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        expr: &crate::filter::FilterExpr,
        key: (crate::filter_cache::LeafKey, Vec<u8>),
        (domain, domain_rows, rows_in_ranges): (&[Range<u32>], &croaring::Bitmap, u64),
        per_tile_only: bool,
    ) -> Result<RoutedRows> {
        let items = self.items_over_fragment(served, side, resolved, cancel, expr)?;
        let row_space = &side.data.row_space;
        let matched = items.cardinality();
        if !crossing_projects(row_space, matched, rows_in_ranges, per_tile_only) {
            let routed = crate::filter::RoutedFilter::Entity(items);
            return self.rows_of_routed(
                served,
                side,
                routed,
                domain,
                rows_in_ranges,
                per_tile_only,
                cancel,
            );
        }
        // Kept only once the whole of it is in hand.
        let rows = project_until(row_space, &items, cancel)?;
        let answer = rows.and(domain_rows);
        let total_rows = u32::try_from(row_space.total_rows()).unwrap_or(u32::MAX);
        crate::filter_cache::keep_projection(&self.filter_cache, side.shard, key, rows, total_rows);
        self.counters
            .filter_crossings_projected
            .fetch_add(1, Ordering::Relaxed);
        Ok(RoutedRows {
            rows: FilterRows::Viewport {
                rows: answer,
                domain: domain.to_vec(),
            },
            region: None,
            unmasked: Some(matched),
        })
    }

    /// `out` with the rows of `expr`'s matches among the request's buffered items added
    /// ([`Self::buffered_matches`]), within `domain_rows` where `out` answers over a domain.
    #[allow(clippy::too_many_arguments)]
    fn with_buffered(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        expr: &crate::filter::FilterExpr,
        domain_rows: &croaring::Bitmap,
        mut out: RoutedRows,
    ) -> Result<RoutedRows> {
        let extra = self.buffered_matches(served, side, resolved, cancel, expr)?;
        if !extra.is_empty() {
            out.rows = match out.rows {
                FilterRows::Complete(rows) => FilterRows::Complete(rows.or(&extra)),
                FilterRows::Viewport { rows, domain } => FilterRows::Viewport {
                    rows: rows.or(&extra.and(domain_rows)),
                    domain,
                },
            };
        }
        Ok(out)
    }

    /// The items `expr` matches among those the session's grant admits in `side`, before deletions
    /// and suppressions: `expr` evaluated in entity space over the grant's fragment, as the filter
    /// cache evaluates a leaf.
    fn items_over_fragment(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        expr: &crate::filter::FilterExpr,
    ) -> Result<croaring::Bitmap> {
        let fragment = self.fragment_for(served.session, side.shard, Some(&side.geometry.fragment))?;
        let fragment = fragment.view();
        let candidate = FilterCandidate::Whole(&fragment);
        self.items_under(served, side, candidate, resolved, cancel, expr)
    }

    /// The items `expr` matches under `candidate` in `side`, evaluated in entity space. `expr`
    /// must route there under [`crate::filter::RouteChoice::Entity`].
    fn items_under(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        candidate: FilterCandidate<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        expr: &crate::filter::FilterExpr,
    ) -> Result<croaring::Bitmap> {
        let routed = self.route_filters_under(served, side, candidate, resolved, cancel, |route| {
            route(expr, crate::filter::RouteChoice::Entity)
        })?;
        match routed {
            crate::filter::RoutedFilter::Entity(items) => Ok(items),
            crate::filter::RoutedFilter::Row(_) => {
                unreachable!("an expression routed to entity space has no row leaf")
            }
        }
    }

    /// The rows in `side` of the items `expr` matches among the request's candidate set's buffered
    /// items with a row in the view: what rows kept over the grant's fragment do not hold, since the
    /// fragment holds no buffered item.
    fn buffered_matches(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        expr: &crate::filter::FilterExpr,
    ) -> Result<croaring::Bitmap> {
        let entity = |entity: &mosaica_types::EntityId| u32::try_from(entity.raw()).ok();
        let buffered: croaring::Bitmap = match side.shard.buffered_rows(served.name) {
            Some(listed) => listed.iter().filter_map(entity).collect(),
            None => side.shard.buffer.iter().filter_map(|(id, _)| entity(id)).collect(),
        };
        if buffered.is_empty() {
            return Ok(croaring::Bitmap::new());
        }
        let candidate = self.filter_candidate_within(
            served.session,
            side.shard,
            Some(&side.geometry.fragment),
            &buffered,
        )?;
        let candidate = FilterCandidate::Within(&candidate);
        let items = self.items_under(served, side, candidate, resolved, cancel, expr)?;
        project_until(&side.data.row_space, &items, cancel)
    }

    /// The visible entities whose rows lie in `domain`, the rows a request's tiles span in `side`
    /// and which `rows` holds, where evaluating the request's filter over them costs less than over
    /// the viewer's whole visible set there ([`narrowing_pays`]); `None` otherwise, or where the
    /// side cannot map its rows to entities. The answer is exact over `domain` and says nothing
    /// outside it.
    fn tiles_candidate(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        domain: &[Range<u32>],
        rows: &croaring::Bitmap,
        rows_in_ranges: u64,
    ) -> Result<Option<croaring::Bitmap>> {
        let always = self.switches.tile_filters_always.load(Ordering::Relaxed);
        let fragment = self.fragment_for(served.session, side.shard, Some(&side.geometry.fragment))?;
        let view = fragment.view();
        let visible = view.andnot_cardinality(&side.shard.overlay.denied());
        let visible_blocks = u64::from(view.statistics().n_containers);
        let row_space = &side.data.row_space;
        // A sample of the tiles' rows finds a lower bound on the blocks their visible entities
        // fall in, so a viewport whose entities are scattered is turned away before every row is
        // mapped to its entity.
        let sampled = sampled_blocks(row_space, domain, rows_in_ranges, &view);
        if !always && sampled.saturating_mul(NARROW_BLOCK_RATIO) > visible_blocks {
            return Ok(None);
        }
        let Some(entities) = self.on_pool(rows_in_ranges, || row_space.entities_of_rows(rows))
        else {
            return Ok(None);
        };
        let within = crate::filter::candidate_within(
            &fragment,
            served.session.satisfied(),
            &side.shard.overlay,
            &side.shard.buffer,
            &entities,
        );
        if !always && !narrowing_pays(&within, visible, visible_blocks) {
            return Ok(None);
        }
        self.counters.tile_filters.fetch_add(1, Ordering::Relaxed);
        Ok(Some(within))
    }

    /// The entity-space set a request's filter is evaluated under in `shard`: the session's
    /// fragment there ([`Self::fragment_for`], given what the request holds as `held`), with the
    /// deny state and the passing buffered entities composed in.
    pub(crate) fn filter_candidate(
        &self,
        session: &Session,
        shard: &crate::Shard,
        held: Option<&Arc<FrozenFragment>>,
    ) -> Result<croaring::Bitmap> {
        let fragment = self.fragment_for(session, shard, held)?;
        Ok(crate::filter::candidate(
            &fragment,
            session.satisfied(),
            &shard.overlay,
            &shard.buffer,
        ))
    }

    /// [`Self::filter_candidate`]'s members among `within`, without composing the whole set.
    pub(crate) fn filter_candidate_within(
        &self,
        session: &Session,
        shard: &crate::Shard,
        held: Option<&Arc<FrozenFragment>>,
        within: &croaring::Bitmap,
    ) -> Result<croaring::Bitmap> {
        let fragment = self.fragment_for(session, shard, held)?;
        Ok(crate::filter::candidate_within(
            &fragment,
            session.satisfied(),
            &shard.overlay,
            &shard.buffer,
            within,
        ))
    }

    /// Route this request's filter expressions in one side under `candidate`. Every scan returns a
    /// subset of the candidate. A region or `member_of` leaf `resolved` holds is answered from it,
    /// and one it does not is resolved and kept there; the caller holds it only while `served` and
    /// its mask are the ones it was filled under. A text or value leaf is answered from the filter
    /// cache ([`crate::filter_cache`]), whose entry is computed over the session's fragment where
    /// the whole candidate is known: for a text leaf only when it is evaluated under the whole
    /// candidate, and for a value leaf under any part of it. A value leaf is evaluated on the
    /// engine's pool. `body` is handed a `route` that puts one expression through them.
    pub(crate) fn route_filters_under<T>(
        &self,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        candidate: FilterCandidate<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
        body: impl FnOnce(
            &RouteFilter<'_>,
        ) -> Result<T>,
    ) -> Result<T> {
        // The fragment a leaf's kept answer is taken over, where the whole candidate is known.
        let fragment = match candidate {
            FilterCandidate::Within(_) => None,
            FilterCandidate::Compose | FilterCandidate::Whole(_) => Some(self.fragment_for(
                served.session,
                side.shard,
                Some(&side.geometry.fragment),
            )?),
        };
        let composed;
        let (candidate, whole) = match candidate {
            FilterCandidate::Compose => {
                composed = self.filter_candidate(served.session, side.shard, fragment.as_ref())?;
                (&composed, Some(&composed))
            }
            FilterCandidate::Whole(whole) => (whole, Some(whole)),
            FilterCandidate::Within(within) => (within, None),
        };
        let regions = |leaf: &crate::filter::RegionLeaf| {
            self.region_rows(leaf, served, side, resolved, cancel)
        };
        let members =
            |leaf: &crate::filter::MemberOfLeaf| self.member_rows(leaf, served, side, resolved);
        let layers = |layer: &str| self.layer_levels(served.session, layer);
        let unique = |column: &str, keys: &[mosaica_store::unique::UniqueKey]| {
            unique_holders(&self.identity_key, served.generation, side.shard, column, keys)
        };
        let fragment = fragment.as_ref().map(|fragment| fragment.view());
        let leaves: &crate::filter::LeafResolver<'_> = &|column, query, at, cancel, evaluate| {
            // A phrase can cost far more over the fragment than over a narrowed candidate, so a
            // text leaf is kept only where it is asked under the whole one. A value leaf costs at
            // most one scan of the fragment, about what the first leaf under the whole candidate
            // costs, and every later request reads it for an intersection.
            let keep = match query {
                crate::filter::LeafQuery::Text(_) => {
                    whole.is_some_and(|whole| std::ptr::eq(whole, at) || whole == at)
                }
                crate::filter::LeafQuery::Value(_) => whole.is_some(),
            };
            let on_pool = |candidate: &croaring::Bitmap, watch: &Option<CancelToken>| match query {
                crate::filter::LeafQuery::Text(_) => evaluate(candidate, watch),
                crate::filter::LeafQuery::Value(_) => {
                    self.pool.install(|| evaluate(candidate, watch))
                }
            };
            crate::filter_cache::answer(
                &self.filter_cache,
                &self.filter_interest,
                side.shard,
                crate::filter_cache::key_of(
                    side.shard,
                    served.session.terms_digest(),
                    column,
                    query,
                ),
                fragment.as_deref().filter(|_| keep),
                at,
                cancel,
                &on_pool,
            )
        };
        let resolvers = crate::filter::RowLeafResolvers {
            regions: &regions,
            members: &members,
            layers: &layers,
            unique: &unique,
            leaves,
        };
        body(&|expr: &crate::filter::FilterExpr, choice: crate::filter::RouteChoice<'_>| {
            side.shard
                .filter_columns
                .evaluate_routed(expr, candidate, choice, &resolvers, cancel)
                .map_err(filter_refusal)
        })
    }

    /// A region leaf's rows in `side`, from `resolved` where it holds them, and resolved and kept
    /// there otherwise.
    pub(crate) fn region_rows(
        &self,
        leaf: &crate::filter::RegionLeaf,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
    ) -> std::result::Result<crate::region::RegionRows, crate::filter::FilterError> {
        let shard = side.shard.id();
        memo(
            &resolved.regions,
            |(at, held)| *at == shard && held == leaf,
            || (shard, leaf.clone()),
            || self.resolve_region(leaf, served, side, resolved, cancel),
        )
    }

    /// A `member_of` leaf's rows in `side`, from `resolved` where it holds them, and resolved and
    /// kept there otherwise.
    pub(crate) fn member_rows(
        &self,
        leaf: &crate::filter::MemberOfLeaf,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
    ) -> std::result::Result<croaring::Bitmap, crate::filter::FilterError> {
        let shard = side.shard.id();
        memo(
            &resolved.members,
            |(at, held)| *at == shard && held == leaf,
            || (shard, leaf.clone()),
            || self.resolve_member_of(leaf, served, side, resolved),
        )
    }

    /// [`Engine::gated_artifact`] for `id` under `resolved`'s mask, once per request.
    fn gated_once(
        &self,
        served: &ServedView<'_>,
        resolved: &ResolvedLeaves<'_>,
        id: MosaicaId,
    ) -> Result<Gated> {
        memo(
            &resolved.gates,
            |held| *held == id,
            || id,
            || {
                Ok(self
                    .gated_artifact(served, resolved.mask, id)?
                    .map(std::rc::Rc::new))
            },
        )
    }

    /// [`Engine::gated_key`] for `key` at `level` of `layer` under `resolved`'s mask, once per
    /// request, so every shard reads the one artifact the key named when the request first asked.
    fn gated_key_once(
        &self,
        served: &ServedView<'_>,
        resolved: &ResolvedLeaves<'_>,
        layer: &str,
        level: u32,
        key: &str,
    ) -> Result<Gated> {
        memo(
            &resolved.keys,
            |(held_layer, held_level, held_key)| {
                held_layer == layer && *held_level == level && held_key == key
            },
            || (layer.to_string(), level, key.to_string()),
            || {
                Ok(self
                    .gated_key(served, resolved.mask, layer, level, key)?
                    .map(std::rc::Rc::new))
            },
        )
    }

    /// How many levels `layer` holds where `/v1/meta` lists it for `session`, and so a
    /// `member_of` may name it; `None` where it does not list it, alike for a layer it does not
    /// reach, a suppressed one and one never registered.
    pub(crate) fn layer_levels(&self, session: &Session, layer: &str) -> Option<u32> {
        self.visible_layer(session, layer)
            .map(|layer| layer.runs.len() as u32)
    }

    /// Answer one region leaf for one request. A drawn shape: its decomposition from the
    /// generation-keyed cache — shared across principals, so it carries no authorisation — with
    /// the boundary rows tested under this request's composed mask. A published shape: the
    /// artifact's held membership, only where this principal would be served the artifact;
    /// otherwise the empty operand, identically for every reason an artifact may be withheld,
    /// including drawing an authored shape, which is content and not a membership.
    pub(crate) fn resolve_region(
        &self,
        leaf: &crate::filter::RegionLeaf,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
    ) -> std::result::Result<crate::region::RegionRows, crate::filter::FilterError> {
        let mask = resolved.leaf(side);
        use crate::filter::{FilterError, RegionLeaf};
        let segments = &side.segments[..];
        let asked = cancel;
        let never_cancelled = CancelToken::new();
        let cancel = cancel.as_ref().unwrap_or(&never_cancelled);
        use crate::region::{digest_of, RegionDecomposition, RegionKey, RegionRows, RegionVerdict};
        match leaf {
            RegionLeaf::Shape(shape) => {
                let max_cells = self.max_region_cells.load(Ordering::Relaxed) as usize;
                let canonical = shape.encode();
                let key = RegionKey {
                    view: served.name.to_string(),
                    shard: side.shard.id(),
                    prefix: served.generation.prefix.clone(),
                    segments_version: side.shard.segments_version,
                    digest: digest_of(&canonical),
                    max_cells,
                };
                let build = || RegionDecomposition::build(Arc::clone(shape), max_cells, segments);
                let entry = match self
                    .region_cache
                    .get_or_derive_waiting(key, None, cancel, |_| build())
                {
                    // A digest collision is detected here and answered from a fresh, unretained
                    // decomposition.
                    Ok(entry) if entry.is_of(&canonical) => entry,
                    Ok(_) => Arc::new(build()),
                    Err(mosaica_cache::WaitEnded::Cancelled) => return Err(FilterError::Cancelled),
                    // Building unretained rather than refusing: a second viewer's identical lasso
                    // must not 429.
                    Err(mosaica_cache::WaitEnded::Budget) => Arc::new(build()),
                };
                Ok(RegionRows {
                    rows: mask.map_or_else(croaring::Bitmap::new, |mask| {
                        entry.rows_under(mask, segments)
                    }),
                    verdict: entry.verdict(),
                    domain: Some(Arc::clone(entry.domain())),
                })
            }
            RegionLeaf::InView { view, region } => {
                let there = self.region_in_view(view, region, served, resolved, asked)?;
                let entities = there
                    .entities
                    .iter()
                    .find(|(shard, _)| *shard == side.shard.id())
                    .map(|(_, entities)| entities);
                let rows = match (entities, mask) {
                    (Some(entities), Some(_)) => {
                        project_until(&side.data.row_space, entities, asked)
                            .map_err(|_| FilterError::Cancelled)?
                    }
                    _ => croaring::Bitmap::new(),
                };
                Ok(RegionRows {
                    rows,
                    verdict: there.verdict,
                    domain: None,
                })
            }
            RegionLeaf::Artifact(id) => {
                let gated = self
                    .gated_once(served, resolved, *id)
                    .map_err(|e| FilterError::RegionUnavailable(e.to_string()))?;
                let rows = match (gated, mask) {
                    (Some(gated), Some(mask))
                        if gated.layer.declaration.drawn_shape()
                            != Some(mosaica_types::layer::DrawnShape::Authored) =>
                    {
                        // `membership ∩ M_auth` in this side, from whichever half of its form
                        // holds it — see [`crate::artifacts::ArtifactRows::visible_rows`].
                        let place = served.place_of(side.shard.id());
                        gated.forms.rows[place].visible_rows(gated.ordinal, mask)
                    }
                    _ => croaring::Bitmap::new(),
                };
                Ok(RegionRows {
                    rows,
                    verdict: RegionVerdict::Exact,
                    domain: None,
                })
            }
        }
    }

    /// The items the viewer sees inside `region` in `view`, by shard, once per request: the
    /// region resolved there as the request's own view resolves one, under that view's mask.
    fn region_in_view(
        &self,
        view: &str,
        region: &crate::filter::RegionLeaf,
        served: &ServedView<'_>,
        resolved: &ResolvedLeaves<'_>,
        cancel: &Option<CancelToken>,
    ) -> std::result::Result<std::rc::Rc<InView>, crate::filter::FilterError> {
        use crate::filter::FilterError;
        let refused = |e: EngineError| match e {
            EngineError::Cancelled => FilterError::Cancelled,
            EngineError::UnknownView(view) => FilterError::RegionViewUnknown(view),
            EngineError::ProjectionBuilding
            | EngineError::FragmentBuilding
            | EngineError::CountsBuilding => FilterError::RegionViewBuilding,
            e => FilterError::RegionUnavailable(e.to_string()),
        };
        memo(
            &resolved.in_view,
            |(held_view, held)| held_view == view && held == region,
            || (view.to_string(), region.clone()),
            || {
                let open = self
                    .open_view(
                        served.session,
                        served.generation,
                        view,
                        cancel,
                        &mut Probe::new(),
                    )
                    .map_err(refused)?;
                let there = ResolvedLeaves::new(&open.mask);
                let mut verdict = crate::region::RegionVerdict::Exact;
                let mut entities = Vec::new();
                for (place, side, leaf) in open.served.filtered_sides(&open.mask) {
                    let rows = self.resolve_region(region, &open.served, side, &there, cancel)?;
                    verdict = verdict.coarser(rows.verdict);
                    // A drawn shape's interior is every row of its tiles; only the visible ones
                    // are crossed.
                    let visible = match &rows.domain {
                        Some(domain) => {
                            let mut visible = croaring::Bitmap::new();
                            for range in domain.iter() {
                                visible.or_inplace(&leaf.rows_in_range(range.clone()));
                            }
                            visible.and(&rows.rows)
                        }
                        None => rows.rows,
                    };
                    let crossed = self
                        .entities_of_rows(&open.served, place, &visible)
                        .map_err(refused)?;
                    entities.push((side.shard.id(), crossed));
                }
                Ok(std::rc::Rc::new(InView { entities, verdict }))
            },
        )
    }

    /// The entities of `rows` in the side at `place`, on the engine's pool.
    pub(crate) fn entities_of_rows(
        &self,
        served: &ServedView<'_>,
        place: usize,
        rows: &croaring::Bitmap,
    ) -> Result<croaring::Bitmap> {
        let row_space = &served.sides[place].data.row_space;
        self.pool
            .install(|| row_space.entities_of_rows(rows))
            .ok_or_else(|| {
                EngineError::Malformed(format!(
                    "view '{}' has no row-to-entity table, so a set routed by its rows cannot be \
                     counted by entity; rebuild the bundle",
                    served.name
                ))
            })
    }

    /// Answer one `member_of` leaf in one side of one request, whose layer the filter's admission
    /// has already found this principal reaches. A key is resolved to its artifact once per
    /// request, and a key naming none is the empty operand. The artifact must pass its own existence
    /// criterion through the same [`Engine::gated_artifact`] the drill-down calls, over every
    /// shard, before this side reads its members. An artifact that does not pass — names nothing,
    /// is of another layer, is suppressed, is below the criterion — is the empty operand, one
    /// answer for every reason: refusing instead would make the leaf an existence oracle over what
    /// the criterion withholds. The membership is read two ways, decided by the level's layout:
    /// artifact-major intersects the held row bitmap with the side's leaf; row-major scans visible
    /// rows comparing labels. Either way the answer is `membership ∩ M_auth` in the side.
    pub(crate) fn resolve_member_of(
        &self,
        leaf: &crate::filter::MemberOfLeaf,
        served: &ServedView<'_>,
        side: &ServedSide<'_>,
        resolved: &ResolvedLeaves<'_>,
    ) -> std::result::Result<croaring::Bitmap, crate::filter::FilterError> {
        use crate::filter::{FilterError, MemberOfArtifact};
        let gated = match &leaf.artifact {
            MemberOfArtifact::Id(id) => self.gated_once(served, resolved, *id),
            MemberOfArtifact::Key { key, level } => {
                self.gated_key_once(served, resolved, &leaf.layer, *level, key)
            }
        }
        .map_err(|e| FilterError::MemberOfUnavailable(e.to_string()))?;
        // An identifier of another layer is answered exactly as one that resolves to nothing.
        let gated = gated.filter(|g| g.name == leaf.layer);
        let (Some(gated), Some(mask)) = (gated, resolved.leaf(side)) else {
            return Ok(croaring::Bitmap::new());
        };
        let rows = &gated.forms.rows[served.place_of(side.shard.id())];
        // The counter says which levels answer `member_of` at the column's cost rather than the
        // bitmap's.
        if !rows.membership().rows_held() {
            self.counters.member_of_column_walks.fetch_add(1, Ordering::Relaxed);
        }
        Ok(rows.visible_rows(gated.ordinal, mask))
    }
}

/// The set a request's filter is evaluated under in one side.
#[derive(Clone, Copy)]
pub(crate) enum FilterCandidate<'c> {
    /// [`Engine::filter_candidate`]'s set, composed over the session's fragment at the shard's
    /// watermark: an earlier one, such as a stale-served projection's, would omit every entity
    /// flushed since.
    Compose,
    /// That set, as the caller composed it with [`Engine::filter_candidate`].
    Whole(&'c croaring::Bitmap),
    /// A subset of that set the caller composed, never wider.
    Within(&'c croaring::Bitmap),
}

/// An artifact a leaf named, as one request's gate answered it.
type Gated = Option<std::rc::Rc<super::artifacts::GatedArtifact>>;

/// A key a leaf named, as `(layer, level, key)`.
type KeyAt = (String, u32, String);

/// The region and `member_of` leaves answered for one request under one view: each artifact a
/// leaf names, by identifier or by key, gated once over every shard under the request's mask
/// before any filter narrows it, and each leaf's rows in each shard it was evaluated in, so a leaf
/// evaluated again is resolved once.
pub(crate) struct ResolvedLeaves<'m> {
    mask: &'m ShardedMask,
    gates: std::cell::RefCell<Vec<(MosaicaId, Gated)>>,
    keys: std::cell::RefCell<Vec<(KeyAt, Gated)>>,
    regions: std::cell::RefCell<
        Vec<((ShardId, crate::filter::RegionLeaf), crate::region::RegionRows)>,
    >,
    members: std::cell::RefCell<Vec<((ShardId, crate::filter::MemberOfLeaf), croaring::Bitmap)>>,
    in_view: std::cell::RefCell<Vec<((String, crate::filter::RegionLeaf), InViewHeld)>>,
}

/// A region of another view as one request holds it, shared by every side that reads it.
type InViewHeld = std::rc::Rc<InView>;

/// A region of another view, answered for one request: the items the viewer sees inside it in
/// each shard, and how exactly it was answered there.
pub(crate) struct InView {
    entities: Vec<(ShardId, croaring::Bitmap)>,
    verdict: crate::region::RegionVerdict,
}

impl<'m> ResolvedLeaves<'m> {
    /// Nothing resolved yet under `mask`, the request's mask before any filter narrows it.
    pub(crate) fn new(mask: &'m ShardedMask) -> Self {
        ResolvedLeaves {
            mask,
            gates: Default::default(),
            keys: Default::default(),
            regions: Default::default(),
            members: Default::default(),
            in_view: Default::default(),
        }
    }

    /// The leaf `side` is read through, the one the gates were taken under, or `None` where the
    /// request leaves the side out.
    fn leaf(&self, side: &ServedSide<'_>) -> Option<&'m EffectiveMask> {
        self.mask.read_leaf(side.shard.id())
    }
}

/// The value in `cell` whose key `matches`, else `f`'s, kept there under `key()`. The key is built
/// only on a miss.
fn memo<K, V: Clone, E>(
    cell: &std::cell::RefCell<Vec<(K, V)>>,
    matches: impl Fn(&K) -> bool,
    key: impl FnOnce() -> K,
    f: impl FnOnce() -> std::result::Result<V, E>,
) -> std::result::Result<V, E> {
    let held = cell
        .borrow()
        .iter()
        .find(|(held, _)| matches(held))
        .map(|(_, value)| value.clone());
    if let Some(value) = held {
        return Ok(value);
    }
    let value = f()?;
    cell.borrow_mut().push((key(), value.clone()));
    Ok(value)
}

/// The entities of `shard` holding `keys` in a unique column, by the shard's index and its live
/// entries in `generation`, deleted entities dropped. Visibility is the caller's.
///
/// `mosaica_id`'s keys are identifiers, resolved as the item card resolves one: an identifier at a
/// tenancy its number no longer holds names nothing.
pub(crate) fn unique_holders(
    key: &mosaica_types::IdentityKey,
    generation: &crate::Generation,
    shard: &crate::Shard,
    column: &str,
    keys: &[mosaica_store::unique::UniqueKey],
) -> std::result::Result<Vec<u32>, crate::filter::FilterError> {
    use mosaica_store::unique::UniqueKey;
    if column == crate::filter::MOSAICA_ID_COLUMN {
        let ids: Vec<mosaica_types::MosaicaId> = keys
            .iter()
            .filter_map(|k| match k {
                UniqueKey::Int(id) => Some(mosaica_types::MosaicaId::new(*id)),
                UniqueKey::Keyword(_) => None,
            })
            .collect();
        let named = crate::edited::entities_named(key, generation, shard, &ids)
            .map_err(|e| crate::filter::FilterError::IdentifiersUnreadable(e.to_string()))?;
        return Ok(named
            .into_iter()
            .flatten()
            .filter(|(entity, current)| *current && !shard.overlay.is_deleted(*entity))
            .map(|(entity, _)| entity.raw() as u32)
            .collect());
    }
    crate::unique::holders_in(generation, shard, column, keys)
        .map(|found| found.into_iter().map(|(_, entity)| entity.raw() as u32).collect())
        .map_err(|e| crate::filter::FilterError::UniqueUnreadable {
            column: column.to_string(),
            detail: e.to_string(),
        })
}

/// A filter's refusal as the engine's: the caller's fault or the deployment's, as [`FilterError`]
/// decides at its variants.
///
/// [`FilterError`]: crate::filter::FilterError
pub(crate) fn filter_refusal(e: crate::filter::FilterError) -> EngineError {
    use crate::filter::FilterError;
    match e {
        FilterError::Cancelled => EngineError::Cancelled,
        FilterError::RegionViewBuilding => EngineError::ProjectionBuilding,
        FilterError::RegionViewUnknown(view) => EngineError::UnknownView(view),
        e if e.is_callers_fault() => EngineError::FilterMalformed(e.to_string()),
        e => EngineError::FilterRefused(e.to_string()),
    }
}

/// Test every row of `domain` against `entities`, giving the rows that matched. `None` where the
/// row space declined to invert a row. Parallel over the domain, on the engine's own pool, so a
/// serial walk here cannot move the crossover against the route it competes with; a domain of no
/// more than one chunk, 4,096 rows, is walked on the calling thread. Chunks are cut by row count
/// and gather consecutive ranges, so neither one huge range nor a thousand slivers defeats the
/// split. The walk ends with [`EngineError::Cancelled`] at the next chunk once the request holding
/// `cancel` has gone.
fn per_tile_crossing(
    row_space: &mosaica_store::permutation::RowSpace,
    entities: &croaring::Bitmap,
    domain: &[Range<u32>],
    rows_in_ranges: u64,
    cancel: &Option<CancelToken>,
) -> Result<Option<croaring::Bitmap>> {
    Ok(
        per_tile_crossing_multi(row_space, &[entities], domain, rows_in_ranges, cancel)?
            .map(|mut images| images.pop().expect("one set in, one image out")),
    )
}

/// [`per_tile_crossing`] over several entity sets at once: one walk, the rows' entities read from
/// the row-to-entity tables a slice at a time. Returns one row image per input set, positionally.
/// `None` where the row space cannot invert a row of the domain.
///
/// Each set is tested through a [`mosaica_roaring::Probe`], which finds an entity's container by
/// one table load where croaring's `contains` binary-searches the keys. A viewport's entities are
/// in no order a cache can follow, so the word a test of a bitset container reads is fetched
/// [`PREFETCH_ROWS_AHEAD`] rows early.
fn per_tile_crossing_multi(
    row_space: &mosaica_store::permutation::RowSpace,
    entity_sets: &[&croaring::Bitmap],
    domain: &[Range<u32>],
    rows_in_ranges: u64,
    cancel: &Option<CancelToken>,
) -> Result<Option<Vec<croaring::Bitmap>>> {
    let Some(tables) = row_space.row_entities() else {
        return Ok(None);
    };
    if domain.iter().any(|range| range.end > tables.len()) {
        return Ok(None);
    }
    let readable: Vec<mosaica_roaring::Readable<'_>> = entity_sets
        .iter()
        .map(|set| mosaica_roaring::Readable::of(set))
        .collect();
    let probes: Vec<mosaica_roaring::Probe<'_>> = readable.iter().map(|r| r.probe()).collect();
    let chunks = domain_chunks(domain, rows_in_ranges);

    let parts = over_chunks(&chunks, cancel, |chunk| {
        // Buffered so `add_many` appends a sorted run, where a per-row `add` re-locates it.
        let mut rows: Vec<croaring::Bitmap> =
            probes.iter().map(|_| croaring::Bitmap::new()).collect();
        let mut bufs: Vec<Vec<u32>> = probes.iter().map(|_| Vec::with_capacity(1024)).collect();
        for range in chunk {
            let mut row = range.start;
            tables.for_each_slice(range.clone(), |entities| {
                for (at, &entity) in entities.iter().enumerate() {
                    if let Some(&ahead) = entities.get(at + PREFETCH_ROWS_AHEAD) {
                        for probe in &probes {
                            probe.prefetch(ahead);
                        }
                    }
                    for (i, probe) in probes.iter().enumerate() {
                        if probe.contains(entity) {
                            bufs[i].push(row);
                            if bufs[i].len() == 1024 {
                                rows[i].add_many(&bufs[i]);
                                bufs[i].clear();
                            }
                        }
                    }
                    row += 1;
                }
            });
        }
        for (image, buf) in rows.iter_mut().zip(&bufs) {
            image.add_many(buf);
        }
        rows
    })?;
    let images = (0..entity_sets.len())
        .map(|i| {
            let refs: Vec<&croaring::Bitmap> = parts.iter().map(|p| &p[i]).collect();
            croaring::Bitmap::fast_or(&refs)
        })
        .collect();
    Ok(Some(images))
}

/// How many rows ahead [`per_tile_crossing_multi`] fetches the word it will test a row's entity
/// against. Measured over a 4,000,000-row domain against a 10⁹-entity result scattered over bitset
/// containers, one thread: 152 ns a row through croaring's `contains`, 26 ns through the probe
/// alone and 16 ns with the fetch 16 rows ahead. A result held in runs is read from cache already,
/// and the fetch, which looks the container up a second time, costs it 14 ns a row against 20.
const PREFETCH_ROWS_AHEAD: usize = 16;

/// Cut `domain` into chunks of about one share of its rows each, shared by the crossing walk and
/// the render-column scan so the two fan out identically. Consecutive ranges are gathered into a
/// chunk until it holds its share and a range longer than a share is split, so a domain of many
/// short ranges is as few chunks as one long range of the same rows, and a domain of no more
/// rows than one share is one chunk.
fn domain_chunks(domain: &[Range<u32>], rows_in_ranges: u64) -> Vec<Vec<Range<u32>>> {
    let threads = rayon::current_num_threads().max(1) as u64;
    let share = (rows_in_ranges / (threads * 8))
        .max(CROSSING_CHUNK_MIN_ROWS as u64)
        .min(u32::MAX as u64) as u32;
    let mut chunks = Vec::new();
    let mut chunk: Vec<Range<u32>> = Vec::new();
    let mut held = 0u32;
    for range in domain {
        let mut start = range.start;
        while start < range.end {
            let take = (range.end - start).min(share - held);
            chunk.push(start..start + take);
            held += take;
            start += take;
            if held == share {
                chunks.push(std::mem::take(&mut chunk));
                held = 0;
            }
        }
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

/// `f` over each chunk, in order: on the calling thread where there is one, and in parallel on
/// the current pool otherwise. `cancel` is read before each chunk, and once the request holding it
/// has gone no further chunk is walked and the answer is [`EngineError::Cancelled`].
fn over_chunks<T: Send>(
    chunks: &[Vec<Range<u32>>],
    cancel: &Option<CancelToken>,
    f: impl Fn(&[Range<u32>]) -> T + Sync + Send,
) -> Result<Vec<T>> {
    let one = |chunk: &Vec<Range<u32>>| check_cancelled(cancel).map(|()| f(chunk));
    match chunks {
        [chunk] => Ok(vec![one(chunk)?]),
        _ => chunks.par_iter().map(one).collect(),
    }
}

/// `entities`' rows in `row_space`, or [`EngineError::Cancelled`] once the request holding
/// `cancel` has gone, which the walk reads as it goes.
pub(crate) fn project_until(
    row_space: &mosaica_store::permutation::RowSpace,
    entities: &croaring::Bitmap,
    cancel: &Option<CancelToken>,
) -> Result<croaring::Bitmap> {
    row_space
        .project_until(entities, &|| crate::cancel::gone(cancel))
        .ok_or(EngineError::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row space over a non-identity row order, with its `row-entity.u32` attached. An identity
    /// order would let a route that returned the row back as the entity pass.
    fn row_space_over(
        dir: &std::path::Path,
        row_order: &[u32],
    ) -> mosaica_store::permutation::RowSpace {
        use mosaica_store::permutation::{Permutation, RowSpace};
        use mosaica_store::row_entity::{write_row_entity, RowToEntity, ROW_ENTITY_FILE};

        let perm_path = dir.join("permutation.bin");
        let entities: Vec<EntityId> = row_order.iter().map(|&e| EntityId::new(e as u64)).collect();
        mosaica_store::write::write_permutation(&perm_path, &entities, row_order.len() as u64)
            .expect("permutation writes");
        let table_path = dir.join(ROW_ENTITY_FILE);
        write_row_entity(&table_path, row_order).expect("table writes");

        RowSpace::new(
            Arc::new(Permutation::load(&perm_path).expect("permutation loads")),
            row_order.len() as u32,
        )
        .with_row_entity(Arc::new(
            RowToEntity::load(&table_path).expect("table loads"),
        ))
    }

    /// The sampled blocks never exceed the blocks the domain's visible entities fall in.
    #[test]
    fn sampled_blocks_is_a_lower_bound_on_the_blocks_visible_entities_fall_in() {
        let dir = tempfile::tempdir().expect("tempdir");
        let n = 1u32 << 18;
        // Rows in an order that scatters a stretch of rows over every block of entities.
        let order: Vec<u32> = (0..n).map(|r| (r % 4) << 16 | (r / 4)).collect();
        let rows = row_space_over(dir.path(), &order);
        let mut visible = croaring::Bitmap::from_range(0..n);
        visible.remove_range(2 << 16..3 << 16);
        let one = |r: Range<u32>| vec![r];
        let domains = [one(0..4), one(0..n), vec![100..200, 9_000..9_001], one(8..12)];
        for domain in &domains {
            let total: u64 = domain.iter().map(|r| u64::from(r.end - r.start)).sum();
            let mut held = croaring::Bitmap::new();
            for r in domain {
                for row in r.clone() {
                    let e = order[row as usize];
                    if visible.contains(e) {
                        held.add(e);
                    }
                }
            }
            let blocks = u64::from(held.statistics().n_containers);
            let sampled = sampled_blocks(&rows, domain, total, &visible);
            assert!(sampled <= blocks, "{domain:?}: {sampled} sampled, {blocks} reached");
            assert!(sampled > 0);
        }
    }

    /// A viewport's few entities in few blocks narrow; as many in every block, or too many
    /// entities, do not.
    #[test]
    fn narrowing_pays_for_few_entities_in_few_blocks() {
        let visible = 64u64 << 16;
        let contiguous = croaring::Bitmap::from_range(5 << 16..(5 << 16) + 2_000);
        let scattered: croaring::Bitmap = (0..2_000u32).map(|i| (i % 64) << 16 | i).collect();
        let crowded = croaring::Bitmap::from_range(0..(visible / 8) as u32);
        assert!(narrowing_pays(&contiguous, visible, 64));
        assert!(!narrowing_pays(&scattered, visible, 64));
        assert!(!narrowing_pays(&crowded, visible, 64));
        assert!(narrowing_may_pay(visible / 16, visible));
        assert!(!narrowing_may_pay(visible / 16 + 1, visible));
    }

    /// Testing the viewport's rows one at a time and projecting the whole result give the same
    /// set over every range the request can ask about.
    #[test]
    fn filter_routes_agree_over_the_domain() {
        let dir = tempfile::tempdir().expect("tempdir");
        // 20,011 is coprime with the row count, so the order is a genuine shuffle.
        let rows = 40_000u32;
        let row_order: Vec<u32> = (0..rows)
            .map(|r| (r as u64 * 20_011 % rows as u64) as u32)
            .collect();
        let space = row_space_over(dir.path(), &row_order);

        // Every seventh entity, plus a dense block — neither uniform nor one run.
        let mut entities = croaring::Bitmap::new();
        entities.add_many(&(0..rows).step_by(7).collect::<Vec<u32>>());
        entities.add_range(1_000u32..9_000);

        let domain = vec![0u32..12_345, 20_000..20_003, 30_000..40_000];
        let mut domain_rows = croaring::Bitmap::new();
        for range in &domain {
            domain_rows.add_range(range.clone());
        }

        let per_tile =
            per_tile_crossing(&space, &entities, &domain, domain_rows.cardinality(), &None)
                .expect("a walk with no token is never cancelled")
                .expect("a row space with a table can always invert");
        let projected = space.project(&entities);

        assert_eq!(
            per_tile,
            projected.and(&domain_rows),
            "the per-tile crossing and the projection disagree inside the domain"
        );
        // The per-tile route claims nothing outside the domain.
        assert!(
            per_tile.andnot(&domain_rows).is_empty(),
            "the per-tile crossing returned rows it never tested"
        );
        assert!(
            !projected.andnot(&domain_rows).is_empty(),
            "the fixture is degenerate: every matching row is inside the domain, so the two routes \
             would agree even if the domain were ignored"
        );
    }

    /// A view with no `row-entity.u32` declines the per-tile route rather than reading
    /// `entity_of`'s `None` as "no entity", which would drop rows silently.
    #[test]
    fn a_row_space_without_a_table_declines_the_per_tile_route() {
        use mosaica_store::permutation::{Permutation, RowSpace};

        let dir = tempfile::tempdir().expect("tempdir");
        let perm_path = dir.path().join("permutation.bin");
        let entities_in_order: Vec<EntityId> = [4u64, 2, 0, 5, 1, 3]
            .iter()
            .map(|&e| EntityId::new(e))
            .collect();
        mosaica_store::write::write_permutation(&perm_path, &entities_in_order, 6)
            .expect("permutation writes");
        let space = RowSpace::new(Arc::new(Permutation::load(&perm_path).expect("loads")), 6);

        assert!(!space.can_invert());
        let mut entities = croaring::Bitmap::new();
        entities.add_many(&[0, 1, 2, 3, 4, 5]);
        let domain = vec![0u32..2, 4..6];
        assert!(
            matches!(per_tile_crossing(&space, &entities, &domain, 4, &None), Ok(None)),
            "the walk must decline rather than return the rows it happened to resolve"
        );
    }

    /// A walk over chunks answers each chunk in order while its request is live, and once the
    /// request has gone it walks no further chunk and answers `Cancelled`, on the calling thread
    /// and on a pool alike.
    #[test]
    fn a_walk_over_chunks_stops_at_the_next_chunk_once_its_request_has_gone() {
        use std::sync::atomic::AtomicUsize;

        let chunks: Vec<Vec<Range<u32>>> = (0..64u32)
            .map(|i| std::iter::once(i * 10..i * 10 + 10).collect())
            .collect();
        let first_row = |chunk: &[Range<u32>]| chunk[0].start;
        let live = Some(CancelToken::new());
        for cancel in [&None, &live] {
            let walked = over_chunks(&chunks, cancel, first_row).expect("a live request answers");
            assert_eq!(walked, (0..64u32).map(|i| i * 10).collect::<Vec<_>>());
        }

        let gone = CancelToken::new();
        gone.cancel();
        let walked = AtomicUsize::new(0);
        let count = |_: &[Range<u32>]| walked.fetch_add(1, Ordering::Relaxed);
        for chunks in [&chunks[..], &chunks[..1]] {
            let stopped = over_chunks(chunks, &Some(gone.clone()), count);
            assert!(matches!(stopped, Err(EngineError::Cancelled)));
        }
        assert_eq!(
            walked.load(Ordering::Relaxed),
            0,
            "no chunk is walked for a request that has gone"
        );

        // Flipped by the first chunk walked: the walk ends there on one thread.
        let token = CancelToken::new();
        let walked = AtomicUsize::new(0);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("a pool of one");
        let stopped = pool.install(|| {
            over_chunks(&chunks, &Some(token.clone()), |_| {
                token.cancel();
                walked.fetch_add(1, Ordering::Relaxed)
            })
        });
        assert!(matches!(stopped, Err(EngineError::Cancelled)));
        assert_eq!(
            walked.load(Ordering::Relaxed),
            1,
            "the chunk in hand finishes and no other begins"
        );
    }

    /// The domain is the request's tile parts in view row space: shifted by each segment's
    /// `row_base`, sorted across segments, and merged where they touch; it must never widen.
    #[test]
    fn the_crossing_domain_shifts_by_row_base_and_merges_only_what_touches() {
        // Segment 0 based at row 0, segment 1 at row 1,000; the third tile splits across both.
        let ranges = vec![
            vec![(0usize, 0u32..10)],
            vec![(0usize, 10u32..25)],
            vec![(0usize, 40u32..50), (1usize, 0u32..5)],
        ];
        let domain = crossing_domain(&ranges, &[0, 1_000]);
        assert_eq!(
            domain,
            vec![0u32..25, 40..50, 1_000..1_005],
            "adjacent tiles merge, a gap survives, and segment 1's rows land at its row_base"
        );
        assert_eq!(
            domain.iter().map(|r| r.len()).sum::<usize>(),
            10 + 15 + 10 + 5,
            "merging changed how many rows the domain covers"
        );
    }

    /// Strictly greater, so a result exactly at the ratio still projects.
    #[test]
    fn the_per_tile_route_is_taken_only_past_the_ratio() {
        let looks_cheaper = |matched: u64, viewport: u64| {
            matched > viewport.saturating_mul(PER_TILE_CROSSING_RATIO)
        };
        assert!(!looks_cheaper(300_000, 300_000), "1x projects");
        assert!(
            !looks_cheaper(900_000, 300_000),
            "exactly at the ratio projects"
        );
        assert!(looks_cheaper(900_001, 300_000), "just past it does not");
        // An empty viewport: the per-tile route walks nothing and is free.
        assert!(looks_cheaper(1, 0));
        assert!(
            !looks_cheaper(0, 0),
            "nothing matched -- either route is empty"
        );
    }

    /// Every row of one run, matched and narrowed to the rows that carry a value — the leaf's
    /// sequence over one segment: prepare the matcher, then scan.
    fn run(
        slice: &ScalarSlice<'_>,
        predicate: &RowPredicate<'_>,
        present: Option<&croaring::Bitmap>,
    ) -> Vec<u32> {
        let rows_in_slice = match slice {
            ScalarSlice::Bool(a) => a.len(),
            ScalarSlice::I32(v) => v.len(),
            ScalarSlice::U8(v) => v.len(),
            ScalarSlice::F64(v) => v.len(),
            ScalarSlice::I64(v) | ScalarSlice::TimestampUs(v) => v.len(),
            _ => unreachable!("the fixtures below use these widths"),
        } as u32;
        // Nothing to prepare is nothing to scan.
        let Some(prepared) = Prepared::of(slice, predicate) else {
            return Vec::new();
        };
        let mut rows = croaring::Bitmap::new();
        let mut buf = Vec::with_capacity(1024);
        scan_run(
            &prepared,
            0,
            0..rows_in_slice,
            present,
            &mut rows,
            &mut buf,
        );
        assert!(buf.is_empty(), "a run must leave its buffer empty");
        rows.iter().collect()
    }

    /// The rows that carry a value, as [`present_rows`] hands them over.
    fn presence(absent: &[u32], rows: u32) -> croaring::Bitmap {
        let mut present = croaring::Bitmap::new();
        present.add_range(0..rows);
        for row in absent {
            present.remove(*row);
        }
        present
    }

    /// A row with no number matches no range, including one containing zero. Rows 1 and 3 carry
    /// no value and hold the stored zero; row 4 carries a genuine zero.
    #[test]
    fn an_absent_number_matches_no_range_not_even_one_containing_zero() {
        // rows:      0    1*   2    3*   4    5     (* = no value, stored as the type's zero)
        let values = [7i32, 0, -3, 0, 0, 40];
        let slice = ScalarSlice::I32(&values);
        let present = presence(&[1, 3], 6);

        let straddling_zero = RowPredicate::Range {
            lo: Some(Endpoint {
                value: Scalar::Int(-10),
                inclusive: true,
            }),
            hi: Some(Endpoint {
                value: Scalar::Int(10),
                inclusive: true,
            }),
        };
        assert_eq!(
            run(&slice, &straddling_zero, Some(&present)),
            vec![0, 2, 4],
            "a row with no number matched a range containing zero"
        );

        // A genuine zero must survive the same rule.
        let zero_only = RowPredicate::Range {
            lo: Some(Endpoint {
                value: Scalar::Int(0),
                inclusive: true,
            }),
            hi: Some(Endpoint {
                value: Scalar::Int(0),
                inclusive: true,
            }),
        };
        assert_eq!(
            run(&slice, &zero_only, Some(&present)),
            vec![4],
            "a real zero stopped matching"
        );

        // An unbounded range is "carries a value", not "every row".
        assert_eq!(
            run(
                &slice,
                &RowPredicate::Range { lo: None, hi: None },
                Some(&present)
            ),
            vec![0, 2, 4, 5]
        );
        // A needle of zero names the genuine zero only.
        assert_eq!(
            run(
                &slice,
                &RowPredicate::NumberIn(&[Scalar::Int(0), Scalar::Int(40)]),
                Some(&present)
            ),
            vec![4, 5]
        );
    }

    /// A category reads absence from its own code 0 and has no bitmap at all, so the scan asks
    /// for no presence on this family.
    #[test]
    fn a_category_reads_absence_from_its_sentinel_and_asks_for_no_bitmap() {
        let codes = [1u8, 0, 2, 0, 1, 3];
        let slice = ScalarSlice::U8(&codes);
        assert!(!RowPredicate::CodeIn(&[1]).reads_presence());

        assert_eq!(run(&slice, &RowPredicate::CodeIn(&[1]), None), vec![0, 4]);
        assert_eq!(
            run(&slice, &RowPredicate::CodeIn(&[1, 2]), None),
            vec![0, 2, 4]
        );
        assert_eq!(
            run(&slice, &RowPredicate::CodeIn(&[0]), None),
            Vec::<u32>::new(),
            "the absent sentinel names no row, even asked for by code"
        );
    }

    /// The bounds are narrowed to the width the hot column stores, so a bound outside the type is
    /// no constraint or no match rather than a wrapped comparison, and a float column is compared
    /// as floats, with NaN unordered.
    #[test]
    fn a_range_over_the_hot_column_is_narrowed_to_the_columns_own_width() {
        let values = [0u8, 1, 2, 254, 255];
        let slice = ScalarSlice::U8(&values);
        let at = |v: i128, inclusive: bool| {
            Some(Endpoint {
                value: Scalar::Int(v),
                inclusive,
            })
        };
        let range = |lo, hi| RowPredicate::Range { lo, hi };

        assert_eq!(
            run(&slice, &range(at(1, true), at(2, true)), None),
            vec![1, 2]
        );
        assert_eq!(
            run(&slice, &range(at(-5, true), None), None),
            vec![0, 1, 2, 3, 4],
            "a bound below the floor constrains nothing"
        );
        assert_eq!(
            run(&slice, &range(at(300, true), None), None),
            Vec::<u32>::new(),
            "a bound above the ceiling excludes everything"
        );
        assert_eq!(
            run(&slice, &range(None, at(-1, true)), None),
            Vec::<u32>::new()
        );
        assert_eq!(
            run(&slice, &range(at(255, false), None), None),
            Vec::<u32>::new(),
            "`> 255` over a u8 is nothing, not everything wrapped"
        );

        let fractional = |v: f64, inclusive: bool| {
            Some(Endpoint {
                value: Scalar::Float(v),
                inclusive,
            })
        };
        // NaN satisfies nothing as a bound.
        assert_eq!(
            run(&slice, &range(fractional(f64::NAN, true), None), None),
            Vec::<u32>::new()
        );
        let floats = [1.0f64, f64::NAN, 3.0];
        assert_eq!(
            run(&ScalarSlice::F64(&floats), &range(None, None), None),
            vec![0, 1, 2],
            "an unbounded range asks only that the row carry a value"
        );
        assert_eq!(
            run(
                &ScalarSlice::F64(&floats),
                &range(at(0, true), at(4, true)),
                None
            ),
            vec![0, 2],
            "NaN is outside every bounded range"
        );
        assert_eq!(
            run(
                &ScalarSlice::F64(&floats),
                &RowPredicate::NumberIn(&[Scalar::Float(f64::NAN)]),
                None
            ),
            Vec::<u32>::new(),
            "NaN equals nothing, itself included"
        );
    }

    /// A bool and a datetime are read as the entity route stores them, so a predicate means the
    /// same thing on both routes.
    #[test]
    fn a_bool_and_a_datetime_compare_as_their_entity_space_storage_does() {
        let flags = arrow::array::BooleanArray::from(vec![true, false, true, false]);
        let slice = ScalarSlice::Bool(&flags);
        let (yes, no) = ([Scalar::Int(1)], [Scalar::Int(0)]);
        assert_eq!(run(&slice, &RowPredicate::NumberIn(&yes), None), vec![0, 2]);
        assert_eq!(run(&slice, &RowPredicate::NumberIn(&no), None), vec![1, 3]);
        // `false` is a value, not a missing one.
        assert_eq!(
            run(
                &slice,
                &RowPredicate::NumberIn(&no),
                Some(&presence(&[3], 4))
            ),
            vec![1],
            "a bool with no value matched `false`"
        );

        let micros = [1_000i64, 2_000, 3_000];
        assert_eq!(
            run(
                &ScalarSlice::TimestampUs(&micros),
                &RowPredicate::Range {
                    lo: Some(Endpoint {
                        value: Scalar::Int(2_000),
                        inclusive: true,
                    }),
                    hi: None,
                },
                None
            ),
            vec![1, 2]
        );
    }
}

