//! A text or value filter leaf's answer, kept across requests: a viewer panning and zooming with
//! the same filter sends it again with every viewport, and over a large corpus a common phrase or
//! a broad range costs the same each time. A leaf on the row route keeps the rows it matched in
//! the rows scanned for it so far, so a pan scans only the rows it brings into view.
//!
//! # What an entry holds, and why a request may read it
//!
//! A leaf evaluated under a candidate `C` answers `C ∩ M`, where `M` is the set of entities whose
//! value in one of the column's layers satisfies the leaf: a text leaf narrows every posting it
//! reads to `C` before anything is counted, and a phrase then tests each survivor's positions
//! alone; a value leaf scans `C`, or reads a category's postings and intersects them with `C`. An
//! entry holds `F ∩ M`, with `F` the session's fragment, the items the grant covers before the
//! overlay's deletions and suppressions are taken out and the buffered entities whose verdict
//! passes are put in. A request's candidate is a subset of `F` plus buffered entities, and a
//! buffered entity has no value in any layer, since the layers hold flushed entities and the fold
//! that frees an id removes its values. So `C ∩ M` is `C ∩ (F ∩ M)`, and a request reads an entry
//! by intersecting it with its own candidate.
//!
//! That makes an entry independent of the overlay and the buffer, so it outlives a window close, a
//! deletion, a suppression and a lift. [`LeafKey`] names what it does depend on: the column's
//! layers and the fragment.
//!
//! An entry holds entities a deny hides from the grant. Nothing read from them reaches a response
//! except through that intersection: the evaluation over `F` counts nothing a request sees, and one
//! that fails is evaluated again under the request's own candidate, whose answer or refusal is the
//! one the request gets.
//!
//! # Row leaves
//!
//! A row leaf's entry holds, for one view in one shard, the rows scanned for it and those of them
//! whose value satisfies it, before any mask: a scan reads every row of its domain whatever the
//! viewer may see. A request reads the entry over its own domain and narrows the answer through
//! its composed mask, as it narrows a scan's, so nothing in the entry reaches a response the mask
//! has not admitted. The rows are those of the shard's segments, which a flush, a merge and a
//! compaction renumber, so the key names the shard's `segments_version` beside the rest, and the
//! grant, as every key here does.
//!
//! # Projections
//!
//! An expression answered over items, leaves combined by conjunction and disjunction, also keeps
//! the rows in one view of the items it matches among those the grant admits, before deletions and
//! suppressions are taken out. Its answer over a request's candidate is that set intersected with
//! the candidate, so a request reads the rows over its own domain and narrows them through its
//! composed mask, which removes exactly the rows of items outside the candidate. The entry is a row
//! entry covering every row of the view, under a key of its own.
//!
//! # Bound and peak
//!
//! `filter_cache_bytes` bounds the bytes resident here. A request holds an entry only while it
//! intersects it with its candidate, and an entry evicted meanwhile is freed when it lets go, so
//!
//! ```text
//! peak ≈ filter_cache_bytes + compute_admission × the largest answer
//! ```
//!
//! An answer is a bitmap over the shard's entities: about 12 MB at 10⁸ entities for a phrase or a
//! range that half of them match, scattered, and kilobytes for a topical one. The leaf's canonical
//! bytes are held and counted beside it.

use std::ops::Range;
use std::sync::{Arc, Weak};

use croaring::{Bitmap, Portable};
use mosaica_cache::{
    CacheWeight, Cancel, NeverCancelled, Peek, SingleFlightCache, WaitEnded, WaitingBuildError,
};
use mosaica_roaring::Runs;
use mosaica_types::ShardId;

use crate::cancel::CancelToken;
use crate::filter::{
    FilterColumns, FilterError, FilterExpr, FilterOperand, LeafQuery, Scalar, TextQuery,
};
use crate::{Generation, Shard};

/// `serve.filter_cache_bytes` where it is not set.
pub const DEFAULT_FILTER_CACHE_BYTES: u64 = 1 << 30;

/// What one leaf's entry in one shard is a function of.
///
/// - `columns`: the address of the shard's filter columns, which hold its layers. They are
///   immutable: a flush, a coalesce, a fold and a schema change each publish new ones, and a window
///   close, a deny and a lift keep them. The entry keeps a [`Weak`] to them, so while the entry
///   exists nothing else can be allocated at that address.
/// - `identity`, `watermark` and `grant`: the fragment, which [`crate::Engine::fragment_for`]
///   takes at the shard's identity and watermark for the session's grant
///   ([`crate::Session::terms_digest`]). A window close, a deny and a lift move neither. A
///   session's satisfied keys are the keys of its grant, or every key for a `read-all` grant, so
///   the grant names them too.
/// - `digest`: of the leaf's [`canonical`] bytes, its column and its operand as the column's
///   analyser reads it. The bytes themselves are held in the entry, where the bound counts them,
///   and compared on every hit.
///
/// - `segments`: for a row leaf, the shard's `segments_version`, which moves whenever its rows do;
///   `None` for an entity leaf. A row leaf's digest also covers the view it was scanned in.
///
/// `shard` is carried beside the rest so a prune can find the shard the entry is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct LeafKey {
    pub shard: ShardId,
    pub columns: usize,
    pub identity: [u8; 32],
    pub watermark: u64,
    pub grant: [u8; 32],
    pub digest: [u8; 16],
    pub segments: Option<u64>,
}

/// One leaf's matches: an entity leaf's within the fragment its key names, a row leaf's within the
/// rows scanned for it.
pub(crate) struct KeptLeaf {
    canonical: Vec<u8>,
    columns: Weak<FilterColumns>,
    held: Held,
}

enum Held {
    Entities(Bitmap),
    Rows { covered: Bitmap, matched: Bitmap },
}

impl KeptLeaf {
    /// An entity leaf's matches. An entity key never names a row entry, since only a row key
    /// names a `segments_version`.
    fn entities(&self) -> &Bitmap {
        match &self.held {
            Held::Entities(entities) => entities,
            Held::Rows { .. } => unreachable!("an entity leaf's key names an entity entry"),
        }
    }

    /// A row leaf's rows scanned and rows matched.
    fn rows(&self) -> (&Bitmap, &Bitmap) {
        match &self.held {
            Held::Rows { covered, matched } => (covered, matched),
            Held::Entities(_) => unreachable!("a row leaf's key names a row entry"),
        }
    }
}

impl CacheWeight for KeptLeaf {
    fn cache_weight_bytes(&self) -> u64 {
        let size = |bitmap: &Bitmap| bitmap.get_serialized_size_in_bytes::<Portable>() as u64;
        let held = match &self.held {
            Held::Entities(entities) => size(entities),
            Held::Rows { covered, matched } => size(covered) + size(matched),
        };
        held + self.canonical.len() as u64
    }
}

/// `column` and `query` as bytes no other leaf has: a tag for the leaf's kind first, then each
/// part, a string or list after its length and a value operand after a tag for its form.
fn canonical(column: &str, query: LeafQuery<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 + column.len());
    let string = |out: &mut Vec<u8>, s: &str| {
        out.extend_from_slice(&(s.len() as u32).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
    };
    let scalar = |out: &mut Vec<u8>, v: &Scalar| match v {
        Scalar::Int(i) => {
            out.push(0);
            out.extend_from_slice(&i.to_le_bytes());
        }
        Scalar::Float(f) => {
            out.push(1);
            out.extend_from_slice(&f.to_bits().to_le_bytes());
        }
    };
    match query {
        LeafQuery::Text(TextQuery::Match { tokens, minimum }) => {
            out.push(0);
            out.extend_from_slice(&minimum.to_le_bytes());
            string(&mut out, column);
            tokens.iter().for_each(|t| string(&mut out, t));
        }
        LeafQuery::Text(TextQuery::Phrase { words }) => {
            out.push(1);
            out.extend_from_slice(&0u32.to_le_bytes());
            string(&mut out, column);
            words.iter().for_each(|t| string(&mut out, t));
        }
        LeafQuery::Value(operand) => {
            out.push(2);
            string(&mut out, column);
            match operand {
                FilterOperand::Equals(code) => {
                    out.push(2);
                    out.extend_from_slice(&code.raw().to_le_bytes());
                }
                FilterOperand::In(codes) => {
                    out.push(3);
                    out.extend_from_slice(&(codes.len() as u32).to_le_bytes());
                    codes
                        .iter()
                        .for_each(|c| out.extend_from_slice(&c.raw().to_le_bytes()));
                }
                FilterOperand::TextEquals(v) => {
                    out.push(4);
                    string(&mut out, v);
                }
                FilterOperand::TextIn(vs) => {
                    out.push(5);
                    out.extend_from_slice(&(vs.len() as u32).to_le_bytes());
                    vs.iter().for_each(|v| string(&mut out, v));
                }
                FilterOperand::TextPrefix(v) => {
                    out.push(6);
                    string(&mut out, v);
                }
                FilterOperand::TextContains(v) => {
                    out.push(7);
                    string(&mut out, v);
                }
                FilterOperand::Match { query, minimum } => {
                    out.push(8);
                    string(&mut out, query);
                    out.extend_from_slice(&minimum.map_or(u64::MAX, u64::from).to_le_bytes());
                }
                FilterOperand::Phrase { query } => {
                    out.push(9);
                    string(&mut out, query);
                }
                FilterOperand::NumEquals(v) => {
                    out.push(10);
                    scalar(&mut out, v);
                }
                FilterOperand::NumIn(vs) => {
                    out.push(11);
                    out.extend_from_slice(&(vs.len() as u32).to_le_bytes());
                    vs.iter().for_each(|v| scalar(&mut out, v));
                }
                FilterOperand::Range { lo, hi } => {
                    out.push(12);
                    for end in [lo, hi] {
                        match end {
                            None => out.push(0),
                            Some(e) => {
                                out.push(if e.inclusive { 2 } else { 1 });
                                scalar(&mut out, &e.value);
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

pub(crate) type FilterCache = SingleFlightCache<LeafKey, KeptLeaf>;

/// Drop every entry over filter columns or a fragment `live` no longer holds: no request can name
/// it again.
pub(crate) fn prune_superseded(cache: &FilterCache, live: &Generation) -> usize {
    cache.retain_keys(|key| {
        live.shard(key.shard).is_some_and(|shard| {
            Arc::as_ptr(&shard.filter_columns) as usize == key.columns
                && shard.identity == key.identity
                && shard.watermark == key.watermark
                && key.segments.is_none_or(|v| v == shard.segments_version)
        })
    })
}

/// The key of `column`'s leaf `query` in `shard` for `grant`, and the leaf's canonical bytes.
pub(crate) fn key_of(
    shard: &Shard,
    grant: [u8; 32],
    column: &str,
    query: LeafQuery<'_>,
) -> (LeafKey, Vec<u8>) {
    key_over(shard, grant, canonical(column, query), None)
}

/// The key of row leaf `column` `operand` in `view`, at `incarnation`, in `shard` for `grant`, and
/// the leaf's canonical bytes.
pub(crate) fn row_key_of(
    shard: &Shard,
    grant: [u8; 32],
    view: &str,
    incarnation: u64,
    column: &str,
    operand: &FilterOperand,
) -> (LeafKey, Vec<u8>) {
    let mut canonical = canonical(column, LeafQuery::Value(operand));
    canonical.push(3);
    canonical.extend_from_slice(&incarnation.to_le_bytes());
    canonical.extend_from_slice(view.as_bytes());
    key_over(shard, grant, canonical, Some(shard.segments_version))
}

fn key_over(
    shard: &Shard,
    grant: [u8; 32],
    canonical: Vec<u8>,
    segments: Option<u64>,
) -> (LeafKey, Vec<u8>) {
    let key = LeafKey {
        shard: shard.id(),
        columns: Arc::as_ptr(&shard.filter_columns) as usize,
        identity: shard.identity,
        watermark: shard.watermark,
        grant,
        digest: crate::region::digest_of(&canonical),
        segments,
    };
    (key, canonical)
}

/// The key of the rows, in `view` at `incarnation`, of the items `expr` matches in `shard` for
/// `grant`, and its canonical bytes; `None` for an expression the shard's filter columns do not
/// answer over items ([`FilterColumns::answered_over_items`]).
pub(crate) fn projection_key_of(
    shard: &Shard,
    grant: [u8; 32],
    view: &str,
    incarnation: u64,
    expr: &FilterExpr,
) -> Option<(LeafKey, Vec<u8>)> {
    fn write(expr: &FilterExpr, out: &mut Vec<u8>) -> Option<()> {
        let kids = match expr {
            FilterExpr::Leaf { column, operand } => {
                let leaf = canonical(column, LeafQuery::Value(operand));
                out.extend_from_slice(&(leaf.len() as u32).to_le_bytes());
                out.extend_from_slice(&leaf);
                return Some(());
            }
            FilterExpr::AllOf(kids) => {
                out.push(0);
                kids
            }
            FilterExpr::AnyOf(kids) => {
                out.push(1);
                kids
            }
            _ => return None,
        };
        out.extend_from_slice(&(kids.len() as u32).to_le_bytes());
        kids.iter().try_for_each(|kid| write(kid, out))
    }
    if !shard.filter_columns.answered_over_items(expr) {
        return None;
    }
    let mut canonical = vec![4];
    write(expr, &mut canonical)?;
    canonical.extend_from_slice(&incarnation.to_le_bytes());
    canonical.extend_from_slice(view.as_bytes());
    Some(key_over(shard, grant, canonical, Some(shard.segments_version)))
}

/// The rows of `wanted` the kept rows for `key` hold, or `None` where none are kept.
pub(crate) fn projected_rows(
    cache: &FilterCache,
    (key, canonical): &(LeafKey, Vec<u8>),
    wanted: &Bitmap,
) -> Option<Bitmap> {
    if cache.bound_bytes() == 0 {
        return None;
    }
    let Peek::Ready(entry) = cache.peek(key) else {
        return None;
    };
    (entry.canonical == *canonical).then(|| entry.rows().1.and(wanted))
}

/// Keep `rows` for `key`, the rows an expression's items hold in a view of `total_rows` rows.
pub(crate) fn keep_projection(
    cache: &FilterCache,
    shard: &Shard,
    (key, canonical): (LeafKey, Vec<u8>),
    rows: Bitmap,
    total_rows: u32,
) {
    if cache.bound_bytes() > 0 {
        keep(cache, shard, key, canonical, Bitmap::from_range(0..total_rows), rows);
    }
}

/// How many of `domain`'s rows, ascending disjoint ranges, `cache`'s entry for a row leaf holds.
pub(crate) fn kept_rows(
    cache: &FilterCache,
    (key, canonical): (LeafKey, Vec<u8>),
    domain: &[Range<u32>],
) -> u64 {
    let Peek::Ready(entry) = cache.peek(&key) else {
        return 0;
    };
    if entry.canonical != canonical {
        return 0;
    }
    let covered = entry.rows().0;
    domain
        .iter()
        .map(|range| covered.range_cardinality(range.clone()))
        .sum()
}

/// An entry is not grown where it already holds this many times the rows a request scans.
const KEEP_GROWTH_RATIO: u64 = 32;

/// The rows of `wanted`, the request's domain `domain` as a bitmap, a row leaf matches: from
/// `cache`'s entry for it over the rows the entry has scanned, and from `scan` over the rest, which
/// the entry then holds as well, unless it would outgrow the bound or already holds
/// [`KEEP_GROWTH_RATIO`] times them. A digest collision is scanned whole.
pub(crate) fn rows_answer<E>(
    cache: &FilterCache,
    shard: &Shard,
    (key, canonical): (LeafKey, Vec<u8>),
    domain: &[Range<u32>],
    wanted: &Bitmap,
    scan: impl FnOnce(&[Range<u32>]) -> Result<Bitmap, E>,
) -> Result<Bitmap, E> {
    if cache.bound_bytes() == 0 {
        return scan(domain);
    }
    let entry = match cache.peek(&key) {
        Peek::Ready(entry) if entry.canonical == canonical => entry,
        Peek::Ready(_) => return scan(domain),
        Peek::Building | Peek::Absent => {
            let fresh = scan(domain)?;
            keep(cache, shard, key, canonical, wanted.clone(), fresh.clone());
            return Ok(fresh);
        }
    };
    let (covered, matched) = entry.rows();
    let mut answer = matched.and(wanted);
    let missing = wanted.andnot(covered);
    if missing.is_empty() {
        return Ok(answer);
    }
    let ranges: Vec<Range<u32>> = Runs::new(&missing)
        .map(|(start, last)| start..last + 1)
        .collect();
    let fresh = scan(&ranges)?;
    answer.or_inplace(&fresh);
    if missing.cardinality().saturating_mul(KEEP_GROWTH_RATIO) >= covered.cardinality() {
        let grown = (covered.or(&missing), matched.or(&fresh));
        keep(cache, shard, key, canonical, grown.0, grown.1);
    }
    Ok(answer)
}

/// Make `covered` and `matched` `key`'s entry, where they fit the bound.
fn keep(
    cache: &FilterCache,
    shard: &Shard,
    key: LeafKey,
    canonical: Vec<u8>,
    mut covered: Bitmap,
    mut matched: Bitmap,
) {
    for bitmap in [&mut covered, &mut matched] {
        bitmap.run_optimize();
        bitmap.shrink_to_fit();
    }
    let kept = KeptLeaf {
        canonical,
        columns: Arc::downgrade(&shard.filter_columns),
        held: Held::Rows { covered, matched },
    };
    if kept.cache_weight_bytes() > cache.bound_bytes() {
        return;
    }
    cache.evict(&key);
    let _ = cache.get_or_try_build(key, || Ok::<_, std::convert::Infallible>(kept));
}

/// `evaluate(candidate)`, from `cache`'s entry for `leaf` in `shard` intersected with `candidate`.
///
/// `fragment` is the grant's fragment in `shard`, given where `candidate` is the request's whole
/// candidate. Only then is a missing entry computed, as `evaluate(fragment)`, and kept; concurrent
/// requests for it compute it once, and a request waiting on another's computation stops when
/// `cancel` reports it gone. A smaller candidate reads an entry already kept and is evaluated
/// otherwise. An evaluation over the fragment that fails, and an entry whose canonical bytes are
/// not the leaf's, which is a digest collision, are answered by evaluating under `candidate`.
pub(crate) fn answer(
    cache: &FilterCache,
    shard: &Shard,
    (key, canonical): (LeafKey, Vec<u8>),
    fragment: Option<&Bitmap>,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
    evaluate: &dyn Fn(&Bitmap) -> Result<Bitmap, FilterError>,
) -> Result<Bitmap, FilterError> {
    if cache.bound_bytes() == 0 {
        return evaluate(candidate);
    }
    let entry = if let Some(fragment) = fragment {
        let waiting: &dyn Cancel = match cancel {
            Some(token) => token,
            None => &NeverCancelled,
        };
        let built = cache.get_or_try_build_waiting(key, waiting, || {
            let mut entities = evaluate(fragment)?;
            entities.run_optimize();
            entities.shrink_to_fit();
            Ok(KeptLeaf {
                canonical: canonical.clone(),
                columns: Arc::downgrade(&shard.filter_columns),
                held: Held::Entities(entities),
            })
        });
        match built {
            Ok(entry) => entry,
            Err(WaitingBuildError::Build(FilterError::Cancelled)) => {
                return Err(FilterError::Cancelled)
            }
            Err(WaitingBuildError::Build(_)) => return evaluate(candidate),
            Err(WaitingBuildError::Wait(WaitEnded::Cancelled)) => {
                return Err(FilterError::Cancelled)
            }
            Err(WaitingBuildError::Wait(WaitEnded::Budget)) => return evaluate(candidate),
        }
    } else {
        match cache.peek(&key) {
            Peek::Ready(entry) => entry,
            Peek::Building | Peek::Absent => return evaluate(candidate),
        }
    };
    if entry.canonical != canonical {
        return evaluate(candidate);
    }
    debug_assert!(std::ptr::eq(
        entry.columns.as_ptr(),
        Arc::as_ptr(&shard.filter_columns)
    ));
    Ok(entry.entities().and(candidate))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use mosaica_lifecycle::{IngestBuffer, Overlay};

    use super::*;

    /// **An entry is computed over the fragment and read under each candidate as its intersection
    /// with it**, an evaluation that is cancelled leaves nothing kept, one that fails over the
    /// fragment is answered under the candidate, and a digest collision is evaluated.
    #[test]
    fn an_entry_is_read_under_each_candidate_and_only_a_good_evaluation_is_kept() {
        let generation = Generation::synthetic("v00000", 1, 0, Overlay::new(), IngestBuffer::new());
        let shard = generation.only_shard();
        let cache = FilterCache::new(1 << 20);
        let query = TextQuery::Phrase {
            words: vec!["of".to_string(), "the".to_string()],
        };
        let key = || key_of(shard, [7; 32], "prose", LeafQuery::Text(&query));
        // 10..20 are the grant's but denied, so in the fragment and not the candidate.
        let fragment = Bitmap::from_range(0..20);
        let whole = Bitmap::from_range(0..10);
        let ask = |leaf, candidate: &Bitmap, evaluate: &dyn Fn(&Bitmap) -> _| {
            answer(
                &cache,
                shard,
                leaf,
                Some(&fragment),
                candidate,
                &None,
                evaluate,
            )
        };

        let cancelled = ask(key(), &whole, &|_| Err(FilterError::Cancelled));
        assert_eq!(cancelled, Err(FilterError::Cancelled));
        assert_eq!(cache.len(), 0, "a cancelled evaluation is not kept");

        let unreadable = |candidate: &Bitmap| match candidate.contains(15) {
            true => Err(FilterError::UniqueUnreadable {
                column: "prose".to_string(),
                detail: "a hidden entity's entry".to_string(),
            }),
            false => Ok(candidate.and(&Bitmap::of(&[2]))),
        };
        assert_eq!(ask(key(), &whole, &unreadable), Ok(Bitmap::of(&[2])));
        assert_eq!(cache.len(), 0, "a failed evaluation is not kept");

        let evaluations = Cell::new(0);
        let evaluate = |candidate: &Bitmap| {
            evaluations.set(evaluations.get() + 1);
            Ok(candidate.and(&Bitmap::of(&[2, 3, 12])))
        };
        for _ in 0..2 {
            assert_eq!(ask(key(), &whole, &evaluate), Ok(Bitmap::of(&[2, 3])));
        }
        assert_eq!(evaluations.get(), 1, "the repeat is the kept entry");

        let lifted = Bitmap::from_range(0..13);
        assert_eq!(ask(key(), &lifted, &evaluate), Ok(Bitmap::of(&[2, 3, 12])));
        let smaller = Bitmap::of(&[3, 4]);
        let got = answer(&cache, shard, key(), None, &smaller, &None, &evaluate);
        assert_eq!(got, Ok(Bitmap::of(&[3])));
        assert_eq!(evaluations.get(), 1, "every candidate reads the kept entry");

        // Another leaf's bytes under this leaf's digest stand for a collision.
        let other = TextQuery::Match {
            tokens: vec!["of".to_string()],
            minimum: 1,
        };
        let colliding = (
            key().0,
            key_of(shard, [7; 32], "prose", LeafQuery::Text(&other)).1,
        );
        let got = ask(colliding, &whole, &|_| Ok(Bitmap::of(&[9])));
        assert_eq!(got, Ok(Bitmap::of(&[9])), "a collision is evaluated");
    }

    /// **A row leaf scans only the rows its entry has not, and reads the rest from it**: a pan
    /// scans what it adds, a window inside what has been scanned scans nothing, rows too few to be
    /// worth copying the entry for are scanned and not kept, another view's leaf is its own entry,
    /// and a digest collision is scanned whole. The rows an entry holds are counted over a domain.
    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn a_row_leaf_scans_only_the_rows_its_entry_has_not() {
        let generation = Generation::synthetic("v00000", 1, 0, Overlay::new(), IngestBuffer::new());
        let shard = generation.only_shard();
        let cache = FilterCache::new(1 << 20);
        let eq = FilterOperand::Equals(mosaica_types::AttrLocalId::new(1));
        let key = |view: &str| row_key_of(shard, [7; 32], view, 0, "n", &eq);
        let matches = Bitmap::of(&[2, 15, 30, 45]);
        let scanned = std::cell::RefCell::new(Vec::new());
        let ask = |leaf, domain: &[Range<u32>]| {
            let wanted = crate::compose::rows_of(domain);
            rows_answer(&cache, shard, leaf, domain, &wanted, |ranges: &[Range<u32>]| {
                scanned.borrow_mut().extend_from_slice(ranges);
                let mut out = Bitmap::new();
                for range in ranges {
                    out |= matches.and(&Bitmap::from_range(range.clone()));
                }
                Ok::<_, ()>(out)
            })
        };
        let scans = |leaf, domain: &[Range<u32>]| {
            scanned.borrow_mut().clear();
            let got = ask(leaf, domain).unwrap();
            (got, scanned.borrow().clone())
        };

        assert_eq!(scans(key("s0"), &[0..20]), (Bitmap::of(&[2, 15]), vec![0..20]));
        assert_eq!(scans(key("s0"), &[10..40]), (Bitmap::of(&[15, 30]), vec![20..40]));
        assert_eq!(scans(key("s0"), &[0..5, 25..35]), (Bitmap::of(&[2, 30]), vec![]));
        assert_eq!(cache.len(), 1);
        assert_eq!(kept_rows(&cache, key("s0"), &[0..5, 35..50]), 10);
        assert_eq!(kept_rows(&cache, key("s1"), &[0..5]), 0);
        for _ in 0..2 {
            let got = scans(key("s0"), &[40..41]);
            assert_eq!(got, (Bitmap::new(), vec![40..41]), "too little to keep");
        }
        assert_eq!(scans(key("s1"), &[0..5]), (Bitmap::of(&[2]), vec![0..5]));

        let other = FilterOperand::Equals(mosaica_types::AttrLocalId::new(2));
        let colliding = (key("s0").0, row_key_of(shard, [7; 32], "s0", 0, "n", &other).1);
        assert_eq!(scans(colliding, &[0..5]), (Bitmap::of(&[2]), vec![0..5]));
    }
}
