use std::sync::Arc;

use mosaica_types::progress::{JobKind, Phase, Tracker, Unit};
use mosaica_types::ShardId;

use crate::write::{Job, Jobs};

/// Resident bytes: total, anonymous, file-backed. Files written through a mapping are
/// file-backed and can be reclaimed once written back; anonymous memory cannot.
fn resident_set() -> (u64, u64, u64) {
    let r = mosaica_types::process::resident_bytes();
    (r.total, r.anon, r.file)
}

/// One pass's wall clock and the resident set when it ended. Sampled at pass boundaries, so it
/// shows which pass the memory grew in but misses the peak within a pass.
#[derive(Debug, Clone, Copy)]
pub struct PassCost {
    pub pass: &'static str,
    pub elapsed: std::time::Duration,
    pub rss: u64,
    pub anon: u64,
    /// What the pass counted and its total, as its step of the compaction's record ended. `None`
    /// for a pass that counts nothing.
    pub done: Option<u64>,
    pub total: Option<u64>,
}

/// The passes of a compaction in the order they run: [`super::execute`]'s on the compaction's
/// thread, then `Executor::publish_fold`'s on the executor's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pass {
    RowSpace,
    Postings,
    TermImages,
    UniqueIndexes,
    EditedItems,
    Attributes,
    EntityTerms,
    FieldTallies,
    Digests,
    /// From the end of the passes to the publication's start, and its checks.
    HandOff,
    Memberships,
    Derived,
    Report,
    Manifest,
    Flip,
    Retire,
    Open,
    Adopt,
    Warm,
    Wal,
    Reclaim,
}

/// The phases of a compaction, in the order they run.
const PHASES: [&str; 4] = [
    "Rewriting rows",
    "Indexing records",
    "Verifying",
    "Publishing",
];

impl Pass {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Pass::RowSpace => "row_space",
            Pass::Postings => "postings",
            Pass::TermImages => "term_images",
            Pass::UniqueIndexes => "unique_indexes",
            Pass::EditedItems => "edited_items",
            Pass::Attributes => "attributes",
            Pass::EntityTerms => "entity_terms",
            Pass::FieldTallies => "field_tallies",
            Pass::Digests => "digests",
            Pass::HandOff => "hand_off",
            Pass::Memberships => "memberships",
            Pass::Derived => "derived",
            Pass::Report => "report",
            Pass::Manifest => "manifest",
            Pass::Flip => "flip",
            Pass::Retire => "retire",
            Pass::Open => "open",
            Pass::Adopt => "adopt",
            Pass::Warm => "warm",
            Pass::Wal => "wal",
            Pass::Reclaim => "reclaim",
        }
    }

    /// The phase this pass runs in. The match has no wildcard arm, so a new pass does not compile
    /// until it has a phase.
    fn phase(self) -> Phase {
        let at = match self {
            Pass::RowSpace => 0,
            Pass::Postings
            | Pass::TermImages
            | Pass::UniqueIndexes
            | Pass::EditedItems
            | Pass::Attributes
            | Pass::EntityTerms
            | Pass::FieldTallies => 1,
            Pass::Digests => 2,
            Pass::HandOff
            | Pass::Memberships
            | Pass::Derived
            | Pass::Report
            | Pass::Manifest
            | Pass::Flip
            | Pass::Retire
            | Pass::Open
            | Pass::Adopt
            | Pass::Warm
            | Pass::Wal
            | Pass::Reclaim => 3,
        };
        Phase::new(at as u32 + 1, PHASES.len() as u32, PHASES[at])
    }
}

/// A compaction's passes: each pass's cost row, and its step of the compaction's record, which
/// `/control/status` serves while the compaction runs.
///
/// Entering a pass ends the one open and begins the next, so the step a reader sees and the pass
/// the time is charged to are the same. The compaction's thread enters its passes, and the
/// publication goes on with a clone that the completed fold carries, so the cost rows and the
/// record cover the compaction end to end. The record is among the running jobs until
/// [`Staircase::finish`] ends it, or until every clone is dropped, which fails it.
#[derive(Clone)]
pub(crate) struct Staircase {
    cost: Vec<PassCost>,
    open: Option<(Pass, std::time::Instant)>,
    job: Arc<Job>,
    shard: String,
}

impl Staircase {
    /// The staircase of a compaction of `shard`, its record listed in `jobs` from now.
    pub(crate) fn start(jobs: &Arc<Jobs>, shard: ShardId) -> Self {
        let first = Pass::RowSpace;
        let shard = format!("shard {shard}");
        let tracker = Tracker::new(JobKind::Compaction, first.phase(), first.name());
        tracker.begin(first.phase(), first.name(), Some(shard.clone()), None);
        Self {
            cost: Vec::with_capacity(21),
            open: None,
            job: Arc::new(Job::begin(jobs, tracker)),
            shard,
        }
    }

    /// End the open pass and begin `pass`. `counted` is the pass's total and what it counts, or
    /// `None` for a pass that counts nothing.
    pub(crate) fn enter(&mut self, pass: Pass, counted: Option<(u64, Unit)>) {
        self.close();
        self.tracker()
            .begin(pass.phase(), pass.name(), Some(self.shard.clone()), counted);
        self.open = Some((pass, std::time::Instant::now()));
    }

    /// End the open pass: its cost row takes the wall clock since it began and the resident set
    /// now. Its step stays the record's until the next pass begins.
    pub(crate) fn close(&mut self) {
        let Some((pass, began)) = self.open.take() else {
            return;
        };
        let (rss, anon, _) = resident_set();
        let step = self.tracker().snapshot();
        self.cost.push(PassCost {
            pass: pass.name(),
            elapsed: began.elapsed(),
            rss,
            anon,
            done: step.done,
            total: step.total,
        });
    }

    /// The record the passes count their work into.
    pub(crate) fn tracker(&self) -> &Tracker {
        self.job.tracker()
    }

    /// End the open pass and the compaction, and return the cost rows.
    pub(crate) fn finish(mut self) -> Vec<PassCost> {
        self.close();
        self.job.finish();
        self.cost
    }
}
