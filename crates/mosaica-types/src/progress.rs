//! How far a long-running job has gone: a build, an ingest, a flush or a compaction.
//!
//! A job is a fixed list of phases, each of which runs one or more steps. A [`Tracker`] holds the
//! job's current step, and the workers of that step add to its count with one relaxed atomic add
//! per batch. A reader (the build's renderer, `/control/status`, a client) takes a [`Progress`]
//! snapshot when it wants one, so the job does no reporting work of its own.
//!
//! The only estimate a snapshot carries is `eta_s`, which is the step's remaining count divided by
//! the rate measured over the last [`RATE_WINDOW`]. A step with no total, such as a sort, has no
//! `eta_s`, and the job as a whole has none.
//!
//! `docs/dev/progress.md` describes the record and every surface that serves it.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The span the rate is measured over.
pub const RATE_WINDOW: Duration = Duration::from_secs(20);

/// A step younger than this has no rate, because its first batches measure start-up rather than
/// throughput.
pub const RATE_SETTLE: Duration = Duration::from_secs(2);

/// What kind of job a [`Progress`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Build,
    Ingest,
    Flush,
    Compaction,
}

/// What a step's `done` and `total` count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Items,
    Rows,
    Pairs,
    Terms,
    Bytes,
    Pages,
}

/// Whether the job is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Finished,
    Failed,
}

/// One phase of a job: its position, counted from 1, among the job's phases, and the name a person
/// reads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Phase {
    pub index: u32,
    pub of: u32,
    pub name: Cow<'static, str>,
}

impl Phase {
    pub const fn new(index: u32, of: u32, name: &'static str) -> Self {
        Phase {
            index,
            of,
            name: Cow::Borrowed(name),
        }
    }
}

/// A snapshot of a job, as every surface serves it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Progress {
    pub job: JobKind,
    pub state: JobState,
    pub phase: Phase,
    /// The step's stable name, such as `filter_postings`, for scripts and for comparison with
    /// `--stage-timings`.
    pub step: Cow<'static, str>,
    /// What part of the step is running, for a person, such as `view papers (1 of 2)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// How much of the step is done, in `unit`. Absent for a step that counts nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
    /// Units per second over the last [`RATE_WINDOW`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    /// Seconds until the step's count reaches its total at `rate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_s: Option<f64>,
    pub elapsed_s: f64,
    pub step_elapsed_s: f64,
    /// The process's resident memory when the snapshot was taken.
    pub rss_bytes: u64,
}

/// The current step of one job, shared by the job's workers and its readers.
#[derive(Debug)]
pub struct Tracker {
    job: JobKind,
    started: Instant,
    done: AtomicU64,
    step: Mutex<Step>,
}

#[derive(Debug)]
struct Step {
    state: JobState,
    phase: Phase,
    step: Cow<'static, str>,
    context: Option<String>,
    counted: Option<(u64, Unit)>,
    started: Instant,
    /// `(when, done)` pairs taken at snapshots, the oldest within [`RATE_WINDOW`] first.
    samples: VecDeque<(Instant, u64)>,
}

impl Tracker {
    /// A job that has started and has not yet begun its first step.
    pub fn new(job: JobKind, phase: Phase, step: &'static str) -> Self {
        Self::new_at(job, phase, step, Instant::now())
    }

    fn new_at(job: JobKind, phase: Phase, step: &'static str, now: Instant) -> Self {
        Tracker {
            job,
            started: now,
            done: AtomicU64::new(0),
            step: Mutex::new(Step::new(phase, Cow::Borrowed(step), None, None, now)),
        }
    }

    pub fn job(&self) -> JobKind {
        self.job
    }

    /// Begin a step, resetting the count. `counted` is the step's total and what it counts, or
    /// `None` for a step whose work is not a count, such as a sort.
    pub fn begin(
        &self,
        phase: Phase,
        step: &'static str,
        context: Option<String>,
        counted: Option<(u64, Unit)>,
    ) {
        self.begin_at(phase, step, context, counted, Instant::now());
    }

    fn begin_at(
        &self,
        phase: Phase,
        step: &'static str,
        context: Option<String>,
        counted: Option<(u64, Unit)>,
        now: Instant,
    ) {
        let mut current = self.lock();
        self.done.store(0, Ordering::Relaxed);
        *current = Step::new(phase, Cow::Borrowed(step), context, counted, now);
    }

    /// Add `n` to the step's count. Call it once per batch, not once per unit; a pass that meets
    /// its units one at a time counts them with [`Batched`].
    #[inline]
    pub fn advance(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }

    /// Give the open step a total and a unit, for a step whose total is known only once it has
    /// begun. The step's count and clock carry on.
    pub fn count(&self, total: u64, unit: Unit) {
        self.lock().counted = Some((total, unit));
    }

    /// Mark the job finished or failed. The step is left as it was, so a failed job's last
    /// snapshot says where it stopped.
    pub fn end(&self, state: JobState) {
        self.lock().state = state;
    }

    pub fn snapshot(&self) -> Progress {
        let rss = crate::process::resident_bytes().total;
        self.snapshot_at(Instant::now(), rss)
    }

    fn snapshot_at(&self, now: Instant, rss_bytes: u64) -> Progress {
        let mut current = self.lock();
        let done = self.done.load(Ordering::Relaxed);
        let step_elapsed = now.saturating_duration_since(current.started);
        let (rate, eta_s) = match current.counted {
            Some((total, _)) => {
                let rate = current.rate(now, done);
                let eta = rate
                    .filter(|r| *r > 0.0)
                    .map(|r| total.saturating_sub(done) as f64 / r);
                (rate, eta)
            }
            None => (None, None),
        };
        Progress {
            job: self.job,
            state: current.state,
            phase: current.phase.clone(),
            step: current.step.clone(),
            context: current.context.clone(),
            done: current.counted.map(|_| done),
            total: current.counted.map(|(total, _)| total),
            unit: current.counted.map(|(_, unit)| unit),
            rate,
            eta_s,
            elapsed_s: now.saturating_duration_since(self.started).as_secs_f64(),
            step_elapsed_s: step_elapsed.as_secs_f64(),
            rss_bytes,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Step> {
        // A panic in a worker poisons nothing a snapshot could misreport, since every field is
        // replaced whole.
        self.step
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// How many units [`Batched`] counts between two calls of [`Tracker::advance`].
pub const ADVANCE_EVERY: u64 = 1 << 16;

/// A count kept by a pass that meets its units one at a time, and added to a tracker once per
/// [`ADVANCE_EVERY`] units and once more when it is dropped. With no tracker it adds to nothing.
pub struct Batched<'a> {
    tracker: Option<&'a Tracker>,
    at: u64,
    added: u64,
}

impl<'a> Batched<'a> {
    pub fn new(tracker: Option<&'a Tracker>) -> Self {
        Batched {
            tracker,
            at: 0,
            added: 0,
        }
    }

    /// Count `n` more units.
    #[inline]
    pub fn add(&mut self, n: u64) {
        self.reach(self.at + n);
    }

    /// Count every unit before `position`, for a pass that visits its units in ascending order
    /// and skips some.
    #[inline]
    pub fn reach(&mut self, position: u64) {
        self.at = position;
        if self.at - self.added >= ADVANCE_EVERY {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if let Some(tracker) = self.tracker {
            tracker.advance(self.at - self.added);
        }
        self.added = self.at;
    }
}

impl Drop for Batched<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

impl Step {
    fn new(
        phase: Phase,
        step: Cow<'static, str>,
        context: Option<String>,
        counted: Option<(u64, Unit)>,
        now: Instant,
    ) -> Self {
        Step {
            state: JobState::Running,
            phase,
            step,
            context,
            counted,
            started: now,
            samples: VecDeque::from([(now, 0)]),
        }
    }

    /// The rate from the oldest sample within the window to now, and record now as a sample.
    ///
    /// The step's start is the first sample, so a reader that polls rarely gets the step's
    /// average rate.
    fn rate(&mut self, now: Instant, done: u64) -> Option<f64> {
        while self.samples.len() > 1
            && now.saturating_duration_since(self.samples[1].0) >= RATE_WINDOW
        {
            self.samples.pop_front();
        }
        let (since, from) = self.samples[0];
        self.samples.push_back((now, done));
        if now.saturating_duration_since(self.started) < RATE_SETTLE {
            return None;
        }
        let seconds = now.saturating_duration_since(since).as_secs_f64();
        (seconds > 0.0).then(|| done.saturating_sub(from) as f64 / seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const READING: Phase = Phase::new(1, 5, "Reading sources");

    fn at(start: Instant, secs: u64) -> Instant {
        start + Duration::from_secs(secs)
    }

    #[test]
    fn a_counted_step_reports_its_count_rate_and_time_remaining() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Build, READING, "source_ids", t0);
        tracker.begin_at(READING, "source_ids", None, Some((1000, Unit::Items)), t0);
        tracker.advance(100);
        tracker.advance(100);
        let p = tracker.snapshot_at(at(t0, 4), 7);
        assert_eq!(
            (p.done, p.total, p.unit),
            (Some(200), Some(1000), Some(Unit::Items))
        );
        assert_eq!(p.rate, Some(50.0));
        assert_eq!(p.eta_s, Some(16.0));
        assert_eq!((p.elapsed_s, p.step_elapsed_s, p.rss_bytes), (4.0, 4.0, 7));
        assert_eq!(p.state, JobState::Running);
    }

    #[test]
    fn a_step_has_no_rate_before_it_settles() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Build, READING, "source_ids", t0);
        tracker.begin_at(READING, "source_ids", None, Some((1000, Unit::Items)), t0);
        tracker.advance(10);
        let p = tracker.snapshot_at(t0 + Duration::from_millis(500), 0);
        assert_eq!((p.rate, p.eta_s), (None, None));
    }

    #[test]
    fn a_step_that_counts_nothing_has_no_count_and_no_time_remaining() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Build, READING, "signature_sort", t0);
        let p = tracker.snapshot_at(at(t0, 30), 0);
        assert_eq!(
            (p.done, p.total, p.unit, p.rate, p.eta_s),
            (None, None, None, None, None)
        );
        let json = serde_json::to_value(&p).unwrap();
        assert!(json.get("done").is_none() && json.get("eta_s").is_none());
    }

    #[test]
    fn the_rate_follows_the_last_window_when_the_reader_polls_often() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Compaction, READING, "postings", t0);
        tracker.begin_at(READING, "postings", None, Some((10_000, Unit::Terms)), t0);
        // Fast for the first 30 s, then slow: the rate at 60 s is the slow one.
        for s in 1..=60 {
            tracker.advance(if s <= 30 { 100 } else { 10 });
            tracker.snapshot_at(at(t0, s), 0);
        }
        let p = tracker.snapshot_at(at(t0, 60), 0);
        let rate = p.rate.unwrap();
        assert!((rate - 10.0).abs() < 1.0, "rate {rate}");
    }

    #[test]
    fn a_rare_reader_gets_the_steps_average_rate() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Flush, READING, "rows", t0);
        tracker.begin_at(READING, "rows", None, Some((900, Unit::Rows)), t0);
        tracker.advance(300);
        let p = tracker.snapshot_at(at(t0, 100), 0);
        assert_eq!(p.rate, Some(3.0));
        assert_eq!(p.eta_s, Some(200.0));
    }

    #[test]
    fn beginning_a_step_resets_its_count_and_keeps_the_jobs_clock() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Build, READING, "source_ids", t0);
        tracker.begin_at(READING, "source_ids", None, Some((10, Unit::Items)), t0);
        tracker.advance(10);
        let next = Phase::new(2, 5, "Assigning ids");
        tracker.begin_at(
            next.clone(),
            "signature_sort",
            Some("batch 1 of 3".into()),
            None,
            at(t0, 5),
        );
        let p = tracker.snapshot_at(at(t0, 8), 0);
        assert_eq!(p.phase, next);
        assert_eq!(p.step, "signature_sort");
        assert_eq!(p.context.as_deref(), Some("batch 1 of 3"));
        assert_eq!((p.elapsed_s, p.step_elapsed_s), (8.0, 3.0));
        assert_eq!(p.done, None);
    }

    #[test]
    fn a_failed_job_keeps_the_step_it_stopped_in() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Build, READING, "source_ids", t0);
        tracker.begin_at(READING, "text_index", None, Some((100, Unit::Items)), t0);
        tracker.advance(61);
        tracker.end(JobState::Failed);
        let p = tracker.snapshot_at(at(t0, 3), 0);
        assert_eq!(
            (p.state, p.step.as_ref(), p.done),
            (JobState::Failed, "text_index", Some(61))
        );
    }

    #[test]
    fn a_count_given_after_the_step_began_keeps_its_clock_and_count() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Build, READING, "manifests", t0);
        tracker.begin_at(READING, "manifests", None, None, t0);
        tracker.advance(30);
        tracker.count(120, Unit::Bytes);
        let p = tracker.snapshot_at(at(t0, 10), 0);
        assert_eq!(
            (p.done, p.total, p.unit),
            (Some(30), Some(120), Some(Unit::Bytes))
        );
        assert_eq!(
            (p.step_elapsed_s, p.rate, p.eta_s),
            (10.0, Some(3.0), Some(30.0))
        );
    }

    #[test]
    fn a_batched_count_reaches_the_tracker_whole_once_dropped() {
        let tracker = Tracker::new(JobKind::Build, READING, "assignment");
        tracker.begin(
            READING,
            "assignment",
            None,
            Some((ADVANCE_EVERY * 3, Unit::Items)),
        );
        {
            let mut batched = Batched::new(Some(&tracker));
            for _ in 0..ADVANCE_EVERY + 5 {
                batched.add(1);
            }
            assert_eq!(tracker.snapshot().done, Some(ADVANCE_EVERY));
            batched.reach(ADVANCE_EVERY * 3);
        }
        assert_eq!(tracker.snapshot().done, Some(ADVANCE_EVERY * 3));
    }

    #[test]
    fn a_snapshot_round_trips_through_json() {
        let t0 = Instant::now();
        let tracker = Tracker::new_at(JobKind::Ingest, READING, "pages", t0);
        tracker.begin_at(
            READING,
            "pages",
            Some("view papers".into()),
            Some((5, Unit::Pages)),
            t0,
        );
        tracker.advance(2);
        let p = tracker.snapshot_at(at(t0, 3), 1);
        let back: Progress = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
}
