//! Work done out of order and taken back in order: the record blob's compressed blocks, and a
//! parallel term merge's ranges.
//!
//! Each piece of work is given a sequence number when it is issued, its result is put back under
//! that number whenever it is done, and the results are taken in sequence: a result put back ahead
//! of the next one waits in a ready map until the ones before it are taken. At most `bound` are
//! issued and not yet taken, which is what bounds the memory the ready map and the work in flight
//! hold.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// Sequence numbers, a ready map, and a bound on what is issued and not yet taken.
pub(crate) struct Ordered<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
    /// The next sequence number to be taken, read without the lock.
    head: AtomicU64,
    bound: u64,
}

struct State<T> {
    issued: u64,
    head: u64,
    ready: HashMap<u64, T>,
    failed: bool,
}

impl<T> Ordered<T> {
    /// At most `bound` issued and not yet taken, and at least one.
    pub(crate) fn new(bound: usize) -> Ordered<T> {
        Ordered {
            state: Mutex::new(State {
                issued: 0,
                head: 0,
                ready: HashMap::new(),
                failed: false,
            }),
            changed: Condvar::new(),
            head: AtomicU64::new(0),
            bound: bound.max(1) as u64,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, state: MutexGuard<'a, State<T>>) -> MutexGuard<'a, State<T>> {
        self.changed
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The next sequence number below `end`, once fewer than the bound are issued and not taken.
    /// `None` once `end` have been issued.
    pub(crate) fn issue(&self, end: u64) -> io::Result<Option<u64>> {
        let mut state = self.lock();
        loop {
            if state.failed {
                return Err(stopped());
            }
            if state.issued >= end {
                return Ok(None);
            }
            if state.issued - state.head < self.bound {
                state.issued += 1;
                return Ok(Some(state.issued - 1));
            }
            state = self.wait(state);
        }
    }

    /// Whether as many as the bound are issued and not taken, so that [`Self::issue`] would wait.
    pub(crate) fn full(&self) -> bool {
        let state = self.lock();
        state.issued - state.head >= self.bound
    }

    /// Put back the result of `seq`.
    pub(crate) fn put(&self, seq: u64, value: T) {
        self.lock().ready.insert(seq, value);
        self.changed.notify_all();
    }

    /// The next result in sequence, with its number, which advances the head. `None` where every
    /// issued result has been taken, and, without `wait`, where the next is not back yet.
    pub(crate) fn take(&self, wait: bool) -> io::Result<Option<(u64, T)>> {
        let mut state = self.lock();
        loop {
            if state.failed {
                return Err(stopped());
            }
            let head = state.head;
            if let Some(value) = state.ready.remove(&head) {
                state.head += 1;
                self.head.store(state.head, Ordering::Release);
                drop(state);
                self.changed.notify_all();
                return Ok(Some((head, value)));
            }
            if !wait || state.head == state.issued {
                return Ok(None);
            }
            state = self.wait(state);
        }
    }

    /// Whether the next result to be taken is back.
    pub(crate) fn head_ready(&self) -> bool {
        let state = self.lock();
        state.ready.contains_key(&state.head)
    }

    /// Whether `seq` is the next to be taken.
    pub(crate) fn is_head(&self, seq: u64) -> bool {
        self.head.load(Ordering::Acquire) == seq
    }

    /// Wait until `seq` is the next to be taken.
    pub(crate) fn wait_head(&self, seq: u64) -> io::Result<()> {
        let mut state = self.lock();
        loop {
            if state.failed {
                return Err(stopped());
            }
            if state.head == seq {
                return Ok(());
            }
            state = self.wait(state);
        }
    }

    /// Stop: every call that waits, and every later one, fails.
    pub(crate) fn fail(&self) {
        self.lock().failed = true;
        self.changed.notify_all();
    }
}

fn stopped() -> io::Error {
    io::Error::other("work done in parallel stopped before its results were taken")
}

/// Fails `ordered` if the thread holding it panics, so that no other thread waits for a result
/// that will not come.
pub(crate) struct FailOnPanic<'a, T>(pub(crate) &'a Ordered<T>);

impl<T> Drop for FailOnPanic<'_, T> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.fail();
        }
    }
}
