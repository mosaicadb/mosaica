//! The records of the flush and the compaction in flight, which `/control/status` serves, and the
//! last record of each kind of job that has ended.

use std::sync::{Arc, Mutex};

use mosaica_types::progress::{JobKind, JobState, Progress, Tracker};

use super::lock_recover;

/// The jobs running, in the order they began, and the last of each kind to end.
#[derive(Debug, Default)]
pub(crate) struct Jobs {
    running: Mutex<Vec<Arc<Tracker>>>,
    ended: Mutex<Vec<Progress>>,
}

impl Jobs {
    /// A record of each job running.
    pub(crate) fn running(&self) -> Vec<Progress> {
        let running = lock_recover(&self.running).clone();
        running
            .iter()
            .map(|tracker| tracker.snapshot())
            .filter(|record| record.state == JobState::Running)
            .collect()
    }

    /// The last record of the last job of kind `job` to end, `finished` or `failed`.
    pub(crate) fn last_ended(&self, job: JobKind) -> Option<Progress> {
        lock_recover(&self.ended)
            .iter()
            .find(|ended| ended.job == job)
            .cloned()
    }
}

/// A job's record, listed among the running jobs from [`Job::begin`] until it ends. A job dropped
/// before [`Job::finish`] has failed, and its last record keeps the step it stopped in.
#[derive(Debug)]
pub(crate) struct Job {
    jobs: Arc<Jobs>,
    tracker: Arc<Tracker>,
}

impl Job {
    pub(crate) fn begin(jobs: &Arc<Jobs>, tracker: Arc<Tracker>) -> Job {
        lock_recover(&jobs.running).push(Arc::clone(&tracker));
        Job {
            jobs: Arc::clone(jobs),
            tracker,
        }
    }

    pub(crate) fn finish(self) {
        self.end(JobState::Finished);
    }

    /// Keep the job's last record among the ended ones, then take it from the running ones, under
    /// the running list's lock, so a reader of the running list and then the ended one finds it
    /// in one of the two. The second call does nothing.
    fn end(&self, state: JobState) {
        let mut running = lock_recover(&self.jobs.running);
        let Some(at) = running
            .iter()
            .position(|held| Arc::ptr_eq(held, &self.tracker))
        else {
            return;
        };
        let mut last = self.tracker.snapshot();
        last.state = state;
        {
            let mut ended = lock_recover(&self.jobs.ended);
            ended.retain(|held| held.job != last.job);
            ended.push(last);
        }
        self.tracker.end(state);
        running.remove(at);
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.end(JobState::Failed);
    }
}
