//! A text filter leaf's answer, kept across requests: a viewer panning and zooming with the same
//! `match` or `phrase` sends it again with every viewport, and over a large corpus a common phrase
//! costs the same each time.
//!
//! # What an entry holds, and why a request may read it
//!
//! A text leaf evaluated under a candidate `C` answers `C ∩ M`, where `M` is the set of entities
//! whose value in one of the column's text layers carries the query: every posting it reads is
//! narrowed to `C` before anything is counted, and a phrase then tests each survivor's positions
//! alone. An entry holds `F ∩ M`, with `F` the session's fragment, the items the grant covers before
//! the overlay's deletions and suppressions are taken out and the buffered entities whose verdict
//! passes are put in. A request's candidate is a subset of `F` plus buffered entities, and a
//! buffered entity has no value in any text layer, since the layers hold flushed entities and the
//! fold that frees an id removes its values. So `C ∩ M` is `C ∩ (F ∩ M)`, and a request reads an
//! entry by intersecting it with its own candidate.
//!
//! That makes an entry independent of the overlay and the buffer, so it outlives a window close, a
//! deletion, a suppression and a lift. [`TextLeafKey`] names what it does depend on: the column's
//! layers and the fragment.
//!
//! An entry holds entities a deny hides from the grant. Nothing read from them reaches a response
//! except through that intersection: the evaluation over `F` counts nothing a request sees, and one
//! that fails is evaluated again under the request's own candidate, whose answer or refusal is the
//! one the request gets.
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
//! An answer is a bitmap over the shard's entities: about 12 MB at 10⁸ entities for a phrase that
//! half of them contain, and kilobytes for a topical one. The leaf's canonical bytes are held and
//! counted beside it.

use std::sync::{Arc, Weak};

use croaring::{Bitmap, Portable};
use mosaica_cache::{
    CacheWeight, Cancel, NeverCancelled, Peek, SingleFlightCache, WaitEnded, WaitingBuildError,
};
use mosaica_types::ShardId;

use crate::cancel::CancelToken;
use crate::filter::{FilterColumns, FilterError, TextQuery};
use crate::{Generation, Shard};

/// `serve.text_filter_cache_bytes` where it is not set.
pub const DEFAULT_TEXT_FILTER_CACHE_BYTES: u64 = 256 << 20;

/// What one text leaf's entry in one shard is a function of.
///
/// - `columns`: the address of the shard's filter columns, which hold its text layers. They are
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
pub(crate) struct TextLeafKey {
    pub shard: ShardId,
    pub columns: usize,
    pub identity: [u8; 32],
    pub watermark: u64,
    pub grant: [u8; 32],
    pub digest: [u8; 16],
}

/// One text leaf's matches within the fragment its key names.
pub(crate) struct TextLeaf {
    canonical: Vec<u8>,
    columns: Weak<FilterColumns>,
    entities: Bitmap,
}

impl CacheWeight for TextLeaf {
    fn cache_weight_bytes(&self) -> u64 {
        self.entities.get_serialized_size_in_bytes::<Portable>() as u64
            + self.canonical.len() as u64
    }
}

/// `column` and `query` as bytes no other leaf has: each string after its length.
fn canonical(column: &str, query: &TextQuery) -> Vec<u8> {
    let (tag, minimum, tokens) = match query {
        TextQuery::Match { tokens, minimum } => (0u8, *minimum, tokens),
        TextQuery::Phrase { words } => (1u8, 0, words),
    };
    let mut out =
        Vec::with_capacity(13 + column.len() + tokens.iter().map(|t| 4 + t.len()).sum::<usize>());
    out.push(tag);
    out.extend_from_slice(&minimum.to_le_bytes());
    for bytes in std::iter::once(column).chain(tokens.iter().map(String::as_str)) {
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes.as_bytes());
    }
    out
}

pub(crate) type TextFilterCache = SingleFlightCache<TextLeafKey, TextLeaf>;

/// Drop every entry over filter columns or a fragment `live` no longer holds: no request can name
/// it again.
pub(crate) fn prune_superseded(cache: &TextFilterCache, live: &Generation) -> usize {
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
    query: &TextQuery,
) -> (TextLeafKey, Vec<u8>) {
    let canonical = canonical(column, query);
    let key = TextLeafKey {
        shard: shard.id(),
        columns: Arc::as_ptr(&shard.filter_columns) as usize,
        identity: shard.identity,
        watermark: shard.watermark,
        grant,
        digest: crate::region::digest_of(&canonical),
    };
    (key, canonical)
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
    cache: &TextFilterCache,
    shard: &Shard,
    (key, canonical): (TextLeafKey, Vec<u8>),
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
            Ok(TextLeaf {
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
        let cache = TextFilterCache::new(1 << 20);
        let query = TextQuery::Phrase {
            words: vec!["of".to_string(), "the".to_string()],
        };
        let key = || key_of(shard, [7; 32], "prose", &query);
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
        let colliding = (key().0, key_of(shard, [7; 32], "prose", &other).1);
        let got = ask(colliding, &whole, &|_| Ok(Bitmap::of(&[9])));
        assert_eq!(got, Ok(Bitmap::of(&[9])), "a collision is evaluated");
    }
}
