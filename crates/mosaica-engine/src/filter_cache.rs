//! A text or value filter leaf's answer, kept across requests: a viewer panning and zooming with
//! the same filter sends it again with every viewport, and over a large corpus a common phrase or
//! a broad range costs the same each time.
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

use std::sync::{Arc, Weak};

use croaring::{Bitmap, Portable};
use mosaica_cache::{
    CacheWeight, Cancel, NeverCancelled, Peek, SingleFlightCache, WaitEnded, WaitingBuildError,
};
use mosaica_types::ShardId;

use crate::cancel::{CancelToken, Interests};
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
/// `shard` is carried beside the rest so a prune can find the shard the entry is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct LeafKey {
    pub shard: ShardId,
    pub columns: usize,
    pub identity: [u8; 32],
    pub watermark: u64,
    pub grant: [u8; 32],
    pub digest: [u8; 16],
}

/// One leaf's matches within the fragment its key names.
pub(crate) struct KeptLeaf {
    canonical: Vec<u8>,
    columns: Weak<FilterColumns>,
    entities: Bitmap,
}

impl CacheWeight for KeptLeaf {
    fn cache_weight_bytes(&self) -> u64 {
        self.entities.get_serialized_size_in_bytes::<Portable>() as u64
            + self.canonical.len() as u64
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

/// The requests waiting on each entry being computed.
pub(crate) type FilterInterest = Interests<LeafKey>;

/// Drop every entry over filter columns or a fragment `live` no longer holds: no request can name
/// it again.
pub(crate) fn prune_superseded(cache: &FilterCache, live: &Generation) -> usize {
    cache.retain_keys(|key| {
        live.shard(key.shard).is_some_and(|shard| {
            Arc::as_ptr(&shard.filter_columns) as usize == key.columns
                && shard.identity == key.identity
                && shard.watermark == key.watermark
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
    };
    (key, canonical)
}

/// The leaf's answer under `candidate`, from `cache`'s entry for `leaf` in `shard` intersected
/// with `candidate`. `evaluate` is the leaf's evaluation under a candidate, ended once the token
/// it is given has gone.
///
/// `fragment` is the grant's fragment in `shard`, given where `candidate` is the request's whole
/// candidate. Only then is a missing entry computed, over the fragment, and kept; concurrent
/// requests for it compute it once, and a request waiting on another's computation stops when
/// `cancel` reports it gone. The computation is every such request's, whichever of them runs it:
/// it watches `interest` and ends only once all of them have gone, so a request that leaves does
/// not take the entry from one still waiting. A smaller candidate reads an entry already kept and
/// is evaluated otherwise. An evaluation over the fragment that fails, and an entry whose
/// canonical bytes are not the leaf's, which is a digest collision, are answered by evaluating
/// under `candidate` for this request alone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn answer(
    cache: &FilterCache,
    interest: &FilterInterest,
    shard: &Shard,
    (key, canonical): (LeafKey, Vec<u8>),
    fragment: Option<&Bitmap>,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
    evaluate: &dyn Fn(&Bitmap, &Option<CancelToken>) -> Result<Bitmap, FilterError>,
) -> Result<Bitmap, FilterError> {
    if cache.bound_bytes() == 0 {
        return evaluate(candidate, cancel);
    }
    let entry = if let Some(fragment) = fragment {
        let waiting: &dyn Cancel = match cancel {
            Some(token) => token,
            None => &NeverCancelled,
        };
        let (wanted, _caller) = interest.register(&key, cancel.as_ref());
        let wanted = Some(CancelToken::shared(wanted));
        let built = cache.get_or_try_build_waiting(key, waiting, || {
            let mut entities = evaluate(fragment, &wanted)?;
            entities.run_optimize();
            entities.shrink_to_fit();
            Ok(KeptLeaf {
                canonical: canonical.clone(),
                columns: Arc::downgrade(&shard.filter_columns),
                entities,
            })
        });
        match built {
            Ok(entry) => entry,
            Err(WaitingBuildError::Build(FilterError::Cancelled)) => {
                return Err(FilterError::Cancelled)
            }
            Err(WaitingBuildError::Build(_)) => return evaluate(candidate, cancel),
            Err(WaitingBuildError::Wait(WaitEnded::Cancelled)) => {
                return Err(FilterError::Cancelled)
            }
            Err(WaitingBuildError::Wait(WaitEnded::Budget)) => return evaluate(candidate, cancel),
        }
    } else {
        match cache.peek(&key) {
            Peek::Ready(entry) => entry,
            Peek::Building | Peek::Absent => return evaluate(candidate, cancel),
        }
    };
    if entry.canonical != canonical {
        return evaluate(candidate, cancel);
    }
    debug_assert!(std::ptr::eq(
        entry.columns.as_ptr(),
        Arc::as_ptr(&shard.filter_columns)
    ));
    Ok(entry.entities.and(candidate))
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
        let interest = FilterInterest::default();
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
                &interest,
                shard,
                leaf,
                Some(&fragment),
                candidate,
                &None,
                &|candidate, _| evaluate(candidate),
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
        let alone = |candidate: &Bitmap, _: &Option<CancelToken>| evaluate(candidate);
        let got = answer(
            &cache,
            &interest,
            shard,
            key(),
            None,
            &smaller,
            &None,
            &alone,
        );
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

    /// **A kept entry's computation is every waiting request's.** It watches a token that has gone
    /// only once the request running it and every request waiting on it have, so the first to
    /// leave does not take the entry from the rest; with all of them gone it ends, and nothing is
    /// kept.
    #[test]
    fn a_computation_ends_only_once_every_request_waiting_on_it_has_gone() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Barrier;

        let generation = Generation::synthetic("v00000", 1, 0, Overlay::new(), IngestBuffer::new());
        let shard = generation.only_shard();
        let cache = FilterCache::new(1 << 20);
        let interest = FilterInterest::default();
        let operand = FilterOperand::TextContains("17".to_string());
        let key = || key_of(shard, [7; 32], "code", LeafQuery::Value(&operand));
        let whole = Bitmap::from_range(0..10);
        let matches = Bitmap::of(&[2, 3]);

        // The first request computes, and is watched while it does: `seen` is what its
        // computation's token said after the first request had gone, with a second one waiting.
        let (first, second) = (CancelToken::new(), CancelToken::new());
        let (started, release) = (Barrier::new(2), Barrier::new(2));
        let seen = AtomicBool::new(true);
        let computing = |candidate: &Bitmap, watch: &Option<CancelToken>| {
            started.wait();
            release.wait();
            seen.store(crate::cancel::gone(watch), Ordering::Relaxed);
            crate::cancel::gone(watch)
                .then(|| Err(FilterError::Cancelled))
                .unwrap_or_else(|| Ok(candidate.and(&matches)))
        };
        let unreached = |_: &Bitmap, _: &Option<CancelToken>| -> Result<Bitmap, FilterError> {
            panic!("the request waiting reads the first's entry")
        };
        let ask = |cancel: &CancelToken, evaluate: &dyn Fn(&Bitmap, &Option<CancelToken>) -> _| {
            let cancel = Some(cancel.clone());
            answer(
                &cache,
                &interest,
                shard,
                key(),
                Some(&whole),
                &whole,
                &cancel,
                evaluate,
            )
        };
        std::thread::scope(|scope| {
            let running = scope.spawn(|| ask(&first, &computing));
            started.wait();
            let waiting = scope.spawn(|| ask(&second, &unreached));
            while interest.callers(&key().0) < 2 {
                std::thread::yield_now();
            }
            first.cancel();
            release.wait();
            assert_eq!(running.join().unwrap(), Ok(matches.clone()));
            assert_eq!(waiting.join().unwrap(), Ok(matches.clone()));
        });
        assert!(
            !seen.load(Ordering::Relaxed),
            "a request still waiting keeps the computation wanted"
        );
        assert_eq!(
            cache.len(),
            1,
            "the entry is kept for the request that waited"
        );

        // Alone, a request that has gone ends its own computation.
        let other = FilterOperand::TextContains("23".to_string());
        let alone = CancelToken::new();
        alone.cancel();
        let ended = answer(
            &cache,
            &interest,
            shard,
            key_of(shard, [7; 32], "code", LeafQuery::Value(&other)),
            Some(&whole),
            &whole,
            &Some(alone),
            &|candidate, watch| match crate::cancel::gone(watch) {
                true => Err(FilterError::Cancelled),
                false => Ok(candidate.clone()),
            },
        );
        assert_eq!(ended, Err(FilterError::Cancelled));
        assert_eq!(cache.len(), 1, "a computation nobody waits for is not kept");
    }
}
