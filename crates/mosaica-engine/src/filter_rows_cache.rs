//! A filter's matches crossed into a view's row space, kept across requests.
//!
//! A viewer panning and zooming with one filter sends it again with every viewport, and where the
//! matches are few beside the viewport's rows the crossing projects every one of them through the
//! view's permutation each time ([`crate::Engine`]'s `cross_filter_into_row_space`). A set of
//! entities' rows are a function of the set and of the row space alone, so an entry keyed by both
//! answers any request that crosses the same set into the same row space, and the next viewport
//! reads it instead of projecting again.
//!
//! # What an entry holds, and why a request may read it
//!
//! An entry holds the entities it was projected from beside their rows, and a request takes the
//! rows only where its own entities are equal to the entry's: the key's digest finds the entry and
//! the comparison decides, so a digest shared by two sets costs a projection and never a wrong
//! row. The rows a request takes are therefore the rows its crossing would have produced, and they
//! meet the request's mask as those would. The entities a request crosses are its filter's matches
//! within its own candidate, so an entry holds nothing outside some request's candidate, and the key
//! carries the grant, so a request reads only entries made under its own.
//!
//! # Bound and peak
//!
//! `filter_rows_cache_bytes` bounds the bytes resident here. An entry is the matches and their rows,
//! each a bitmap, so about twice the rows' size. A request clones the rows it takes, and an entry
//! evicted while a request reads it is freed when the request lets go, so
//!
//! ```text
//! peak ≈ filter_rows_cache_bytes + compute_admission × twice the largest entry
//! ```

use std::sync::{Arc, Weak};

use croaring::{Bitmap, Portable};
use mosaica_cache::{CacheWeight, NeverCancelled, Peek, SingleFlightCache, WaitingBuildError};
use mosaica_roaring::{Container, Readable};
use mosaica_store::read::ShardData;
use mosaica_types::ShardId;

use crate::{Generation, Shard};

/// `serve.filter_rows_cache_bytes` where it is not set.
pub const DEFAULT_FILTER_ROWS_CACHE_BYTES: u64 = 256 << 20;

/// What one set of entities' rows in one view of one shard are a function of.
///
/// - `data` and `view`: the row space. `data` is the address of the shard's data, which holds
///   every view's row space and is immutable: a flush, a merge, a fold and a view's creation or
///   drop each publish new data. The entry keeps a [`Weak`] to it, so while the entry exists
///   nothing else can be allocated at that address.
/// - `grant`: the grant ([`crate::Session::terms_digest`]) whose request made the entry, which
///   confines a hit to requests under the same grant.
/// - `digest`: of the entities, which finds the entry; the entities themselves are held in it,
///   where the bound counts them, and compared on every hit.
///
/// `shard` is carried beside the rest so a prune can find the shard the entry is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FilterRowsKey {
    pub shard: ShardId,
    pub data: usize,
    pub view: String,
    pub grant: [u8; 32],
    pub digest: u64,
}

/// One set of entities and its rows in the row space its key names.
pub(crate) struct FilterRowsEntry {
    data: Weak<ShardData>,
    entities: Bitmap,
    rows: Bitmap,
}

impl CacheWeight for FilterRowsEntry {
    fn cache_weight_bytes(&self) -> u64 {
        (self.entities.get_serialized_size_in_bytes::<Portable>()
            + self.rows.get_serialized_size_in_bytes::<Portable>()) as u64
    }
}

pub(crate) type FilterRowsCache = SingleFlightCache<FilterRowsKey, FilterRowsEntry>;

/// Drop every entry over shard data `live` no longer holds: no request can name it again.
pub(crate) fn prune_superseded(cache: &FilterRowsCache, live: &Generation) -> usize {
    cache.retain_keys(|key| {
        live.shard(key.shard)
            .is_some_and(|shard| Arc::as_ptr(&shard.data) as usize == key.data)
    })
}

/// The key of `entities` in `view` of `shard` for `grant`.
pub(crate) fn key_of(
    shard: &Shard,
    view: &str,
    grant: [u8; 32],
    entities: &Bitmap,
) -> FilterRowsKey {
    FilterRowsKey {
        shard: shard.id(),
        data: Arc::as_ptr(&shard.data) as usize,
        view: view.to_string(),
        grant,
        digest: digest_of(entities),
    }
}

/// The rows `entities` hold in `view` of `shard`, read from the cache where an equal set is kept,
/// and otherwise taken from `project` and kept. Concurrent requests for one set project it once; a
/// request finding another's projection under way waits for it up to the cache's wait budget, and
/// past it projects alone.
pub(crate) fn rows_of(
    cache: &FilterRowsCache,
    shard: &Shard,
    view: &str,
    grant: [u8; 32],
    entities: &Bitmap,
    project: impl FnOnce() -> Bitmap,
) -> Bitmap {
    if cache.bound_bytes() == 0 {
        return project();
    }
    let key = key_of(shard, view, grant, entities);
    let mut project = Some(project);
    let built = cache.get_or_try_build_waiting(key, &NeverCancelled, || {
        let project = project.take().expect("built once");
        let mut rows = project();
        rows.shrink_to_fit();
        Ok::<_, std::convert::Infallible>(FilterRowsEntry {
            data: Arc::downgrade(&shard.data),
            entities: entities.clone(),
            rows,
        })
    });
    match built {
        Ok(entry) if entry.entities == *entities => {
            debug_assert!(std::ptr::eq(entry.data.as_ptr(), Arc::as_ptr(&shard.data)));
            entry.rows.clone()
        }
        // Another set under the same digest, or no answer within the wait budget.
        Ok(_) | Err(WaitingBuildError::Wait(_)) => match project {
            Some(project) => project(),
            None => unreachable!("an entry built by this call holds this call's entities"),
        },
        Err(WaitingBuildError::Build(never)) => match never {},
    }
}

/// The rows `entities` hold in `view` of `shard`, where the cache already keeps an equal set.
pub(crate) fn kept_rows(
    cache: &FilterRowsCache,
    shard: &Shard,
    view: &str,
    grant: [u8; 32],
    entities: &Bitmap,
) -> Option<Bitmap> {
    if cache.bound_bytes() == 0 {
        return None;
    }
    match cache.peek(&key_of(shard, view, grant, entities)) {
        Peek::Ready(entry) if entry.entities == *entities => Some(entry.rows.clone()),
        Peek::Ready(_) | Peek::Building | Peek::Absent => None,
    }
}

/// A digest of `entities` that finds an entry and decides nothing: every container's key, kind and
/// payload, mixed a word at a time.
fn digest_of(entities: &Bitmap) -> u64 {
    const MIX: u64 = 0x9E37_79B9_7F4A_7C15;
    let mix = |h: u64, word: u64| (h ^ word).wrapping_mul(MIX).rotate_left(29);
    let readable = Readable::of(entities);
    let mut h = 0u64;
    for (key, container) in readable.containers().iter() {
        h = mix(h, u64::from(key));
        match container {
            Container::Bitset(words) => {
                h = mix(h, 1);
                for &word in words.iter() {
                    h = mix(h, word);
                }
            }
            Container::Array(lows) => {
                h = mix(h, 2 | (lows.len() as u64) << 8);
                for lows in lows.chunks(4) {
                    let word = lows
                        .iter()
                        .enumerate()
                        .fold(0u64, |w, (i, &low)| w | u64::from(low) << (16 * i));
                    h = mix(h, word);
                }
            }
            Container::Run(runs) => {
                h = mix(h, 3 | (runs.len() as u64) << 8);
                for run in runs {
                    h = mix(h, u64::from(run.start) | u64::from(run.length) << 16);
                }
            }
        }
    }
    h
}
