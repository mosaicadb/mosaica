//! A text filter leaf's answer, kept across requests: a viewer panning and zooming with the same
//! `match` or `phrase` sends it again with every viewport, and over a large corpus a common phrase
//! costs the same each time.
//!
//! # What an answer is a function of
//!
//! A text leaf evaluated under a candidate answers the candidate's members whose value in one of the
//! column's layers carries the query, so its answer is `P ∩ C`: `P` read from the shard's text
//! layers and `C` the candidate. The entry holds `P ∩ C` for the candidate
//! [`crate::Engine::filter_candidate`] composes, and that candidate is the session's fragment less
//! the shard's overlay, plus the buffered entities whose verdict passes. Every request's candidate
//! is that set or a subset of it, and the answer under a subset `S` is `P ∩ S`, so an entry
//! intersected with the request's candidate is that request's answer exactly.
//!
//! That needs the entry's candidate to be the one the request's is a subset of, and `P` to be the
//! one the request would read. [`TextLeafKey`] names both.
//!
//! # Bound and peak
//!
//! `text_filter_cache_bytes` bounds the bytes resident here. A request holds an entry only while it
//! intersects it with its candidate, and an entry evicted meanwhile is freed when it lets go, so
//!
//! ```text
//! peak ≈ text_filter_cache_bytes + compute_admission × the largest answer
//! ```
//!
//! An answer is a bitmap over the shard's entities: about 12 MB at 10⁸ entities for a phrase half
//! of them say, and kilobytes for a topical one.

use std::sync::{Arc, Weak};

use croaring::{Bitmap, Portable};
use mosaica_cache::{
    CacheWeight, Cancel, NeverCancelled, Peek, SingleFlightCache, WaitEnded, WaitingBuildError,
};
use mosaica_types::ShardId;

use crate::cancel::CancelToken;
use crate::filter::{FilterError, TextQuery};
use crate::{Generation, Shard};

/// `serve.text_filter_cache_bytes` where it is not set.
pub const DEFAULT_TEXT_FILTER_CACHE_BYTES: u64 = 256 << 20;

/// What one text leaf's answer in one shard is a function of.
///
/// - `snapshot`: the address of the shard as the request's generation holds it
///   ([`Generation::shard_arc`]). Its text layers, its overlay, its buffer, its watermark and its
///   identity are all held in that one immutable value, and a publication that changes any of them
///   holds a new one. The entry keeps a [`Weak`] to it, so while the entry exists no other shard
///   can be allocated at that address.
/// - `grant`: the session's grant ([`crate::Session::terms_digest`]), which the fragment is the
///   union of. A session's satisfied keys are the keys of its grant, or every key for a `read-all`
///   grant, so this names them too.
/// - `column` and `query`: the leaf, with its operand as the column's analyser reads it.
///
/// `shard` is carried beside the address so a prune can find the shard the entry is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TextLeafKey {
    pub shard: ShardId,
    pub snapshot: usize,
    pub grant: [u8; 32],
    pub column: String,
    pub query: TextQuery,
}

/// One text leaf's answer under the whole candidate of its key.
pub(crate) struct TextLeaf {
    snapshot: Weak<Shard>,
    entities: Bitmap,
}

impl CacheWeight for TextLeaf {
    fn cache_weight_bytes(&self) -> u64 {
        self.entities.get_serialized_size_in_bytes::<Portable>() as u64
    }
}

pub(crate) type TextFilterCache = SingleFlightCache<TextLeafKey, TextLeaf>;

/// Drop every answer taken over a shard `live` does not hold: no request can name it again.
pub(crate) fn prune_superseded(cache: &TextFilterCache, live: &Generation) -> usize {
    cache.retain_keys(|key| {
        live.shard_arc(key.shard)
            .is_some_and(|shard| Arc::as_ptr(shard) as usize == key.snapshot)
    })
}

/// The key of `column`'s leaf `query` in `shard` for `grant`.
pub(crate) fn key_of(
    shard: &Arc<Shard>,
    grant: [u8; 32],
    column: &str,
    query: &TextQuery,
) -> TextLeafKey {
    TextLeafKey {
        shard: shard.id(),
        snapshot: Arc::as_ptr(shard) as usize,
        grant,
        column: column.to_string(),
        query: query.clone(),
    }
}

/// `evaluate(candidate)`, read from `cache` under `key` where it holds the answer.
///
/// `whole` is the candidate `key` names, where the caller composed it. Only an answer under it is
/// kept, and concurrent requests for it compute it once; a request waiting on another's computation
/// stops when `cancel` reports it gone. A smaller candidate reads an answer already kept and
/// computes its own otherwise. Whichever answer is used is intersected with `candidate`.
pub(crate) fn answer(
    cache: &TextFilterCache,
    shard: &Arc<Shard>,
    key: TextLeafKey,
    whole: Option<&Bitmap>,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
    evaluate: &dyn Fn(&Bitmap) -> Result<Bitmap, FilterError>,
) -> Result<Bitmap, FilterError> {
    if cache.bound_bytes() == 0 {
        return evaluate(candidate);
    }
    let is_whole = whole.is_some_and(|whole| std::ptr::eq(whole, candidate) || whole == candidate);
    let entry = if is_whole {
        let waiting: &dyn Cancel = match cancel {
            Some(token) => token,
            None => &NeverCancelled,
        };
        let built = cache.get_or_try_build_waiting(key, waiting, || {
            Ok(TextLeaf {
                snapshot: Arc::downgrade(shard),
                entities: evaluate(candidate)?,
            })
        });
        match built {
            Ok(entry) => entry,
            Err(WaitingBuildError::Build(e)) => return Err(e),
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
    debug_assert!(std::ptr::eq(entry.snapshot.as_ptr(), Arc::as_ptr(shard)));
    Ok(entry.entities.and(candidate))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use mosaica_lifecycle::{IngestBuffer, Overlay};

    use super::*;

    /// **An evaluation that fails leaves nothing kept**, so the next request evaluates again, and
    /// a kept answer is read under a smaller candidate as its intersection with it.
    #[test]
    fn a_failed_evaluation_is_not_kept_and_a_kept_one_is_read_under_any_candidate() {
        let generation = Generation::synthetic("v00000", 1, 0, Overlay::new(), IngestBuffer::new());
        let shard = generation.shard_arc(ShardId::ZERO).expect("one shard");
        let cache = TextFilterCache::new(1 << 20);
        let query = TextQuery::Phrase {
            words: vec!["of".to_string(), "the".to_string()],
        };
        let key = || key_of(shard, [7; 32], "prose", &query);
        let whole = Bitmap::from_range(0..10);

        let cancelled = answer(&cache, shard, key(), Some(&whole), &whole, &None, &|_| {
            Err(FilterError::Cancelled)
        });
        assert_eq!(cancelled, Err(FilterError::Cancelled));
        assert_eq!(cache.len(), 0, "a cancelled evaluation is not kept");

        let evaluations = Cell::new(0);
        let evaluate = |candidate: &Bitmap| {
            evaluations.set(evaluations.get() + 1);
            Ok(candidate.and(&Bitmap::of(&[2, 3, 12])))
        };
        for _ in 0..2 {
            let got = answer(&cache, shard, key(), Some(&whole), &whole, &None, &evaluate);
            assert_eq!(got, Ok(Bitmap::of(&[2, 3])));
        }
        assert_eq!(evaluations.get(), 1, "the repeat is the kept answer");

        let smaller = Bitmap::of(&[3, 4]);
        let got = answer(
            &cache,
            shard,
            key(),
            Some(&whole),
            &smaller,
            &None,
            &evaluate,
        );
        assert_eq!(got, Ok(Bitmap::of(&[3])));
        assert_eq!(
            evaluations.get(),
            1,
            "a smaller candidate reads the kept answer"
        );
    }
}
