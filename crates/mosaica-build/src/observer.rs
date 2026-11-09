//! Build-stage observation.
//!
//! `mosaica build` is a numbered sequence of stages (see `pipeline.rs`), and "blank-database
//! ingest as a function of point count" is really a question about *which* of them bends with
//! scale: the sorts are n log n, the digest pass is linear in bytes, and several passes are driven
//! by the size of the source files rather than by `--limit` at all.
//!
//! **An observer rather than a feature.** [`build`](crate::build) delegates to
//! [`build_observed`] with a no-op, so there is no API break, no `#[cfg]` in the pipeline, and no
//! second code path that could drift from the one that ships. The observer sees only durations
//! and counts — nothing derived from the corpus content.

use std::time::Duration;

use mosaica_types::progress::{context, Batched, JobKind, Part, Phase, Tracker, Unit};

/// The pipeline's stages. [`BuildStage::ALL`] lists them in the order they run.
///
/// Names match the `// ---- n. ...` comments in `pipeline.rs`. Adding a stage means adding a
/// variant; renaming one means changing both, which is the point — a stage that exists in the
/// code and not here would silently vanish from every measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStage {
    /// 1. Two passes over the points file, plus a sort and a duplicate check.
    SourceIds,
    /// 2. One pass over the pairs file, interning descriptors.
    Dictionary,
    /// 3. One pass over the pairs file, packing `ordinal << 32 | term_id`, then sort and dedup.
    PairsPack,
    /// 3b. **The one pass over the points file's geometry**, scattered by ordinal — early, because
    ///    the signature sort breaks ties on the Morton code (decision 0073) and entity ids do not
    ///    exist yet. It is put into entity order by [`BuildStage::Assignment`], which is walking
    ///    both indices anyway; there is no second geometry pass and no permute stage.
    GeometryRead,
    /// 4. The signature sort — permanent under I9, and the reason entity IDs cannot be
    ///    reassigned later.
    SignatureSort,
    /// 5. Entity id = position in the signature order.
    Assignment,
    /// 6. One pass over the pairs file, writing `postings` and `pairs.parquet`.
    PostingsWrite,
    /// 8. Geometry permuted from ordinal into entity order, plus the declared attribute tail. No
    ///    points-file I/O: [`BuildStage::GeometryRead`] did the reading.
    AttributeTail,
    /// 8b. Entity-space filter postings for every column that has a **value column** — one file
    ///    per `index = true` column. Its own stage rather than a rider on `PostingsWrite`, which
    ///    runs before the attribute values have been read; zero-length for a schema that declares
    ///    no filterable column.
    ///
    ///    Excludes the text columns, which [`BuildStage::TextIndex`] carries: the two share a
    ///    loop but not a code path, and at 7.4×10⁷ names they did not share an order of magnitude
    ///    either.
    FilterPostings,
    /// 8b′. The text columns' entity-space index — the token dictionary and the postings over it —
    ///    charged out of [`BuildStage::FilterPostings`]'s block rather than measured beside it,
    ///    because the two interleave over one column loop.
    TextIndex,
    /// 8b″. The record blob: the values of every column with no other home, in entity order.
    ///    Reported separately from the postings it shares a stage boundary with — one number over
    ///    three jobs is what made the 615 s this block cost at 7.4×10⁷ points a thing to model
    ///    rather than to read.
    RecordBlob,
    /// 8b‴. Releasing the non-render columns, which for a text column is one `String` free per
    ///    entity. The passes that wanted those values have just run and the tiler wants only the
    ///    render columns, so this is where the corpus's prose leaves memory.
    ColumnRelease,
    /// 8c. The declared layers and their artifacts: the member tables read, resolved against the
    ///    entity ids this build assigned, and published through the same registry the control
    ///    plane runs.
    ///
    ///    **Its own stage because it owned an unattributed share of the peak.** It ran inside
    ///    [`BuildStage::AttributeTail`], which is where the campaign's OOM kills landed
    /// — and a stage boundary is what lets
    ///    an observer say which of the two it was.
    Layers,
    /// 8c′. The unique columns' indexes, each through a spill sort that finds every value held
    ///    twice.
    UniqueIndexes,
    /// 9. The tiler sort: `(morton, mosaica_id)` ascending.
    TilerSort,
    /// 10. Segment files: `morton.u32`, `permutation.bin`, `columns.arrow`, and their fsyncs.
    SegmentWrite,
    /// 10b. The artifact pass: each level's layout chosen, its tile index or row-major column
    ///    written, every spatial level's resolution and its decomposition filed, and the prefix's
    ///    containment partitions composed.
    ///
    ///    **Its own stage because it ran inside [`BuildStage::Manifests`]**, which is a re-read and
    ///    a digest of the whole bundle and says nothing about a pass that resolves polygons and
    ///    composes a column per level per view. One number over the two is a number an operator
    ///    cannot act on.
    ArtifactPass,
    /// 10c. The term images: every authorisation term's base posting projected into each view's
    ///    row space and written (`crate::term_images_pass`).
    ///
    ///    Its own stage because it is a projection per term over the whole dictionary, and at
    ///    corpus scale it is minutes rather than the seconds the pass above it takes. Folded into
    ///    [`BuildStage::ArtifactPass`] it would read as a polygon resolution that had become slow.
    TermImages,
    /// 11. Manifests — including a full SHA-256 re-read of every byte written.
    Manifests,
}

impl BuildStage {
    pub fn name(&self) -> &'static str {
        match self {
            BuildStage::SourceIds => "source_ids",
            BuildStage::Dictionary => "dictionary",
            BuildStage::PairsPack => "pairs_pack",
            BuildStage::GeometryRead => "geometry_read",
            BuildStage::SignatureSort => "signature_sort",
            BuildStage::Assignment => "assignment",
            BuildStage::PostingsWrite => "postings_write",
            BuildStage::AttributeTail => "attribute_tail",
            BuildStage::FilterPostings => "filter_postings",
            BuildStage::TextIndex => "text_index",
            BuildStage::RecordBlob => "record_blob",
            BuildStage::ColumnRelease => "column_release",
            BuildStage::Layers => "layers",
            BuildStage::UniqueIndexes => "unique_indexes",
            BuildStage::TilerSort => "tiler_sort",
            BuildStage::SegmentWrite => "segment_write",
            BuildStage::ArtifactPass => "artifact_pass",
            BuildStage::TermImages => "term_images",
            BuildStage::Manifests => "manifests",
        }
    }

    /// The phase of a build this stage runs in, as `docs/dev/progress.md` names them. The match
    /// has no wildcard arm, so a new stage does not compile until it has a phase.
    pub fn phase(&self) -> Phase {
        let at = match self {
            BuildStage::SourceIds
            | BuildStage::Dictionary
            | BuildStage::PairsPack
            | BuildStage::GeometryRead => 0,
            BuildStage::SignatureSort | BuildStage::Assignment => 1,
            BuildStage::PostingsWrite
            | BuildStage::AttributeTail
            | BuildStage::Layers
            | BuildStage::UniqueIndexes
            | BuildStage::FilterPostings
            | BuildStage::TextIndex
            | BuildStage::RecordBlob
            | BuildStage::ColumnRelease => 2,
            BuildStage::TilerSort
            | BuildStage::SegmentWrite
            | BuildStage::ArtifactPass
            | BuildStage::TermImages => 3,
            BuildStage::Manifests => 4,
        };
        Phase::nth(&PHASES, at)
    }

    pub const ALL: [BuildStage; 19] = [
        BuildStage::SourceIds,
        BuildStage::Dictionary,
        BuildStage::PairsPack,
        BuildStage::GeometryRead,
        BuildStage::SignatureSort,
        BuildStage::Assignment,
        BuildStage::PostingsWrite,
        BuildStage::AttributeTail,
        BuildStage::Layers,
        BuildStage::UniqueIndexes,
        BuildStage::FilterPostings,
        BuildStage::TextIndex,
        BuildStage::RecordBlob,
        BuildStage::ColumnRelease,
        BuildStage::TilerSort,
        BuildStage::SegmentWrite,
        BuildStage::ArtifactPass,
        BuildStage::TermImages,
        BuildStage::Manifests,
    ];
}

/// The phases of a build, in the order they run.
static PHASES: [&str; 5] = [
    "Reading sources",
    "Assigning ids",
    "Indexing records",
    "Laying out views",
    "Verifying the bundle",
];

/// A tracker for one build, its step the build's first stage until the build begins one.
pub fn tracker() -> Tracker {
    let first = BuildStage::ALL[0];
    Tracker::new(JobKind::Build, first.phase(), first.name())
}

/// Notified as each stage completes.
///
/// `rows` is whatever the stage counted — items, pairs, terms — and is stage-specific rather than
/// uniform, because a uniform "rows" would be a lie for half of them. `passes` is the cascade
/// passes the stage's term indexes took over their runs, summed, and zero for a stage that merges
/// none. `peak_rss_kib` is sampled at stage end, which is enough to see which stage owns the
/// build's high-water mark.
pub trait BuildObserver: Send + Sync {
    fn stage_end(
        &self,
        stage: BuildStage,
        elapsed: Duration,
        rows: u64,
        passes: u64,
        peak_rss_kib: u64,
    );

    /// The tracker the build opens a step in as each stage begins and counts that step's work
    /// into, or `None` for an observer of stage ends alone.
    fn progress(&self) -> Option<&Tracker> {
        None
    }

    /// Called on the build's thread as a step in [`Self::progress`]'s tracker ends, before the
    /// next one begins, so that a reader can take the step's last snapshot.
    fn step_ended(&self) {}

    /// Called on the build's thread just after a step begins in [`Self::progress`]'s tracker, so
    /// that a reader can take the step's first snapshot however briefly the step runs.
    fn step_begun(&self) {}
}

/// The default: observes nothing, costs nothing.
pub struct NoopObserver;

impl BuildObserver for NoopObserver {
    #[inline(always)]
    fn stage_end(&self, _: BuildStage, _: Duration, _: u64, _: u64, _: u64) {}
}

/// Peak RSS in KiB from `/proc/self/status`'s `VmHWM`, or 0 where unavailable.
pub(crate) fn peak_rss_kib() -> u64 {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Opens each stage of the build, times it and reports it, and opens the stage's step in the
/// observer's tracker.
///
/// A stage's time runs from the end of the stage before it, so the stages' times sum to the wall
/// clock. The pipeline begins each stage directly after it ends the one before, so the step a
/// reader sees and the stage the time is charged to are the same for every second of the build.
pub(crate) struct StageTimer<'a> {
    observer: &'a dyn BuildObserver,
    steps: Steps<'a>,
    start: std::time::Instant,
    open: Option<BuildStage>,
    /// The cascade passes counted into the open stage.
    passes: u64,
}

impl<'a> StageTimer<'a> {
    pub(crate) fn new(observer: &'a dyn BuildObserver) -> Self {
        StageTimer {
            observer,
            steps: Steps::of(observer),
            start: std::time::Instant::now(),
            open: None,
            passes: 0,
        }
    }

    /// The steps of this build, for a pass that opens steps within a stage or counts its work.
    pub(crate) fn steps(&self) -> Steps<'a> {
        self.steps
    }

    /// Open `stage`, as the part of it `parts` names where it runs more than once. `counted` is
    /// the step's total and unit, or `None` for a stage whose work is not a pass over a count
    /// known now ([`Steps::count`] gives one later).
    pub(crate) fn begin(
        &mut self,
        stage: BuildStage,
        parts: &[Part<'_>],
        counted: Option<(u64, Unit)>,
    ) {
        debug_assert_eq!(self.open, None, "{stage:?} begun inside another stage");
        self.open = Some(stage);
        self.steps.open(stage, parts, counted, false);
    }

    /// Count `passes` cascade passes into the open stage's figures.
    pub(crate) fn passes(&mut self, passes: u64) {
        self.passes += passes;
    }

    /// Close the open stage, `stage`. `rows` is its count.
    ///
    /// **The allocator's free pages go back to the kernel here**, which is what makes a stage
    /// boundary the point the next stage's page cache is decided at ([`crate::trim_heap`]). The
    /// trim runs before the report, so the peak this reads is the stage's own.
    pub(crate) fn end(&mut self, stage: BuildStage, rows: u64) {
        debug_assert_eq!(self.open.take(), Some(stage), "{stage:?} ended unopened");
        crate::trim_heap();
        let now = std::time::Instant::now();
        let passes = std::mem::take(&mut self.passes);
        self.observer
            .stage_end(stage, now - self.start, rows, passes, peak_rss_kib());
        if self.steps.live.is_some() {
            self.observer.step_ended();
        }
        self.start = now;
    }

    /// Report a stage whose duration was measured **inside** the open block, and charge it to
    /// that block: the start moves forward by `elapsed`, so the enclosing [`Self::end`] reports
    /// the remainder and the two sum to the wall clock rather than double-counting it.
    ///
    /// For the passes that interleave rather than follow one another — the filter-postings loop
    /// visits a text column and a category column in whatever order the schema declares them, so
    /// their costs cannot be separated by a boundary in time. The pass opens the charged stage's
    /// steps itself ([`Steps::begin`]).
    pub(crate) fn charge(&mut self, stage: BuildStage, elapsed: Duration, rows: u64, passes: u64) {
        self.observer
            .stage_end(stage, elapsed, rows, passes, peak_rss_kib());
        self.start += elapsed;
    }
}

/// The steps of a build, opened in the [`BuildObserver::progress`] tracker. It is copied into the
/// passes that count their work, and with no tracker each call is a test of `None`.
#[derive(Clone, Copy, Default)]
pub(crate) struct Steps<'a> {
    live: Option<(&'a Tracker, &'a dyn BuildObserver)>,
    /// The parts of the stage the steps opened from here are within, outermost first.
    within: [Option<Part<'a>>; 2],
}

impl<'a> Steps<'a> {
    fn of(observer: &'a dyn BuildObserver) -> Self {
        Steps {
            live: observer.progress().map(|tracker| (tracker, observer)),
            within: [None; 2],
        }
    }

    /// These steps, every step opened from them a step of `part`.
    pub(crate) fn within(mut self, part: Part<'a>) -> Self {
        let free = self.within.iter_mut().find(|slot| slot.is_none());
        *free.expect("steps are within at most two parts") = Some(part);
        self
    }

    /// End the open step and begin one of `stage` for the part `parts` names, inside a stage that
    /// [`StageTimer`] opened: a file of the identity pass, or a column of the filter postings.
    pub(crate) fn begin(self, stage: BuildStage, parts: &[Part<'_>], counted: Option<(u64, Unit)>) {
        self.open(stage, parts, counted, true);
    }

    fn open(
        self,
        stage: BuildStage,
        parts: &[Part<'_>],
        counted: Option<(u64, Unit)>,
        ends_one: bool,
    ) {
        let Some((tracker, observer)) = self.live else {
            return;
        };
        if ends_one {
            observer.step_ended();
        }
        let within: Vec<Part<'_>> = self.within.iter().flatten().chain(parts).copied().collect();
        tracker.begin(stage.phase(), stage.name(), context(&within), counted);
        observer.step_begun();
    }

    /// Give the open step its total, found by `total` only where there is a tracker.
    pub(crate) fn count(
        self,
        unit: Unit,
        total: impl FnOnce() -> crate::Result<u64>,
    ) -> crate::Result<()> {
        if let Some((tracker, _)) = self.live {
            tracker.count(total()?, unit);
        }
        Ok(())
    }

    /// Add `n` to the step's count. Called once per batch of a pass, never once per unit.
    #[inline]
    pub(crate) fn advance(self, n: u64) {
        if let Some((tracker, _)) = self.live {
            tracker.advance(n);
        }
    }

    /// A count for a pass that meets its units one at a time.
    pub(crate) fn batched(self) -> Batched<'a> {
        Batched::new(self.live.map(|(tracker, _)| tracker))
    }
}

// ---------------------------------------------------------------------------------------------
// The JSON sink — `mosaica build --stage-timings-json <path>`
// ---------------------------------------------------------------------------------------------

/// One stage's record, as `--stage-timings-json` writes it.
///
/// **Timestamps are wall clock, seconds since the Unix epoch**, so a stage record can be lined up
/// against anything else measured on the box during the build — an RSS sampler, a cgroup's
/// `memory.peak`, another process's log. `started_at` is derived as `ended_at - wall_s` rather
/// than recorded at the stage's opening: the observer is only notified at the end, and deriving it
/// keeps the two fields exactly consistent with `wall_s`.
///
/// ⊘ **For a *charged* stage the pair is not a real boundary.** [`StageTimer::charge`] reports a
/// duration measured inside an enclosing block, at the moment that block ends, so its `ended_at`
/// is the enclosing block's end and its `started_at` is that minus its own measured cost. The
/// duration is the measurement; the two timestamps are an interval of that length placed at the
/// report, which for `text_index`, `record_blob`, `column_release` and `filter_postings` — the
/// four that interleave over one column loop — is not when they ran.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StageRecord {
    pub stage: String,
    pub wall_s: f64,
    /// Whatever the stage counted — items, pairs, terms. Stage-specific, never uniform.
    pub rows: u64,
    /// The cascade passes the stage's term indexes took over their runs, summed.
    #[serde(default)]
    pub cascade_passes: u64,
    /// The **process's** high-water mark at the moment the stage ended, not the stage's own: it
    /// only rises, so a stage that adds nothing repeats the last figure. What it locates is the
    /// stage the peak arrived in.
    pub peak_rss_kib: u64,
    pub started_at: f64,
    pub ended_at: f64,
}

/// Collects every stage's record, for writing as JSON at the end of the build.
///
/// `Mutex` rather than `RefCell` because [`BuildObserver`] is `Send + Sync`. The lock is taken
/// once per stage report, which is a few dozen times in a build of several batches and views.
#[derive(Default)]
pub struct JsonStageTimings {
    records: std::sync::Mutex<Vec<StageRecord>>,
}

impl JsonStageTimings {
    pub fn new() -> Self {
        Self::default()
    }

    /// The records collected so far, in report order.
    pub fn records(&self) -> Vec<StageRecord> {
        self.records
            .lock()
            .expect("the stage-record lock is never held across a panic")
            .clone()
    }

    /// Write the records to `path` as a JSON array.
    pub fn write(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self.records())
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(path, json)
    }
}

impl BuildObserver for JsonStageTimings {
    fn stage_end(
        &self,
        stage: BuildStage,
        elapsed: Duration,
        rows: u64,
        passes: u64,
        peak_rss_kib: u64,
    ) {
        let ended_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let wall_s = elapsed.as_secs_f64();
        self.records
            .lock()
            .expect("the stage-record lock is never held across a panic")
            .push(StageRecord {
                stage: stage.name().to_string(),
                wall_s,
                rows,
                cascade_passes: passes,
                peak_rss_kib,
                started_at: ended_at - wall_s,
                ended_at,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_sink_records_one_row_per_report_in_order() {
        let sink = JsonStageTimings::new();
        sink.stage_end(
            BuildStage::Dictionary,
            Duration::from_millis(1500),
            7,
            0,
            1024,
        );
        sink.stage_end(
            BuildStage::TextIndex,
            Duration::from_millis(500),
            9,
            2,
            2048,
        );
        let records = sink.records();
        assert_eq!(
            records.iter().map(|r| r.stage.as_str()).collect::<Vec<_>>(),
            ["dictionary", "text_index"]
        );
        assert!((records[0].wall_s - 1.5).abs() < 1e-9);
        assert_eq!(records[0].rows, 7);
        assert_eq!(records[1].cascade_passes, 2);
        assert_eq!(records[1].peak_rss_kib, 2048);
    }

    #[test]
    fn a_stages_start_is_its_end_less_its_wall_time() {
        let sink = JsonStageTimings::new();
        sink.stage_end(BuildStage::TextIndex, Duration::from_secs(3), 0, 0, 0);
        let r = &sink.records()[0];
        assert!((r.ended_at - r.started_at - 3.0).abs() < 1e-6);
        assert!(
            r.started_at > 1_700_000_000.0,
            "wall clock, not a monotonic reading"
        );
    }

    #[test]
    fn the_written_file_is_a_json_array_of_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stages.json");
        let sink = JsonStageTimings::new();
        sink.stage_end(BuildStage::Manifests, Duration::from_millis(250), 3, 0, 16);
        sink.write(&path).expect("write");
        let back: Vec<StageRecord> =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].stage, "manifests");
        assert_eq!(back[0].rows, 3);
    }
}
