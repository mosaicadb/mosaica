//! Cooperative cancellation for a request whose client may go away.
//!
//! [`CancelToken`] is a newtype over `Arc<AtomicBool>` with no `tokio` dependency, so the engine API
//! stays synchronous. `mosaica-server` mints one per request, moves a clone into the blocking call,
//! and flips the original from a drop-guard wired to client disconnect (`mosaica-server`'s
//! `CancelGuard`). The engine polls it at checkpoints, filter evaluation among them, and ends the
//! request with [`crate::EngineError::Cancelled`] the moment it observes the flip, never with a
//! partial answer.
//!
//! **Ordering: `Relaxed` for both the flip and the check.** The flag carries no payload, so nothing
//! needs publishing alongside it, and no ordering bounds *when* another thread observes the flip.
//! `Relaxed` gives the one guarantee this needs, that every clone eventually observes a flip made
//! on any other, at the lowest cost per check, which matters on the per-tile hot path.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cheaply-cloneable cooperative-cancellation flag. Every clone shares the same underlying
/// `AtomicBool` via the `Arc`, so flipping any one clone is visible (eventually, per the module
/// doc's ordering note) to every other.
#[derive(Debug, Clone)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A fresh, not-yet-cancelled token.
    pub fn new() -> Self {
        CancelToken(Arc::new(AtomicBool::new(false)))
    }

    /// Flip the flag. Idempotent — cancelling an already-cancelled token (via this handle or any
    /// clone) has no further effect.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Has this token — this handle or any clone of it — been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
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

    #[test]
    fn cancelling_twice_is_a_harmless_no_op() {
        let token = CancelToken::new();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }
}
