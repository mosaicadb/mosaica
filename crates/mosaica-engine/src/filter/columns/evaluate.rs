use std::borrow::Cow;

use croaring::Bitmap;
use mosaica_filter::ColumnPostings;
use mosaica_types::AttrLocalId;

use super::{ColumnLayers, FilterColumns, Layer, Route, TextLayer};
use crate::cancel::CancelToken;
use crate::filter::declared::Placement;
use crate::filter::error::FilterError;
use crate::filter::expr::{
    evaluate_leaf, FilterExpr, FilterOperand, LeafQuery, LeafResolver, MemberOfArtifact,
    RoutedFilter, RowExpr, RowLeafResolvers, TextQuery, UniqueResolver, MEMBER_OF_COLUMN,
    MOSAICA_ID_COLUMN, REGION_COLUMN,
};
use mosaica_store::unique::{key_of_integer, UniqueKey};
use crate::filter::scan::text::{phrase_match, text_match};
use crate::filter::scan::{scan, scan_layer};
use crate::filter::check_cancelled;

/// Which route a leaf over a column affording both takes, for one request.
#[derive(Clone, Copy)]
pub enum RouteChoice<'a> {
    Entity,
    Row,
    /// Whichever costs less, where the request reads `rows` rows, the viewer sees `visible`
    /// entities, and `leaf` prices each leaf over every shard the request reads, so a leaf takes
    /// the same route in each.
    Cheaper {
        rows: u64,
        visible: u64,
        leaf: &'a dyn LeafCosts,
    },
}

/// What routing one leaf over a column with both routes costs, for one request.
pub trait LeafCosts: Sync {
    fn cost(&self, column: &str, operand: &FilterOperand) -> LeafCost;
}

/// One leaf's [`LeafCosts::cost`].
#[derive(Debug, Clone, Copy, Default)]
pub struct LeafCost {
    /// How many entities the leaf's postings hold, where its column answers it from them
    /// ([`FilterColumns::posting_entities`]).
    pub posting: Option<u64>,
    /// How many of the request's rows the filter cache already holds the leaf's rows for, which the
    /// row route reads without a scan.
    pub kept: u64,
}

/// How many rows the row route scans in the time the entity route projects one matched entity
/// into row space.
const ROWS_PER_PROJECTED_ENTITY: u64 = 2;

/// Whether a leaf is cheaper scanned over `rows` rows than evaluated in entity space, where the
/// viewer sees `visible` entities and `posting` is how many entities the leaf's postings hold, for
/// a leaf its column answers from postings. Over a scanned column the entity route reads every
/// visible entity, at about a row's cost each. Over postings it reads only the postings named, and
/// costs the projection of their matches into row space, which number at most the smaller of
/// `posting` and `visible`.
fn row_route_pays(rows: u64, visible: u64, posting: Option<u64>) -> bool {
    match posting {
        None => rows <= visible,
        Some(posting) => rows <= ROWS_PER_PROJECTED_ENTITY.saturating_mul(posting.min(visible)),
    }
}

/// The space a sub-tree evaluates in: [`FilterColumns::space_of`]'s answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    Entity,
    Row,
    Mixed,
}

impl FilterColumns {
    /// Entities in `candidate` whose value for `column` satisfies `operand`, a subset of
    /// `candidate` by construction. Every layer is scanned and the results unioned; the layers
    /// partition entity space, so an entity is tested against exactly one value.
    pub fn resolve(
        &self,
        column: &str,
        operand: &FilterOperand,
        candidate: &Bitmap,
    ) -> Result<Bitmap, FilterError> {
        self.resolve_until(column, operand, candidate, &evaluate_leaf, &None)
    }

    /// [`Self::resolve`], with a text or value leaf answered through `leaves`, ended with
    /// [`FilterError::Cancelled`] once the request holding `cancel` has gone.
    fn resolve_until(
        &self,
        column: &str,
        operand: &FilterOperand,
        candidate: &Bitmap,
        leaves: &LeafResolver<'_>,
        cancel: &Option<CancelToken>,
    ) -> Result<Bitmap, FilterError> {
        let name = column;
        let column = self
            .columns
            .get(name)
            .filter(|column| column.filterable)
            .ok_or_else(|| FilterError::UndeclaredColumn(name.to_string()))?;

        // Text answers from postings only: no value column and no scan.
        let (layers, postings, route) = match &column.layers {
            ColumnLayers::Text {
                layers, analyser, ..
            } => {
                return resolve_text(name, layers, analyser, operand, candidate, leaves, cancel);
            }
            ColumnLayers::Values {
                layers,
                postings,
                route,
                ..
            } => (layers, postings.as_deref(), *route),
        };

        leaves(name, LeafQuery::Value(operand), candidate, &|candidate| {
            resolve_values(name, layers, postings, route, operand, candidate)
        })
    }

    /// Evaluate a filter expression against `candidate`.
    ///
    /// Every node is evaluated under the candidate: a conjunction narrows it as it goes, and a
    /// disjunction evaluates each branch under the original candidate and unions.
    pub fn evaluate(&self, expr: &FilterExpr, candidate: &Bitmap) -> Result<Bitmap, FilterError> {
        expr.check()?;
        let no_index = |column: &str, _: &[UniqueKey]| {
            Err(FilterError::UniqueUnreadable {
                column: column.to_string(),
                detail: "this evaluation holds no unique index".to_string(),
            })
        };
        self.eval(expr, candidate, &no_index, &evaluate_leaf, &None)
    }

    /// The keys a leaf names where its column's unique index answers it: `eq` or `in` on a unique
    /// column. A number the column's type cannot hold names no key, so it matches nothing.
    /// `mosaica_id` is answered as a unique `u64` column, each key the identifier itself.
    pub(crate) fn unique_keys(
        &self,
        column: &str,
        operand: &FilterOperand,
    ) -> Option<Vec<UniqueKey>> {
        let ty = match column {
            MOSAICA_ID_COLUMN => mosaica_spatial::tiler::ScalarType::U64,
            _ => *self.unique.get(column)?,
        };
        let number = |value: &crate::filter::Scalar| match *value {
            crate::filter::Scalar::Int(i) => key_of_integer(ty, i),
            crate::filter::Scalar::Float(f) if f.is_finite() && f.fract() == 0.0 => {
                key_of_integer(ty, f as i128)
            }
            crate::filter::Scalar::Float(_) => None,
        };
        Some(match operand {
            FilterOperand::NumEquals(value) => number(value).into_iter().collect(),
            FilterOperand::NumIn(values) => values.iter().filter_map(number).collect(),
            FilterOperand::TextEquals(value) => vec![UniqueKey::keyword(value)],
            FilterOperand::TextIn(values) => values.iter().map(|v| UniqueKey::keyword(v)).collect(),
            _ => return None,
        })
    }

    /// Whether `column` has a filterable value column or text index beside any unique index.
    fn has_value_route(&self, column: &str) -> bool {
        self.columns.get(column).is_some_and(|held| held.filterable)
    }

    /// Whether `expr` is answered over items by every route alike, so its answer over a candidate is
    /// its answer over any wider set, intersected with the candidate: a leaf a value column or a
    /// text index answers, or a conjunction or disjunction of such expressions. A negation keeps the
    /// candidate's items that carry no value, and a unique index answers a buffered item, so neither
    /// is.
    pub(crate) fn answered_over_items(&self, expr: &FilterExpr) -> bool {
        match expr {
            FilterExpr::Leaf { column, operand } => {
                self.unique_keys(column, operand).is_none()
                    && self.has_value_route(column)
                    && self.placements.get(column).is_some_and(|held| held.entity)
            }
            FilterExpr::AllOf(kids) | FilterExpr::AnyOf(kids) => {
                !kids.is_empty() && kids.iter().all(|kid| self.answered_over_items(kid))
            }
            _ => false,
        }
    }

    /// How many entities the base postings hold for the values an `eq` or `in` names, over the whole
    /// shard, where the column answers it from them; `None` for any other leaf, or a posting that
    /// will not read. Only a `visibility = "public"` column answers from its postings.
    pub(crate) fn posting_entities(&self, column: &str, operand: &FilterOperand) -> Option<u64> {
        let ColumnLayers::Values {
            postings: Some(postings),
            route: Route::Postings,
            ..
        } = &self.columns.get(column)?.layers
        else {
            return None;
        };
        codes_of(operand)?
            .iter()
            .map(|code| {
                let posting = postings.posting(*code).ok()?;
                Some(posting.map_or(0, |posting| posting.cardinality()))
            })
            .sum()
    }

    /// One leaf in entity space: from the unique index where it answers the operand, from the
    /// value layers otherwise.
    fn eval_leaf(
        &self,
        column: &str,
        operand: &FilterOperand,
        candidate: &Bitmap,
        unique: &UniqueResolver<'_>,
        leaves: &LeafResolver<'_>,
        cancel: &Option<CancelToken>,
    ) -> Result<Bitmap, FilterError> {
        if let Some(keys) = self.unique_keys(column, operand) {
            let mut out = Bitmap::new();
            if !keys.is_empty() {
                out.add_many(&unique(column, &keys)?);
            }
            out.and_inplace(candidate);
            return Ok(out);
        }
        if self.unique.contains_key(column) && !self.has_value_route(column) {
            return Err(FilterError::UniqueOnly {
                column: column.to_string(),
                operator: operator_name(operand).to_string(),
            });
        }
        self.resolve_until(column, operand, candidate, leaves, cancel)
    }

    /// Evaluate a filter expression's entity-space part and route the rest.
    ///
    /// `choice` decides a leaf over a column with both routes.
    ///
    /// A tree with no row-space leaf returns [`RoutedFilter::Entity`]. Otherwise every maximal
    /// entity-space sub-tree is evaluated here, under `candidate`, and the returned [`RowExpr`]
    /// awaits the row-space leaves, which need the request's tile ranges.
    ///
    /// Once the request holding `cancel` has gone, the evaluation ends with
    /// [`FilterError::Cancelled`] at the next leaf, or within [`crate::filter::CANCEL_EVERY`] entities of a walk
    /// inside one. `None` is never cancelled, which is what [`Self::evaluate`] passes.
    pub fn evaluate_routed(
        &self,
        expr: &FilterExpr,
        candidate: &Bitmap,
        choice: RouteChoice<'_>,
        resolvers: &RowLeafResolvers<'_>,
        cancel: &Option<CancelToken>,
    ) -> Result<RoutedFilter, FilterError> {
        if self.admitted_space(expr, choice, resolvers.layers)? == Space::Entity {
            return Ok(RoutedFilter::Entity(self.eval(
                expr,
                candidate,
                resolvers.unique,
                resolvers.leaves,
                cancel,
            )?));
        }
        Ok(RoutedFilter::Row(
            self.route(expr, candidate, choice, resolvers, cancel)?,
        ))
    }

    /// Refuse every filter that no candidate could make answerable, from the declarations alone:
    /// its depth, every column it names, and every layer and level a `member_of` names, which
    /// `layer_levels` answers: the number of levels a layer `/v1/meta` lists to this principal
    /// holds, and `None` for any other. [`Self::evaluate_routed`] refuses exactly these
    /// before it reads a row, and the evaluation after it refuses only an artefact that will not
    /// read.
    pub fn admit(
        &self,
        expr: &FilterExpr,
        layer_levels: &dyn Fn(&str) -> Option<u32>,
    ) -> Result<(), FilterError> {
        self.admitted_space(expr, RouteChoice::Entity, layer_levels)
            .map(|_| ())
    }

    fn admitted_space(
        &self,
        expr: &FilterExpr,
        choice: RouteChoice<'_>,
        layer_levels: &dyn Fn(&str) -> Option<u32>,
    ) -> Result<Space, FilterError> {
        fn layers(
            expr: &FilterExpr,
            levels_of: &dyn Fn(&str) -> Option<u32>,
        ) -> Result<(), FilterError> {
            match expr {
                FilterExpr::MemberOf(leaf) => {
                    let held = levels_of(&leaf.layer)
                        .ok_or_else(|| FilterError::UnknownLayer(leaf.layer.clone()))?;
                    match leaf.artifact {
                        MemberOfArtifact::Key { level, .. } if level >= held => {
                            Err(FilterError::UnknownLevel {
                                layer: leaf.layer.clone(),
                                level,
                                held,
                            })
                        }
                        _ => Ok(()),
                    }
                }
                FilterExpr::AllOf(kids) | FilterExpr::AnyOf(kids) | FilterExpr::NoneOf(kids) => {
                    kids.iter().try_for_each(|kid| layers(kid, levels_of))
                }
                _ => Ok(()),
            }
        }
        expr.check()?;
        let space = self.space_of(expr, choice)?;
        layers(expr, layer_levels)?;
        Ok(space)
    }

    /// One leaf's routed space, the single transcription of the leaf-routing rule: a unique index
    /// answers `eq` and `in` in entity space, and nothing else.
    fn leaf_space(
        &self,
        column: &str,
        operand: &FilterOperand,
        choice: RouteChoice<'_>,
    ) -> Result<Space, FilterError> {
        if column == REGION_COLUMN || column == MEMBER_OF_COLUMN {
            return Ok(Space::Row);
        }
        if self.unique_keys(column, operand).is_some() {
            return Ok(Space::Entity);
        }
        let placement = self.placement_of(column)?;
        let entity = placement.entity
            && (!self.unique.contains_key(column) || self.has_value_route(column));
        Ok(match (entity, placement.row) {
            (true, false) => Space::Entity,
            (false, true) => Space::Row,
            (true, true) => match choice {
                RouteChoice::Entity => Space::Entity,
                RouteChoice::Row => Space::Row,
                RouteChoice::Cheaper {
                    rows,
                    visible,
                    leaf,
                } => {
                    let cost = leaf.cost(column, operand);
                    if row_route_pays(rows.saturating_sub(cost.kept), visible, cost.posting) {
                        Space::Row
                    } else {
                        Space::Entity
                    }
                }
            },
            // A unique column with no value route, asked for an operator its index cannot answer;
            // the evaluation refuses it naming the operator.
            (false, false) => Space::Entity,
        })
    }

    /// One filterable column's placement, refused exactly as [`FilterColumns::resolve`] refuses an
    /// undeclared one.
    fn placement_of(&self, column: &str) -> Result<Placement, FilterError> {
        self.placements
            .get(column)
            .copied()
            .ok_or_else(|| FilterError::UndeclaredColumn(column.to_string()))
    }

    /// Which space `expr` evaluates in. `Entity` means the whole sub-tree can be answered by the
    /// existing entity-space evaluation.
    fn space_of(&self, expr: &FilterExpr, choice: RouteChoice<'_>) -> Result<Space, FilterError> {
        match expr {
            FilterExpr::Leaf { column, operand } => self.leaf_space(column, operand, choice),
            FilterExpr::Region(_) | FilterExpr::MemberOf(_) => Ok(Space::Row),
            FilterExpr::AllOf(kids) | FilterExpr::AnyOf(kids) | FilterExpr::NoneOf(kids) => {
                let mut all_entity = true;
                for kid in kids {
                    if self.space_of(kid, choice)? != Space::Entity {
                        all_entity = false;
                    }
                }
                Ok(if all_entity {
                    Space::Entity
                } else {
                    Space::Mixed
                })
            }
        }
    }

    /// Build the routed tree: entity-pure sub-trees evaluated to verdicts under `candidate`,
    /// row-space leaves carried through for the per-tile evaluation.
    fn route(
        &self,
        expr: &FilterExpr,
        candidate: &Bitmap,
        choice: RouteChoice<'_>,
        resolvers: &RowLeafResolvers<'_>,
        cancel: &Option<CancelToken>,
    ) -> Result<RowExpr, FilterError> {
        if self.space_of(expr, choice)? == Space::Entity {
            return Ok(RowExpr::Entity(self.eval(
                expr,
                candidate,
                resolvers.unique,
                resolvers.leaves,
                cancel,
            )?));
        }
        match expr {
            FilterExpr::Leaf { column, operand } => Ok(RowExpr::Leaf {
                column: column.clone(),
                family: self.placement_of(column)?.family,
                operand: operand.clone(),
            }),
            FilterExpr::Region(leaf) => Ok(RowExpr::Region((resolvers.regions)(leaf)?)),
            FilterExpr::MemberOf(leaf) => Ok(RowExpr::MemberOf((resolvers.members)(leaf)?)),
            FilterExpr::AllOf(kids) => Ok(RowExpr::AllOf(
                kids.iter()
                    .map(|kid| self.route(kid, candidate, choice, resolvers, cancel))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FilterExpr::AnyOf(kids) => Ok(RowExpr::AnyOf(
                kids.iter()
                    .map(|kid| self.route(kid, candidate, choice, resolvers, cancel))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FilterExpr::NoneOf(kids) => Ok(RowExpr::NotInRows(
                kids.iter()
                    .map(|kid| self.route(kid, candidate, choice, resolvers, cancel))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
        }
    }

    fn eval(
        &self,
        expr: &FilterExpr,
        candidate: &Bitmap,
        unique: &UniqueResolver<'_>,
        leaves: &LeafResolver<'_>,
        cancel: &Option<CancelToken>,
    ) -> Result<Bitmap, FilterError> {
        match expr {
            FilterExpr::Leaf { column, operand } => {
                check_cancelled(cancel)?;
                self.eval_leaf(column, operand, candidate, unique, leaves, cancel)
            }
            FilterExpr::Region(_) => Err(FilterError::RegionInEntitySpace),
            FilterExpr::MemberOf(_) => Err(FilterError::MemberOfInEntitySpace),
            FilterExpr::AllOf(kids) => {
                let mut live = candidate.clone();
                for kid in kids {
                    live = self.eval(kid, &live, unique, leaves, cancel)?;
                    if live.is_empty() {
                        break;
                    }
                }
                Ok(live)
            }
            FilterExpr::AnyOf(kids) => {
                let mut out = Bitmap::new();
                for kid in kids {
                    out |= self.eval(kid, candidate, unique, leaves, cancel)?;
                }
                Ok(out)
            }
            FilterExpr::NoneOf(kids) => {
                let mut out = candidate.clone();
                for kid in kids {
                    // Under `out`, not `candidate`, the same narrowing `AllOf` does.
                    out.andnot_inplace(&self.eval(kid, &out, unique, leaves, cancel)?);
                    if out.is_empty() {
                        break;
                    }
                }
                Ok(out)
            }
        }
    }
}

/// A leaf over one column's value layers under `candidate`. The base build's answer comes from
/// the postings where the column's route reads them; every extent layer is scanned, since no flush
/// writes postings.
fn resolve_values(
    name: &str,
    layers: &[Layer],
    postings: Option<&ColumnPostings>,
    route: Route,
    operand: &FilterOperand,
    candidate: &Bitmap,
) -> Result<Bitmap, FilterError> {
    if let (Route::Postings, Some(postings), Some(values)) = (route, postings, codes_of(operand)) {
        let mut out = Bitmap::new();
        for value in values {
            out |= postings
                .narrow(*value, candidate)
                .map_err(|e| FilterError::postings_unreadable(name, e))?;
        }
        // Selected by the absence of a manifest path, not position: a coalesce appends its
        // replacement at the end of the list.
        for layer in layers.iter().filter(|l| l.values_rel.is_some()) {
            out |= scan(&layer.values, operand, candidate);
        }
        return Ok(out);
    }
    let mut out = Bitmap::new();
    for layer in layers {
        out |= scan_layer(name, layer, operand, candidate)?;
    }
    Ok(out)
}

/// A `match` or a `phrase` over one text column's layers: [`FilterColumns::resolve`]'s text half,
/// answered through `resolver`.
fn resolve_text(
    name: &str,
    text: &[TextLayer],
    analyser: &mosaica_analyse::Analyser,
    operand: &FilterOperand,
    candidate: &Bitmap,
    resolver: &LeafResolver<'_>,
    cancel: &Option<CancelToken>,
) -> Result<Bitmap, FilterError> {
    let Some(query) = text_query(analyser, operand) else {
        return Ok(Bitmap::new());
    };
    resolver(name, LeafQuery::Text(&query), candidate, &|candidate| {
        text_answer(name, text, &query, candidate, cancel)
    })
}

/// `operand` as `analyser` reads it, or `None` where it is not a text operand.
fn text_query(analyser: &mosaica_analyse::Analyser, operand: &FilterOperand) -> Option<TextQuery> {
    Some(match operand {
        FilterOperand::Match { query, minimum } => {
            let tokens = distinct(analyser.tokens(query));
            TextQuery::Match {
                minimum: minimum.unwrap_or(tokens.len() as u32),
                tokens,
            }
        }
        FilterOperand::Phrase { query } => {
            let words = analyser.tokens(query);
            if words.len() < 2 {
                let tokens = distinct(words);
                TextQuery::Match {
                    minimum: tokens.len() as u32,
                    tokens,
                }
            } else {
                TextQuery::Phrase { words }
            }
        }
        _ => return None,
    })
}

/// `tokens` sorted, each once.
fn distinct(mut tokens: Vec<String>) -> Vec<String> {
    tokens.sort();
    tokens.dedup();
    tokens
}

/// The entities of `candidate` whose value in one of `text`'s layers answers `query`.
///
/// A phrase is the conjunction of its words in each layer, narrowed by that layer's positions to
/// the items saying them adjacent and in order.
fn text_answer(
    name: &str,
    text: &[TextLayer],
    query: &TextQuery,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
) -> Result<Bitmap, FilterError> {
    let (tokens, minimum, phrase) = match query {
        TextQuery::Match { tokens, minimum } => (Cow::Borrowed(tokens), *minimum, None),
        TextQuery::Phrase { words } => {
            let tokens = distinct(words.clone());
            let minimum = tokens.len() as u32;
            (Cow::Owned(tokens), minimum, Some(words))
        }
    };
    let mut out = Bitmap::new();
    for layer in text {
        let survivors = text_match(
            name,
            &layer.dict,
            &layer.postings,
            &tokens,
            minimum,
            candidate,
            cancel,
        )?;
        let Some(words) = phrase else {
            out |= survivors;
            continue;
        };
        // `text_match` intersects with the candidate as it reads each posting, so this cannot
        // fire; it is checked before any positions are read on a survivor's behalf.
        if !survivors.is_subset(candidate) {
            return Err(FilterError::postings_unreadable(
                name,
                "the phrase conjunction named an entity outside the candidate",
            ));
        }
        out |= phrase_match(
            name,
            &layer.dict,
            &layer.postings,
            &layer.positions,
            words,
            &survivors,
            cancel,
        )?;
    }
    Ok(out)
}

/// An operand's operator, as a filter spells it.
fn operator_name(operand: &FilterOperand) -> &'static str {
    match operand {
        FilterOperand::Equals(_) | FilterOperand::TextEquals(_) | FilterOperand::NumEquals(_) => {
            "eq"
        }
        FilterOperand::In(_) | FilterOperand::TextIn(_) | FilterOperand::NumIn(_) => "in",
        FilterOperand::TextPrefix(_) => "prefix",
        FilterOperand::TextContains(_) => "contains",
        FilterOperand::Match { .. } => "match",
        FilterOperand::Phrase { .. } => "phrase",
        FilterOperand::Range { .. } => "range",
    }
}

/// The vocabulary codes an operand names, or `None` where the operand is not a category one.
///
/// The match is on the operand's shape, never on the values it carries, so the route cannot
/// become a function of which value was asked for.
fn codes_of(operand: &FilterOperand) -> Option<&[AttrLocalId]> {
    match operand {
        FilterOperand::Equals(v) => Some(std::slice::from_ref(v)),
        FilterOperand::In(vs) => Some(vs),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The keyword route: a dictionary per layer, an ordinal question per operand
// ---------------------------------------------------------------------------------------------
