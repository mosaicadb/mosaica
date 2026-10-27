use std::sync::Arc;

use croaring::Bitmap;
use mosaica_types::AttrLocalId;

use super::{Endpoint, Family, FilterError, Scalar};

/// What a request may ask of one column.
///
/// `PartialEq` but not `Eq`: a float bound may be NaN, which matches no range under IEEE rules.
/// Negation is absent: it is principal-dependent by construction.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOperand {
    /// A category code. One value.
    Equals(AttrLocalId),
    /// A set of category codes, evaluated in one pass regardless of visibility.
    In(Vec<AttrLocalId>),
    /// Exact string match against the stored bytes.
    TextEquals(String),
    /// Stored value equals any of these.
    TextIn(Vec<String>),
    /// Stored value starts with this.
    TextPrefix(String),
    /// Stored value contains this. Needs no trigram index: the value column is its own route.
    TextContains(String),
    /// `match`, and its m-of-n form in one operand: `minimum` is how many of the analysed `tokens`
    /// must appear, `None` meaning all. The query is carried unanalysed so the column's own
    /// analyser segments it. Duplicate tokens collapse before counting.
    Match { query: String, minimum: Option<u32> },
    /// Exact phrase: the postings narrow to items carrying every word, then each survivor's own
    /// prose is re-analysed and checked for the sequence, exact, at no extra storage.
    Phrase { query: String },
    /// Numeric equality.
    NumEquals(Scalar),
    /// Numeric set membership.
    NumIn(Vec<Scalar>),
    /// A numeric range. Either endpoint may be absent; both absent still requires a value present.
    Range {
        lo: Option<Endpoint>,
        hi: Option<Endpoint>,
    },
}

/// The code a predicate names when its value does not resolve.
///
/// An unresolvable value is an empty operand, never a refusal: refusing would say the value exists,
/// which is exactly what `visibility = "derived"` hides. Code 0 is the vocabulary's reserved absent
/// sentinel, so it is also the code an item carrying no value carries.
pub const UNRESOLVABLE_VALUE: AttrLocalId = AttrLocalId::new(0);

/// The leaf name a region leaf answers to, the one word a column may not be called.
pub const REGION_COLUMN: &str = "region";

/// The `region` leaf's two spellings: a row-space operand over the whole view.
///
/// A shape arrives already canonical, so the same shape from two callers is one row set. A
/// published shape this principal would not be served, or an id naming nothing, is an empty
/// operand, indistinguishable from the other.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionLeaf {
    /// A box, circle, ellipse or polygon in its canonical grid-unit form.
    Shape(Arc<mosaica_spatial::shape::Shape>),
    /// A published artifact's membership.
    Artifact(mosaica_types::MosaicaId),
}

/// The leaf name a `member_of` leaf answers to, reserved exactly as `region` is.
pub const MEMBER_OF_COLUMN: &str = "member_of";

/// The `member_of` leaf: one artifact of one layer, resolved to `membership ∩ M_auth` in row space
/// over the whole view.
///
/// The layer is deployment schema and the artifact is a value: a layer this principal does not
/// reach is [`FilterError::UnknownLayer`], a refusal, while an artifact naming nothing this
/// principal may see is the empty operand, whether it is named by identifier or by key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberOfLeaf {
    pub layer: String,
    pub artifact: MemberOfArtifact,
}

/// How a `member_of` leaf names its artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberOfArtifact {
    /// By the `mosaica_id` a response handed out.
    Id(mosaica_types::MosaicaId),
    /// By the key it was published under at `level`, in the request's own view on a layer scoped
    /// to a group. A key the level does not hold is the empty operand, as an identifier naming
    /// nothing is.
    Key { key: String, level: u32 },
}

/// A filter expression: a leaf predicate over one column, or a combinator over sub-expressions.
///
/// Every node returns a subset of the candidate, so a filter narrows the viewer's selection and
/// never widens it.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpr {
    /// One column's predicate.
    Leaf {
        column: String,
        operand: FilterOperand,
    },
    /// A region, a drawn shape or a published one, as rows over the whole view. Row space only,
    /// resolved through the caller's resolver.
    Region(RegionLeaf),
    /// One artifact's membership, as rows over the whole view. Row space only.
    MemberOf(MemberOfLeaf),
    /// Every sub-expression must match. Empty matches the whole candidate.
    AllOf(Vec<FilterExpr>),
    /// At least one sub-expression must match. Empty matches nothing, the identity for union.
    AnyOf(Vec<FilterExpr>),
    /// Every entity of the candidate that no sub-expression matches, an entity with no value in a
    /// column included. Empty matches the whole candidate.
    NoneOf(Vec<FilterExpr>),
}

/// How deep a filter expression may nest. Refused rather than truncated: a silently flattened
/// expression answers a question the caller did not ask.
pub const MAX_FILTER_DEPTH: usize = 4;

impl FilterExpr {
    /// Nesting depth, with a leaf at 1.
    pub fn depth(&self) -> usize {
        match self {
            FilterExpr::Leaf { .. } | FilterExpr::Region(_) | FilterExpr::MemberOf(_) => 1,
            FilterExpr::AllOf(kids) | FilterExpr::AnyOf(kids) | FilterExpr::NoneOf(kids) => {
                1 + kids.iter().map(FilterExpr::depth).max().unwrap_or(0)
            }
        }
    }

    /// Checks depth against [`MAX_FILTER_DEPTH`].
    pub(in crate::filter) fn check(&self) -> Result<(), FilterError> {
        let depth = self.depth();
        if depth > MAX_FILTER_DEPTH {
            return Err(FilterError::TooDeep {
                depth,
                max: MAX_FILTER_DEPTH,
            });
        }
        Ok(())
    }
}

/// A filter expression routed for one request; see
/// [`crate::filter::FilterColumns::evaluate_routed`].
#[derive(Debug)]
pub enum RoutedFilter {
    /// Every leaf routed entity space, awaiting the existing crossing.
    Entity(Bitmap),
    /// At least one leaf routes row space, evaluated over the request's own rows and combined in
    /// row space (`viewport.rs`).
    Row(RowExpr),
}

/// A filter tree ready for row-space evaluation: entity-space sub-trees collapsed to their
/// verdicts, row-space leaves awaiting the hot column. The [`RowExpr::Entity`] verdicts are crossed
/// into row space once per request, jointly.
#[derive(Debug)]
pub enum RowExpr {
    /// An entity-space sub-tree's verdict, evaluated under the composed candidate.
    Entity(Bitmap),
    /// A render-column leaf, tested against the hot column. The family is carried, not inferred: a
    /// scoped column is never row-routed since a pin may name another view's column, and a guessed
    /// family would read one family's absence as the other's value.
    Leaf {
        column: String,
        family: Family,
        operand: FilterOperand,
    },
    /// Intersection in row space.
    AllOf(Vec<RowExpr>),
    /// Union in row space. Empty matches nothing, exactly as [`FilterExpr::AnyOf`] does.
    AnyOf(Vec<RowExpr>),
    /// A region leaf, already resolved: its rows over the whole view.
    Region(crate::region::RegionRows),
    /// A `member_of` leaf, already resolved: `membership ∩ M_auth`, which has already met the mask,
    /// unlike a region, so its cardinality is already readable off the artifacts frame.
    MemberOf(Bitmap),
    /// `none_of`: every row in scope, the whole view or the request's domain, that no kid matches.
    /// Rows outside the mask are among them, so a caller narrows the answer through the mask.
    NotInRows(Vec<RowExpr>),
}

impl RowExpr {
    /// Every entity-space verdict in this tree, in a fixed pre-order the joint crossing also walks.
    pub fn entity_verdicts(&self) -> Vec<&Bitmap> {
        let mut out = Vec::new();
        self.collect_verdicts(&mut out);
        out
    }

    fn collect_verdicts<'a>(&'a self, out: &mut Vec<&'a Bitmap>) {
        match self {
            RowExpr::Entity(bitmap) => out.push(bitmap),
            RowExpr::Leaf { .. } | RowExpr::Region(_) | RowExpr::MemberOf(_) => {}
            RowExpr::AllOf(kids) | RowExpr::AnyOf(kids) | RowExpr::NotInRows(kids) => {
                for kid in kids {
                    kid.collect_verdicts(out);
                }
            }
        }
    }

    /// Whether every row-space node answers over the whole view; decides `FilterRows::Complete`.
    pub fn is_whole_view(&self) -> bool {
        match self {
            RowExpr::Entity(_) | RowExpr::Region(_) | RowExpr::MemberOf(_) => true,
            RowExpr::Leaf { .. } => false,
            RowExpr::AllOf(kids) | RowExpr::AnyOf(kids) | RowExpr::NotInRows(kids) => {
                kids.iter().all(RowExpr::is_whole_view)
            }
        }
    }

    /// Whether a region leaf or a negation is anywhere in the tree; their rows have not yet met
    /// the mask.
    pub fn has_region(&self) -> bool {
        match self {
            // A `member_of` leaf is not one: its set has already met the mask.
            RowExpr::Region(_) | RowExpr::NotInRows(_) => true,
            RowExpr::Entity(_) | RowExpr::Leaf { .. } | RowExpr::MemberOf(_) => false,
            RowExpr::AllOf(kids) | RowExpr::AnyOf(kids) => kids.iter().any(RowExpr::has_region),
        }
    }

    /// The coarsest verdict any region leaf reached, or `None`: the `x-mosaica-region` header's
    /// value.
    pub fn region_verdict(&self) -> Option<crate::region::RegionVerdict> {
        match self {
            RowExpr::Region(rows) => Some(rows.verdict),
            RowExpr::Entity(_) | RowExpr::Leaf { .. } | RowExpr::MemberOf(_) => None,
            RowExpr::AllOf(kids) | RowExpr::AnyOf(kids) | RowExpr::NotInRows(kids) => kids
                .iter()
                .filter_map(RowExpr::region_verdict)
                .reduce(|a, b| a.coarser(b)),
        }
    }
}

/// How a routed evaluation answers a region leaf: the caller's, since the answer needs the
/// request's mask, the view's segments and the engine's cache.
pub type RegionResolver<'a> =
    dyn Fn(&RegionLeaf) -> Result<crate::region::RegionRows, FilterError> + 'a;

/// How a routed evaluation answers a `member_of` leaf. The answer is `membership ∩ M_auth`; an
/// artifact this principal would not be served is the empty bitmap, never an error.
pub type MemberResolver<'a> = dyn Fn(&MemberOfLeaf) -> Result<Bitmap, FilterError> + 'a;

/// The entities holding each of a unique column's keys, from the column's index and the live
/// entries beside it, deleted entities dropped. Visibility is the evaluator's: it intersects the
/// answer with its candidate.
pub type UniqueResolver<'a> =
    dyn Fn(&str, &[mosaica_store::unique::UniqueKey]) -> Result<Vec<u32>, FilterError> + 'a;

/// The two row-space leaves' resolvers, how many levels a layer a `member_of` names holds where
/// `/v1/meta` lists it to this principal (`None` where not, refused before either resolver runs),
/// and the unique columns' lookup.
pub struct RowLeafResolvers<'a> {
    pub regions: &'a RegionResolver<'a>,
    pub members: &'a MemberResolver<'a>,
    pub layers: &'a dyn Fn(&str) -> Option<u32>,
    pub unique: &'a UniqueResolver<'a>,
}
