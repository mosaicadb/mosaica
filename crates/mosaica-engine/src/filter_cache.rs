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
//! compaction renumber, so the key names the shard's `segments_version` beside the rest. It names
//! the grant too, as every entry here does, though the rows do not depend on it, so whether an
//! entry is held says nothing about sessions under other grants.
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
use mosaica_roaring::for_each_run_in;
use mosaica_types::ShardId;

use crate::cancel::CancelToken;
use crate::filter::{FilterColumns, FilterError, FilterOperand, LeafQuery, Scalar, TextQuery};
use crate::{Generation, Shard};

/// `serve.filter_cache_bytes` where it is not set.
pub const DEFAULT_FILTER_CACHE_BYTES: u64 = 256 << 20;

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
    let canonical = canonical(column, query);
    let key = LeafKey {
        shard: shard.id(),
        columns: Arc::as_ptr(&shard.filter_columns) as usize,
        identity: shard.identity,
        watermark: shard.watermark,
        grant,
        digest: crate::region::digest_of(&canonical),
        segments: None,
    };
    (key, canonical)
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
    let leaf = canonical(column, LeafQuery::Value(operand));
    let mut canonical = Vec::with_capacity(13 + view.len() + leaf.len());
    canonical.push(3);
    canonical.extend_from_slice(&(view.len() as u32).to_le_bytes());
    canonical.extend_from_slice(view.as_bytes());
    canonical.extend_from_slice(&incarnation.to_le_bytes());
    canonical.extend_from_slice(&leaf);
    let key = LeafKey {
        shard: shard.id(),
        columns: Arc::as_ptr(&shard.filter_columns) as usize,
        identity: shard.identity,
        watermark: shard.watermark,
        grant,
        digest: crate::region::digest_of(&canonical),
        segments: Some(shard.segments_version),
    };
    (key, canonical)
}

/// How many times the rows a request scans its entry must hold before the entry is left as it was:
/// keeping them copies the entry, at about a fiftieth of a scan's cost a row. A records walk reads
/// a view a few thousand rows a page, so an entry it grows stops growing rather than being copied
/// once a page.
const KEEP_GROWTH_RATIO: u64 = 32;

/// The rows over `domain`, ascending disjoint ranges, a row leaf matches: from `cache`'s entry for
/// it over the rows the entry has scanned, and from `scan` over the rest, which the entry then
/// holds as well. An entry is left as it was where it would outgrow the bound, or where it holds
/// [`KEEP_GROWTH_RATIO`] times the rows scanned. A digest collision is scanned whole.
pub(crate) fn rows_answer<E>(
    cache: &FilterCache,
    shard: &Shard,
    (key, canonical): (LeafKey, Vec<u8>),
    domain: &[Range<u32>],
    scan: impl FnOnce(&[Range<u32>]) -> Result<Bitmap, E>,
) -> Result<Bitmap, E> {
    if cache.bound_bytes() == 0 {
        return scan(domain);
    }
    let entry = match cache.peek(&key) {
        Peek::Ready(entry) if entry.canonical == canonical => Some(entry),
        Peek::Ready(_) => return scan(domain),
        Peek::Building | Peek::Absent => None,
    };
    let held = match entry.as_deref().map(|entry| &entry.held) {
        Some(Held::Rows { covered, matched }) => Some((covered, matched)),
        Some(Held::Entities(_)) | None => None,
    };
    let mut wanted = Bitmap::new();
    for range in domain {
        wanted.add_range(range.clone());
    }
    let missing = match held {
        Some((covered, _)) => wanted.andnot(covered),
        None => wanted.clone(),
    };
    let mut answer = match held {
        Some((_, matched)) => matched.and(&wanted),
        None => Bitmap::new(),
    };
    if missing.is_empty() {
        return Ok(answer);
    }
    let mut ranges = Vec::new();
    for range in domain {
        for_each_run_in(&missing, range.clone(), &mut |run| ranges.push(run));
    }
    let fresh = scan(&ranges)?;
    answer.or_inplace(&fresh);
    if held.is_some_and(|(covered, _)| {
        missing.cardinality().saturating_mul(KEEP_GROWTH_RATIO) < covered.cardinality()
    }) {
        return Ok(answer);
    }
    let held_bytes = entry.as_ref().map_or(0, |entry| entry.cache_weight_bytes());
    let grown = held_bytes
        + missing.get_serialized_size_in_bytes::<Portable>() as u64
        + fresh.get_serialized_size_in_bytes::<Portable>() as u64;
    if grown > cache.bound_bytes() {
        return Ok(answer);
    }
    let (mut covered, mut matched) = match held {
        Some((covered, matched)) => (covered.or(&missing), matched.or(&fresh)),
        None => (missing, fresh),
    };
    for bitmap in [&mut covered, &mut matched] {
        bitmap.run_optimize();
        bitmap.shrink_to_fit();
    }
    let kept = KeptLeaf {
        canonical,
        columns: Arc::downgrade(&shard.filter_columns),
        held: Held::Rows { covered, matched },
    };
    drop(entry);
    cache.evict(&key);
    let _ = cache.get_or_try_build(key, || Ok::<_, std::convert::Infallible>(kept));
    Ok(answer)
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
    let Held::Entities(entities) = &entry.held else {
        return evaluate(candidate);
    };
    if entry.canonical != canonical {
        return evaluate(candidate);
    }
    debug_assert!(std::ptr::eq(
        entry.columns.as_ptr(),
        Arc::as_ptr(&shard.filter_columns)
    ));
    Ok(entities.and(candidate))
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
    /// and a digest collision is scanned whole.
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
            rows_answer(&cache, shard, leaf, domain, |ranges: &[Range<u32>]| {
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
