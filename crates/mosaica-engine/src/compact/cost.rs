use std::sync::Arc;

use mosaica_types::progress::{context, JobKind, Part, Phase, Stage, Stages, Tracker, Unit};
use mosaica_types::ShardId;

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

/// The passes of a compaction in the order they run: [`super::execute`]'s and the derived files'
/// on the compaction's thread, then `Executor::publish_fold`'s on the executor's.
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
    Derived,
    /// From the end of the passes to the publication's start, and its checks.
    HandOff,
    Memberships,
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
static PHASES: [&str; 4] = [
    "Rewriting rows",
    "Indexing records",
    "Verifying",
    "Publishing",
];

impl Stage for Pass {
    fn name(self) -> &'static str {
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

    /// The match has no wildcard arm, so a new pass does not compile until it has a phase.
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
            Pass::Derived
            | Pass::HandOff
            | Pass::Memberships
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
        Phase::nth(&PHASES, at)
    }
}

impl Pass {
    /// What a failure in `part` of this pass names, so that the error and the step its record
    /// stopped in agree.
    pub(crate) fn at(self, part: impl std::fmt::Display) -> String {
        format!("{} ({part})", self.name())
    }
}

/// A compaction's passes: each pass's cost row, and its step of the compaction's record, which
/// `/control/status` serves while the compaction is in flight.
///
/// [`Stages`] opens each pass's step as the pass begins, so the step a reader sees and the cost
/// row the time is charged to are the same. The compaction's thread enters its passes, and the
/// publication goes on with a clone that the completed fold carries, so the cost rows and the
/// record cover the compaction end to end.
#[derive(Clone)]
pub(crate) struct Staircase {
    cost: Vec<PassCost>,
    stages: Stages<Pass>,
    record: Arc<Tracker>,
    shard: Option<String>,
}

impl Staircase {
    /// The staircase of a compaction of `shard`, whose views hold `rows` rows, in its first pass
    /// from now.
    pub(crate) fn start(shard: ShardId, rows: u64) -> Self {
        let shard = context(&[Part::new("shard", &shard.to_string(), 0, 1)]);
        let counted = Some((rows, Unit::Rows));
        let (stages, record) =
            Stages::begin(JobKind::Compaction, Pass::RowSpace, shard.clone(), counted);
        Self {
            cost: Vec::new(),
            stages,
            record: Arc::new(record),
            shard,
        }
    }

    /// End the open pass and begin `pass`. `counted` is the pass's total and what it counts, or
    /// `None` for a pass that counts nothing. The open pass is closed first, so its cost row reads
    /// its own count before the next pass's step begins.
    pub(crate) fn enter(&mut self, pass: Pass, counted: Option<(u64, Unit)>) {
        self.close();
        self.stages
            .enter(&self.record, pass, self.shard.clone(), counted);
    }

    /// End the open pass: its cost row takes its wall clock, its count and the resident set now.
    /// Its step stays the record's until the next pass begins.
    pub(crate) fn close(&mut self) {
        let Some((pass, elapsed)) = self.stages.close() else {
            return;
        };
        let (rss, anon, _) = resident_set();
        let counted = self.record.counted();
        self.cost.push(PassCost {
            pass: pass.name(),
            elapsed,
            rss,
            anon,
            done: counted.map(|(done, _)| done),
            total: counted.map(|(_, total)| total),
        });
    }

    /// The compaction's record, which the passes count their work into.
    pub(crate) fn record(&self) -> &Arc<Tracker> {
        &self.record
    }

    /// End the open pass, and return the cost rows.
    pub(crate) fn finish(mut self) -> Vec<PassCost> {
        self.close();
        self.cost
    }
}
