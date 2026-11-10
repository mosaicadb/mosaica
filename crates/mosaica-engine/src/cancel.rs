//! Cooperative cancellation for a request whose client may go away.
//!
//! [`CancelToken`] is a newtype over `Arc<AtomicBool>` with no `tokio` dependency, so the engine API
//! stays synchronous. `mosaica-server` mints one per request, moves a clone into the blocking call,
//! and flips the original from a drop-guard wired to client disconnect (`mosaica-server`'s
//! `CancelGuard`), and nothing else flips it. The engine polls it at checkpoints, filter
//! evaluation among them, and stops the request at the first that observes the flip: with
//! [`crate::EngineError::Cancelled`], or on a paged read sometimes with an end by `deadline`.
//!
//! Work several requests wait on, as one build of a kept answer, belongs to all of them. It
//! watches a token made from their [`Interest`], which has gone only once every one of them has.
//!
//! **Ordering: `Relaxed` for both the flip and the check.** The flag carries no payload, so nothing
//! needs publishing alongside it, and no ordering bounds *when* another thread observes the flip.
//! `Relaxed` gives the one guarantee this needs, that every clone eventually observes a flip made
//! on any other, at the lowest cost per check, which matters on the per-tile hot path.

use std::hash::Hash;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use rustc_hash::FxHashMap;

/// A cheaply-cloneable cooperative-cancellation flag. Every clone shares the same underlying
/// `AtomicBool` via the `Arc`, so flipping any one clone is visible (eventually, per the module
/// doc's ordering note) to every other.
#[derive(Debug, Clone)]
pub struct CancelToken(Flag);

#[derive(Debug, Clone)]
enum Flag {
    /// One request's, flipped when its client goes.
    Own(Arc<AtomicBool>),
    /// Work several requests wait on, gone once every one of them has.
    Shared(Arc<Interest>),
}

impl CancelToken {
    /// A fresh, not-yet-cancelled token.
    pub fn new() -> Self {
        CancelToken(Flag::Own(Arc::new(AtomicBool::new(false))))
    }

    /// The token of work every caller of `interest` waits on.
    pub(crate) fn shared(interest: Arc<Interest>) -> Self {
        CancelToken(Flag::Shared(interest))
    }

    /// Flip the flag. Idempotent — cancelling an already-cancelled token (via this handle or any
    /// clone) has no further effect. A shared token has no flag of its own and is left as it is.
    pub fn cancel(&self) {
        if let Flag::Own(flag) = &self.0 {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Has this token — this handle or any clone of it — been cancelled.
    pub fn is_cancelled(&self) -> bool {
        match &self.0 {
            Flag::Own(flag) => flag.load(Ordering::Relaxed),
            Flag::Shared(interest) => interest.abandoned(),
        }
    }
}

/// The requests waiting for one piece of shared work, the one doing it included, each by its
/// cancellation. `None` stands for a caller with no client to lose.
#[derive(Debug, Default)]
pub(crate) struct Interest {
    callers: Mutex<Vec<(u64, Option<CancelToken>)>>,
}

impl Interest {
    /// Every caller that wanted this work has gone.
    pub(crate) fn abandoned(&self) -> bool {
        let callers = self.callers.lock().unwrap_or_else(PoisonError::into_inner);
        callers.iter().all(|(_, cancel)| gone(cancel))
    }
}

/// Each piece of shared work's [`Interest`], by the work's key, held while it has a caller.
#[derive(Debug)]
pub(crate) struct Interests<K> {
    by_key: Mutex<FxHashMap<K, Arc<Interest>>>,
    next_caller: AtomicU64,
}

impl<K> Default for Interests<K> {
    fn default() -> Self {
        Interests {
            by_key: Mutex::new(FxHashMap::default()),
            next_caller: AtomicU64::new(0),
        }
    }
}

impl<K: Hash + Eq + Clone> Interests<K> {
    /// How many callers `key`'s work has.
    #[cfg(test)]
    pub(crate) fn callers(&self, key: &K) -> usize {
        let map = self.by_key.lock().unwrap_or_else(PoisonError::into_inner);
        map.get(key).map_or(0, |interest| {
            let callers = interest
                .callers
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            callers.len()
        })
    }

    /// Adds a caller with `cancel` to `key`'s interest, until the guard drops.
    pub(crate) fn register<'a>(
        &'a self,
        key: &K,
        cancel: Option<&CancelToken>,
    ) -> (Arc<Interest>, impl Drop + 'a) {
        struct Caller<'a, K: Hash + Eq> {
            interests: &'a Interests<K>,
            key: K,
            interest: Arc<Interest>,
            id: u64,
        }
        impl<K: Hash + Eq> Drop for Caller<'_, K> {
            fn drop(&mut self) {
                let mut map = self
                    .interests
                    .by_key
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let mut callers = self
                    .interest
                    .callers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                callers.retain(|(id, _)| *id != self.id);
                if callers.is_empty() {
                    map.remove(&self.key);
                }
            }
        }
        let id = self.next_caller.fetch_add(1, Ordering::Relaxed);
        let interest = {
            let mut map = self.by_key.lock().unwrap_or_else(PoisonError::into_inner);
            let interest = Arc::clone(map.entry(key.clone()).or_default());
            interest
                .callers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((id, cancel.cloned()));
            interest
        };
        let caller = Caller {
            interests: self,
            key: key.clone(),
            interest: Arc::clone(&interest),
            id,
        };
        (interest, caller)
    }
}

/// Whether the request holding `cancel` has gone. `None` never goes.
pub(crate) fn gone(cancel: &Option<CancelToken>) -> bool {
    cancel.as_ref().is_some_and(CancelToken::is_cancelled)
}

/// `Err(EngineError::Cancelled)` once the request holding `cancel` has gone, `Ok(())` otherwise.
#[inline]
pub(crate) fn check_cancelled(cancel: &Option<CancelToken>) -> crate::error::Result<()> {
    if gone(cancel) {
        return Err(crate::EngineError::Cancelled);
    }
    Ok(())
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// What lets a caller waiting on a single-flight build carry this token into the wait. The cache
/// sits below this crate and cannot name [`CancelToken`], so the trait is how the two meet.
impl mosaica_cache::Cancel for CancelToken {
    fn is_cancelled(&self) -> bool {
        CancelToken::is_cancelled(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_token_is_not_cancelled() {
        assert!(!CancelToken::new().is_cancelled());
    }

    #[test]
    fn cancelling_one_clone_is_visible_on_another() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(
            clone.is_cancelled(),
            "the flip must be visible through every clone"
        );
    }

    /// Shared work has gone once every caller registered for it has, a caller with no token never
    /// goes, and a key's interest is forgotten with its last caller.
    #[test]
    fn shared_work_has_gone_only_once_every_caller_has() {
        let interests = Interests::<&str>::default();
        let (first, second) = (CancelToken::new(), CancelToken::new());
        let (interest, one) = interests.register(&"build", Some(&first));
        let shared = CancelToken::shared(Arc::clone(&interest));
        assert!(!shared.is_cancelled());
        first.cancel();
        assert!(shared.is_cancelled(), "its only caller has gone");

        let (joined, two) = interests.register(&"build", Some(&second));
        assert!(
            Arc::ptr_eq(&interest, &joined),
            "a second caller joins the first's interest"
        );
        assert!(
            !shared.is_cancelled(),
            "a live caller keeps the work wanted"
        );
        shared.cancel();
        assert!(
            !shared.is_cancelled(),
            "only its callers' tokens end shared work"
        );
        second.cancel();
        assert!(shared.is_cancelled());

        let (_, three) = interests.register(&"build", None);
        assert!(
            !shared.is_cancelled(),
            "a caller with no client to lose never goes"
        );
        drop((one, two, three));
        let (fresh, _four) = interests.register(&"build", None);
        assert!(
            !Arc::ptr_eq(&interest, &fresh),
            "the interest went with its last caller"
        );
    }

    #[test]
    fn cancelling_twice_is_a_harmless_no_op() {
        let token = CancelToken::new();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }
}
