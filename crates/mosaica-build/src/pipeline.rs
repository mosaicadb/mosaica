//! The streaming batch build: the same bundle [`crate::build_in_memory`] produces, computed
//! within a bounded working set.
//!
//! ## Why this exists
//!
//! The linear build materialises one struct per point (each carrying its own heap-allocated
//! signature) and the whole `term -> entity list` relation as a `Vec<Vec<u32>>` before it writes
//! a byte. At the probe corpus's 10⁹ items that is tens of gigabytes of small allocations and it
//! was OOM-killed on a 47 GiB box during staging, before any output existed.
//!
//! ## The construction
//!
//! Every intermediate here is a **flat, packed array indexed by an integer**, anything
//! recomputable from the input parquet is recomputed rather than retained, and anything whose
//! size scales with the corpus rather than with the budget either lives behind the plan's
//! arithmetic or spills ([`spill`]). The passes:
//!
//! | pass | produces | bounded by |
//! |---|---|---|
//! | every file ×1 | each row's number — an item's **ordinal** is the number the rule gave it ([`crate::ids`]) | a sort share, 4 B/row mapped |
//! | pairs ×1 | dictionary (streamed), term-lookup arrays, pre-dedup `row_counts`, histogram | 12T + 8T |
//! | pairs ×1 | resolved `ordinal << 32 \| term` per **batch bucket** (RAM when it fits, spilled otherwise) | plan |
//! | per batch | sort+dedup the bucket; per-ordinal `starts`; signature sort + refinement; entity ids; **band emit** | plan |
//! | per band | cursor-scatter (already sorted), Roaring-encode in sub-chunks, append to `postings` + `pairs.parquet` | plan |
//! | — | postings completion: the offsets, spooled at 8 B a term, appended after the records | 8(T+1) on disk |
//! | points ×1 | geometry, the tiler sort, and the segment | 28N |
//!
//! ## A column no pass reads at an entity is never permuted
//!
//! A declared value is placed at its entity index as the join resolves it, and for a string
//! column that is a permutation of the source's characters through a mapping larger than memory.
//! A column whose only reader is the record blob may be spared it: each join chunk of it is
//! written as one record-blob extent in that chunk's entity order ([`ColumnRoutes`],
//! [`crate::extents`], `build-column-extents.md`), the blob merges the extents, and a `text`
//! column's token index reads them in block windows first.
//!
//! ## Batch-scoped signature assignment (§11.1)
//!
//! Entity ids are assigned by signature order **within each batch and only within one** — the
//! design's own scope for the sort (I10's per-batch clause; the serving allocator's group
//! commit is the same mechanism at window scale). A batch is a contiguous ordinal range of
//! `batch_items` stride; ids are batch-major, so per-term posting lists stay globally
//! ascending as the concatenation of per-batch runs, and one batch covering the corpus
//! reproduces the historical global sort byte for byte. The batch size is **identity-bearing**
//! (I9): derived deterministically from the memory budget (largest feasible, on a coarse
//! grid), recorded in MANIFEST provenance whenever the build actually batches, replayed — not
//! re-derived — by identity-preserving rebuilds, and refused when a stated size would
//! needlessly fragment posting runs the budget could have kept whole.
//!
//! ## The spill discipline
//!
//! Bucket and band files live under `<out>/.build-tmp/`, are deleted as consumed and
//! on every exit path, and each carries a write-side `(count, splitmix64-sum)` receipt verified on
//! read — the cross-pass anchor discipline extends across every spill boundary. Bands arrive
//! with each term's entities strictly ascending by construction (bases ascend across batches,
//! positions within one), which `encode_posting`'s unconditional sortedness check re-verifies
//! from disk: a corrupted or reordered spill fails the build, never bends a posting.
//!
//! ## An ordinal is a number, and a pass walks a file in number order
//!
//! Every row a pass reads arrives with its number ([`crate::ids`]): the item the identity rule
//! decided it names or creates, numbered in creation order. The numbers are `0..n`, so an item's
//! ordinal is its number with nothing to look up, and a view's points arrive in ascending ordinals
//! in file order. Each pass that maps rows to ordinals buffers a bounded chunk, sorts it by
//! ordinal ([`join_chunk`]) and walks the ordinal-indexed files it writes forwards.
//!
//! Within a chunk, rows carrying the same ordinal keep the order the file gave them; every
//! consumer is insensitive to the order otherwise (each call site argues why), so build output
//! stays byte-deterministic. The ordinal is what the entity-id assignment breaks its last tie on
//! (I9), so two builds of one corpus number its items identically.
//!
//! ## Parallelism does not participate in ordering decisions
//!
//! The large sorts run under rayon (`par_sort_unstable*`). Every parallel sort site is a
//! **total order on unique keys** — the pre-sort triple carries the ordinal, the tiler
//! comparator refines through the full `mosaica_id` bijection, and `packed`'s duplicates are bit-identical `u64`s — so an unstable,
//! nondeterministically-scheduled sort still has exactly one output. The equivalence suite's
//! byte-identity assertion is the oracle that keeps this true.
//!
//! The other parallel shape here is **one lane per declared column** — the attribute join stages,
//! scatters and tallies each column on a thread of its own
//! ([`read_one_attribute_source`]). It participates in no ordering decision either, and for a
//! stronger reason than the sorts do: the lanes never meet. Each column is its own mapped array
//! with its own presence bits and its own arena, indexed by entity, and each spilled column its
//! own extent writer ([`crate::extents`]), so no two lanes can name the same byte. The only
//! shared state is read-only — the join's answer and the declaration — and the
//! only shared *result* is the coverage tally, which is returned per lane and folded in
//! declaration order rather than accumulated across threads. Minting stays where it was — a
//! serial pre-pass per decoded batch, in file order — so the vocabulary codes a build assigns are
//! not a function of how the lanes were scheduled.
//!
//! ## The two assumptions this construction makes
//!
//! **The inputs do not change while the build runs.** The linear build reads each file once;
//! this one reads the points file four times and the pairs file three, and it carries counts and
//! offsets from an earlier pass into a later one. A points or pairs file rewritten mid-build
//! would therefore be read as two different corpora. That cannot be prevented from inside the
//! process, so it is checked instead: every place a later pass depends on an earlier one — a
//! source id that must resolve to an ordinal, a term bucket that must have room — is a typed
//! error, never an `unwrap`; each later pass additionally re-accumulates an order-independent
//! **content anchor** ([`splitmix64`] sums over the ids, and over the resolved `(ordinal, term)`
//! relation) and compares it against the first pass's, because counts alone accept
//! substitutions that preserve them. A mutated input fails the build. (The anchors are
//! avalanche-mixed sums, not cryptographic hashes: they make an accidental compensating
//! mutation implausible, and an adversary who can rewrite build inputs mid-run is outside this
//! defence's scope.)
//!
//! **An item's index keys are those of its row's labels read together.** [`build_dictionary`]
//! relies on a source term being one dictionary key, derived from the source term alone, so the
//! whole item never has to be assembled. [`crate::plan_access`] makes it one: each distinct label is
//! read once into the keys it is indexed under ([`mosaica_authz::index_keys`]), and the scan visits
//! a row's keys in place of its labels. A row whose labels are several and name a conjunction is
//! read again as one disjunction, because a conjunction one label names may be absorbed by another.
//! Each key is a term or a conjunction, and a principal satisfies the item exactly when it holds or
//! satisfies one of them.
//!
//! ## Byte-for-byte identity is the correctness condition
//!
//! Entity IDs are assigned by signature order and are **permanent** (I9): a build that ordered
//! items differently would not be a slower or faster build, it would be a *different corpus*,
//! and every posting, permutation and handle derived from it would be invalidated. So this
//! module is not licensed to reorder anything, and `tests/build_equivalence.rs` holds it to
//! producing directory-recursively byte-identical bundles against `build_in_memory`.
//!
//! The three places the equivalence is subtle, and how it is preserved:
//!
//! * **Term-id assignment.** The linear build interns descriptors while walking items in
//!   ascending source-id order, each item's terms in ascending source-term order. A term's id is
//!   therefore its rank under `(first ordinal that carries it, source term id)` — which
//!   [`build_dictionary`] computes directly from one pass over the pairs relation, without
//!   holding a single item.
//! * **The signature sort.** `recs` is pre-sorted on a 64-bit key built from the signature's
//!   first two terms, which totally orders every item whose signature is at most two terms long
//!   (the overwhelming majority — the probe corpus averages 1.72). Only groups that tie on
//!   that key *and* contain a longer signature are refined by a full signature comparison. The
//!   pre-sort key is order-consistent with the lexicographic signature order, so refining within
//!   ties reproduces it exactly.
//! * **The postings order.** Entity lists come out of a term-bucketed scatter and are sorted per
//!   term, which is the same ascending list the linear build accumulates by walking items in
//!   entity order.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use croaring::Bitmap;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use mosaica_authz::encode_posting;
use mosaica_filter::{
    Codes, RecordFieldRef, RecordValueRef, ValueColumnWriter, RECORD_BLOCKS_FILE,
    RECORD_BLOCK_TARGET, RECORD_DIRECTORY_FILE, RECORD_HASROW_FILE,
};
use mosaica_spatial::split32;
use mosaica_spatial::tiler::{ScalarType, ScalarValue};
use mosaica_store::manifest::DeclaredScalar;
use mosaica_store::ShardLayout;
use mosaica_types::SMALL_TERM_THRESHOLD_DEFAULT;

use crate::column::EntityColumn;
use crate::error::{BuildError, Result};
use crate::input;
use crate::observer::{BuildObserver, BuildStage, StageTimer, Steps};
use crate::spill;
use crate::{
    fsync_file, validate_args, write_manifests, BuildArgs, BuildReport, BundleFiles,
    PairsParquetWriter, PREFIX, SEG_ID,
};
use mosaica_filter_write::{
    cascade_sorted_runs, HeldRecords, RunPayload, SpillReceipt, TermAccumulator, TermMerge,
    TextRunWriter,
};
use mosaica_types::progress::{Batched, Part, Unit};
use mosaica_types::splitmix64;

/// One item's position in the signature sort. 16 bytes, four-byte aligned — every field is a
/// component of the sort key, and holding the key in the record is what keeps the comparator a
/// pure function of it.
///
/// **The `morton` field is [decision 0073](../../../docs/decisions/0073-entity-ties-are-ordered-by-morton-code.md)**:
/// ties within a signature group order by the item's Morton **code**, so a spatially coherent set
/// lands in a contiguous run of entity ids (*measured* 4.08× on artifact membership's disk form,
/// with term postings byte-identical). It is the same `split32` cell code the tiler ranks rows by,
/// so the two orders agree on what "nearby" means.
///
/// **The record grew from 12 bytes to hold it, and [`plan_build`]'s residency model was widened to
/// match** — a batch is sized against the memory budget, so an unwidened model would plan a batch
/// it cannot hold. ⊘ The alternative — keeping 12 bytes and reading the code out of the mapped
/// `morton-of-ordinal.u32` inside the comparator — trades that anonymous 4 B/item for an
/// indirection per comparison, and is **unmeasured**: it is the layout question the decision
/// leaves to this stage.
#[derive(Clone, Copy)]
#[repr(C)]
struct SortRec {
    key_hi: u32,
    key_lo: u32,
    morton: u32,
    ordinal: u32,
}

impl SortRec {
    fn order(&self) -> (u32, u32, u32, u32) {
        (self.key_hi, self.key_lo, self.morton, self.ordinal)
    }
}

/// The failure every "a later pass disagrees with an earlier one" check reports.
///
/// The only way to reach one of these is for an input file to have changed under the build (see
/// the module docs) or for a counting pass to be wrong. Both are corruption of the relation
/// between an item's geometry and its authorisation data, so both fail the build.
fn input_changed(detail: &str) -> BuildError {
    BuildError::Invalid(format!(
        "the input files changed while the build was reading them ({detail}); \
         the files a build reads must not change until it ends"
    ))
}

/// Bit `i` of a packed bitset.
fn bit_set(bits: &mut [u64], i: usize) {
    bits[i / 64] |= 1u64 << (i % 64);
}

fn bit_get(bits: &[u64], i: usize) -> bool {
    bits[i / 64] & (1u64 << (i % 64)) != 0
}

fn term_of(packed_entry: u64) -> u32 {
    packed_entry as u32
}

/// Rows buffered per [`join_chunk`] flush: 2²⁶ rows × 16 B ≈ 1 GiB of transient, constant in N.
const JOIN_CHUNK_ROWS: usize = 1 << 26;

/// The attribute join's staging buffer, **in bytes**.
///
/// A row count is the wrong unit for this buffer, and at corpus scale it stops being a bound at
/// all: [`JOIN_CHUNK_ROWS`] is 67,108,864, so a 73,631,092-point corpus staged 91% of itself in one
/// chunk — a near-complete second copy of every column the source carries, two flushes, and none of
/// the chunking the sweep is chunked for. Sized in bytes the same buffer is a constant the schema
/// cannot inflate: a wider schema takes fewer rows per chunk and more chunks, which is what a
/// staging buffer is supposed to do.
///
/// **Chunk boundaries are unobservable in the output**, which is what makes this number free to
/// choose. Source ids are duplicate-checked before this pass, so no entity is written twice;
/// [`join_chunk`]'s own contract leaves the order *within* a chunk unspecified; and vocabulary
/// codes are minted in `scan_attributes`'s per-batch pre-pass, driven by parquet batch order and
/// not by this. Nothing identity-bearing — entity ids, ordinals, minting order — is a function of
/// where a chunk ends.
///
/// What it bounds is the headers: [`staging_rows`] prices a row at the join key plus each column's
/// fixed width, and a string's *contents* ride on top of that, so a column of long prose overshoots
/// this figure by whatever it averages per value. **Since the columns were mapped (`column.rs`)
/// what it bounds is a file rather than the heap** — page cache the kernel may reclaim, not memory
/// the machine must have.
const JOIN_STAGE_BYTES: usize = 256 << 20;

/// How many rows of `attributes` fit in [`JOIN_STAGE_BYTES`], at least one and never more than the
/// corpus.
pub(crate) fn staging_rows(attributes: &[&crate::config::Attribute], n: u64) -> usize {
    // The join key beside each staged row — `(source_id, pos)`, 16 bytes and not the 12 an earlier
    // comment claimed — the 8 bytes the sweep's answer takes beside it (`(entity, pos)`), and one
    // typed slot per column. Each column's presence bit adds an eighth of a byte per row on top,
    // which is left out rather than rounded up to a whole one.
    let per_row: usize = 24 + attributes.iter().map(|a| staged_width(a.ty)).sum::<usize>();
    (JOIN_STAGE_BYTES / per_row).clamp(1, n.max(1) as usize)
}

/// One staged slot's width, as the budget above prices it.
///
/// ⊘ **The string families are priced at the 24-byte `String` header they no longer cost** — a
/// staged string slot is an 8-byte arena offset now, and the characters are the arena's
/// (`column.rs`). The figure stands where it is: it makes the buffer smaller than the budget rather
/// than larger, and moving it moves every chunk boundary.
fn staged_width(ty: ScalarType) -> usize {
    match ty {
        ScalarType::Bool | ScalarType::U8 | ScalarType::I8 => 1,
        ScalarType::U16 | ScalarType::I16 => 2,
        ScalarType::U32 | ScalarType::I32 | ScalarType::F32 => 4,
        ScalarType::U64 | ScalarType::I64 | ScalarType::F64 | ScalarType::TimestampUs => 8,
        ScalarType::Utf8 | ScalarType::Keyword | ScalarType::Text => std::mem::size_of::<String>(),
    }
}

/// Sort a chunk of `(number, payload)` rows by number and hand each to `on_row` with its ordinal,
/// which is its number, or `None` for a number outside the build's `n` items: whether that is an
/// error, and which error, is the calling pass's decision.
///
/// **Stable**, so rows carrying the same number reach `on_row` in the order the file gave them,
/// and a later row's value is the one an entity keeps (§7's last-write-wins). Sorted so that each
/// caller walks the ordinal-indexed files it writes forwards. The chunk is drained; capacity is
/// retained for reuse.
fn join_chunk<P: Copy + Send>(
    chunk: &mut Vec<(u64, P)>,
    n: u64,
    mut on_row: impl FnMut(Option<u32>, u64, P) -> Result<()>,
) -> Result<()> {
    chunk.par_sort_by_key(|entry| entry.0);
    for &(id, payload) in chunk.iter() {
        on_row((id < n).then_some(id as u32), id, payload)?;
    }
    chunk.clear();
    Ok(())
}

/// Resolve a chunk of `(source_id, source_term)` rows to packed `ordinal << 32 | term` values
/// by **two sorted merges** — the ordinal join ([`join_chunk`]) and then a term-sorted sweep
/// over the dictionary's `term_keys` — never a per-row probe: at T = 117M a per-row hash or
/// binary-search lookup is a random walk over gigabytes, which is the cost class this pipeline
/// exists to avoid. `resolved` is caller-owned scratch, reused across chunks.
///
/// Chunk-order insensitivity: `emit` receives values in term-sorted chunk order; every consumer
/// sorts or is commutative, so no order is observable downstream.
fn resolve_pairs_chunk(
    chunk: &mut Vec<(u64, u64)>,
    resolved: &mut Vec<(u64, u64)>,
    n: u64,
    term_keys: &[u64],
    term_ids: &[u32],
    distinct_of_ordinal: Option<&mut [u32]>,
    mut emit: impl FnMut(u64) -> Result<()>,
) -> Result<()> {
    resolved.clear();
    join_chunk(chunk, n, |ordinal, _, source_term| {
        // Established by the dictionary pass over this same file; a miss means the file is not
        // the one that pass read.
        let Some(ordinal) = ordinal else {
            return Err(input_changed("the pairs file names an item its first pass did not"));
        };
        resolved.push((source_term, ordinal as u64));
        Ok(())
    })?;
    // Equal (term, ordinal) tuples are bit-identical, so the parallel unstable sort has one
    // output; the sweep cursor then only moves forward.
    resolved.par_sort_unstable();
    // **How many distinct terms this chunk gave each ordinal**, counted here because this is the
    // one place they are sorted and the chunk holds exactly one view's rows (`views.md` §7). The
    // label-agreement refusal in the batch loop is a count identity over these tallies, which a
    // build of one view has no use for; the emit below is deliberately *not* deduplicated, the
    // bucket sweep doing that and the pair total being checked against the dictionary pass's own
    // row count.
    if let Some(distinct_of_ordinal) = distinct_of_ordinal {
        let mut previous: Option<(u64, u64)> = None;
        for &pair in resolved.iter() {
            if previous != Some(pair) {
                let slot = &mut distinct_of_ordinal[pair.1 as usize];
                *slot = slot.saturating_add(1);
                previous = Some(pair);
            }
        }
    }
    let mut i = 0usize;
    for &(source_term, ordinal) in resolved.iter() {
        while i < term_keys.len() && term_keys[i] < source_term {
            i += 1;
        }
        if i >= term_keys.len() || term_keys[i] != source_term {
            return Err(input_changed(&format!(
                "the pairs file names term {source_term}, which its first pass did not"
            )));
        }
        emit((ordinal << 32) | term_ids[i] as u64)?;
    }
    resolved.clear();
    Ok(())
}

/// Where resolved pairs accumulate before the batch loop: in RAM when the plan proved the
/// whole relation fits (the historical single-batch path — same allocation, same lifecycle),
/// spilled to one file per batch otherwise.
enum BucketSink {
    Ram(Vec<u64>),
    Files {
        writers: Vec<spill::SpillWriter>,
        batch_items: u64,
    },
}

impl BucketSink {
    fn push(&mut self, value: u64) -> Result<()> {
        match self {
            BucketSink::Ram(vec) => {
                vec.push(value);
                Ok(())
            }
            BucketSink::Files {
                writers,
                batch_items,
            } => {
                let batch = ((value >> 32) / *batch_items) as usize;
                writers[batch].push(value).map_err(BuildError::from)
            }
        }
    }

    fn finish(self) -> Result<BucketStore> {
        match self {
            BucketSink::Ram(vec) => Ok(BucketStore::Ram(Some(vec))),
            BucketSink::Files { writers, .. } => Ok(BucketStore::Files(
                writers
                    .into_iter()
                    .map(|w| w.finish().map(Some))
                    .collect::<mosaica_store::Result<_>>()?,
            )),
        }
    }
}

enum BucketStore {
    Ram(Option<Vec<u64>>),
    Files(Vec<Option<SpillReceipt>>),
}

impl BucketStore {
    /// Batch `k`'s packed values, moved out (RAM) or read and integrity-verified (file).
    fn load(&mut self, k: u64) -> Result<Vec<u64>> {
        match self {
            BucketStore::Ram(slot) => {
                debug_assert_eq!(k, 0, "the RAM backing exists only for a single batch");
                slot.take()
                    .ok_or_else(|| BuildError::Invalid("bucket 0 loaded twice".into()))
            }
            BucketStore::Files(receipts) => {
                let receipt = receipts[k as usize]
                    .as_ref()
                    .ok_or_else(|| BuildError::Invalid(format!("bucket {k} loaded twice")))?;
                Ok(spill::read_bucket(receipt)?)
            }
        }
    }

    /// Release batch `k`'s backing (deletes the spill file; no-op for RAM, whose vector was
    /// moved out by `load`).
    fn delete(&mut self, k: u64) -> Result<()> {
        if let BucketStore::Files(receipts) = self {
            if let Some(receipt) = receipts[k as usize].take() {
                std::fs::remove_file(&receipt.path)
                    .map_err(|e| BuildError::io(&receipt.path, e))?;
            }
        }
        Ok(())
    }
}

/// Everything the memory budget decides, decided once and printed. See `BuildArgs::batch_items`
/// for why the batch size is derived deterministically and recorded rather than re-derived.
struct BuildPlan {
    /// The memory budget this plan was derived under — the operator's `--memory-budget` or the
    /// detected one. Carried rather than re-detected, so every stage that sizes itself against the
    /// budget sizes against the *same* number: `detect_memory_budget` reads `MemAvailable`, which
    /// falls as the build fills memory, and a second reading late in the run would derive a
    /// smaller budget from the build's own success at using the first.
    budget: u64,
    /// Which string columns spill their characters as record-blob extents rather than filling an
    /// entity-ordered arena. Derived from the free space rather than from the budget beside it,
    /// and the one choice in this plan that changes no byte of the bundle ([`ColumnRoutes`],
    /// [`crate::residency::plan_routes`]).
    routes: ColumnRoutes,
    batch_items: u64,
    batches: u64,
    bucket_in_ram: bool,
    /// Term-id band ranges `[lo, hi)`, covering `0..term_count`, each band's pre-dedup rows
    /// within the flat budget (a term never splits across bands).
    band_bounds: Vec<(u32, u32)>,
    /// What provenance records: the batch size iff the build actually batched.
    recorded_batch_items: Option<u64>,
    /// The publication batch, the unique spill, the term-image workers and the term indexes'
    /// allowances, each sized to what its stage has left of the budget
    /// ([`crate::residency::Sizing`]).
    sizing: crate::residency::Sizing,
}

impl BuildPlan {
    /// A term index's allowance from the sizing, which is `None` only where no column takes the
    /// pass and so nothing reads it: the pass's floor, in that case.
    fn term_index_allowance(
        &self,
        sized: Option<mosaica_filter_write::Allowance>,
    ) -> mosaica_filter_write::Allowance {
        sized.unwrap_or_else(|| {
            mosaica_filter_write::Allowance::new(0, 0, rayon::current_num_threads())
        })
    }
}

/// The budget when none is given: what this process plans to take
/// ([`mosaica_types::process::memory_to_take`]), clamped so a tiny CI box still gets a workable
/// floor.
pub(crate) fn detect_memory_budget() -> u64 {
    const FALLBACK: u64 = 24 << 30;
    match mosaica_types::process::memory_to_take() {
        Some(bound) => bound.clamp(2 << 30, 1 << 40),
        None => FALLBACK,
    }
}

/// Free bytes on the filesystem holding `path`, or `None` where unknowable — the disk
/// pre-flight then simply does not run, rather than refusing builds on a guess.
#[cfg(unix)]
pub(crate) fn available_disk(path: &std::path::Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    Some(stats.f_bavail as u64 * stats.f_frsize as u64)
}

#[cfg(not(unix))]
pub(crate) fn available_disk(_path: &std::path::Path) -> Option<u64> {
    None
}

/// The auto-batching grid: derived batch sizes are multiples of 2^24 items, so small budget
/// differences between machines derive the same size — an accidental identity fork needs a
/// budget step of a whole grid cell, not a few megabytes.
const BATCH_GRID: u64 = 1 << 24;

#[allow(clippy::too_many_arguments)]
fn plan_build(
    args: &BuildArgs,
    n: u64,
    route: crate::ExtentRoute,
    numbers_bytes: u64,
    pair_rows: usize,
    row_counts: &[u64],
    histogram: &[u64],
    histogram_shift: u32,
) -> Result<BuildPlan> {
    let budget = args.memory_budget.unwrap_or_else(detect_memory_budget);

    // ---- the other peak: everything the batch loop below does not hold ----------------------
    // **The batch stride reaches none of it** (`residency.rs`): a column in entity order is `n`
    // values by construction and a member table is its own size, so there is no smaller plan to
    // fall back to and the honest answer is a refusal with the arithmetic printed. This is the half
    // `--memory-budget` did not reach — the campaign's builds were OOM-killed at 47.3–47.6 GB under
    // a 12 GB budget, three runs and one number, because the flag only ever sized the loop.
    //
    // Checked **before** the batch plan, and refused rather than warned: this is the larger term
    // and the one no stride can move, so an operator reading a refusal should read this one first.
    // The plan below already refuses an infeasible stride, and a budget the operator named is a
    // bound they asked to have enforced. An auto-derived budget is `MemAvailable` damped, so exceeding *that* is the kill
    // this exists to replace.
    // **The string columns' characters are measured once here**, both models wanting them and the
    // sample decoding rows to get them (`residency::payloads_per_item`).
    let payloads = crate::residency::payloads_per_item(args);
    // **The route each string column takes, and the model it settles on.** A column with two
    // routes takes the arena while the entity-order stages have the disk for it and the extents
    // when they do not (`residency::plan_routes`, `build-column-extents.md` §2). The refusal below
    // is not affected by the choice: every column's storage is a mapped term and the refusal counts
    // the anonymous ones.
    let free = available_disk(&args.out);
    let (routes, tail) =
        crate::residency::routes_for(args, n, numbers_bytes, &payloads, free, route, budget);
    report_column_routes(args, &routes, &tail, free);
    let (phase, peak) = tail.memory_peak();
    // Over budget with the stages themselves within it is the term index's floor: its allowance is
    // what the phase leaves, and at least 8 MiB a thread.
    let floor = tail
        .term_index_at(phase)
        .filter(|_| peak > budget && tail.memory_outside_term_indexes(phase) <= budget);
    if let Some(floor) = floor {
        return Err(BuildError::Invalid(format!(
            "this build's {} phase holds about {} MiB beside its term index, which leaves under \
             the {} MiB a term index's pass needs, {} MiB for each of its {} thread(s), of the {} \
             MiB memory budget. Raise --memory-budget, or run on fewer threads \
             (RAYON_NUM_THREADS). Where the bytes are:{}",
            phase.name(),
            tail.memory_outside_term_indexes(phase) >> 20,
            floor.bytes >> 20,
            mosaica_filter_write::WORKER_BYTES_MIN >> 20,
            floor.threads,
            budget >> 20,
            tail.describe_memory(phase)
        )));
    }
    if peak > budget {
        return Err(BuildError::Invalid(format!(
            "this build's entity-order stages need about {} MiB in the {} phase, over the {} MiB \
             memory budget, with every stage that can shrink already at its smallest. A declared \
             column holds one value per item and a member table holds its own rows, so a smaller \
             --batch-items does not help. Raise --memory-budget, drop a member source, or narrow \
             the schema. Where the bytes are:{}",
            peak >> 20,
            phase.name(),
            budget >> 20,
            tail.describe_memory(phase)
        )));
    }
    // **A warning where the model's own error bar reaches the budget**, and not a refusal:
    // `residency.rs` is a lower bound — it enumerates what the stages hold and not what the
    // Parquet readers, the analysers and the allocator hold around them, and the one build it was
    // measured against read 208 MiB against a 338 MiB peak. Refusing on twice the model would
    // block builds that fit; saying nothing leaves the operator with the same silence the campaign
    // met. So the numbers are printed and the decision is theirs — the house rule for a thing that
    // is recoverable and discloses nothing. A term index's allowance is left out of the doubling:
    // it is what the phase leaves of the budget, and a bound its pass keeps rather than a figure
    // the model estimates.
    let (held_phase, held) = crate::residency::Phase::ALL
        .iter()
        .map(|&phase| (phase, tail.memory_outside_term_indexes(phase)))
        .max_by_key(|&(_, bytes)| bytes)
        .expect("Phase::ALL is not empty");
    if held.saturating_mul(2) > budget {
        eprintln!(
            "warning: this build's entity-order stages need at least {} MiB in the {} phase \
             against a {} MiB budget, and that figure is a lower bound — it counts what the stages \
             hold, not the readers and allocator around them (measured at roughly half the real \
             peak). Where the bytes are:{}",
            held >> 20,
            held_phase.name(),
            budget >> 20,
            tail.describe_memory(held_phase)
        );
    }

    // What the stages hold, and the term index's allowance beside it where that phase has one: the
    // allowance is what the phase leaves, so the phase's total would read as the budget.
    let allowance = tail
        .term_index_at(held_phase)
        .map_or(String::new(), |allowance| {
            format!(" and a term index's {} MiB", allowance.bytes >> 20)
        });
    eprintln!(
        "memory: about {} MiB at the {} phase{allowance}, of a {} MiB budget; publication batches \
         of at most {} member entries, a {} MiB unique index spill, {} term-image worker(s)",
        held >> 20,
        held_phase.name(),
        budget >> 20,
        tail.sizing.publication_batch,
        tail.sizing.unique_spill >> 20,
        tail.sizing.term_image_workers
    );
    report_term_indexes(args, n, &payloads, &tail.sizing);
    // The worst batch's pre-dedup pairs for stride `b`, bounded by summing every histogram
    // range a batch window overlaps — conservative by at most the two boundary ranges.
    let prefix: Vec<u64> = std::iter::once(0)
        .chain(histogram.iter().scan(0u64, |acc, &v| {
            *acc += v;
            Some(*acc)
        }))
        .collect();
    let worst_pairs = |b: u64| -> u64 {
        let mut worst = 0u64;
        let mut lo = 0u64;
        while lo < n {
            let hi = (lo + b).min(n);
            let r_lo = (lo >> histogram_shift) as usize;
            let r_hi = (((hi - 1) >> histogram_shift) as usize + 1).min(histogram.len());
            worst = worst.max(prefix[r_hi] - prefix[r_lo]);
            lo = hi;
        }
        worst
    };
    // The batch loop's residency model, stated so a refusal can print it: the batch's packed
    // bucket (8 bytes/pair) + recs (16/item — 12 before decision 0073 added the Morton tiebreak
    // to the sort key, and a model left at 12 would plan a batch the loop cannot hold) +
    // starts (4/item) + long bitset (1/8 per item),
    // beside the loop-wide distinct-terms-per-ordinal tally (4/item over all n), per-term
    // counters (4/term), the join-chunk buffer (which scales down with the corpus, so a tiny test
    // budget stays feasible for a tiny corpus) and a fixed slack for band buffers, decoders and
    // allocator.
    //
    // **The leading `4 * n` was that tally's, and what it now pays for is the assignment walk's
    // entity window.** The tally became a file under `.build-tmp/` on 2026-09-10, as the
    // ordinal→entity map beside it did before, and the term was kept because it feeds `feasible`,
    // `feasible` picks `auto_batch`, and a batch stride partitions entity-id space — so dropping
    // it would give every budget-constrained corpus a different stride and with it a different
    // permanent entity-id assignment, which I9 does not allow to change. The walk now fills a
    // heap `Vec<u32>` of the batch's length and writes it into the entity map in one call, which
    // is `batch_items * 4 ≤ n * 4`: the retained term is that window, named rather than
    // coincidental. Removing it, to buy larger batches, is a separate decision the owner has not
    // taken — ruling 6 of the disk-use campaign, 2026-09-10.
    const SLACK: u64 = 64 << 20;
    let chunk_bytes = 16 * (JOIN_CHUNK_ROWS as u64).min(pair_rows.max(1) as u64);
    let loop_fixed = 4 * n + 4 * row_counts.len() as u64 + chunk_bytes + SLACK;
    let per_batch = |b: u64| 8 * worst_pairs(b) + 16 * b + 4 * (b + 1) + b / 8;
    let feasible = |b: u64| per_batch(b).saturating_add(loop_fixed) <= budget;

    // The largest feasible stride on the grid (or the whole corpus). If even one grid cell is
    // infeasible the corpus cannot be built under this budget, and the refusal states the
    // arithmetic rather than thrashing.
    let auto_batch = if feasible(n) {
        n
    } else {
        let mut b = (n / BATCH_GRID).saturating_mul(BATCH_GRID).max(BATCH_GRID);
        while b > BATCH_GRID && !feasible(b) {
            b -= BATCH_GRID;
        }
        if !feasible(b) {
            return Err(BuildError::Invalid(format!(
                "no feasible signature batch under the {budget}-byte memory budget: even \
                 {BATCH_GRID} items need {} bytes beside the {loop_fixed}-byte loop floor \
                 (n = {n}, pairs = {pair_rows}); raise --memory-budget",
                per_batch(BATCH_GRID)
            )));
        }
        b
    };

    let batch_items = match args.batch_items {
        None => auto_batch,
        Some(b) if b >= n => n,
        Some(b) => {
            if !feasible(b) {
                return Err(BuildError::Invalid(format!(
                    "--batch-items {b} needs {} bytes beside the {loop_fixed}-byte loop \
                     floor, over the {budget}-byte budget; the largest feasible batch is \
                     {auto_batch}",
                    per_batch(b)
                )));
            }
            // Needlessly small batches permanently forfeit posting compression (I9; §11.1
            // §11.1's container model): refuse unless the budget itself is the reason. An
            // operator who wants small batches states the matching budget, which makes the
            // choice deliberate and reproducible.
            if b.saturating_mul(2) < auto_batch {
                return Err(BuildError::Invalid(format!(
                    "--batch-items {b} is far below the {auto_batch} the budget supports; \
                     smaller batches permanently fragment posting runs (§11.1 r23). Pass a \
                     larger --batch-items, or lower --memory-budget to make this size the \
                     derived choice"
                )));
            }
            b
        }
    };
    // **A stride of at least one item**, so an empty corpus divides. `n = 0` derives a batch of
    // zero from every arm above (the whole corpus is the whole corpus), and the batch loop below
    // runs `0..batches` — so the stride only has to be a legal divisor, and the plan it produces
    // is zero batches over zero items.
    let batch_items = batch_items.max(1);
    let batches = n.div_ceil(batch_items);

    // The RAM backing needs the WHOLE relation beside the resolve scan's own residents.
    let bucket_in_ram =
        batches == 1 && (8 * pair_rows as u64).saturating_add(8 * n + loop_fixed) <= budget;

    // Bands: pre-dedup row counts partition term space (post-dedup <= pre-dedup, so a band's
    // flat is always large enough); the floor is the largest single term, which can never
    // split. Flat budget: a quarter of the memory budget at 4 bytes per row, unless the
    // explicit band-size seam overrides it (a corpus small enough to test cannot otherwise
    // force more than one band).
    let max_term_rows = row_counts.iter().copied().max().unwrap_or(0);
    let band_rows_budget = args
        .band_rows
        .unwrap_or(budget / 16)
        .max(max_term_rows)
        .max(1);
    let mut band_bounds: Vec<(u32, u32)> = Vec::new();
    let mut lo = 0u32;
    let mut acc = 0u64;
    for (term, &rows) in row_counts.iter().enumerate() {
        if acc + rows > band_rows_budget && term as u32 > lo {
            band_bounds.push((lo, term as u32));
            lo = term as u32;
            acc = 0;
        }
        acc += rows;
    }
    band_bounds.push((lo, row_counts.len() as u32));

    // ---- the disk pre-flight ------------------------------------------------------------------
    // The build's transient spills and its outputs coexist in phases, so the forecast is the
    // largest phase and not a total nothing ever holds. `residency::disk` names every term and the
    // phases it stands through; nothing is derived twice here.
    //
    // **It warns and does not refuse.** The model can be wrong in either direction — several of its
    // terms are stated ceilings a corpus can exceed, and several corpus shapes cost bytes no term
    // covers — so it is not a figure to hold a door with. A build that runs out of disk writes no
    // `CURRENT`, publishes no identity and mints no id that survives, and the partial prefix is
    // swept, so what a wrong admission costs is the build's wall clock. What a wrong refusal cost
    // was the same wall clock with no way to say "I accept the risk": there is no flag,
    // environment variable or configuration key that moves this figure. That is the house rule for
    // something recoverable that discloses nothing — print the numbers and leave the decision with
    // the operator — and it is the shape the memory model above already uses for its own band.
    let p = pair_rows as u64;
    let corpus = crate::residency::Corpus {
        n,
        pair_rows: p,
        term_rows: row_counts,
        batches,
        bucket_in_ram,
    };
    let disk = crate::residency::disk(args, corpus, &payloads, &tail);
    let (phase, disk_need) = disk.peak();
    // **Printed whatever the free space is.** An operator sizing a corpus has no other way to ask
    // what a build will cost the disk, and the campaign's rung 6 died at hour three on a forecast
    // nobody could read.
    eprintln!(
        "disk: ~{} MiB at peak, in the {} phase{}",
        disk_need >> 20,
        phase.name(),
        crate::residency::Phase::ALL
            .iter()
            .map(|&ph| format!(" ({} {} MiB)", ph.name(), disk.at(ph) >> 20))
            .collect::<String>()
    );
    if let Some(warning) =
        free.and_then(|free| crate::residency::forecast_warning(&disk, corpus, free))
    {
        eprintln!("{warning}");
    }

    Ok(BuildPlan {
        budget,
        routes,
        batch_items,
        batches,
        bucket_in_ram,
        band_bounds,
        recorded_batch_items: (batches > 1).then_some(batch_items),
        sizing: tail.sizing,
    })
}

/// **What each term index's pass may hold, and the cascade its merge is expected to take**, printed
/// beside the memory and disk lines before the passes run. The run count is a forecast from the
/// largest bundle-wide text column's characters ([`crate::residency::expected_text_runs`]); the
/// passes each text index took are printed as it finishes ([`write_text_index`]).
fn report_term_indexes(
    args: &BuildArgs,
    n: u64,
    payloads: &[f64],
    sizing: &crate::residency::Sizing,
) {
    if let Some(allowance) = sizing.term_index {
        let characters = args
            .schema
            .attributes
            .iter()
            .zip(payloads)
            .filter(|(attribute, _)| {
                attribute.ty == ScalarType::Text && postings_are_owed(&args.schema, attribute)
            })
            .map(|(_, &per_item)| crate::residency::payload(per_item, n))
            .max();
        let cascade = match characters {
            Some(characters) => {
                let runs = crate::residency::expected_text_runs(characters, allowance);
                // What each thread of the merge holds beside its readers, the held terms aside.
                let slice = (mosaica_filter_write::TEXT_MERGE_SLICE_CAP * 4) as u64;
                format!(
                    "; a text index over {} MiB of characters spills about {runs} run(s), {} \
                     cascade pass(es) under a fan-in of {}",
                    characters >> 20,
                    allowance.cascade_passes(runs, slice),
                    allowance.fan_in(slice)
                )
            }
            None => String::new(),
        };
        eprintln!(
            "text: each term index may hold {} MiB over {} thread(s), what the index phase \
             leaves of the budget{cascade}",
            allowance.bytes >> 20,
            allowance.threads
        );
    }
    if let Some(allowance) = sizing.scoped_term_index {
        eprintln!(
            "text: each group-scoped term index may hold {} MiB over {} thread(s), what the join \
             phase leaves of the budget",
            allowance.bytes >> 20,
            allowance.threads
        );
    }
}

/// **Printed, because the route the build took is what makes two runs' timings comparable.**
///
/// It is derived from the space free on the output filesystem, so a box that fills up can route a
/// corpus one way today and the other way tomorrow. Nothing downstream can see the difference —
/// the bundle is byte-identical either way — but the build's own wall clock moves with it, 5 to
/// 10% at the row counts measured, and a run whose log does not say which route it took cannot be
/// read against another's.
fn report_column_routes(
    args: &BuildArgs,
    routes: &ColumnRoutes,
    tail: &crate::residency::Residency,
    free: Option<u64>,
) {
    let available = args
        .schema
        .attributes
        .iter()
        .filter(|a| may_take_extents(&args.schema, a))
        .count();
    if available == 0 {
        return;
    }
    let spilled = routes.spilled();
    eprintln!(
        "columns: {} of {available} string column(s) spill their characters as record-blob \
         extents, the entity-order stages modelling {} MiB of scratch against {} free{}",
        spilled.len(),
        crate::residency::stage_scratch(tail) >> 20,
        match free {
            Some(bytes) => format!("{} MiB", bytes >> 20),
            None => "an unreadable amount of space".to_string(),
        },
        match spilled.is_empty() {
            true => String::new(),
            false => format!(
                ": {}",
                spilled
                    .iter()
                    .map(|&index| {
                        let attribute = &args.schema.attributes[index];
                        let why = match extents_are_forced(&args.schema, attribute) {
                            true => "text",
                            false => "no room for its arena",
                        };
                        format!("{} ({why})", attribute.name)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    );
}

/// Run the build, and **take the partial prefix with it if it does not finish**.
///
/// `.build-tmp/` has always gone back on the way out ([`spill::TmpDir`]), and the prefix under
/// `<out>/` did not — so a build that ran out of disk left its bytes behind and the retry started
/// with less free space than the first attempt had. The rung-6 attempt died at hour three with
/// 93 GB of bundle written. Nothing published it: `CURRENT` is written last and
/// [`crate::validate_args`] refuses to start where one already exists, so a prefix here is a
/// prefix no reader can resolve, which is the proof
/// [`mosaica_store::reclaim_unpublished_prefix`] makes for itself before deleting anything.
///
/// That refusal is why the argument check runs here rather than inside [`build_bundle`]. It is
/// the one error that fires while `<out>/CURRENT` exists, which is the one state the sweep
/// refuses to delete in, so leaving it under the sweep would print a warning naming a published
/// bundle on every build into a directory that already holds one.
///
/// **A failure to sweep is a warning and nothing else.** The build has already failed and the
/// error the caller gets is the one worth having; a tree that cannot be removed is the residual
/// that stood before this existed.
pub(crate) fn build(
    args: &BuildArgs,
    frames: &[crate::Framing],
    observer: &dyn BuildObserver,
    route: crate::ExtentRoute,
) -> Result<BuildReport> {
    // **First, before any validation**, so a run whose log is all that survives says which source
    // produced it. A campaign of 2026-09-12 lost two and three-quarter hours to a binary seven
    // commits behind the tree its figures were read against.
    eprintln!("mosaica build: commit {}", crate::BUILD_COMMIT);
    validate_args(args)?;
    let outcome = build_bundle(args, frames, observer, route);
    if outcome.is_err() {
        let prefix = args.out.join(PREFIX);
        if prefix.is_dir() {
            match mosaica_store::reclaim_unpublished_prefix(&prefix) {
                Ok(()) => eprintln!(
                    "swept the partial bundle at {}: this build wrote no CURRENT, so nothing \
                     names it and a retry has the disk back",
                    prefix.display()
                ),
                Err(e) => eprintln!(
                    "warning: the partial bundle at {} could not be swept ({e}); it stands, and \
                     its disk with it",
                    prefix.display()
                ),
            }
        }
    }
    outcome
}

/// One shard's part of a view, its rows partitioned and its segment not yet written.
struct ShardRows<'a> {
    span: &'a crate::ShardSpan,
    view_dir: PathBuf,
    segment_dir: PathBuf,
    label: String,
    rows: crate::assembly::PartitionedRows,
    boundaries: crate::assembly::MortonBoundaries,
    page_plan: mosaica_store::write::PagePlan,
}

fn build_bundle(
    args: &BuildArgs,
    frames: &[crate::Framing],
    observer: &dyn BuildObserver,
    route: crate::ExtentRoute,
) -> Result<BuildReport> {
    let mut timer = StageTimer::new(observer);
    let steps = timer.steps();
    timer.begin(BuildStage::SourceIds, &[], None);

    // ---- 1. pass zero: which item each row of each file names (`crate::ids`) -----------
    // An item's *ordinal* is the number the identity rule gave it, in creation order: the order
    // the linear build walks items in, so "first appearance" below, and the tiebreak in the
    // signature sort, are both ordinal comparisons.
    //
    // **`n = 0` is a bundle, not a refusal** (decision 0091): a deployment must be able to start
    // from a bundle with no points, with its frame stated, and take the whole corpus through
    // `/control/ingest`. Every stage below is walked for that case — empty segments, empty
    // postings, a dictionary holding only what the declaration mints, a high water of 0,
    // every declared column present and empty, every declared layer registered with no artifacts.
    // What is still refused is `extent = "auto"` over no rows, because a frame cannot be fitted to
    // nothing (`config::empty_auto_source`) — and that refusal already names the remedy.
    // Open here rather than at the bucket sink below, because the first thing under it is the
    // identity pass: `.build-tmp/` is the build's scratch from pass zero to the segment write, and
    // every structure in it is swept by the `close` at the end or by the next build's `create`.
    let tmp = spill::TmpDir::create(&args.out)?;
    crate::residency::report_identity_disk(args, available_disk(&args.out))?;
    let mut numbering = crate::ids::number_streaming(args, tmp.path(), steps)?;
    let n = numbering.items;
    crate::config::require_sources(&args.schema, &args.attribute_sources, n > 0)?;
    let refused = std::mem::take(&mut numbering.refused);
    // The frames fitted over the rows the pass kept, which every view reads from here on.
    let fitted;
    let args = match frames {
        [] => args,
        frames => {
            // The files' steps are over, and fitting the frames is the stage's own.
            steps.begin(BuildStage::SourceIds, &[], None);
            fitted = crate::fit_frames(args, frames, &numbering)?;
            &fitted
        }
    };
    // Each view's own anchors — how many of its rows name an item and an order-independent mixed
    // sum of their numbers ([`splitmix64`]) — are on its numbers, and the geometry pass below is
    // checked against them: a points file swapped mid-build would otherwise hand every item of
    // that view another item's position, with nothing to notice.

    timer.end(BuildStage::SourceIds, n);
    timer.begin(BuildStage::Dictionary, &[], None);
    // What the reading stages count, from the files' footers, read only for a reader.
    let mut source_rows = None;
    steps.count(Unit::Rows, || {
        let rows = crate::SourceRows::of(args, &numbering)?;
        let access = rows.access;
        source_rows = Some(rows);
        Ok(access)
    })?;
    let rows_of = |rows: fn(&crate::SourceRows) -> u64| source_rows.as_ref().map(rows);
    // The shards the items fill, in allocation order. Everything up to the
    // assignment walk is global; everything after it that writes entity space writes each
    // shard's files at its own ids.
    let spans = crate::ShardSpan::of(n, args.shard_size())?;
    // The shards whose base files the stages below write. An empty build's one shard is written
    // whole by `mosaica_store::empty_shard`, the writer a running service opens a shard with.
    let filled: &[crate::ShardSpan] = match n {
        0 => &[],
        _ => &spans,
    };

    // ---- 2. the dictionary -----------------------------------------------------------
    let prefix_dir = args.out.join(PREFIX);
    let dict_dir = prefix_dir.join(mosaica_store::layout::DICTIONARY_DIR);
    std::fs::create_dir_all(&dict_dir).map_err(|e| BuildError::io(&dict_dir, e))?;
    // What every source term is called, established before any term id exists — a field-sourced
    // view's sorted vocabulary, or the relation's own integers (`crate::AccessPlan`).
    let access = crate::plan_access(args, &numbering)?;
    // Whether the label-agreement identity applies at all: a shared relation is entity space and
    // is scanned once, so its rows cannot disagree between views (`crate::AccessRoute`).
    let per_view_labels = !matches!(access.descriptors, crate::input::TermDescriptors::Ids);
    let Dictionary {
        term_keys,
        term_ids,
        row_counts,
        term_count,
        pair_rows,
        histogram,
        histogram_shift,
        dict_paths,
    } = build_dictionary(args, &access, n, &dict_dir, &numbering, steps)?;
    if term_count >= u32::MAX as u64 {
        return Err(BuildError::Invalid(format!(
            "{term_count} distinct terms exceeds the 2^32 term-ID space"
        )));
    }

    timer.end(BuildStage::Dictionary, term_count);
    timer.begin(
        BuildStage::PairsPack,
        &[],
        rows_of(|rows| rows.access).map(|rows| (rows, Unit::Rows)),
    );

    // ---- 2b. the plan: batch size, bucket backing, band boundaries, pre-flight --------
    // Everything the budget arithmetic decides, decided in one place and printed — the batch
    // size is identity-bearing (I9), so it is derived deterministically here, recorded in
    // provenance when it batches, and never silently re-derived on a rebuild (the CLI replays
    // a carried bundle's recorded value).
    let numbers_bytes = numbering.disk_bytes();
    let plan = plan_build(
        args,
        n,
        route,
        numbers_bytes,
        pair_rows,
        &row_counts,
        &histogram,
        histogram_shift,
    )?;
    drop(histogram);
    if plan.batches > 1 {
        eprintln!(
            "batching: {} batches of <= {} items (signature order is per-batch — \u{a7}11.1 r23; \
             recorded in provenance; a rebuild preserving this identity must replay it)",
            plan.batches, plan.batch_items
        );
    }

    // ---- 3. the pairs relation, packed as `ordinal << 32 | term_id`, into buckets -----
    // One bucket per batch — in RAM when the plan says the whole relation fits (the historical
    // single-batch path, bit for bit), spilled per batch otherwise. Chunk-order insensitivity
    // ([`join_chunk`]): every bucket is sorted and deduplicated before anything reads it, so
    // the push order never reaches the output.
    let mut sink = if plan.bucket_in_ram {
        BucketSink::Ram(Vec::with_capacity(pair_rows))
    } else {
        let mut writers = Vec::with_capacity(plan.batches as usize);
        for k in 0..plan.batches {
            writers.push(spill::SpillWriter::create(
                &tmp.path().join(format!("bucket-{k}.u64")),
            )?);
        }
        BucketSink::Files {
            writers,
            batch_items: plan.batch_items,
        }
    };
    let mut pushed = 0u64;
    let mut failure: Option<BuildError> = None;
    let mut chunk: Vec<(u64, u64)> = Vec::with_capacity(JOIN_CHUNK_ROWS.min(pair_rows.max(1)));
    let mut resolved: Vec<(u64, u64)> = Vec::new();
    // Each view's own distinct contribution per ordinal, summed over the views — the numerator
    // of the label-agreement identity the batch loop checks (`views.md` §7).
    //
    // **File-backed**, which is [`spill::MappedArray`]'s case exactly: written at a scattered
    // index by the sweep in [`resolve_pairs_chunk`], read in ordinal order by the
    // label-agreement pass the batch loop runs ahead of its assignment walk, never sorted. At 4 B/item it was the build's largest anonymous
    // structure — 13.0 GiB at the GBIF rung's 3.50×10⁹ items (modelled, items × 4 B) — and mapped
    // it is page cache the kernel may evict rather than memory the machine must have.
    // `plan_build`'s `loop_fixed` charges the same 4 B/item still, on purpose and for I9's sake:
    // the comment at that term says why.
    //
    // **Only where two views can disagree.** One view's labels are the item's, so a build of one
    // view, and one whose labels come from a shared relation, writes neither this nor the
    // appearances beside the geometry.
    let agreement = per_view_labels && args.views.len() > 1;
    let mut distinct_map = match agreement {
        true => spill::MappedU32::zeroed(tmp.path(), "distinct-of-ordinal.u32", n as usize)?,
        false => spill::MappedU32::empty(),
    };
    let distinct_of_ordinal = distinct_map.as_mut_slice();
    let mut resolve = |chunk: &mut Vec<(u64, u64)>,
                       resolved: &mut Vec<(u64, u64)>,
                       distinct_of_ordinal: &mut [u32],
                       sink: &mut BucketSink| {
        resolve_pairs_chunk(
            chunk,
            resolved,
            n,
            &term_keys,
            &term_ids,
            agreement.then_some(distinct_of_ordinal),
            |value| {
                pushed += 1;
                sink.push(value)
            },
        )
    };
    let mut current: Option<(usize, u64)> = None;
    crate::scan_access(
        args,
        &access,
        &numbering,
        steps,
        |view, source_id, source_term| {
            // A chunk **never spans two views**, and never splits a row: rows of one view arrive
            // contiguously (a view holds one row per entity), so the boundary is taken at the change
            // of view — always — or at the next change of entity once the chunk is full.
            let changed_view = current.is_some_and(|(previous, _)| previous != view);
            let changed_row = current != Some((view, source_id));
            if changed_view || (changed_row && chunk.len() >= JOIN_CHUNK_ROWS) {
                if let Err(e) = resolve(&mut chunk, &mut resolved, distinct_of_ordinal, &mut sink) {
                    failure = Some(e);
                    return ControlFlow::Break(());
                }
            }
            current = Some((view, source_id));
            chunk.push((source_id, source_term));
            ControlFlow::Continue(())
        },
    )?;
    if let Some(error) = failure {
        return Err(error);
    }
    resolve(&mut chunk, &mut resolved, distinct_of_ordinal, &mut sink)?;
    drop(chunk);
    drop(resolved);
    if pushed as usize != pair_rows {
        return Err(input_changed(&format!(
            "the pairs file yielded {pair_rows} rows, then {pushed}"
        )));
    }
    drop(term_keys);
    drop(term_ids);
    drop(row_counts);
    let mut store = sink.finish()?;
    timer.end(BuildStage::PairsPack, pushed);
    timer.begin(
        BuildStage::GeometryRead,
        &[],
        rows_of(|rows| rows.views).map(|rows| (rows, Unit::Rows)),
    );
    // ---- 3b. geometry, by ordinal, once per (item, view) (`views.md` §7) --------------
    // **Every view's geometry is read exactly once, and it is read here** — before entity ids
    // exist, so it lands in *ordinal* space. Pass two permutes it into entity space rather than
    // re-reading a parquet file per view: the transform (project, then quantise against that
    // view's own frame) runs once per (item, view) and nowhere else.
    //
    // The pass exists because decision 0073 breaks signature ties on the Morton code, so the sort
    // needs geometry it previously did not, and decision 0112 says *which* view's: the declared
    // anchor's, with the first-declared view that holds the item standing in where the anchor
    // does not.
    //
    // **Mapped rather than heap-allocated**, on step 8's own argument: 4 B per item per axis is
    // 8 GB at 10⁹ of memory the kernel cannot reclaim. See [`spill::MappedU32`] — the bytes become
    // page cache, so a machine short of RAM pages instead of failing. They are deliberately outside
    // [`plan_build`]'s residency model, which counts the anonymous allocations a batch must hold.
    //
    // The two integrity checks are the ones step 8 used to make, moved with the read: a shrunk file
    // resolves every id it still presents and would otherwise leave the missing items at (0, 0)
    // with no error, and a count alone accepts a repeat that compensates a removal ({1,2,3} become
    // {2,2,2}), so the multiset of ids must be that view's first pass's.
    let mut geometry: Vec<ViewGeometry> = Vec::with_capacity(args.views.len());
    // How many of this build's views hold each item — the denominator of the label-agreement
    // identity below, and the population of each view's permutation.
    //
    // **A mapped file**, on the same argument as the geometry beside it: 4 B an item is 13.3 GiB
    // at the GBIF rung, held from here to the end of the batch loop, and as anonymous memory it
    // was the largest term of that loop's residency that no model named.
    let mut appearances = match agreement {
        true => spill::MappedU32::zeroed(tmp.path(), "appearances.u32", n as usize)?,
        false => spill::MappedU32::empty(),
    };
    for (index, view) in args.views.iter().enumerate() {
        let mut x_map =
            spill::MappedU32::zeroed(tmp.path(), &format!("x-of-ordinal-{index}.u32"), n as usize)?;
        let mut y_map =
            spill::MappedU32::zeroed(tmp.path(), &format!("y-of-ordinal-{index}.u32"), n as usize)?;
        let mut present: Vec<u64> = vec![0; (n as usize).div_ceil(64)];
        {
            let xs = x_map.as_mut_slice();
            let ys = y_map.as_mut_slice();
            let apps = appearances.as_mut_slice();
            let mut points_seen = 0u64;
            let mut geom_anchor = 0u64;
            let mut chunk: Vec<(u64, (u32, u32))> =
                Vec::with_capacity(JOIN_CHUNK_ROWS.min(n as usize));
            let mut failure: Option<BuildError> = None;
            let resolve = |chunk: &mut Vec<(u64, (u32, u32))>,
                           xs: &mut [u32],
                           ys: &mut [u32],
                           present: &mut [u64],
                           appearances: &mut [u32],
                           points_seen: &mut u64,
                           geom_anchor: &mut u64| {
                join_chunk(chunk, n, |ordinal, source_id, (x, y)| {
                    let Some(ordinal) = ordinal else {
                        return Err(input_changed(
                            "the points file names an item its first pass did not",
                        ));
                    };
                    xs[ordinal as usize] = x;
                    ys[ordinal as usize] = y;
                    bit_set(present, ordinal as usize);
                    if let Some(count) = appearances.get_mut(ordinal as usize) {
                        *count += 1;
                    }
                    *points_seen += 1;
                    *geom_anchor = geom_anchor.wrapping_add(splitmix64(source_id));
                    Ok(())
                })
            };
            input::scan_points(
                crate::view_source(args, index, &numbering).counted(steps),
                view.projection,
                &view.extent,
                |point| {
                    chunk.push((point.source_id, (point.qx, point.qy)));
                    if chunk.len() == JOIN_CHUNK_ROWS {
                        if let Err(e) = resolve(
                            &mut chunk,
                            xs,
                            ys,
                            &mut present,
                            apps,
                            &mut points_seen,
                            &mut geom_anchor,
                        ) {
                            failure = Some(e);
                            return ControlFlow::Break(());
                        }
                    }
                    ControlFlow::Continue(())
                },
            )?;
            if let Some(error) = failure {
                return Err(error);
            }
            resolve(
                &mut chunk,
                xs,
                ys,
                &mut present,
                apps,
                &mut points_seen,
                &mut geom_anchor,
            )?;
            let anchor = numbering.points(index);
            if points_seen != anchor.named {
                return Err(input_changed(&format!(
                    "view '{}': the points file yielded {points_seen} geometry rows, but the \
                     identity pass numbered {}",
                    view.view_id, anchor.named
                )));
            }
            if geom_anchor != anchor.mixed {
                return Err(input_changed(&format!(
                    "view '{}': the points file's geometry pass carries different ids than its \
                     first pass did (row count unchanged)",
                    view.view_id
                )));
            }
        }
        geometry.push(ViewGeometry {
            x: x_map,
            y: y_map,
            present,
            rows: numbering.points(index).named,
        });
    }
    // **The anchor's Morton code, with the declared fallback** (decision 0112): an item absent
    // from the anchor takes its code in the first-declared view that holds it. Materialised here
    // rather than chosen inside the sort, so the batch loop reads one array and the choice is
    // made once per item.
    //
    // **Where the anchor holds every item there is nothing to materialise**, the fallback reaching
    // no ordinal and the array it would write being the anchor view's own, value for value. A
    // view's ids are distinct (pass one checks each view's for duplicates as it fills its segment),
    // so a view whose row count is the union's is a view holding every ordinal. That is every
    // single-view corpus and every multi-view one whose anchor covers the union — 8 B/item of
    // reserved disk and an `n`-length copy, both for a file byte-identical to one already written.
    // A build of no view has no anchor, and no item to order.
    let anchor = geometry.get(args.anchor);
    let anchor_fallback_reaches_an_item = anchor.map_or(0, |g| g.rows) != n;
    let anchor_maps = if anchor_fallback_reaches_an_item {
        let mut anchor_x_map = spill::MappedU32::zeroed(tmp.path(), "x-anchor.u32", n as usize)?;
        let mut anchor_y_map = spill::MappedU32::zeroed(tmp.path(), "y-anchor.u32", n as usize)?;
        {
            let xs = anchor_x_map.as_mut_slice();
            let ys = anchor_y_map.as_mut_slice();
            let order: Vec<usize> = std::iter::once(args.anchor)
                .chain((0..args.views.len()).filter(|&v| v != args.anchor))
                .collect();
            for (ordinal, (x, y)) in xs.iter_mut().zip(ys.iter_mut()).enumerate() {
                let held = order
                    .iter()
                    .copied()
                    .find(|&v| bit_get(&geometry[v].present, ordinal))
                    .expect("the union of the views' ids is where this ordinal came from");
                *x = geometry[held].x.as_slice()[ordinal];
                *y = geometry[held].y.as_slice()[ordinal];
            }
        }
        Some((anchor_x_map, anchor_y_map))
    } else {
        None
    };
    let (x_of_ordinal, y_of_ordinal): (&[u32], &[u32]) = match (&anchor_maps, anchor) {
        (Some((x, y)), _) => (x.as_slice(), y.as_slice()),
        (None, Some(anchor)) => (anchor.x.as_slice(), anchor.y.as_slice()),
        (None, None) => (&[], &[]),
    };
    timer.end(BuildStage::GeometryRead, n);
    // What precedes the batch loop is the first batch's sort, or the assignment's where a build of
    // no items has no batch.
    let batch = |k: u64| Part::new("batch", "", k as usize, plan.batches as usize);
    match plan.batches {
        0 => timer.begin(BuildStage::Assignment, &[], None),
        _ => timer.begin(BuildStage::SignatureSort, &[batch(0)], None),
    }

    // ---- 4–5. per batch: sort, signature-refine, assign, emit bands (§11.1) -----------
    // Entity id = batch base + position in the batch's signature order. Batches partition
    // ordinal space, so per-batch sort+dedup of the packed relation equals the historical
    // global sort+dedup (a duplicate pair shares its ordinal, hence its batch), and one batch
    // covering everything reproduces the pre-batching assignment exactly.
    //
    // **File-backed**, which is [`spill::MappedArray`]'s case exactly: written once at a scattered
    // index by the walk below, read once at a scattered index by every pass after it, never
    // sorted. 0.87 GiB at rung 5's 2.33×10⁸ items and 13.2 GiB at the GBIF rung's 3.54×10⁹
    // (modelled, items × 4 B), alive from here to the last view's permutation. `entities` is the
    // assignment walk's writable binding; every reader after the walk takes the shared slice
    // bound below.
    let mut entity_map = spill::MappedU32::zeroed(tmp.path(), "entity-of-ordinal.u32", n as usize)?;
    let entities = entity_map.as_mut_slice();
    // Exact per-term post-dedup counts, accumulated as bands are emitted; drives the band
    // sweep's offsets. u32 is sound (a term's entities are distinct, so count <= n < 2^32).
    let mut term_counts: Vec<u32> = vec![0; term_count as usize];
    let mut band_writers: Vec<spill::BandWriter> = plan
        .band_bounds
        .iter()
        .enumerate()
        .map(|(j, &(lo, _))| {
            spill::BandWriter::create(&tmp.path().join(format!("band-{j}.pairs")), lo)
        })
        .collect::<Result<_>>()?;
    let band_los: Vec<u32> = plan.band_bounds.iter().map(|&(lo, _)| lo).collect();
    let mut pair_count = 0u64;
    let mut over_bound_items = 0u64;
    let mut entity_base = 0u64;
    // The entity→term transpose (contracts §2.4), written in the same walk that assigns entity
    // ids: entities ascend with position within a batch and bases ascend across batches, so this
    // loop already visits them in the strictly ascending order the writer requires, and each
    // item's `sig` is already its sorted, deduplicated term list. Writing it here rather than from
    // the bands costs no second pass and no second relation in memory.
    //
    // One transpose per shard, each opened as the walk reaches the shard's first entity.
    let transpose = |e| BuildError::Invalid(format!("entity-terms transpose: {e}"));
    let entity_terms_writer = |span: &crate::ShardSpan| {
        mosaica_store::EntityTermsWriter::create(&prefix_dir.join(span.layout().entity_terms_rel()))
            .map_err(transpose)
    };
    let mut entity_terms_paths: Vec<PathBuf> = Vec::new();
    let mut entity_terms_shard = 0usize;
    let mut entity_terms = filled.first().map(entity_terms_writer).transpose()?;
    let mut sig_terms: Vec<u32> = Vec::new();
    for k in 0..plan.batches {
        if k > 0 {
            timer.begin(BuildStage::SignatureSort, &[batch(k)], None);
        }
        let ordinal_lo = k * plan.batch_items;
        let ordinal_hi = ((k + 1) * plan.batch_items).min(n);
        let batch_len = (ordinal_hi - ordinal_lo) as usize;
        let mut packed = store.load(k)?;
        // A total order on (at worst bit-identical) u64s: one output under the parallel
        // unstable sort. The label set is a *set*: a repeated input row must not become a
        // repeated posting (per-batch dedup == global dedup, ordinals partition by batch).
        packed.par_sort_unstable();
        packed.dedup();
        pair_count += packed.len() as u64;

        // Per-ordinal signature starts (u32: the plan caps any batch's pairs well below
        // 2^32), the long-signature bitset, and the pre-sort keys — today's stage 4 over one
        // batch, with `starts` subsuming the old long-only start index because the band emit
        // below needs every item's view, not only the long ones.
        let mut starts: Vec<u32> = Vec::with_capacity(batch_len + 1);
        let mut long_sig: Vec<u64> = vec![0; batch_len.div_ceil(64)];
        let mut recs: Vec<SortRec> = Vec::with_capacity(batch_len);
        let mut cursor = 0usize;
        for local in 0..batch_len {
            let ordinal = ordinal_lo + local as u64;
            starts.push(cursor as u32);
            let start = cursor;
            while cursor < packed.len() && (packed[cursor] >> 32) == ordinal {
                cursor += 1;
            }
            let sig = &packed[start..cursor];
            if sig.len() > mosaica_authz::MAX_KEYS_PER_ITEM {
                // A declared bound is a *declaration*: record it and carry on. Dropping
                // terms here would silently widen the item's visibility (I2/I3).
                over_bound_items += 1;
            }
            if sig.len() > 2 {
                bit_set(&mut long_sig, local);
            }
            // `term + 1` so that "no term at this position" (0) sorts before every real
            // term, which is what makes a signature order before any signature extending it.
            let key: u64 = match sig.len() {
                0 => 0,
                1 => (term_of(sig[0]) as u64 + 1) << 32,
                _ => ((term_of(sig[0]) as u64 + 1) << 32) | (term_of(sig[1]) as u64 + 1),
            };
            recs.push(SortRec {
                key_hi: (key >> 32) as u32,
                key_lo: key as u32,
                // From the quantised form directly, exactly as the tiler does — `split32`'s cell
                // half is the code `morton_of` would give for the same point, so entity order and
                // row order agree about what is nearby. Computed rather than stored: a third
                // mapped array would cost 4 B/item to save one interleave per item.
                morton: split32(
                    x_of_ordinal[ordinal as usize],
                    y_of_ordinal[ordinal as usize],
                )
                .0
                .raw(),
                ordinal: ordinal as u32,
            });
        }
        starts.push(cursor as u32);
        if cursor != packed.len() {
            return Err(input_changed(&format!(
                "bucket {k} holds pairs outside its ordinal range [{ordinal_lo}, {ordinal_hi})"
            )));
        }

        // **The label is the entity's, not the row's** (`views.md` §7): every view holding an
        // item must have given it the same term set. The item's signature is the deduplicated
        // union over the views, and `distinct_of_ordinal` is the sum of each view's own distinct
        // count — so the two agree exactly when every view contributed the whole union, and the
        // identity is a refusal rather than a hash comparison.
        //
        // Checked only where two views can disagree: on the per-view route, over more than one
        // view. A shared relation is entity space already and is scanned once
        // (`crate::AccessRoute`).
        //
        // Here, ahead of the sort and the assignment walk, because it wants only `starts` and the
        // two counters: the three arrays are read in ordinal order rather than the entity order
        // the walk visits records in. Over a 125,789,091-row prefix of the GBIF ladder corpus in
        // three batches of 50,331,648 items, the signature sort and the assignment together
        // measure 333 ns a record against 379 with the check inside the walk (median of five
        // paired runs). ⊘ Prefetching the walk's remaining gather, the ordinal-indexed write and
        // the `starts` read 24 records ahead, measured 35% slower on the same prefix's assignment
        // loop.
        if agreement {
            let appearances = appearances.as_slice();
            let disagreeing = (0..batch_len)
                .filter(|&local| {
                    let ordinal = ordinal_lo as usize + local;
                    let sig_len = (starts[local + 1] - starts[local]) as u64;
                    distinct_of_ordinal[ordinal] as u64 != sig_len * appearances[ordinal] as u64
                })
                .count();
            if disagreeing > 0 {
                {
                    return Err(BuildError::Invalid(format!(
                        "{disagreeing} item(s) carry different access labels in different views. \
                         A label is the item's, not the row's (views §7): it is one set wherever \
                         the item appears, and a re-label is a delete plus a re-ingest. The views \
                         this build reads are {}",
                        args.views
                            .iter()
                            .map(|v| match &v.points {
                                Some(points) => format!("'{}' ({})", v.view_id, points.display()),
                                None => format!("'{}'", v.view_id),
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
            }
        }
        // The triple (key_hi, key_lo, ordinal) is unique per rec — a total order, so the
        // parallel unstable sort has exactly one output; refinement makes it the reference
        // `(signature, source_id)` order.
        recs.par_sort_unstable_by_key(|r| r.order());
        refine_signature_ties(&mut recs, &packed, &starts, ordinal_lo, &long_sig);
        drop(long_sig);
        timer.end(BuildStage::SignatureSort, recs.len() as u64);
        timer.begin(
            BuildStage::Assignment,
            &[batch(k)],
            Some((recs.len() as u64, Unit::Items)),
        );

        // **The batch's slice of the entity map, filled here and written once.** A batch's
        // ordinals are the contiguous range `[ordinal_lo, ordinal_hi)`, and the walk visits them
        // in signature order — so writing each entity straight into the mapping scattered the
        // writes over a 1.5 GB slice of a shared file mapping, and the kernel wrote a page back,
        // write-protected it, and took another fault on the next write to it. Measured at rung 6:
        // 200 to 380 MB/s of writes to grow the bundle at 20, 50,000 to 90,000 minor faults a
        // second, and an assignment stage that rose from 65 s to 233 s across identical batches
        // as the dirty set grew
        // (`docs/evidence/memos/2026-09-12-gbif-whole-corpus-build-observations.md` §1). The
        // window is
        // `batch_items * 4 ≤ n * 4`, which is what `plan_build`'s retained 4 B an item pays for.
        let mut assigned: Vec<u32> = vec![0; batch_len];
        // Assignment, and the band emit in the same walk: entities ascend with position, so
        // every term's entity list arrives ascending — within this batch here, and across
        // batches because bases ascend and the loop is sequential. `encode_posting`'s
        // unconditional sortedness check later re-verifies exactly this property from disk.
        let mut assigned_count = steps.batched();
        for (position, rec) in recs.iter().enumerate() {
            assigned_count.add(1);
            let entity = (entity_base + position as u64) as u32;
            let local = (rec.ordinal as u64 - ordinal_lo) as usize;
            assigned[local] = entity;
            let sig = &packed[starts[local] as usize..starts[local + 1] as usize];
            sig_terms.clear();
            for &value in sig {
                let term = term_of(value);
                let band = band_los.partition_point(|&lo| lo <= term) - 1;
                sig_terms.push(term);
                band_writers[band].push(term, entity)?;
                term_counts[term as usize] =
                    term_counts[term as usize].checked_add(1).ok_or_else(|| {
                        BuildError::Invalid(format!(
                            "term {term} exceeds 2^32 postings, which the entity ceiling makes \
                             impossible for an unmutated input"
                        ))
                    })?;
            }
            let writer = entity_terms
                .as_mut()
                .expect("an assigned entity is in a filled shard");
            while u64::from(entity) >= spans[entity_terms_shard].hi {
                entity_terms_shard += 1;
                let next = entity_terms_writer(&spans[entity_terms_shard])?;
                entity_terms_paths.extend(
                    std::mem::replace(writer, next)
                        .finish()
                        .map_err(transpose)?,
                );
            }
            writer
                .push(entity - spans[entity_terms_shard].lo as u32, &sig_terms)
                .map_err(transpose)?;
        }
        drop(assigned_count);
        entities[ordinal_lo as usize..ordinal_lo as usize + batch_len].copy_from_slice(&assigned);
        entity_base += recs.len() as u64;
        store.delete(k)?;
        timer.end(BuildStage::Assignment, recs.len() as u64);
    }
    // The bands' and the transposes' last writes, which close the assignment.
    if plan.batches > 0 {
        timer.begin(BuildStage::Assignment, &[], None);
    }
    if entity_base != n {
        return Err(input_changed(&format!(
            "batches assigned {entity_base} entities for {n} items"
        )));
    }
    // The assignment walk is the only writer, so the mapping is read-only from here on and the
    // passes below take it as the plain `&[u32]` they always did.
    let entity_of_ordinal = entity_map.as_slice();
    // The two ordinal-space counters of the label-agreement identity (`views.md` §7) have served
    // their only reader, the ordinal-order pass each batch runs ahead of its assignment walk, and
    // are released here rather than at the end of the build — across every stage from the postings write to the last segment. 4 B/item
    // each, both mapped files, so what this returns at the GBIF rung is 13.0 GiB of disk apiece
    // (modelled, items × 4 B). Each file is unlinked by `MappedArray`'s own `Drop`.
    drop(distinct_map);
    drop(appearances);
    // The anchor's Morton geometry has served its one reader — the sort's tiebreak — and is
    // released here rather than at the end of the build. At 10⁹ that is 8 GB of dirty mapped
    // pages returned before the band sweep and the postings write start competing for page
    // cache. Each view's own geometry stays: pass two is what reads it. Nothing to release where
    // the anchor covered the union and no array was written.
    drop(anchor_maps);
    let band_receipts: Vec<SpillReceipt> = band_writers
        .into_iter()
        .map(|w| w.finish())
        .collect::<Result<_>>()?;
    if over_bound_items > 0 {
        eprintln!(
            "warning: {over_bound_items} item(s) are indexed under more than {} keys; no key \
             was dropped",
            mosaica_authz::MAX_KEYS_PER_ITEM
        );
    }

    if let Some(writer) = entity_terms {
        entity_terms_paths.extend(writer.finish().map_err(transpose)?);
    }
    for path in &entity_terms_paths {
        fsync_file(path)?;
    }

    for span in &spans {
        let layout = span.layout();
        for dir in [
            prefix_dir.join(layout.terms_rel()),
            prefix_dir.join(layout.entities_rel()),
        ] {
            std::fs::create_dir_all(&dir).map_err(|e| BuildError::io(&dir, e))?;
        }
    }

    timer.end(BuildStage::Assignment, n);
    timer.begin(
        BuildStage::PostingsWrite,
        &[],
        Some((pair_count, Unit::Pairs)),
    );

    // ---- 6. postings and pairs.parquet, one term band at a time -----------------------
    // Bands arrive from the batch loop with every term's entities strictly ascending (bases
    // ascend across batches, positions within one); the cursor-scatter needs no sort, and
    // `encode_posting`'s unconditional sortedness check re-verifies the whole spill path from
    // disk, fail-closed. The integrity chain replacing the old third pairs scan: bucket
    // receipts -> in-process emit -> band receipts (count + anchor, verified on read) ->
    // per-term exact counts -> the total against the deduplicated relation.
    //
    // **One postings file per shard**, each holding every term: a term's ascending entities are
    // cut at the shards' bounds, and each part is written at its shard's ids.
    let postings_paths: Vec<PathBuf> = spans
        .iter()
        .map(|span| prefix_dir.join(span.layout().postings_rel()))
        .collect();
    let pairs_paths: Vec<PathBuf> = match args.emit_oracle_pairs {
        true => filled
            .iter()
            .map(|span| prefix_dir.join(span.layout().pairs_rel()))
            .collect(),
        false => Vec::new(),
    };
    let mut pairs_writers: Vec<PairsParquetWriter> = pairs_paths
        .iter()
        .map(|path| PairsParquetWriter::create(path))
        .collect::<std::result::Result<_, _>>()?;
    let mut postings: Vec<mosaica_authz::RecordWriter> = postings_paths[..filled.len()]
        .iter()
        .map(|path| {
            mosaica_authz::RecordWriter::create(path, mosaica_authz::POSTINGS_COLUMN)
                .map_err(|e| BuildError::io(path, e))
        })
        .collect::<Result<_>>()?;
    let mut written = 0u64;
    for (j, &(band_lo, band_hi)) in plan.band_bounds.iter().enumerate() {
        let width = (band_hi - band_lo) as usize;
        let mut offsets: Vec<u64> = Vec::with_capacity(width + 1);
        let mut total = 0u64;
        offsets.push(0);
        for term in band_lo..band_hi {
            total += term_counts[term as usize] as u64;
            offsets.push(total);
        }
        let mut flat: Vec<u32> = vec![0; total as usize];
        let mut cursor: Vec<u64> = offsets[..width].to_vec();
        let mut reader = spill::BandReader::open(&band_receipts[j])?;
        while let Some((term, entity)) = reader.next()? {
            if term < band_lo || term >= band_hi {
                return Err(input_changed(&format!(
                    "band {j} holds term {term}, outside its [{band_lo}, {band_hi}) range"
                )));
            }
            let local = (term - band_lo) as usize;
            let slot = &mut cursor[local];
            // The band was sized by the emit's exact count for this term. Writing past its
            // end would land in the next term's postings — one term's entities silently
            // becoming another's, which is a disclosure. Check rather than trust.
            if *slot >= offsets[local + 1] {
                return Err(input_changed(&format!(
                    "term {term} received more postings than the {} its emit counted",
                    term_counts[term as usize]
                )));
            }
            flat[*slot as usize] = entity;
            *slot += 1;
        }
        // The mirror: a short-filled term would leave zeroed slots, and zero is a valid
        // entity id — catch it by count, not by value.
        for (local, slot) in cursor.iter().enumerate() {
            if *slot != offsets[local + 1] {
                return Err(input_changed(&format!(
                    "term {} expected {} postings, received {}",
                    band_lo as usize + local,
                    offsets[local + 1] - offsets[local],
                    slot - offsets[local]
                )));
            }
        }
        // Encode in bounded sub-chunks — parallel, collected in term order — and append to
        // the postings file; never one live record per term (T = 117M of those is gigabytes of Vec
        // headers before a byte is written).
        const ENCODE_CHUNK_TERMS: usize = 1 << 16;
        let mut t = 0usize;
        while t < width {
            let hi = (t + ENCODE_CHUNK_TERMS).min(width);
            for (k, span) in filled.iter().enumerate() {
                let encoded: Vec<(Vec<u8>, std::borrow::Cow<'_, [u32]>)> = (t..hi)
                    .into_par_iter()
                    .map(|local| {
                        let slice = span
                            .slice(&flat[offsets[local] as usize..offsets[local + 1] as usize]);
                        let record = encode_posting(
                            band_lo as usize + local,
                            &slice,
                            SMALL_TERM_THRESHOLD_DEFAULT,
                        )
                        .map_err(|e| BuildError::io(&postings_paths[k], e))?;
                        Ok((record, slice))
                    })
                    .collect::<Result<_>>()?;
                for (local, (record, slice)) in (t..hi).zip(encoded) {
                    postings[k]
                        .append(&record)
                        .map_err(|e| BuildError::io(&postings_paths[k], e))?;
                    written += slice.len() as u64;
                    if let Some(writer) = pairs_writers.get_mut(k) {
                        writer.push_run(band_lo + local as u32, &slice)?;
                    }
                }
            }
            steps.advance(offsets[hi] - offsets[t]);
            t = hi;
        }
        drop(flat);
        std::fs::remove_file(&band_receipts[j].path)
            .map_err(|e| BuildError::io(&band_receipts[j].path, e))?;
    }
    drop(term_counts);
    if written != pair_count {
        return Err(BuildError::Invalid(format!(
            "postings hold {written} pairs but the relation has {pair_count}"
        )));
    }
    for writer in pairs_writers {
        writer.finish()?;
    }
    for (file, path) in postings.into_iter().zip(&postings_paths) {
        file.finish().map_err(|e| BuildError::io(path, e))?;
        fsync_file(path)?;
    }

    timer.end(BuildStage::PostingsWrite, pair_count);
    timer.begin(BuildStage::AttributeTail, &[], None);

    // ---- 8. the declared attribute tail ----------------------------------------------
    // **Geometry is already in entity order**: the assignment walk placed it as it assigned, so
    // there is no geometry stage here at all — no read, and no permute. 32-bit fixed point per
    // axis, not coordinates: the cell code and its residual both fall out by shift and mask, so
    // no stage re-quantises (see `input::PointRow`).
    // The declared attribute tail, read **here** and not at the segment write, because this is
    // where the two things that resolve it are still alive: each file's numbers name a row's
    // ordinal, and `entity_of_ordinal` maps that ordinal to the entity the build assigned it.
    // Entity ids are assigned in *signature-sorted* order (§11.1), so a number is emphatically not
    // its own entity id — indexing the attribute arrays by number gives every item another item's
    // attributes, coherently and with no error anywhere.
    //
    // `minters` seeds one live minter per discovered vocabulary from what the schema already
    // pins; the scan mints into it for every novel key, and its final state — carried past this
    // call — is what step 11 below records into `MANIFEST.vocabularies`.
    let mut minters = args.schema.open_minters();
    crate::mint_metadata_keys(args, &mut minters)?;
    // The columns' files live in the same `.build-tmp/` as the spill and band files, so a killed
    // build leaves them to the next `TmpDir::create` exactly as it leaves those.
    let scratch = crate::column::ColumnScratch::new(tmp.path());
    let (mut attributes_by_entity, mut spilled, coverage) = read_attributes_by_entity(
        args,
        n,
        &plan.routes,
        &numbering,
        entity_of_ordinal,
        &mut minters,
        &scratch,
        tmp.path(),
    )?;
    // **Printed here, where the join has just happened and the numbers are the join's own.** The
    // linear build reports the identical figures from its own pass, so the two builds agree about
    // coverage exactly as they agree about bytes.
    crate::report_attribute_coverage(&coverage);

    // ---- 8b. the group-scoped column families (`views.md` §5) --------------------------
    // Here, beside the attribute tail, because it wants exactly what the tail wants: the numbers
    // and `entity_of_ordinal`, both alive, and entity ids final under I9. One column per view of
    // the group, in entity space; nothing per row space.
    let (scoped_paths, mut scoped_render, scoped_cost) = write_scoped_columns(
        args,
        &prefix_dir,
        filled,
        n,
        &numbering,
        entity_of_ordinal,
        &mut minters,
        &scratch,
        plan.term_index_allowance(plan.sizing.scoped_term_index),
        steps,
    )?;
    for (stage, cost) in [
        (BuildStage::FilterPostings, scoped_cost.filter),
        (BuildStage::TextIndex, scoped_cost.text),
    ] {
        if !cost.elapsed.is_zero() {
            timer.charge(stage, cost.elapsed, cost.rows, cost.passes);
        }
    }
    // Which of those columns each view's row space carries (`views.md` §5) — resolved once, here,
    // rather than per view inside the loop below, so the rule that decides it is stated in one
    // place and the loop is an index.
    let scoped_render_targets = scoped_render_targets(args, &scoped_render);

    timer.end(BuildStage::AttributeTail, n);
    timer.begin(BuildStage::Layers, &[], None);

    // ---- 8c. layers and their artifacts ------------------------------------------------
    // Resolved here for the reason the attribute tail is: this is where what turns a member's
    // number into the entity this build assigned it is still alive.
    //
    // **Its own stage boundary**, so an observer can say how much of the peak is here: the member
    // tables are read whole and the published memberships stay resident, and this ran unattributed
    // inside the attribute tail until the campaign's kills made the distinction worth having
    // (`residency.rs`).
    let view_ids: Vec<String> = args.views.iter().map(|v| v.view_id.clone()).collect();
    let view_frames: Vec<mosaica_store::derived::ViewFrame> = args
        .views
        .iter()
        .map(|v| mosaica_store::derived::ViewFrame::new(&v.view_id, v.projection, v.extent))
        .collect();
    let mut published_layers = if args.layers.is_empty() {
        drop(numbering);
        crate::layers::PublishedLayers::default()
    } else {
        let publication_batch = plan.sizing.publication_batch;
        let mut plan = crate::layers::read(
            &args.layers,
            &args.layer_inputs,
            &numbering,
            &args.scoped_layers,
            // **A frame per view, never the anchor's for all of them** (decision 0111): a
            // shape layer is canonicalised in each view it is drawn in, against that view's
            // own projection and extent, so a layer spanning frames stores a different
            // canonical form under each view's name.
            &view_frames,
            &args.groups,
            mosaica_types::layer::DEFAULT_MAX_SHAPE_VERTICES,
            // The build's own `.build-tmp/`, which the member spill writes its runs into —
            // still open here, and swept by the `close` below whether this stage succeeds or
            // not.
            tmp.path(),
            plan.budget,
        )?;
        crate::report_shapes(&plan.shape_reports);
        let derived = crate::layers::predicate_artifact_keys(
            &args.layers,
            &args.schema,
            &minters,
            &|index| distinct_codes(attributes_by_entity[index].iter()),
        )?;
        // The member rows are read, so every file's numbers have served their last reader.
        drop(numbering);
        crate::layers::publish(
            &mut plan,
            &|number| (number < n).then(|| u64::from(entity_of_ordinal[number as usize])),
            n,
            &prefix_dir,
            &spans,
            tmp.path(),
            &view_ids,
            &derived,
            publication_batch,
        )?
    };

    crate::write_containment_report(&args.out, &published_layers)?;

    timer.end(BuildStage::Layers, published_layers.layers.len() as u64);
    timer.begin(BuildStage::UniqueIndexes, &[], None);

    // ---- 8d. attribute filter postings (filter-index §4) -------------------------------
    // Its own stage, after entity assignment and before the tiler sort: entity ids are final
    // here (stage 5, permanent under I9) and the values have just been read, which are the two
    // things the emit needs. It cannot ride on `PostingsWrite` — that stage runs before the
    // attribute values exist.
    // The extents the join spilled, opened once for every reader: the record blob merges them,
    // and a `text` column's token index tokenises them in block windows first (`crate::extents`).
    // Folded first only where a column spilled more extents than the budget lets one merge hold
    // open, which no corpus built so far reaches.
    let fan_in = crate::extents::merge_fan_in(plan.budget);
    for column in spilled.iter_mut() {
        column.cascade(fan_in)?;
    }
    let mut open_extents: Vec<crate::extents::OpenExtents> = spilled
        .iter()
        .map(crate::extents::ExtentColumn::open)
        .collect::<Result<_>>()?;
    // ---- 8c′. the unique indexes -------------------------------------------------------
    // Here, where every column's values are still readable from the column or the extents the
    // join spilled, and before the passes that release them. A value held twice refuses the
    // build before any row space is written.
    //
    // Each shard's runs hold its own items: the identity rule has already decided, over every
    // file, which item holds each value.
    let mut unique = Vec::with_capacity(spans.len());
    for span in &spans {
        window_shard(&mut attributes_by_entity, &mut open_extents, span);
        unique.push(crate::unique_index::write_unique_indexes(
            &prefix_dir,
            span.layout(),
            &args.schema,
            |column| match open_extents.iter().find(|extents| extents.column == column) {
                Some(extents) => crate::unique_index::UniqueSource::Extents(extents),
                None => crate::unique_index::UniqueSource::Column(&attributes_by_entity[column]),
            },
            tmp.path(),
            plan.sizing.unique_spill as usize,
        )?);
    }
    timer.end(BuildStage::UniqueIndexes, n);
    timer.begin(BuildStage::FilterPostings, &[], None);
    let mut filter_paths = Vec::new();
    for (k, span) in filled.iter().enumerate() {
        window_shard(&mut attributes_by_entity, &mut open_extents, span);
        let (paths, costs) = write_filter_postings(
            &prefix_dir,
            span.layout(),
            &args.schema,
            &attributes_by_entity,
            &open_extents,
            plan.term_index_allowance(plan.sizing.term_index),
            steps.within(Part::new("shard", "", k, filled.len())),
        )?;
        filter_paths.extend(paths);
        // The text columns' share, charged out of the block rather than measured beside it — the
        // two interleave over one column loop, so a boundary in time cannot separate them.
        timer.charge(
            BuildStage::TextIndex,
            costs.text.elapsed,
            costs.text.rows,
            costs.text.passes,
        );
        timer.passes(costs.filter.passes);
    }
    // The columns' steps are over, and what is left of the stage is its own.
    let owed = |attribute| postings_are_owed(&args.schema, attribute);
    if !filled.is_empty() && args.schema.attributes.iter().any(owed) {
        steps.begin(BuildStage::FilterPostings, &[], None);
    }
    // **A column with no blob row is dead here, not two stages further on.** The blob reads the
    // columns whose values have no other home ([`blob_resident`]); the segment write reads the
    // render columns. A column that is neither — an `index = true` keyword, say — has just met its
    // last reader, and holding it to the release below costs the blob's whole stage. Measured on
    // 125,789,091 GBIF occurrences: `specieskey`'s offsets and arena are 3.15 GB against a 19.4 GB
    // peak that fell on the record blob.
    for (column, attribute) in attributes_by_entity
        .iter_mut()
        .zip(args.schema.attributes.iter())
    {
        if !attribute.render
            && !blob_resident(&args.schema, attribute)
            && !crate::band_copied(attribute)
        {
            column.release();
        }
    }
    // **And the same line for a spilled column's extents.** An indexed keyword's extents are the
    // dictionary pass's alone, so they go back to the disk here rather than standing through the
    // record blob, which is the phase the measured peak falls on.
    open_extents
        .retain(|extents| blob_resident(&args.schema, &args.schema.attributes[extents.column]));
    for column in spilled.iter_mut() {
        if !blob_resident(&args.schema, &args.schema.attributes[column.column]) {
            column.remove();
        }
    }
    timer.end(BuildStage::FilterPostings, n);
    timer.begin(BuildStage::RecordBlob, &[], None);

    // The record blob wants the same two things the postings did — entity ids final under I9, and
    // the attribute values in hand — so it runs here. Its files join the manifest digest at step 11
    // with everything else. **Its own stage**: it and the postings and the release below were one
    // number for three jobs, which is why the 615 s this block cost at 7.4×10⁷ points could be
    // modelled and not read.
    let mut record_paths = Vec::new();
    for span in filled {
        window_shard(&mut attributes_by_entity, &mut open_extents, span);
        record_paths.extend(write_record_blob(
            &prefix_dir.join(span.layout().record_rel()),
            &args.schema,
            span.len(),
            &plan.routes,
            &attributes_by_entity,
            &open_extents,
        )?);
    }
    // Both readers are done, so the extents go back to the disk before the row spaces are
    // written. A build that failed above leaves them to `TmpDir::close`.
    drop(open_extents);
    for column in spilled.iter_mut() {
        column.remove();
    }
    timer.end(BuildStage::RecordBlob, n);
    timer.begin(BuildStage::ColumnRelease, &[], None);
    // **Everything past here wants only the render columns**, and the pass that wanted the rest has
    // just run. The assembly reads a non-render column not at all (its home is entity space, and
    // giving it a slot in every row is the per-row cost §10.3's routing exists to avoid), so a
    // blob-resident column is dead from this line and would otherwise live to the end of the
    // segment write, straight through the row partition's 12 B a row and the columns file beside
    // it.
    //
    // A spilled column has nothing left to release — its characters went out as extents and the
    // extents are unlinked above. What is left is the blob-resident columns, the postings pass
    // having already released the rest; `release` is idempotent, so the loop states the whole rule
    // rather than the half of it this line reaches.
    //
    // The columns the bands copy are kept too: each segment's band file reads its banded rows'
    // values from them.
    for (column, attribute) in attributes_by_entity
        .iter_mut()
        .zip(args.schema.attributes.iter())
    {
        if !attribute.render && !crate::band_copied(attribute) {
            column.release();
        }
    }

    let mut empty_files: Vec<PathBuf> = Vec::new();
    if filled.is_empty() {
        let declared = crate::declared_scalars_of(&args.schema);
        let vocabularies = crate::manifest_vocabularies(args, &minters);
        let scoped: Vec<mosaica_store::manifest::ScopedScalar> = args
            .scoped_attributes
            .iter()
            .map(|family| crate::scoped_scalar_of(args, family))
            .collect();
        // The render columns, then the view's scoped ones, as the segment write below orders them.
        let views: Vec<mosaica_store::empty_shard::EmptyView<'_>> = args
            .views
            .iter()
            .zip(&scoped_render_targets)
            .map(|(view, targets)| mosaica_store::empty_shard::EmptyView {
                id: &view.view_id,
                incarnation: mosaica_store::manifest::DECLARED_INCARNATION,
                render: crate::scalar_schema_of(&args.schema)
                    .into_iter()
                    .chain(
                        targets
                            .iter()
                            .map(|&at| (scoped_render[at].name.clone(), scoped_render[at].ty)),
                    )
                    .collect(),
            })
            .collect();
        for span in &spans {
            let written = mosaica_store::empty_shard::write_empty_shard(
                &mosaica_store::empty_shard::EmptyShard {
                    prefix_dir: &prefix_dir,
                    layout: span.layout(),
                    pairs: args.emit_oracle_pairs,
                    declared: &declared,
                    vocabularies: &vocabularies,
                    scoped: &scoped,
                    views: &views,
                    seg_id: SEG_ID,
                },
            )?;
            empty_files.extend(written.files.into_iter().map(|(_, path)| path));
        }
    }

    // ---- 9/10. pass two: one row space per view and shard (`views.md` §7) ---------------
    // **The build below is what it always was, run once per view.** What is not one-view-at-a-
    // time is everything above: identity, the dictionary, the postings and the attribute
    // columns are entity space and were built once, over the union of every view's
    // items. Here each view transforms through its own projection, quantises against its own
    // frame (decision 0040), Morton-sorts and writes its segment, its permutation and its
    // row→entity file.
    //
    // **Each shard's part of a view is sorted on its own**, over its own ids.
    //
    // A view holds a **subset** of entity space — its `present` bits — so its permutation is
    // sentinel wherever it does not, and `row_count` is the view's population rather than `n`.
    let mut view_files: Vec<PathBuf> = Vec::new();
    let mut shards: Vec<crate::ShardWritten> = spans
        .iter()
        .zip(unique)
        .map(|(span, unique)| crate::ShardWritten {
            span: *span,
            segments: Vec::new(),
            occupancies: Vec::new(),
            term_images: Vec::new(),
            unique,
            membership_extents: Vec::new(),
        })
        .collect();
    let mut artifact_paths: Vec<PathBuf> = Vec::new();
    // Accumulated across the views, like the extents beside them: what a scoped layer's artifacts
    // came to in each view is per view or it says nothing (`views.md` §3.5).
    let mut artifact_levels: Vec<crate::artifact_pass::LevelLayoutReport> = Vec::new();
    // **One counter for the whole prefix.** The artifact pass runs per view and each of its calls
    // names files from this, so the second view's files cannot be named after the first's — see
    // `mosaica_store::derived::DerivedIndex`.
    let mut derived_index = mosaica_store::derived::DerivedIndex::default();
    // **Opened once for the build, not once per view.** A shard's postings are its entity space
    // and every view projects the same ones through its own permutation
    // (`crate::term_images_pass`).
    let postings: Vec<mosaica_authz::postings::PostingsReader> = postings_paths
        .iter()
        .map(|path| {
            mosaica_authz::postings::PostingsReader::open(path, true)
                .map_err(|e| BuildError::io(path, e))
        })
        .collect::<Result<_>>()?;
    let several = spans.len() > 1;
    timer.end(BuildStage::ColumnRelease, n);
    for (index, view) in args.views.iter().enumerate() {
        let view_part = Part::new("view", &view.view_id, index, args.views.len());
        timer.begin(BuildStage::TilerSort, &[view_part], None);
        // ---- the assembly: the view's rows, Morton-partitioned, straight from ordinal order ----
        //
        // **No entity-order geometry.** What stood here permuted `x` and `y` into two
        // entity-indexed files at a scattered index, sorted a 12 B record per row of the view on
        // the heap, and built the row→entity, residual and `mosaica_id` columns as three more
        // vectors beside it. At rung 6 that was a 70 GB peak the residency model had no term for.
        // `crate::assembly` is the same rows in the same order with one bucket held at a time.
        //
        // Every shard's rows are partitioned before any segment is written, so the view's
        // geometry is released before the first of them.
        let mut partitioned = Vec::with_capacity(spans.len());
        let mut view_rows = 0u64;
        for span in filled {
            let layout = span.layout();
            let view_dir = prefix_dir.join(layout.view_rel(&view.view_id));
            let segment_dir = prefix_dir.join(layout.segment_rel(&view.view_id, SEG_ID));
            std::fs::create_dir_all(&segment_dir).map_err(|e| BuildError::io(&segment_dir, e))?;
            // Names this part's spill files apart from every other's.
            let label = format!("{}-{}", view.view_id, span.id);
            let (rows, boundaries, page_plan) = {
                let source = crate::assembly::RowSource {
                    view_dir: &view_dir,
                    view: &label,
                    tmp: tmp.path(),
                    x: geometry[index].x.as_slice(),
                    y: geometry[index].y.as_slice(),
                    present: &geometry[index].present,
                    entity_of_ordinal,
                    ordinals: batch_ordinals(span, plan.batch_items, n),
                    entity_lo: span.lo as u32,
                    n: span.len(),
                    identity_key: &args.identity_key,
                    shard: span.id,
                    rows_hint: geometry[index].rows.min(span.len()),
                };
                crate::assembly::partition_rows(&source)?
            };
            view_rows += rows.rows();
            partitioned.push(ShardRows {
                span,
                view_dir,
                segment_dir,
                label,
                rows,
                boundaries,
                page_plan,
            });
        }
        if view_rows != geometry[index].rows {
            return Err(input_changed(&format!(
                "view '{}': {view_rows} rows in the segments for the {} its points file selected",
                view.view_id, geometry[index].rows
            )));
        }
        // **This view's ordinal-space geometry has served its only reader.** The row partition
        // carries each row's residual, so nothing after this reads a coordinate — 8 B/item per
        // view, released before the segment write rather than after the artifact pass.
        geometry[index].x = spill::MappedU32::empty();
        geometry[index].y = spill::MappedU32::empty();
        timer.end(BuildStage::TilerSort, view_rows);

        let mut parts: Vec<(crate::ShardSpan, u32)> = Vec::with_capacity(spans.len());
        for (k, part) in partitioned.into_iter().enumerate() {
            timer.begin(
                BuildStage::SegmentWrite,
                &[view_part, Part::new("shard", "", k, filled.len())],
                Some((part.rows.rows(), Unit::Rows)),
            );
            let ShardRows {
                span,
                view_dir,
                segment_dir,
                label,
                rows,
                boundaries,
                page_plan,
            } = part;
            window_shard(&mut attributes_by_entity, &mut [], span);
            for column in &mut scoped_render {
                column.values.window(span.lo as usize, span.hi as usize);
            }
            let assembled = {
                // The declared render columns in schema order, then this view's scoped ones — the
                // order `columns.arrow` carries them in, and the order every reader that resolves
                // a tail column by name is indifferent to but the bytes are not.
                let mut render: Vec<crate::assembly::RenderColumn<'_>> = Vec::new();
                for (attribute, values) in args
                    .schema
                    .attributes
                    .iter()
                    .zip(attributes_by_entity.iter())
                {
                    if attribute.render {
                        render.push(crate::assembly::RenderColumn {
                            name: attribute.name.clone(),
                            ty: attribute.ty,
                            values,
                        });
                    }
                }
                for &column in &scoped_render_targets[index] {
                    render.push(crate::assembly::RenderColumn {
                        name: scoped_render[column].name.clone(),
                        ty: scoped_render[column].ty,
                        values: &scoped_render[column].values,
                    });
                }
                let indexed: Vec<crate::assembly::RenderColumn<'_>> =
                    crate::band_copied_columns(&args.schema)
                        .map(|(at, attribute)| crate::assembly::RenderColumn {
                            name: attribute.name.clone(),
                            ty: attribute.ty,
                            values: &attributes_by_entity[at],
                        })
                        .collect();
                let job = crate::assembly::Assembly {
                    view_dir: &view_dir,
                    view: &label,
                    segment_dir: &segment_dir,
                    tmp: tmp.path(),
                    n: span.len(),
                    identity_key: &args.identity_key,
                    shard: span.id,
                    render,
                    indexed,
                    steps,
                };
                crate::assembly::write_segment(&job, rows, &boundaries, page_plan)?
            };
            eprintln!(
                "  view '{}'{}: {} rows, largest Morton bucket {} rows over {} buckets",
                view.view_id,
                match several {
                    true => format!(" in shard {}", span.id),
                    false => String::new(),
                },
                assembled.rows_in_view,
                assembled.largest_bucket,
                boundaries.buckets()
            );
            let rows_in_view = assembled.rows_in_view;
            shards[k].occupancies.push(assembled.occupancy);
            let segment_paths: Vec<std::path::PathBuf> = mosaica_store::SEGMENT_FILES
                .iter()
                .map(|name| segment_dir.join(name))
                .collect();
            fsync_file(&assembled.permutation_path)?;
            fsync_file(&assembled.row_entity_path)?;
            for path in &segment_paths {
                fsync_file(path)?;
            }
            view_files.push(assembled.permutation_path);
            view_files.push(assembled.row_entity_path);
            view_files.extend(segment_paths);
            view_files.extend(assembled.presence_paths);
            parts.push((*span, rows_in_view));
            timer.end(BuildStage::SegmentWrite, rows_in_view as u64);
        }
        timer.begin(BuildStage::ArtifactPass, &[view_part], None);
        if filled.is_empty() {
            for (k, span) in spans.iter().enumerate() {
                shards[k].occupancies.push(crate::Occupancy {
                    points: 0,
                    cells: 0,
                });
                parts.push((*span, 0));
            }
        }

        // ---- 10b. the post-bundle artifact pass, per view (decision 0094's first half) ----
        //
        // **Here and not at step 8c**, where the layers were published: the pick reads where each
        // membership landed in *row* space, and this view's row spaces did not exist until the
        // permutations above.
        let artifact_store = std::mem::take(&mut published_layers.store);
        let artifact_pass = crate::artifact_pass::run(
            &mut published_layers,
            &artifact_store,
            &prefix_dir,
            &parts,
            &view.view_id,
            tmp.path(),
            &mut derived_index,
        );
        published_layers.store = artifact_store;
        crate::artifact_pass::report(&artifact_pass);
        artifact_levels.extend(artifact_pass.levels.iter().cloned());
        // **Accumulated across views, not replaced.** Every artifact extent is keyed by
        // `(view, layer, level)`, so each view's pass adds its own; assigning would leave the
        // manifest carrying the last view's alone.
        published_layers
            .derived_extents
            .extend(artifact_pass.derived_extents.iter().cloned());
        artifact_paths.extend(artifact_pass.paths.iter().cloned());
        timer.end(
            BuildStage::ArtifactPass,
            artifact_pass.derived_extents.len() as u64,
        );

        for (k, &(span, rows_in_view)) in parts.iter().enumerate() {
            let layout = span.layout();
            timer.begin(
                BuildStage::TermImages,
                &[view_part, Part::new("shard", "", k, parts.len())],
                (rows_in_view > 0).then(|| (u64::from(postings[k].term_count()), Unit::Terms)),
            );
            // ---- 10c. this view's term images (`crate::term_images_pass`) ------------------
            //
            // Beside the artifact pass and after it, for the reason it sits here: the images are
            // the view's permutation applied to every term's posting, and this view's row space
            // did not exist until that permutation was written.
            let images = crate::term_images_pass::run(
                &postings[k],
                &prefix_dir,
                layout,
                &view.view_id,
                rows_in_view,
                &mut derived_index,
                plan.sizing.term_image_workers as usize,
                steps,
            )?;
            artifact_paths.extend(images.iter().map(|images| images.path.clone()));
            let kept = images.as_ref().map_or(0, |images| images.report.kept);
            shards[k].term_images.push(images);

            // ---- 10d. this view's field tallies (`mosaica_store::field_tallies`) -----------
            //
            // After every file they read: the segment, the row-to-entity table, the entity terms
            // and the value columns. The in-memory build calls the same function at the same
            // point.
            if !filled.is_empty() {
                view_files.push(mosaica_store::field_tallies::derive_view(
                    &prefix_dir,
                    layout,
                    &view.view_id,
                    SEG_ID,
                    rows_in_view,
                    &crate::declared_scalars_of(&args.schema),
                    &|name| mosaica_filter::base_numbers(&prefix_dir.join(layout.column_rel(name))),
                )?);
            }
            shards[k]
                .segments
                .push(mosaica_store::manifest::SegmentDescriptor {
                    view: view.view_id.clone(),
                    // **The declared incarnation** (decision 0115): a build coins each key once.
                    incarnation: mosaica_store::manifest::DECLARED_INCARNATION,
                    seg_id: SEG_ID.to_string(),
                    row_count: rows_in_view,
                    entity_lo: 0,
                    entity_hi: span.len(),
                });
            timer.end(BuildStage::TermImages, u64::from(kept));
        }
    }
    // ---- 10e. the containment partitions, once a shard ------------------------------------
    //
    // **Outside the view loop**, because a partition is a function of the level's records and the
    // shard's postings and carries no view. Composing it inside the pass would write one identical
    // file and one manifest entry per view.
    timer.begin(BuildStage::ArtifactPass, &[], None);
    // Every view has been through the term images, so the postings mappings are unbusy.
    drop(postings);
    {
        let artifact_store = std::mem::take(&mut published_layers.store);
        let containment = crate::artifact_pass::containment(
            &artifact_store,
            &prefix_dir,
            &spans,
            &mut derived_index,
        );
        eprintln!("  wrote {} containment partition(s)", containment.len());
        artifact_paths.extend(
            containment
                .iter()
                .map(|(_, entry)| args.out.join(crate::PREFIX).join(&entry.path)),
        );
        let composed = containment.len() as u64;
        published_layers.derived_extents.extend(containment);
        timer.end(BuildStage::ArtifactPass, composed);
    }

    // ---- 11. manifests ---------------------------------------------------------------
    timer.begin(BuildStage::Manifests, &[], None);
    for (shard, extents) in shards
        .iter_mut()
        .zip(std::mem::take(&mut published_layers.membership_extents))
    {
        shard.membership_extents = extents;
    }

    drop(geometry);
    drop(entity_map);
    // The spill directory closes **here**: every view's ordinal-space geometry and every
    // entity-space scatter are dropped by this point, so the tree is unbusy.
    tmp.close()?;

    let mut other_paths = empty_files;
    if !filled.is_empty() {
        other_paths.extend(postings_paths);
    }
    other_paths.extend(entity_terms_paths);
    other_paths.extend(view_files);
    other_paths.extend(filter_paths);
    other_paths.extend(scoped_paths);
    other_paths.extend(record_paths);
    other_paths.extend(pairs_paths);
    other_paths.extend(published_layers.paths.iter().cloned());
    other_paths.extend(artifact_paths);
    let mut report = write_manifests(
        args,
        &BundleFiles {
            dict_paths,
            dict_records: term_count,
            other_paths,
        },
        n,
        term_count,
        pair_count,
        plan.recorded_batch_items,
        &minters,
        &published_layers,
        &shards,
        steps,
    )?;
    // Reported in bytes, not rows: this stage re-reads and SHA-256s every byte the build wrote,
    // so it scales with bundle size rather than with item count.
    timer.end(BuildStage::Manifests, report.bundle_bytes);
    report.attribute_coverage = coverage;
    report.refused = refused;
    report.artifact_levels = artifact_levels;
    report.spilled_columns = crate::spilled_column_names(&args.schema, &plan.routes);
    Ok(report)
}

/// The ordinals whose items a shard can hold. A signature batch's items take the entities of its
/// own ordinal range, so a shard's are among the ordinals of the batches its range meets.
fn batch_ordinals(span: &crate::ShardSpan, batch_items: u64, n: u64) -> std::ops::Range<usize> {
    let batch = batch_items.max(1);
    let lo = span.lo / batch * batch;
    let hi = (span.hi.div_ceil(batch) * batch).min(n);
    lo as usize..hi.max(lo) as usize
}

/// Narrow the attribute columns and the extents to one shard's entities, each at its entity less
/// the shard's first.
fn window_shard(
    columns: &mut [EntityColumn],
    extents: &mut [crate::extents::OpenExtents],
    span: &crate::ShardSpan,
) {
    for column in columns {
        column.window(span.lo as usize, span.hi as usize);
    }
    for extents in extents {
        extents.window(span.lo as u32, span.hi as u32);
    }
}

/// Read the declared attribute columns into **entity-major** vectors, one per declared attribute,
/// with one pass per attribute source.
///
/// Resolved through each row's number → ordinal → `entity_of_ordinal`, exactly as the geometry
/// pass above resolves its own rows, and for the same reason: entity ids are assigned in
/// signature-sorted order (§11.1), so a number is not its own entity id and a direct index hands
/// every item another item's attributes. That is a defect with no symptom — every value is
/// present, every value is well-typed, and every value belongs to a different item.
///
/// Returns an empty vector when the schema declares nothing, which is what keeps a schema-less
/// build's `columns.arrow` byte-identical to the one it wrote before this existed.
///
/// **Every item has a row in each source**, as a new item at a running service carries every
/// declared column, and a row of nulls is how a source says it has none. Which entities came away
/// with a value is counted per source and printed. The sweeps are [`crate::attribute_scans`], in
/// the order the identity rule read the files, so a unique field's value is the one the rule
/// decided.
#[allow(clippy::too_many_arguments)]
fn read_attributes_by_entity(
    args: &BuildArgs,
    n: u64,
    routes: &ColumnRoutes,
    numbering: &crate::ids::Numbering,
    entity_of_ordinal: &[u32],
    minters: &mut std::collections::HashMap<String, mosaica_store::vocabulary::VocabularyMinter>,
    scratch: &crate::column::ColumnScratch,
    tmp: &Path,
) -> Result<(
    Vec<EntityColumn>,
    Vec<crate::extents::ExtentColumn>,
    Vec<crate::AttributeCoverage>,
)> {
    if args.schema.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let attributes = &args.schema.attributes;
    // One typed column per attribute, indexed by entity — see [`EntityColumn`] for why this is not
    // the `ScalarValue` vector it reads like, and what that costs at 10⁸ items and above.
    //
    // **A spilled column carries no values here.** Its characters are spilled as record-blob
    // extents by the sweep below and read from them by the record blob, and by the token index too
    // where the column is `text` ([`ColumnRoutes`], [`crate::extents`]). What is kept here is the
    // length, so every later pass can go on indexing this vector by an attribute's declaration
    // position.
    let mut by_entity: Vec<EntityColumn> = attributes
        .iter()
        .enumerate()
        .map(|(i, a)| match routes.takes_extents(i) {
            true => EntityColumn::spilled(scratch, a.ty, n as usize),
            false => EntityColumn::filled(scratch, a.ty, n as usize),
        })
        .collect::<Result<_>>()?;
    let mut spilled: Vec<crate::extents::ExtentColumn> = attributes
        .iter()
        .enumerate()
        .filter(|&(i, _)| routes.takes_extents(i))
        .map(|(i, a)| crate::extents::ExtentColumn::new(tmp, i, &a.name))
        .collect();
    // **A partition per entity-order column** ([`ValueLane`]), carrying the value for a
    // fixed-width column and the `at` word for a string column on the arena route. Nothing the
    // join produces is written at a scattered index: the arena beside a string column is appended
    // in arrival order, and the word that says where a value went is replayed into `at` as a run
    // with every other word of its entity range.
    let mut value_lanes: Vec<Option<ValueLane>> = by_entity
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let width = match (column.fixed_width(), column.is_arena_strings()) {
                (Some(width), _) => width,
                // The `at` word, which is what a string column holds at an entity.
                (None, true) => 8,
                (None, false) => return Ok(None),
            };
            let boundaries = value_lane_boundaries(n);
            Ok(Some(ValueLane {
                column: index,
                width,
                chars: column.is_arena_strings(),
                partition: spill::Partition::create(
                    tmp,
                    &format!("value-{index}"),
                    boundaries.clone(),
                    4 + width,
                    n,
                )?,
                boundaries,
            }))
        })
        .collect::<Result<_>>()?;
    // **One sweep per source, not one over a single corpus file.** Each declared attribute names
    // the file it is read from, so the sources are the passes; a build whose columns sit in three
    // files reads three files.
    let mut tallies: Vec<SourceTally> = args
        .attribute_sources
        .iter()
        .map(|group| SourceTally {
            met: Vec::new(),
            matched: 0,
            present: vec![0; group.attributes.len()],
        })
        .collect();
    for scan in crate::attribute_scans(args, numbering)? {
        read_one_attribute_scan(
            args,
            &scan,
            n,
            entity_of_ordinal,
            minters,
            scratch,
            &mut by_entity,
            &mut spilled,
            &mut value_lanes,
            &mut tallies,
        )?;
    }
    let mut coverage = Vec::with_capacity(args.attribute_sources.len());
    for (group, tally) in args.attribute_sources.iter().zip(tallies) {
        let columns: Vec<&crate::config::Attribute> = group
            .attributes
            .iter()
            .map(|&i| &args.schema.attributes[i])
            .collect();
        let met: u64 = tally.met.iter().map(|w| u64::from(w.count_ones())).sum();
        if let Some(refusal) = input::items_without_a_row(&group.path, &columns, n - met) {
            return Err(refusal);
        }
        coverage.push(crate::AttributeCoverage {
            source: group.name.clone(),
            entities: n,
            matched_rows: tally.matched,
            columns: columns
                .iter()
                .zip(&tally.present)
                .map(|(a, &count)| (a.name.clone(), count))
                .collect(),
        });
    }
    // **The replay, once every source has been read.** A bucket at a time, in ascending entity
    // order: the records are scattered into a window over the bucket's own range and the window
    // is written into the column as one sequential run, with its presence bits beside it. A string
    // column's run is its `at` words, the arena behind them already written.
    for lane in value_lanes.into_iter().flatten() {
        let name = &attributes[lane.column].name;
        let record_width = 4 + lane.width;
        let mut store = lane.partition.finish()?;
        for (k, &lo) in lane.boundaries.iter().enumerate() {
            let hi = lane
                .boundaries
                .get(k + 1)
                .map(|&next| next as u64)
                .unwrap_or(n)
                .min(n);
            let span = (hi.saturating_sub(lo as u64)) as usize;
            let mut window = vec![0u8; span * lane.width];
            let mut present = vec![0u64; span.div_ceil(64)];
            for record in store.load(k)?.chunks_exact(record_width) {
                let entity =
                    u32::from_le_bytes(record[..4].try_into().expect("a record carries its key"));
                let at = (entity as u64)
                    .checked_sub(lo as u64)
                    .filter(|&at| at < span as u64);
                let Some(at) = at.map(|at| at as usize) else {
                    return Err(BuildError::Invalid(format!(
                        "attribute '{name}': bucket {k} of its value partition holds entity \
                         {entity}, outside the range [{lo}, {hi}) it addresses"
                    )));
                };
                window[at * lane.width..(at + 1) * lane.width].copy_from_slice(&record[4..]);
                present[at / 64] |= 1u64 << (at % 64);
            }
            store.delete(k)?;
            by_entity[lane.column].write_value_run(lo as usize, &window, &present, name)?;
        }
    }
    Ok((by_entity, spilled, coverage))
}

/// What one column's share of a resolved chunk is: a record pushed into its value partition, or,
/// for a column that has none ([`takes_extents`]), an extent written in the chunk's entity order.
/// Nothing here writes at a scattered entity: a value goes to the partition and is replayed into
/// the column as a run, and a string's characters are appended to the arena in arrival order.
///
/// The lanes share nothing. Each entity-order column has its own partition, each string column its
/// own arena, and each spilled column its own extent writer, so a chunk's work splits across them
/// with no synchronisation: `resolved` is read-only and every write a lane makes lands in storage
/// no other lane can name.
enum JoinLane<'a> {
    Partitioned {
        src: &'a mut EntityColumn,
        lane: &'a mut ValueLane,
    },
    /// A string column on the arena route: the characters go into `home`'s arena in arrival order
    /// and the word they landed at goes to the lane, where a fixed-width column's value goes.
    Chars {
        src: &'a mut EntityColumn,
        home: &'a mut EntityColumn,
        lane: &'a mut ValueLane,
    },
    Extent {
        src: &'a mut EntityColumn,
        out: &'a mut crate::extents::ExtentColumn,
    },
}

/// One fixed-width column's `(entity, value)` partition, and the entity ranges its buckets
/// address.
///
/// **Why a partition and not the scatter it replaces.** The join reads its source in the source's
/// own order and each value's home is its entity, which is signature-then-Morton order — so
/// writing each value where it belongs wrote the column's pages back and re-dirtied them many
/// times over: 123 GB written to grow the bundle by 34 in one stage at rung 6, over value columns
/// of 10.5 GB (`docs/evidence/memos/2026-09-12-gbif-whole-corpus-build-observations.md` §4). The
/// values go to buckets by entity range instead, and each bucket is replayed into a window and
/// written into the column as one sequential run.
///
/// **Replayed in append order, which is the order today's scatter writes in**: sweep order within
/// a chunk, chunk order across chunks, source order across sources. So which of two rows carrying
/// one entity wins is unchanged.
struct ValueLane {
    column: usize,
    /// What one record's payload is wide: the column's own width, or the eight bytes of a string
    /// column's `at` word.
    width: usize,
    /// **A string column on the arena route**, whose payload is the word and whose characters went
    /// into the arena as the join read them. The replay is the same one either way; what differs is
    /// where the join takes the payload from.
    chars: bool,
    /// The first entity of each bucket, ascending from 0 and **a multiple of 64** — so a bucket's
    /// presence bits are whole words of the column's own bitmap.
    boundaries: Vec<u32>,
    partition: spill::Partition,
}

/// The entity boundaries a value lane's buckets take: [`spill::boundaries_uniform`] rounded down
/// to presence-word boundaries, so a bucket's bits are whole words.
fn value_lane_boundaries(n: u64) -> Vec<u32> {
    spill::boundaries_uniform(n)
        .into_iter()
        .map(|first| first & !63)
        .collect::<std::collections::BTreeSet<u32>>()
        .into_iter()
        .collect()
}

/// Write one chunk's values as an extent, and return how many of its rows carried a value.
///
/// `resolved` ascends in the entity and its sort is stable, so an entity written twice within the
/// chunk keeps the last row that carried a value — the same answer the scatter reaches by writing
/// them in order, and the same one an absent later row leaves alone. Counted per resolved row
/// rather than per entity, which is what the coverage report says.
fn spill_extent_chunk(
    src: &EntityColumn,
    resolved: &[(u32, u32)],
    out: &mut crate::extents::ExtentColumn,
) -> Result<u64> {
    let mut rows: Vec<(u32, u32)> = Vec::new();
    let mut count = 0u64;
    for &(entity, pos) in resolved {
        if !src.is_present(pos as usize) {
            continue;
        }
        count += 1;
        if rows.last().map(|&(held, _)| held) == Some(entity) {
            rows.pop();
        }
        rows.push((entity, pos));
    }
    let values: Vec<(u32, &str)> = rows
        .iter()
        .map(|&(entity, pos)| {
            (
                entity,
                src.str_at(pos as usize)
                    .expect("the row was tested for presence"),
            )
        })
        .collect();
    out.push_extent(&values)?;
    Ok(count)
}

/// What one attribute source's sweeps met: the items with a row, the rows, and the values each
/// column came away with.
struct SourceTally {
    met: Vec<u64>,
    matched: u64,
    present: Vec<u64>,
}

/// One sweep of the attribute join into the entity-major columns.
#[allow(clippy::too_many_arguments)]
fn read_one_attribute_scan(
    args: &BuildArgs,
    scan: &crate::AttributeScan<'_>,
    n: u64,
    entity_of_ordinal: &[u32],
    minters: &mut std::collections::HashMap<String, mosaica_store::vocabulary::VocabularyMinter>,
    scratch: &crate::column::ColumnScratch,
    by_entity: &mut [EntityColumn],
    spilled: &mut [crate::extents::ExtentColumn],
    value_lanes: &mut [Option<ValueLane>],
    tallies: &mut [SourceTally],
) -> Result<()> {
    let filled: Vec<usize> = scan.attributes.clone();
    let columns: Vec<&crate::config::Attribute> =
        filled.iter().map(|&i| &args.schema.attributes[i]).collect();
    let attributes = &columns;
    let mut matched_rows = 0u64;
    // Which of the build's items met a row, by ordinal, so one that met none is refused.
    let mut met = vec![0u64; (n as usize).div_ceil(64)];

    // **[`join_chunk`]'s merge sweep, not a lookup per row.** Chunked, both sides ascend and the
    // sweep is sequential, which is the same trade the geometry pass makes. The cost is a staging buffer, and **it is sized in bytes**:
    // see [`JOIN_STAGE_BYTES`] for why a row count is the wrong unit here.
    //
    // **At least one whole decoded batch**, because a batch is staged as a unit: the flush below
    // happens between batches, so the buffer has to hold the largest one whatever the byte budget
    // works out at. On a corpus smaller than a batch that is the only term that matters, and it is
    // a few hundred kilobytes of file per column.
    let staged_rows = staging_rows(attributes, n).max(input::ATTRIBUTE_BATCH_ROWS);
    let mut staged: Vec<EntityColumn> = attributes
        .iter()
        .map(|a| EntityColumn::filled(scratch, a.ty, staged_rows))
        .collect::<Result<_>>()?;
    let mut chunk: Vec<(u64, u32)> = Vec::with_capacity(staged_rows);
    // The join's answer, held apart from the scatter that consumes it: `(entity, staged position)`
    // for every row of the chunk that resolved, in the sweep's order. **This is what makes the
    // scatter divisible** — the columns can then be walked independently over one shared,
    // read-only list, where the old shape did every column's write inside the sweep's own callback
    // and so did all of them on the sweep's thread.
    let mut resolved: Vec<(u32, u32)> = Vec::with_capacity(staged_rows);

    // How many entities came away with a value in each of this group's columns — counted where the
    // value is moved across, which is the only place presence is known without a second scan.
    let mut present = vec![0u64; attributes.len()];

    // Everything mutable is a parameter rather than a capture, so the scan's callback and this can
    // both hold it — the shape the geometry pass's `resolve` uses, and for the same borrow reason.
    let flush = |chunk: &mut Vec<(u64, u32)>,
                 resolved: &mut Vec<(u32, u32)>,
                 staged: &mut [EntityColumn],
                 by_entity: &mut [EntityColumn],
                 spilled: &mut [crate::extents::ExtentColumn],
                 value_lanes: &mut [Option<ValueLane>],
                 matched: &mut u64,
                 met: &mut [u64],
                 present: &mut [u64]|
     -> Result<()> {
        resolved.clear();
        join_chunk(chunk, n, |ordinal, _, pos| {
            let Some(ordinal) = ordinal else {
                return Err(input_changed(
                    "an attribute file names an item the identity pass did not number",
                ));
            };
            *matched += 1;
            met[ordinal as usize / 64] |= 1 << (ordinal % 64);
            resolved.push((entity_of_ordinal[ordinal as usize], pos));
            Ok(())
        })?;
        // **Ascending in the entity, so every lane's scatter is a forward sweep.** The sweep's own
        // answer arrives in source-id order, and entity ids are signature-then-Morton order, so
        // without this each lane writes its column at a uniformly random index — free while the
        // column fits in the page cache and not free otherwise. Measured on the 10⁷ MedCPT sample
        // under `MemoryMax=4G`, before this sort was added: **347,009 major faults and 480 GB read
        // for a 10.4 GiB arena, 1,335 s into a stage that costs 68 s once the scatter ascends and
        // had not finished**.
        //
        // **Stable**, so which of two rows carrying one entity is written last agrees with
        // `join_chunk`'s own answer to the same question (§7's last-write-wins).
        resolved.par_sort_by_key(|&(entity, _)| entity);
        {
            // **One lane per column, and the columns share nothing.** Each entity-order column is
            // its own mapped array with its own presence bits, so a chunk's scatter splits across
            // them with no synchronisation at all: `resolved` is read-only, and every write a lane
            // makes — the value, the presence bit, the arena append, its own share of `present` —
            // lands in storage no other lane can name.
            //
            // **Indexed rather than zipped**, because a group's columns are a subsequence of the
            // declaration: the staged buffer is this group's, and each of its columns lands in the
            // slot the declaration gave that attribute.
            let mut homes: Vec<Option<&mut EntityColumn>> =
                by_entity.iter_mut().map(Some).collect();
            let mut spills: Vec<Option<&mut crate::extents::ExtentColumn>> =
                (0..homes.len()).map(|_| None).collect();
            for column in spilled.iter_mut() {
                let at = column.column;
                spills[at] = Some(column);
            }
            let mut partitions: Vec<Option<&mut ValueLane>> =
                (0..homes.len()).map(|_| None).collect();
            for lane in value_lanes.iter_mut().flatten() {
                let at = lane.column;
                partitions[at] = Some(lane);
            }
            let mut lanes: Vec<JoinLane<'_>> = filled
                .iter()
                .zip(staged.iter_mut())
                .map(|(&column, src)| match spills[column].take() {
                    Some(out) => JoinLane::Extent { src, out },
                    None => {
                        // Every entity-order column has a value lane: a fixed-width one carries
                        // its value and a string one the `at` word, and a column with neither is
                        // spilled and was answered above.
                        let lane = partitions[column].take().expect(
                            "an entity-order column has a value lane, a spilled one an extent \
                             writer, and no column has both",
                        );
                        match lane.chars {
                            true => {
                                let home = homes[column].take().expect(
                                    "an attribute is read from exactly one source, so one lane \
                                     owns it",
                                );
                                JoinLane::Chars { src, home, lane }
                            }
                            false => JoinLane::Partitioned { src, lane },
                        }
                    }
                })
                .collect();
            // Collected per lane and folded in lane order, so a build that fails here fails with
            // the same column's message every time: which lane finished first is a scheduling
            // detail, and a build error that moves with it cannot be reproduced from its report.
            let counted: Vec<Result<u64>> = lanes
                .par_iter_mut()
                .map(|lane| match lane {
                    JoinLane::Partitioned { src, lane } => {
                        let mut count = 0u64;
                        let mut record = vec![0u8; 4 + lane.width];
                        for &(entity, pos) in resolved.iter() {
                            // **An absent row pushes nothing**, as the scatter this replaces
                            // wrote nothing for one: the column was filled absent before a row
                            // was read, and absence is the state it keeps.
                            let Some(value) = src.raw_at(pos as usize) else {
                                continue;
                            };
                            count += 1;
                            record[..4].copy_from_slice(&entity.to_le_bytes());
                            record[4..].copy_from_slice(value);
                            lane.partition.push(&record)?;
                        }
                        Ok(count)
                    }
                    JoinLane::Chars { src, home, lane } => {
                        let name = &args.schema.attributes[lane.column].name;
                        let mut count = 0u64;
                        let mut record = [0u8; 12];
                        for &(entity, pos) in resolved.iter() {
                            // **An absent row appends nothing and pushes nothing**, as the scatter
                            // this replaces wrote nothing for one.
                            let Some(word) =
                                home.take_chars_from(entity as usize, src, pos as usize, name)?
                            else {
                                continue;
                            };
                            count += 1;
                            record[..4].copy_from_slice(&entity.to_le_bytes());
                            record[4..].copy_from_slice(&word.to_le_bytes());
                            lane.partition.push(&record)?;
                        }
                        Ok(count)
                    }
                    JoinLane::Extent { src, out } => spill_extent_chunk(src, resolved, out),
                })
                .collect();
            for (slot, lane) in present.iter_mut().zip(counted) {
                *slot += lane?;
            }
        }
        // Every string in the chunk has been moved across, so the staging arena starts the next
        // chunk empty rather than growing to the whole source's payload — the buffer is reused and
        // its bytes are appended (`column.rs`).
        for column in staged.iter_mut() {
            column.reset_staging();
        }
        Ok(())
    };

    input::scan_attributes(
        input::Source::new(Some(scan.path), scan.fields, scan.rows),
        attributes,
        minters,
        |batch| {
            // Flushed **before** the batch rather than after a row count is reached, because a
            // batch is staged as a unit. Chunk boundaries are unobservable in the output — see
            // [`JOIN_STAGE_BYTES`] — so where one falls is free to be whatever keeps the buffer
            // bounded.
            if !chunk.is_empty() && chunk.len() + batch.rows.len() > staged_rows {
                flush(
                    &mut chunk,
                    &mut resolved,
                    &mut staged,
                    by_entity,
                    spilled,
                    value_lanes,
                    &mut matched_rows,
                    &mut met,
                    &mut present,
                )?;
            }
            let base = chunk.len();
            for (offset, &row) in batch.rows.iter().enumerate() {
                chunk.push((batch.ids[row as usize], (base + offset) as u32));
            }
            // The same lane-per-column split the scatter makes, over the same argument: resolving
            // a row's value and staging it are per column, and the staging columns share nothing.
            // This is where the pass spent most of its time — ~11 s of GeoNames' 17.6 s, against
            // 1.6 s in the Parquet reader (`input::scan_attributes`).
            let filled: Vec<Result<()>> = staged
                .par_iter_mut()
                .zip(batch.decoded.par_iter())
                .zip(attributes.par_iter())
                .map(|((dst, decoded), attribute)| {
                    for (offset, &row) in batch.rows.iter().enumerate() {
                        let value = decoded.value(row as usize, attribute, &args.schema)?;
                        dst.set(base + offset, value, &attribute.name)?;
                    }
                    Ok(())
                })
                .collect();
            // Folded in declaration order, for the reason the scatter's tally is.
            for lane in filled {
                lane?;
            }
            Ok(())
        },
    )?;
    if !chunk.is_empty() {
        flush(
            &mut chunk,
            &mut resolved,
            &mut staged,
            by_entity,
            spilled,
            value_lanes,
            &mut matched_rows,
            &mut met,
            &mut present,
        )?;
    }
    drop(staged);
    drop(chunk);
    drop(resolved);
    // A sweep of a source's own columns counts toward its coverage; one reading only the unique
    // fields a file carries beside them counts toward none.
    if let Some(group) = scan.group {
        let tally = &mut tallies[group];
        tally.matched += matched_rows;
        if tally.met.is_empty() {
            tally.met = met;
        } else {
            for (into, from) in tally.met.iter_mut().zip(&met) {
                *into |= from;
            }
        }
        for (into, from) in tally.present.iter_mut().zip(&present) {
            *into += from;
        }
    }
    Ok(())
}

/// **The group-scoped attribute column families** (`views.md` §5): one entity-space column per
/// view of the group, each with its own presence bitmap, under `attrs/<column>/<group>/<key>/`.
///
/// **Entity space, one column per view, and nothing per row space** — which is what keeps a scoped
/// attribute inside I2's argument: every value is indexed by entity, so a predicate over it would
/// answer a bitmap in entity space and meet the mask there, before any permutation.
///
/// The values are each view's own: the column is read from the view's points file, under that
/// view's selection where a group's views share one file (`views.md` §3.1's form B). An entity the
/// view does not hold, and one whose row carries a null, are the same state — absent, the presence
/// bitmap's ordinary case (decision 0064).
///
/// **The family's record is `MANIFEST.groups[..].scoped_scalars`** (contracts §2.2), written from
/// [`BuildArgs::scoped_attributes`] beside these files: `MANIFEST.declared_scalars` is one flat
/// bundle-wide list with no slot for a family, so the group — which is what a pin resolves
/// against — is where the declaration is recorded. Every family is a filter operand from there,
/// one column per view, resolved by the request's view or by a pinned leaf.
///
/// **What each family owes per view is what its entity-scoped counterpart owes bundle-wide**: a
/// numeric or keyword column owes values, presence and — for a keyword — its dictionary; a
/// **category** owes those and the keyed per-value postings a filter and `/v1/categories`' value
/// list are both answered from; a **text** column owes no value column at all and owes instead a
/// token dictionary and the positional postings over it. One writer per family, the same one the
/// entity-scoped pass calls, pointed at this view's directory.
///
/// **`render = true` is carried into the hot tail of each view of the group** (`views.md` §5,
/// built 2026-08-31), and of any group sharing those views via `members`. The values are entity
/// space like every other column here; what the scope decides is *which* row spaces they are
/// permuted into, which is the rule `per-point-attributes.md` §3.9 gives `render_in` with the view
/// set derived from the scope instead of listed. So a render family's columns are **retained** rather than
/// written and dropped — returned to pass two, exactly as the entity-scoped render columns are
/// held across it — and every other family's are released with the file they wrote.
#[allow(clippy::too_many_arguments)]
fn write_scoped_columns(
    args: &BuildArgs,
    prefix_dir: &Path,
    spans: &[crate::ShardSpan],
    n: u64,
    numbering: &crate::ids::Numbering,
    entity_of_ordinal: &[u32],
    minters: &mut std::collections::HashMap<String, mosaica_store::vocabulary::VocabularyMinter>,
    scratch: &crate::column::ColumnScratch,
    allowance: mosaica_filter_write::Allowance,
    steps: Steps<'_>,
) -> Result<(Vec<PathBuf>, Vec<ScopedRenderColumn>, IndexCosts)> {
    let mut paths = Vec::new();
    let mut render_columns: Vec<ScopedRenderColumn> = Vec::new();
    let plans = ColumnPlans::for_allowance(allowance);
    let mut cost = IndexCosts::default();
    for (family_index, family) in args.scoped_attributes.iter().enumerate() {
        let attribute = &family.attribute;
        // **What `render` buys, and the one place it still does not reach.** The build's own
        // views of the group get the column in their row tails below, and a view created while
        // the service runs gets one at the first flush that covers it (`views.md` §5, r24) — so
        // the rebuild this once warned about is no longer owed. What is left is the views of a
        // group that only *shares* this family's: they render the column and no batch into one
        // may carry a value, the column being the owner's. Printed once per family, where an
        // operator can act on it.
        if attribute.render
            && args
                .groups
                .iter()
                .any(|g| g.members_of.as_deref() == Some(family.group.as_str()))
        {
            eprintln!(
                "attribute '{}': `render` is carried in the hot row tail of every view of group \
                 '{}' and of every group sharing its views; ⊘ a batch into a sharing group's view \
                 may not carry a value — send it to '{}'s own view of the key, where it is entity \
                 space and reaches both (views §5)",
                attribute.name, family.group, family.group
            );
        }
        for (at, &index) in family.views.iter().enumerate() {
            let view = &args.views[index];
            let rows = match &family.source {
                Some(_) => numbering
                    .of(crate::ids::ReadKind::Scoped {
                        family: family_index,
                        view: index,
                    })
                    .expect("a scoped attribute's own file is numbered for each view"),
                None => numbering.points(index),
            };
            let mut column = read_scoped_column(
                args,
                family,
                view,
                rows,
                n,
                entity_of_ordinal,
                minters,
                scratch,
            )?;
            let view_part = Part::new("view", &view.view_id, at, family.views.len());
            for (k, span) in spans.iter().enumerate() {
                column.values.window(span.lo as usize, span.hi as usize);
                let started = std::time::Instant::now();
                let (written, indexed) = write_scoped_column(
                    prefix_dir,
                    span.layout(),
                    attribute,
                    view,
                    &column.values,
                    plans,
                    steps
                        .within(view_part)
                        .within(Part::new("shard", "", k, spans.len())),
                )?;
                paths.extend(written);
                // An indexed column is a step of its stage and its time that stage's, and the
                // attribute tail resumes after it.
                if let Some(indexed) = indexed {
                    let charge = match indexed.stage {
                        BuildStage::TextIndex => &mut cost.text,
                        _ => &mut cost.filter,
                    };
                    charge.elapsed += started.elapsed();
                    charge.rows += indexed.rows;
                    charge.passes += indexed.passes as u64;
                    steps.begin(BuildStage::AttributeTail, &[], None);
                }
            }
            column.values.window(0, n as usize);
            report_scoped_coverage(attribute, view, column.present, n);
            // **Retained for pass two, and only for a render family.** Everything else is on disc
            // and the values are dead — releasing here is what the entity-scoped pass does to its
            // own non-render columns before the row spaces are written.
            match attribute.render {
                true => render_columns.push(ScopedRenderColumn {
                    name: attribute.name.clone(),
                    ty: attribute.ty,
                    group: family.group.clone(),
                    key: key_of(&view.view_id).to_string(),
                    values: column.values,
                }),
                false => drop(column.values),
            }
        }
    }
    Ok((paths, render_columns, cost))
}

/// One shard's files of one view's column of a group-scoped family, under that shard.
///
/// An indexed column is a step of `steps`, and what it wrote is returned beside its paths.
fn write_scoped_column(
    prefix_dir: &Path,
    layout: ShardLayout<'_>,
    attribute: &crate::config::Attribute,
    view: &crate::ViewArgs,
    values: &EntityColumn,
    plans: ColumnPlans,
    steps: Steps<'_>,
) -> Result<(Vec<PathBuf>, Option<Indexed>)> {
    let mut paths = Vec::new();
    let column_part = Part::new("column", &attribute.name, 0, 1);
    let column_dir = prefix_dir.join(layout.scoped_column_rel(
        &attribute.name,
        &view.view_id,
        mosaica_types::view::DECLARED_INCARNATION,
    ));
    std::fs::create_dir_all(&column_dir).map_err(|e| BuildError::io(&column_dir, e))?;
    // **A text column has no value column**, per view exactly as bundle-wide: its entity-space
    // artefacts are the token dictionary and the postings over it, and there is no per-entity
    // slot for a scan to read. It leaves before the value column below is written rather than
    // writing one nothing opens.
    //
    // Its prose is kept beside the postings, in a record blob of its own: the bundle-wide blob is
    // addressed by a column's position in `declared_scalars`, which a family has none of, and an
    // edit of the item carries the prose to its new entity.
    if attribute.ty == ScalarType::Text {
        // **And it therefore writes no row lane**, which is only sound because `render` on `text`
        // is refused at the declaration (`config::compile_attributes`'s "`render` on `text` is
        // refused" arm). A `BuildArgs` assembled programmatically could still carry one, and it
        // would publish `render: true` on `/v1/meta` while no view's tail ever held the column —
        // the one shape serving reads as ordinary absence and could not tell from a build that
        // failed. Fenced here rather than trusted from the parser.
        debug_assert!(
            !attribute.render,
            "attribute '{}': `render` on a scoped `text` family — refused at the declaration, \
             and this pass writes no row lane for it (views §5)",
            attribute.name
        );
        // `index = false` is refused at the declaration for exactly this reason — with no blob row
        // and no index the prose would have no home at all — so the guard here is against a
        // `Schema` built programmatically rather than parsed.
        let mut indexed = None;
        if attribute.index {
            steps.begin(BuildStage::TextIndex, &[column_part], None);
            let written = write_text_index(
                &column_dir,
                attribute,
                TextValues::Arena(values),
                plans.text,
                steps,
            )?;
            paths.extend(written.paths);
            indexed = Some(Indexed {
                stage: BuildStage::TextIndex,
                rows: written.terms,
                passes: written.passes,
            });
        }
        paths.extend(write_scoped_prose(&column_dir, values)?);
        return Ok((paths, indexed));
    }
    let mut indexed = (attribute.index || scoped_postings_are_owed(attribute)).then_some(Indexed {
        stage: BuildStage::FilterPostings,
        rows: values.len() as u64,
        passes: 0,
    });
    let column_steps = match indexed {
        Some(_) => {
            steps.begin(
                BuildStage::FilterPostings,
                &[column_part],
                Some((values.len() as u64, Unit::Items)),
            );
            steps
        }
        None => Steps::default(),
    };
    let values_path = column_dir.join(mosaica_filter::VALUES_FILE);
    let presence_path = column_dir.join(mosaica_filter::PRESENCE_FILE);
    let written = write_column_values(
        &column_dir,
        attribute,
        values,
        // **A group-scoped family is not routed** ([`may_take_extents`]): its columns are this
        // pass's own and there is no blob row for an extent to land in, so the characters are
        // always the column's.
        None,
        plans.keyword,
        column_steps,
    )?;
    if let Some(indexed) = &mut indexed {
        indexed.passes = written.passes;
    }
    fsync_file(&values_path)?;
    paths.push(values_path);
    if let Some(dict_path) = written.dict {
        fsync_file(&dict_path)?;
        paths.push(dict_path);
    }
    if written.presence {
        fsync_file(&presence_path)?;
        paths.push(presence_path);
    }
    // **A category's postings, per view** — the same keyed file the entity-scoped pass writes, in
    // this view's own directory, and owed on the same predicate: an indexed category is answered
    // from them, and a `derived` vocabulary's membership is derived from them whatever `index`
    // says (`filter-index.md` §2.3). Written after the values, which are the artefact of record,
    // so a build interrupted between the two leaves the record without its accelerator rather than
    // the reverse.
    if scoped_postings_are_owed(attribute) {
        let path = column_dir.join(mosaica_filter::POSTINGS_FILE);
        write_category_postings(&path, &attribute.name, values, POSTINGS_BAND_ROWS)?;
        fsync_file(&path)?;
        paths.push(path);
    }
    Ok((paths, indexed))
}

/// An indexed column of a group-scoped family: the stage its step was of, and what that stage
/// counts of it.
struct Indexed {
    stage: BuildStage,
    rows: u64,
    /// The cascade passes its term index took.
    passes: usize,
}

/// What indexed columns cost, by the stage each is charged to.
#[derive(Default)]
pub(crate) struct IndexCosts {
    pub(crate) filter: IndexCost,
    pub(crate) text: IndexCost,
}

/// One view's column of a group-scoped `text` family, its prose as a record blob under
/// [`mosaica_store::manifest::SCOPED_PROSE_DIR`]. Returns the files written.
fn write_scoped_prose(
    column_dir: &Path,
    values: &crate::column::EntityColumn,
) -> Result<Vec<PathBuf>> {
    let dir = column_dir.join(mosaica_store::manifest::SCOPED_PROSE_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| BuildError::io(&dir, e))?;
    let files = [
        mosaica_filter::RECORD_BLOCKS_FILE,
        mosaica_filter::RECORD_HASROW_FILE,
        mosaica_filter::RECORD_DIRECTORY_FILE,
    ]
    .map(|name| dir.join(name));
    let rows = values
        .present_entities()
        .filter_map(|entity| Some((u32::try_from(entity).ok()?, values.str_at(entity)?)));
    mosaica_filter_write::write_prose(&files[0], &files[1], &files[2], rows)
        .map_err(|e| BuildError::io(&dir, e))?;
    for file in &files {
        fsync_file(file)?;
    }
    Ok(files.to_vec())
}

/// One view's column of a **rendered** group-scoped attribute, held from the pass that read it
/// until the row space that renders it is written (`views.md` §5).
///
/// Addressed by `(group, key)` rather than by a view index because the row spaces it reaches are
/// not only the owning group's: a group declaring `members` of it shares the same keys under its
/// own view ids, and its views render the family too.
struct ScopedRenderColumn {
    name: String,
    ty: ScalarType,
    group: String,
    key: String,
    values: EntityColumn,
}

/// Which retained scoped render columns each view's row space carries — one entry per
/// [`BuildArgs::views`], holding indices into `columns` (`views.md` §5).
///
/// **The view set is the scope's**, which is the whole of what `render` on a scoped attribute
/// means: a view of the family's own group renders it, so does a view of a group declaring
/// `members` of that group — the keys being the owner's by construction (`views.md` §3.3) — and
/// no other view gets a slot for it at all.
///
/// **This is `viewport::owning_key_of`'s rule, asked of the build's own arguments**, and the two
/// must agree: a column written into a row space no request reads it from is a silent nothing, and
/// one a request reads from a row space no build wrote is served as absence. It is stated twice
/// rather than shared because the serving side's inputs are a manifest and this side's are
/// `BuildArgs`, in a crate that does not depend on the engine. What keeps the one case where they
/// could differ unreachable is a **declaration refusal**: `scope` naming a group that declares
/// `members` is refused at parse (`config::compile_scope`), so a family is always the owner
/// group's and `members_of` is only ever followed in one direction.
fn scoped_render_targets(args: &BuildArgs, columns: &[ScopedRenderColumn]) -> Vec<Vec<usize>> {
    args.views
        .iter()
        .map(|view| {
            let Some((group, key)) = view.view_id.split_once(mosaica_store::GROUP_SEPARATOR) else {
                // A plain view is in no group, so no scope reaches it.
                return Vec::new();
            };
            let owner = args
                .groups
                .iter()
                .find(|g| g.name == group)
                .and_then(|g| g.members_of.as_deref())
                .unwrap_or(group);
            columns
                .iter()
                .enumerate()
                .filter(|(_, column)| column.group == owner && column.key == key)
                .map(|(index, _)| index)
                .collect()
        })
        .collect()
}

/// Does this view's column of a scoped **category** family owe its keyed postings?
///
/// [`postings_are_owed`]'s question, asked of a family: the postings are what an `eq` or an `in`
/// is answered from on a `public` vocabulary, and what `/v1/categories` derives value visibility
/// from on a `derived` one. Non-categories owe none — a number's values are near-unique and a
/// keyword's are a dictionary, so for either a posting per value is a second copy of the column
/// (this module's [`write_filter_postings`]).
///
/// **The family's filter admission is the whole condition** — `index` or `render`, the engine's
/// `filter::scoped_is_filterable` (2026-08-31) — where the entity-scoped rule is `index` *or* a
/// `derived` vocabulary. The difference is that a scoped family's admission decides both surfaces
/// at once: a family on no filter surface has no `/v1/categories` answer either, so postings
/// written for one would be read by nothing. It must agree with the engine's
/// `ScopedScalar::has_postings`, or the open demands a file no pass wrote.
///
/// **So it is one predicate, called from both** — `manifest::ScopedScalar::licence_of`, over the
/// four facts a declaration and a manifest record both carry. It lives at the record rather than
/// in the engine because `check-layers.sh` denies this crate the engine. The two used to agree by
/// argument: this pass spelled the licence `index || render` and the engine spelled it with the
/// `text` arm the declaration refuses anyway, and either could have been edited alone.
fn scoped_postings_are_owed(attribute: &crate::config::Attribute) -> bool {
    attribute.vocabulary.is_some()
        && mosaica_store::manifest::ScopedScalar::licence_of(
            attribute.ty,
            attribute.vocabulary.is_some(),
            attribute.index,
            attribute.render,
        )
}

/// Printed per column of the family, where an entity-scoped column's coverage is printed: a scoped
/// column covers the view's own rows, so *fewer than the corpus* is its ordinary state rather than
/// a symptom.
fn report_scoped_coverage(
    attribute: &crate::config::Attribute,
    view: &crate::ViewArgs,
    present: u64,
    n: u64,
) {
    eprintln!(
        "attribute '{}' in view '{}': {} of {} entities have a value",
        attribute.name,
        view.view_id,
        crate::thousands(present),
        crate::thousands(n)
    );
}

/// The key half of a view id — `2026-Q3` of `quarter:2026-Q3`.
///
/// A view of a group always carries the joined form, so the split always finds a separator; a
/// plain view's id is returned whole, which is the answer that names nothing in a group's roster
/// and is therefore selected by no row.
fn key_of(view_id: &str) -> &str {
    view_id
        .split_once(mosaica_store::GROUP_SEPARATOR)
        .map_or(view_id, |(_, key)| key)
}

/// One view's column of a group-scoped attribute, in entity space ([`write_scoped_columns`]).
struct ScopedColumn {
    values: EntityColumn,
    present: u64,
}

/// Read one view's values of a group-scoped attribute — out of that view's points file, or out of
/// the attribute's own source under that view's key.
///
/// **Two files, one selector.** Where the family declares no source of its own the view's points
/// file is the file, under the view's own selection where a form B group shares one. Where it
/// declares one, that file carries one row per `(entity, view)` and this view's rows are the ones
/// whose `fields.view` discriminator is this view's key — the same [`ViewSelector`] a form B
/// roster and a scoped layer are read through, so a value naming a key the roster does not carry
/// is the refusal that names both, and a view with no rows in the file simply has no values.
///
/// The same resolution every other attribute pass makes — a row's number → ordinal →
/// `entity_of_ordinal` — because entity ids are assigned in signature-sorted order (§11.1) and a
/// number is not its own entity id. `rows` are the file's numbers under this view's selection.
#[allow(clippy::too_many_arguments)]
fn read_scoped_column(
    args: &BuildArgs,
    family: &crate::ScopedColumnFamily,
    view: &crate::ViewArgs,
    rows: &crate::ids::ReadRows,
    n: u64,
    entity_of_ordinal: &[u32],
    minters: &mut std::collections::HashMap<String, mosaica_store::vocabulary::VocabularyMinter>,
    scratch: &crate::column::ColumnScratch,
) -> Result<ScopedColumn> {
    let attribute = &family.attribute;
    let (path, fields) = match &family.source {
        Some(source) => (Some(source.path.as_path()), &source.fields),
        None => (view.points.as_deref(), &view.point_fields),
    };
    let mut values = EntityColumn::filled(scratch, attribute.ty, n as usize)?;
    let columns = [attribute];
    let staged_rows = staging_rows(&columns, n).max(input::ATTRIBUTE_BATCH_ROWS);
    let mut staged = EntityColumn::filled(scratch, attribute.ty, staged_rows)?;
    let mut chunk: Vec<(u64, u32)> = Vec::with_capacity(staged_rows);
    let mut present = 0u64;
    // The sweep the entity-scoped pass uses, over one column.
    let flush = |chunk: &mut Vec<(u64, u32)>,
                 staged: &mut EntityColumn,
                 values: &mut EntityColumn,
                 present: &mut u64|
     -> Result<()> {
        let mut moved: Vec<(u32, u32)> = Vec::with_capacity(chunk.len());
        join_chunk(chunk, n, |ordinal, _number, pos| {
            if let Some(ordinal) = ordinal {
                moved.push((entity_of_ordinal[ordinal as usize], pos));
            }
            Ok(())
        })?;
        for (entity, pos) in moved {
            if staged.is_present(pos as usize) {
                *present += 1;
            }
            values.take_from(entity as usize, staged, pos as usize, &attribute.name)?;
        }
        staged.reset_staging();
        Ok(())
    };
    input::scan_attributes(
        input::Source::new(path, fields, rows),
        &columns,
        minters,
        |batch| {
            if !chunk.is_empty() && chunk.len() + batch.rows.len() > staged_rows {
                flush(&mut chunk, &mut staged, &mut values, &mut present)?;
            }
            let base = chunk.len();
            for (offset, &row) in batch.rows.iter().enumerate() {
                chunk.push((batch.ids[row as usize], (base + offset) as u32));
                let value = batch.decoded[0].value(row as usize, attribute, &args.schema)?;
                staged.set(base + offset, value, &attribute.name)?;
            }
            Ok(())
        },
    )?;
    if !chunk.is_empty() {
        flush(&mut chunk, &mut staged, &mut values, &mut present)?;
    }
    Ok(ScopedColumn { values, present })
}

/// Write the entity-space filter postings for every column declared `index = true`, and
/// return the paths so the manifest digests them.
///
/// **One file per column** — `attrs/<column>/postings` — never one file for the whole
/// schema. A shared file would have to address a value as `base + local` over per-column extents,
/// and under ingest a value minted after the build takes an ordinal belonging to the *next*
/// column; the base-union-tiers read then merges one value's members into another's. Because
/// vocabulary visibility is membership-derived, that shows a value to a principal on the strength
/// of a different value's members — leak-register row C11, reachable by ordinary operation
/// (filter-index §2.2).
///
/// **The keyed record format** (filter-index §2.5), not the positional one the authorisation index
/// uses. A category's identifier is its vocabulary code, and codes are a sparse subset of a
/// 32-bit space by construction — [`mosaica_store::vocabulary`] mints them scattered so that the
/// code itself carries no ordering information. A positional file would need a record per code up
/// to the largest one drawn; the keyed file stores `(code, posting)` ascending and binary-searches.
///
/// **Code 0 is the reserved *absent* code**, so an entity carrying it gets no posting. This is the
/// same zero the entity-major buffer is initialised to, which is safe only because the reader's
/// count check upstream proves every entity was visited — an unvisited entity would be
/// indistinguishable from one with no value, and here that is the difference between "absent from
/// every filter result" and "silently in whichever bucket zero names".
///
/// Entities are swept ascending, so each posting's entity list arrives sorted and
/// `encode_posting`'s unconditional sortedness check re-verifies the property this loop
/// established rather than taking it on trust.
///
/// **The emit bands code space** (filter-index §6.2), which is what keeps its transient a constant
/// the caller chooses rather than 4 B per present entity — ~4 GB per fully covered category column
/// at 10⁹, on every build, in the same pipeline whose *authorisation* emit bands term space
/// precisely to avoid this shape. A counting pass sizes each band from `band_rows`, a code is never
/// split across a band, and each band is one column scan cursor-scattering into a flat buffer laid
/// out by prefix sums. Bands partition ascending code space, so the keyed file's ascending-key
/// check ([`mosaica_authz::RecordWriter::append_keyed`]) holds across them unchanged.
///
/// What the banding does *not* bound is the attribute tail this reads from — but that tail is no
/// longer memory: `by_entity` is mapped (`column.rs`), so what this pass reads is page cache the
/// kernel may reclaim, and the band budget bounds the only anonymous buffer left in the emit. This
/// pass adds no second copy of the column to either.
pub(crate) fn write_filter_postings(
    prefix_dir: &Path,
    layout: ShardLayout<'_>,
    schema: &crate::config::Schema,
    by_entity: &[EntityColumn],
    spilled: &[crate::extents::OpenExtents],
    allowance: mosaica_filter_write::Allowance,
    steps: Steps<'_>,
) -> Result<(Vec<PathBuf>, IndexCosts)> {
    write_filter_postings_banded(
        prefix_dir,
        layout,
        schema,
        by_entity,
        spilled,
        ColumnPlans::for_allowance(allowance),
        steps,
    )
}

/// How the text and keyword passes are sized, derived from the allowance their phase leaves: a plan
/// bounds the transient one column's pass holds, and both are a function of the machine and the
/// budget rather than of which column is indexed.
#[derive(Clone, Copy)]
struct ColumnPlans {
    text: TextIndexPlan,
    keyword: KeywordDictPlan,
}

impl ColumnPlans {
    fn for_allowance(allowance: mosaica_filter_write::Allowance) -> Self {
        ColumnPlans {
            text: TextIndexPlan::for_allowance(allowance),
            keyword: KeywordDictPlan::for_allowance(allowance),
        }
    }
}

/// What indexed columns cost inside another stage's block, for [`StageTimer::charge`]: the wall
/// time and what the stage counts. The text columns are charged out of
/// [`write_filter_postings`]'s loop, and a group-scoped family's indexed columns out of the
/// attribute tail ([`write_scoped_columns`]).
///
/// **Measured here rather than at a stage boundary** because the loop visits a text column and a
/// category column in whatever order the schema declares them, and the two are different code paths
/// at different costs — a tokenise and a dictionary against a fixed-width scan. One number over
/// both is what a reader cannot act on.
#[derive(Default)]
pub(crate) struct IndexCost {
    pub(crate) elapsed: std::time::Duration,
    pub(crate) rows: u64,
    /// The cascade passes the columns' term indexes took, summed.
    pub(crate) passes: u64,
}

/// Entity ids the postings emit holds in flight — the shared emit's own constant, so the build and
/// the fold band identically (filter-index §6.2).
use mosaica_filter_write::POSTINGS_BAND_ROWS;

/// Each column is a step of its own, a step of [`BuildStage::TextIndex`] for a text column and of
/// [`BuildStage::FilterPostings`] for any other.
fn write_filter_postings_banded(
    prefix_dir: &Path,
    layout: ShardLayout<'_>,
    schema: &crate::config::Schema,
    by_entity: &[EntityColumn],
    spilled: &[crate::extents::OpenExtents],
    plans: ColumnPlans,
    steps: Steps<'_>,
) -> Result<(Vec<PathBuf>, IndexCosts)> {
    let mut paths = Vec::new();
    let mut costs = IndexCosts::default();
    let owed = schema
        .attributes
        .iter()
        .filter(|attribute| postings_are_owed(schema, attribute))
        .count();
    let mut part = 0;
    for (column, (attribute, values)) in schema.attributes.iter().zip(by_entity).enumerate() {
        if !postings_are_owed(schema, attribute) {
            continue;
        }
        let column_part = Part::new("column", &attribute.name, part, owed);
        part += 1;
        let column_dir = prefix_dir.join(layout.column_rel(&attribute.name));
        std::fs::create_dir_all(&column_dir).map_err(|e| BuildError::io(&column_dir, e))?;

        // **A text column owes no value column**, so it leaves before the one below is written.
        // Its entity-space artefact is the token dictionary and the postings over it; the values
        // themselves are in the record blob, which no scan reads (records §4.4). That is
        // [`value_column_is_owed`]'s whole exception, and the loop asks it rather than the type so
        // that the pass and [`may_take_extents`] cannot part company about which columns keep an
        // arena. A `text` column's extents are there whatever the budget said
        // ([`extents_are_forced`]), which is what lets this arm require them.
        if !value_column_is_owed(schema, attribute) {
            let extents = spilled
                .iter()
                .find(|column_extents| column_extents.column == column)
                .ok_or_else(|| {
                    BuildError::Invalid(format!(
                        "attribute '{}' is text and the join spilled no extents for it; a text \
                         column's values are its extents and there is nothing to index",
                        attribute.name
                    ))
                })?;
            let started = std::time::Instant::now();
            steps.begin(BuildStage::TextIndex, &[column_part], None);
            let written = write_text_index(
                &column_dir,
                attribute,
                TextValues::Extents(extents),
                plans.text,
                steps,
            )?;
            costs.text.elapsed += started.elapsed();
            costs.text.rows += written.terms;
            costs.text.passes += written.passes as u64;
            paths.extend(written.paths);
            continue;
        }

        // The value column is the artefact of record (filter-index §2.1); the postings below are
        // derived from it. Written first so that a build interrupted between the two leaves the
        // record without its accelerator rather than an accelerator with no record.
        //
        // **A keyword column's characters are wherever its route put them** ([`ColumnRoutes`]):
        // the column's own arena, or the extents the join spilled, which this pass is the only
        // reader of. A column of another family never spills, so the lookup answers `None` for it.
        let extents = spilled
            .iter()
            .find(|column_extents| column_extents.column == column);
        let values_path = column_dir.join(mosaica_filter::VALUES_FILE);
        let presence_path = column_dir.join(mosaica_filter::PRESENCE_FILE);
        steps.begin(
            BuildStage::FilterPostings,
            &[column_part],
            Some((values.len() as u64, Unit::Items)),
        );
        let written = write_column_values(
            &column_dir,
            attribute,
            values,
            extents,
            plans.keyword,
            steps,
        )?;
        costs.filter.passes += written.passes as u64;
        fsync_file(&values_path)?;
        paths.push(values_path);
        // The dictionary is not an accelerator and the ordering above does not apply to it: a
        // keyword's value column holds ordinals, which name nothing without the dictionary they
        // index. Both are digested, so a build interrupted between them refuses at open either
        // way — the pair is the artefact of record, not the values file alone (records §7).
        if let Some(dict_path) = written.dict {
            fsync_file(&dict_path)?;
            paths.push(dict_path);
        }
        if written.presence {
            fsync_file(&presence_path)?;
            paths.push(presence_path);
        }

        // **Only a category earns an accelerator**, and the test is *has a vocabulary* rather than
        // *is not a string*: its values already carry a code and repeat heavily, so one bitmap
        // replaces millions of repeated codes. A string's values carry no such identity, and a
        // numeric's are typically near-unique — for either, a posting per value is a second copy of
        // the column and nothing more. Both are the value column alone (filter-index §2.3, §3),
        // which is also what makes the scan the whole operation rather than a verification step
        // behind an index.
        if attribute.vocabulary.is_none() {
            continue;
        }

        let path = column_dir.join(mosaica_filter::POSTINGS_FILE);
        write_category_postings(&path, &attribute.name, values, POSTINGS_BAND_ROWS)?;
        fsync_file(&path)?;
        paths.push(path);
    }
    Ok((paths, costs))
}

/// Write the record blob — `attrs/record/{blocks.bin,hasrow.roaring,directory.arrow}` — for every
/// declared column that is neither indexed nor rendered, and return the paths so the manifest
/// digests them like every base artefact (records §3, §7).
///
/// **The blob is the third home**: a column with neither flag has no entity-space structure and no
/// slot in the hot column, so these files are the only place its values exist. When the schema
/// declares no such column the stage writes nothing and the open demands nothing — the file set
/// stays a function of the schema. (On this branch the schema *parse* still refuses a
/// neither-column; the declaration surface that admits one lands with the `used_for` migration in
/// this same epic, so the case is reachable programmatically and, after integration, from TOML.)
///
/// **Absence is per family, exactly as the value column spells it** (`write_column_values`): a
/// category's absence is the reserved code 0, everything else's is `ScalarValue::Null`. An entity
/// absent from every blob column gets no row and no has-row bit. The stage is otherwise
/// family-agnostic — a blob row is bytes, whatever family supplied them — which is what makes the
/// keyword epic's storage swap a tag rename plus rebuild rather than a format change. Whether a
/// vocabulary-controlled category may be blob-resident at all is store-once's open ruling; this
/// stage writes what the compiled placement says and takes no view.
///
/// The field tag is the column's position among `declared_scalars` — the same positional identity
/// the hot column's tail and the ingest row vector already rely on — so drill-down resolves it
/// against the manifest without any name table in the artefact.
///
/// **A spilled column arrives as extents and everything else as columns**, and the two are merged
/// here (`build-column-extents.md`). A column [`ColumnRoutes`] routed to extents was never placed
/// at an entity index: the join spilled each of its chunks as a blob extent in that chunk's entity
/// order, so this stage reads each extent front to back and takes the lowest head across them.
/// What that removes is a random read per row into a file larger than the machine, which at
/// 1.02×10⁸ abstracts wrote 52 MB in thirteen minutes at 144 major faults a second.
pub(crate) fn write_record_blob(
    record_dir: &Path,
    schema: &crate::config::Schema,
    n: u64,
    routes: &ColumnRoutes,
    by_entity: &[EntityColumn],
    spilled: &[crate::extents::OpenExtents],
) -> Result<Vec<PathBuf>> {
    // **Blob-resident is "no other home", not "no flags"** — and for a category the two differ.
    // Records §4.2 exempts categories from the blob because their entity-space structures are the
    // vocabulary machinery's constant floor, but that floor is `postings_are_owed`, which holds
    // for an *indexed* or `derived` category and not for a `public` one. A `public` category
    // declared with neither flag therefore has no hot column, no value column and no postings, so
    // excluding every category here stored its values nowhere at all and refused nothing —
    // silent loss of a field the caller declared. Asking the same question the entity-space pass
    // asks is what keeps the two exhaustive between them: a field is in exactly one home, and
    // records §3's rule that every declared field answers `entity → value` holds by construction.
    let blob_columns: Vec<usize> = schema
        .attributes
        .iter()
        .enumerate()
        .filter(|(_, a)| blob_resident(schema, a))
        .map(|(i, _)| i)
        .collect();
    if blob_columns.is_empty() {
        return Ok(Vec::new());
    }
    std::fs::create_dir_all(record_dir).map_err(|e| BuildError::io(record_dir, e))?;
    let blocks_path = record_dir.join(RECORD_BLOCKS_FILE);
    let hasrow_path = record_dir.join(RECORD_HASROW_FILE);
    let directory_path = record_dir.join(RECORD_DIRECTORY_FILE);
    let n = n as usize;
    // The columns that are still columns. A column the join spilled has its tag carried by its
    // extents instead ([`ColumnRoutes`]), and no tag is carried by both.
    let column_tags: Vec<usize> = blob_columns
        .iter()
        .copied()
        .filter(|&column| !routes.takes_extents(column))
        .collect();
    let mut columns = ColumnRows {
        schema,
        by_entity,
        columns: &column_tags,
        entity: 0,
        n,
        row: Vec::new(),
        at: 0,
    };
    // **Only a blob-resident column's extents are the blob's.** An indexed keyword spills its
    // characters the same way and its reader is the dictionary pass, which has already run and
    // unlinked them; its values belong in `values.arrow` and its tag in no blob row at all.
    let mut extents: Vec<Vec<crate::extents::ExtentRows<'_>>> = spilled
        .iter()
        .filter(|column| blob_columns.contains(&column.column))
        .map(crate::extents::ExtentRows::over)
        .collect();
    let mut sources: Vec<&mut dyn mosaica_filter_write::RecordRows> = Vec::new();
    if !column_tags.is_empty() {
        sources.push(&mut columns);
    }
    for column in extents.iter_mut() {
        for extent in column.iter_mut() {
            sources.push(extent);
        }
    }
    mosaica_filter_write::merge_record_rows(
        &mut sources,
        &croaring::Bitmap::new(),
        &blocks_path,
        &hasrow_path,
        &directory_path,
        RECORD_BLOCK_TARGET,
    )
    .map_err(|e| BuildError::io(&blocks_path, e))?;
    for path in [&blocks_path, &hasrow_path, &directory_path] {
        fsync_file(path)?;
    }
    Ok(vec![blocks_path, hasrow_path, directory_path])
}

/// [`DeclaredScalar::blob_resident`] over the build's declaration.
pub(crate) fn blob_resident(
    schema: &crate::config::Schema,
    attribute: &crate::config::Attribute,
) -> bool {
    let indexed = postings_are_owed(schema, attribute);
    DeclaredScalar::blob_resident_of(attribute.ty, indexed, attribute.render)
}

/// [`DeclaredScalar::owes_value_column`] over the build's declaration.
pub(crate) fn value_column_is_owed(
    schema: &crate::config::Schema,
    attribute: &crate::config::Attribute,
) -> bool {
    DeclaredScalar::value_column_of(attribute.ty, postings_are_owed(schema, attribute))
}

/// Could this column's characters take the record blob's extents instead of an entity-ordered
/// arena ([`crate::extents`])? Whether they do is [`ColumnRoutes`].
///
/// **The order a reader wants decides, not the declared type.** An arena is a permutation of the
/// source, and the entity-indexed offset array beside it exists to answer `entity → value` at
/// random. Every pass that reads a string column reads it once, ascending in the entity: the
/// record blob's merge, a `text` column's token index, and the keyword dictionary's chunk walk.
/// An extent delivers that order directly — each one is a join chunk in the chunk's own entity
/// order, and a merge across them is the column ascending — so the offset array buys nothing any
/// of them needs. What closes the route instead is `render`, whose row tail reads the column at a
/// row and not at an entity.
///
/// That answers `true` for every bundle-wide string column: a `text` column, a `keyword` or
/// `utf8` column declared with neither flag — the shape that cost 5.7 GB of arena and offsets at
/// 125,789,091 GBIF occurrences to hand the blob 1.55 GB
/// — and an **indexed** `keyword` or `utf8` column,
/// whose extents the dictionary pass reads through
/// [`crate::extents::OpenExtents::for_each_live_record`] where it used to read an arena at
/// `8 B/item` of offsets.
///
/// **The extents' reader differs by column and every routed column has one.** A
/// [`blob_resident`] column's are merged into the blob; an indexed one's are the dictionary
/// pass's alone and are unlinked when it ends. Extents nothing reads would be a column stored
/// nowhere, which is what the `render` term above refuses.
///
/// **`render` is refused on every string type at the declaration**
/// (`config::compile_attributes`), and `mosaica_store::write::columns_layout` refuses one that
/// reaches it anyway, so that term fires only for a `Schema` assembled programmatically.
///
/// **A group-scoped family is not asked.** Its columns are the scoped pass's own, one per view,
/// and the join that fills them is `write_scoped_columns`'s rather than this one's (`views.md`
/// §5). So a scoped string column keeps its arena whatever its flags say.
pub(crate) fn may_take_extents(
    _schema: &crate::config::Schema,
    attribute: &crate::config::Attribute,
) -> bool {
    matches!(
        attribute.ty,
        ScalarType::Utf8 | ScalarType::Keyword | ScalarType::Text
    ) && !attribute.render
}

/// Is the arena route closed to this column whatever the arena would cost?
///
/// **`text` takes extents on the permutation and not on the size** (`build-column-extents.md` §2).
/// Its arena is filled by a second decode of the source's prose, in entity order, because the
/// token index and the record blob both walk entity space and neither can afford a random read per
/// document. That second decode was the largest single cost in the build — `attribute_tail` 6,369.1 s
/// against 890.2 s at the 10⁸ PaperSeek rung — and it is paid whether or not the arena would have
/// fitted in memory. So a `text` column has one route and the choice below does not reach it.
pub(crate) fn extents_are_forced(
    schema: &crate::config::Schema,
    attribute: &crate::config::Attribute,
) -> bool {
    attribute.ty == ScalarType::Text && may_take_extents(schema, attribute)
}

/// **Which declared columns the join spills as record-blob extents, decided once for the build.**
///
/// [`may_take_extents`] says which columns have two routes; this says which of them take the
/// spilled one. The choice is [`crate::residency::plan_routes`]'s and is the modelled arena
/// against the space free on the output filesystem: an arena the disk cannot hold is the ENOSPC
/// that stops a build at hour three, and an arena it can hold costs the build a compression pass
/// it did not need. Both routes write the same bundle, byte for byte, so nothing outside the build
/// can observe which was taken.
///
/// Indexed by declaration position, and `false` for every column the route is not available to, so
/// a caller asks one question of one structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnRoutes {
    extents: Vec<bool>,
}

impl ColumnRoutes {
    /// Every column the extent route is available to takes it — the route a build takes when no
    /// arena fits, and the one the record blob's own tests want.
    pub(crate) fn every_available(schema: &crate::config::Schema) -> Self {
        ColumnRoutes {
            extents: schema
                .attributes
                .iter()
                .map(|a| may_take_extents(schema, a))
                .collect(),
        }
    }

    /// Only the columns with no arena route at all — the `text` family ([`extents_are_forced`]).
    pub(crate) fn forced_only(schema: &crate::config::Schema) -> Self {
        ColumnRoutes {
            extents: schema
                .attributes
                .iter()
                .map(|a| extents_are_forced(schema, a))
                .collect(),
        }
    }

    /// Does the column at this declaration position spill its characters as extents?
    ///
    /// A position past the schema answers `false`: the record blob's tests build a `ColumnRoutes`
    /// over the schema they are given, and a column that is not in it has no route either way.
    pub(crate) fn takes_extents(&self, column: usize) -> bool {
        self.extents.get(column).copied().unwrap_or(false)
    }

    /// Move one column onto the arena route. [`crate::residency::plan_routes`]'s only writer.
    pub(crate) fn take_arena(&mut self, column: usize) {
        self.extents[column] = false;
    }

    /// The declaration positions that spilled, for the line the build prints and the report it
    /// returns.
    pub(crate) fn spilled(&self) -> Vec<usize> {
        self.extents
            .iter()
            .enumerate()
            .filter(|&(_, &spilled)| spilled)
            .map(|(index, _)| index)
            .collect()
    }
}

/// The entity-ordered columns' rows as one ascending stream, for the blob's merge.
///
/// An entity carrying no value in any of them has no row, exactly as it had none when this stage
/// was a loop over entity space.
struct ColumnRows<'a> {
    schema: &'a crate::config::Schema,
    by_entity: &'a [EntityColumn],
    columns: &'a [usize],
    entity: usize,
    n: usize,
    /// The row the stream is at. A string field borrows the column's arena, which outlives this
    /// stream, so the row is held rather than rebuilt when the merge reads it.
    row: Vec<RecordFieldRef<'a>>,
    at: u32,
}

impl<'a> mosaica_filter_write::RecordRows for ColumnRows<'a> {
    fn advance(&mut self) -> std::io::Result<bool> {
        while self.entity < self.n {
            let entity = self.entity;
            self.entity += 1;
            self.row.clear();
            for &column in self.columns {
                let attribute = &self.schema.attributes[column];
                let value = record_value_of(&self.by_entity[column], entity, attribute)
                    .map_err(std::io::Error::other)?;
                let Some(value) = value else { continue };
                let tag = u16::try_from(column).map_err(|_| {
                    std::io::Error::other(format!(
                        "attribute '{}' is declared at position {column}, past the u16 \
                         field-tag space",
                        attribute.name
                    ))
                })?;
                self.row.push(RecordFieldRef { tag, value });
            }
            if self.row.is_empty() {
                continue;
            }
            self.at = entity as u32;
            return Ok(true);
        }
        self.row.clear();
        Ok(false)
    }
    fn entity(&self) -> u32 {
        self.at
    }
    fn field_count(&self) -> usize {
        self.row.len()
    }
    fn field(&self, i: usize) -> std::io::Result<RecordFieldRef<'_>> {
        self.row.get(i).copied().ok_or_else(|| {
            std::io::Error::other(format!(
                "field {i} was asked for of entity {}'s row, which carries {}",
                self.at,
                self.row.len()
            ))
        })
    }
}

/// One staged value as the blob row carries it, or `None` where the entity carries nothing in
/// this column — the per-family absence rule `write_record_blob`'s doc states.
///
/// **The column and the entity, not the value**, so that the string arm can borrow: reading through
/// `EntityColumn::value_at` clones the `String` out of the arena, and the row that carried it
/// owned a second copy. Both are gone — the value the blob's merge is handed borrows the arena
/// and is copied once, into the block it is encoded in.
fn record_value_of<'a>(
    values: &'a EntityColumn,
    entity: usize,
    attribute: &crate::config::Attribute,
) -> Result<Option<RecordValueRef<'a>>> {
    if attribute.vocabulary.is_some() {
        let code = category_code(&values.value_at(entity), &attribute.name)?;
        if code == mosaica_store::vocabulary::ABSENT_CODE {
            return Ok(None);
        }
        // The code at the declared width — the value the entity-space column would have stored,
        // resolved to its key at drill-down through the manifest's vocabulary, never in the
        // artefact.
        return Ok(Some(match attribute.ty {
            ScalarType::U8 => RecordValueRef::U8(code as u8),
            ScalarType::U16 => RecordValueRef::U16(code as u16),
            _ => RecordValueRef::U32(code),
        }));
    }
    // The string families first, borrowed. `str_at` answers `None` for an absent entity and for a
    // column that is not string-backed, and the match below then reads the same absence out of
    // `value_at` — so the two agree without either having to know which family it is looking at.
    if let Some(text) = values.str_at(entity) {
        return Ok(Some(RecordValueRef::Utf8(text)));
    }
    Ok(match values.value_at(entity) {
        ScalarValue::Null => None,
        ScalarValue::Bool(v) => Some(RecordValueRef::Bool(v)),
        ScalarValue::U8(v) => Some(RecordValueRef::U8(v)),
        ScalarValue::U16(v) => Some(RecordValueRef::U16(v)),
        ScalarValue::U32(v) => Some(RecordValueRef::U32(v)),
        ScalarValue::U64(v) => Some(RecordValueRef::U64(v)),
        ScalarValue::I8(v) => Some(RecordValueRef::I8(v)),
        ScalarValue::I16(v) => Some(RecordValueRef::I16(v)),
        ScalarValue::I32(v) => Some(RecordValueRef::I32(v)),
        ScalarValue::I64(v) => Some(RecordValueRef::I64(v)),
        ScalarValue::F32(v) => Some(RecordValueRef::F32(v)),
        ScalarValue::F64(v) => Some(RecordValueRef::F64(v)),
        ScalarValue::TimestampUs(v) => Some(RecordValueRef::TimestampUs(v)),
        // Unreachable: a string-backed column answered `str_at` above, and no other storage
        // yields a string. A column that reached here carrying one would be a column whose two
        // accessors disagree about its family.
        ScalarValue::Utf8(_) => {
            return Err(BuildError::Invalid(format!(
                "attribute '{}' answered a string at entity {entity} through the scalar \
                 accessor and not through the arena; its storage and its family disagree",
                attribute.name
            )))
        }
    })
}

/// The staged attribute values of one category column, as the shared postings emit reads them.
///
/// **The emit itself lives in `mosaica-filter`** (`fold::write_category_postings`), because the
/// fold rebuilds these postings from the folded column and filter-index §6.2 makes one writer
/// rather than two producers that agree the byte-identity argument. What is here is the adaptation:
/// the build's source is a `ScalarValue` per entity, where the fold's is a value column.
struct StagedCategory<'a> {
    values: &'a EntityColumn,
    column: &'a str,
}

impl mosaica_filter_write::CategorySource for StagedCategory<'_> {
    fn for_each(&self, f: &mut dyn FnMut(u32, u32) -> std::io::Result<()>) -> std::io::Result<()> {
        for (entity, value) in self.values.iter().enumerate() {
            let code = category_code(&value, self.column).map_err(std::io::Error::other)?;
            // Code 0 is the reserved *absent* code, so an entity carrying it gets no posting. This
            // is the same zero the entity-major buffer is initialised to, which is safe only
            // because the reader's count check upstream proves every entity was visited — an
            // unvisited entity would otherwise be indistinguishable from one with no value.
            if code == mosaica_store::vocabulary::ABSENT_CODE {
                continue;
            }
            f(entity as u32, code)?;
        }
        Ok(())
    }
}

fn write_category_postings(
    path: &Path,
    column: &str,
    values: &EntityColumn,
    band_rows: usize,
) -> Result<()> {
    mosaica_filter_write::write_category_postings(
        path,
        column,
        &StagedCategory { values, column },
        band_rows,
    )
    .map_err(|e| BuildError::io(path, e))
}

/// What writing one column's values produced beside the values file itself.
struct WrittenColumn {
    /// Whether a presence bitmap was written — see [`write_column_values`] on why its absence is
    /// meaningful rather than an omission.
    presence: bool,
    /// The layer's sorted dictionary, for the one family that has one.
    dict: Option<PathBuf>,
    /// The cascade passes the dictionary's merge took.
    passes: usize,
}

/// Write one column's values in entity order, its presence bitmap where presence is partial, and
/// its dictionary where the family stores one.
///
/// **A column every entity carries a value in gets no presence bitmap**, and that is the fast path
/// rather than an omission: the entity id is then the array index, which measured 28.7 ms against a
/// presence-addressed 1,078 ms at 10⁹. Writing an all-ones
/// bitmap would be correct and would cost the scan that path, so the distinction lives in the file
/// set rather than in the bitmap's contents.
///
/// **Absence is out of band in every family, and by different means.** A category spends the
/// reserved code 0, which its vocabulary reserves out of the value space. A string has no spare
/// value to spend — the empty string is one a corpus may legitimately hold, and contracts §2.4
/// already refuses it on the ingest plane because an unset field and a client bug both produce it —
/// so absence arrives as `ScalarValue::Null`. Folding the two together would report an item as
/// matching a value it does not have. A keyword inherits the string rule exactly: absence is
/// `ScalarValue::Null` and never ordinal 0, which is an ordinary key like any other.
///
/// **A keyword's values file holds `u32` ordinals into the dictionary written beside it, and both
/// belong to this layer alone** (records §4.3). The base build is one layer, so the ordinals here
/// are positions in *this* base's dictionary and mean nothing against any extent's. Nothing
/// downstream may assume otherwise — which is what keeps a durable manufactured identity, and the
/// reuse hazard that comes with one, out of the family.
///
/// The values file is `column_dir`'s [`mosaica_filter::VALUES_FILE`] and the presence bitmap its
/// [`mosaica_filter::PRESENCE_FILE`].
fn write_column_values(
    column_dir: &Path,
    attribute: &crate::config::Attribute,
    values: &EntityColumn,
    extents: Option<&crate::extents::OpenExtents>,
    keyword_plan: KeywordDictPlan,
    steps: Steps<'_>,
) -> Result<WrittenColumn> {
    let values_path = &column_dir.join(mosaica_filter::VALUES_FILE);
    let presence_path = &column_dir.join(mosaica_filter::PRESENCE_FILE);
    let mut presence = Presence::default();
    let mut dict = None;
    let mut passes = 0;
    // The entities the pass over the values has reached, all of them once it ends.
    let mut reached = steps.batched();

    let kind =
        mosaica_store::empty_shard::value_column_kind(attribute.ty, attribute.vocabulary.is_some());
    // A flat column is laid out before its values arrive, so it is told how many there will be:
    // every present entity of a number, and every present code of a `u8` category, whose absent
    // code is no value. A column written a block at a time is told nothing.
    let rows = match kind {
        mosaica_filter::ColumnKind::Ordinals | mosaica_filter::ColumnKind::Ordinals16 => None,
        _ if attribute.vocabulary.is_some() => Some(
            values
                .present_entities()
                .map(|entity| category_code(&values.value_at(entity), &attribute.name))
                .filter(|code| !matches!(code, Ok(mosaica_store::vocabulary::ABSENT_CODE)))
                .count(),
        ),
        _ => Some(values.present_count()),
    };
    let mut writer = ValueColumnWriter::create(values_path, presence_path, kind, rows)
        .map_err(|e| BuildError::io(values_path, e))?;
    macro_rules! push {
        ($chunk:expr) => {
            writer
                .push(&$chunk)
                .map_err(|e| BuildError::io(values_path, e))?
        };
    }

    // **A string's absence is not an empty string**, and a category's is not code 0 by coincidence:
    // in both families the "carries nothing" marker is out of band. A category spends the reserved
    // code 0, which its vocabulary reserves out of the value space; a string has no spare value to
    // spend — the empty string is one a corpus may legitimately hold — so absence arrives as
    // `ScalarValue::Null` and the empty string arrives as itself.
    if attribute.ty == ScalarType::Keyword {
        // The characters are wherever the route put them. A column that spilled carries no
        // values here at all ([`EntityColumn::spilled`]), so the two cases are exclusive and the
        // column is still what says how many entities there are.
        let source = match extents {
            Some(extents) => KeywordValues::Extents(extents),
            None => KeywordValues::Column(values),
        };
        let (dict_path, taken) = write_keyword_column(
            column_dir,
            values_path,
            attribute,
            source,
            &mut presence,
            &mut reached,
            &mut writer,
            keyword_plan,
        )?;
        dict = Some(dict_path);
        passes = taken;
    } else if attribute.vocabulary.is_some() {
        let mut held: Vec<u32> = Vec::new();
        for (entity, value) in values.iter().enumerate() {
            reached.add(1);
            let code = category_code(&value, &attribute.name)?;
            if code == mosaica_store::vocabulary::ABSENT_CODE {
                continue;
            }
            presence.present(entity as u32);
            held.push(code);
            if held.len() >= VALUE_CHUNK {
                push!(category_chunk(attribute.ty, &held));
                held.clear();
            }
        }
        if !held.is_empty() {
            push!(category_chunk(attribute.ty, &held));
        }
    } else {
        // **A number's absence is the presence bitmap, because a number has no spare value to
        // spend** (decision 0064). A category reserves code 0 out of its vocabulary and a string
        // carries an explicit null; every bit pattern of an integer is a legal integer, so there is
        // nothing in band to mean "no score". The bitmap beside the column is where it goes, which
        // is the same mechanism the other two families already use — so absence has one
        // representation across all three rather than three.
        //
        // Reached only where the source said so: `BatchColumn::value` returns `ScalarValue::Null`
        // for a null slot and the values buffer's zero otherwise, which is the distinction that
        // used to be dropped at decode.
        //
        // The presence bit *is* that distinction — a slot is null exactly where the bit is clear,
        // whatever the family — so this walks the bits and skips an absent run 64 at a time rather
        // than materialising a `ScalarValue` per entity to ask it the same question.
        for entity in values.present_entities() {
            presence.present(entity as u32);
        }
        push_numeric_chunks(&mut writer, values_path, attribute, values, &mut reached)?;
    }
    reached.reach(values.len() as u64);
    let presence = presence.written(values.len());
    writer
        .finish(presence)
        .map_err(|e| BuildError::io(values_path, e))?;
    Ok(WrittenColumn {
        presence: presence.is_some(),
        dict,
        passes,
    })
}

/// The presence bitmap a column may or may not owe, **populated only once an absence proves it
/// will be written**.
///
/// A column every entity carries a value in gets no bitmap at all ([`write_column_values`] on why
/// that is the fast path and not an omission), and the pass that discovers this used to build the
/// bitmap anyway: 7.4×10⁷ `add` calls per column, thrown away at the last line. Here the sweep
/// reports only the entities that carry a value, ascending; absence is inferred from the gaps and
/// from the tail, and the bitmap is materialised at the first gap by replaying the run before it.
///
/// **The replay is a loop of `add`, not an `add_range`**, so the bitmap receives exactly the call
/// sequence the eager version made — same entities, same ascending order, same container
/// promotions, and so the same serialised bytes. A range insert may produce a run container where
/// individual inserts produce an array one, which is a different file for the same set.
#[derive(Default)]
struct Presence {
    bitmap: croaring::Bitmap,
    /// The entity after the last one reported present, while no gap has been seen.
    run: u32,
    materialised: bool,
}

impl Presence {
    /// Report that `entity` carries a value. Entities must arrive ascending.
    fn present(&mut self, entity: u32) {
        if !self.materialised {
            if entity == self.run {
                self.run = entity + 1;
                return;
            }
            self.materialise();
        }
        self.bitmap.add(entity);
        self.run = entity + 1;
    }

    /// The bitmap to write, or `None` where the column is universal — every entity of `len`
    /// present, which is the case the file set states by leaving the bitmap out.
    fn written(&mut self, len: usize) -> Option<&croaring::Bitmap> {
        if !self.materialised {
            if self.run as usize == len {
                return None;
            }
            self.materialise();
        }
        Some(&self.bitmap)
    }

    fn materialise(&mut self) {
        for entity in 0..self.run {
            self.bitmap.add(entity);
        }
        self.materialised = true;
    }
}

/// Visit one keyword column's present values in entity order, as `(row, value)`, with `presence`
/// told which entities carry one. Returns how many rows carry a value.
///
/// **The row is the value's slot, not the entity id**: the k-th set bit's value is at slot k
/// (filter-index §2.1). The pass that decides *presence* is therefore the pass that decides
/// *slots*, and both come out of this one walk, so the two cannot come to disagree about an absent
/// entity.
///
/// **A keyword's values arrive as [`ScalarValue::Utf8`]**, because that is what the wire carries
/// (records §7) — the type names the storage, not the value in flight.
///
/// **The empty string is refused, where a `utf8` column stores it.** That is the families
/// differing, not this pass being stricter than it need be: records §7 refuses an empty keyword on
/// the ingest wire for the reason contracts §2.4 gives — an unset field and a client bug both
/// produce it — and the dictionary has no key for it either. A points file is not the ingest plane
/// and has no upstream check, so the refusal is here, naming the column and the entity a build
/// operator has to go and fix.
fn for_each_keyword(
    attribute: &crate::config::Attribute,
    values: KeywordValues<'_>,
    presence: &mut Presence,
    reached: &mut Batched<'_>,
    visit: &mut dyn FnMut(u32, &str) -> Result<()>,
) -> Result<u64> {
    let mut rows = 0u64;
    let mut step = |entity: u32, text: &str| -> Result<()> {
        reached.reach(u64::from(entity) + 1);
        if text.is_empty() {
            return Err(BuildError::Invalid(format!(
                "attribute '{}' is declared `keyword` and entity {entity} carries the empty \
                 string, which is not a value (records §7, contracts §2.4 — an unset field and a \
                 client bug both produce it). Leave the cell null for absence",
                attribute.name
            )));
        }
        // **A row is addressed by a `u32` in the run files below**, so a column with more rows
        // than that is refused here rather than wrapping into another row's ordinal. Entity ids
        // are `u32` and a row exists only where an entity does, so this is reachable only at the
        // very top of entity space.
        let row = u32::try_from(rows).map_err(|_| {
            BuildError::Invalid(format!(
                "attribute '{}': more than {} rows carry a value, which is more than the \
                 dictionary pass addresses a row with",
                attribute.name,
                u32::MAX
            ))
        })?;
        presence.present(entity);
        visit(row, text)?;
        rows += 1;
        Ok(())
    };
    match values {
        // Absent runs are skipped a word at a time; absence itself is what [`Presence`] reads out
        // of the gaps this leaves.
        KeywordValues::Column(values) => {
            for entity in values.present_entities() {
                // Borrowed, not read through `value_at`: this visits one `&str` per entity across
                // the whole column, so copying here would be a second copy of every keyword in the
                // corpus.
                let Some(text) = values.str_at(entity) else {
                    return Err(BuildError::Invalid(format!(
                        "attribute '{}' is declared `keyword` but carries {:?}",
                        attribute.name,
                        values.value_at(entity)
                    )));
                };
                step(entity as u32, text)?;
            }
        }
        KeywordValues::Extents(extents) => {
            extents.for_each_live_record(&mut |entity, text| step(entity, text))?;
        }
    }
    Ok(rows)
}

/// Where one keyword column's characters are read from — the two routes [`ColumnRoutes`] chooses
/// between, seen from the one pass that consumes them.
///
/// Both deliver each present entity once, ascending, carrying the value the join wrote last for
/// it: the column through its presence bits and its offset array, the extents through a merge
/// across the join chunks with each one's live set applied (`crate::extents`). So the dictionary,
/// the ordinals and the presence bitmap are the same bytes either way, and the choice is the
/// build's disk and not the bundle's.
#[derive(Clone, Copy)]
pub(crate) enum KeywordValues<'a> {
    Column(&'a EntityColumn),
    Extents(&'a crate::extents::OpenExtents),
}

impl KeywordValues<'_> {
    /// A ceiling on the present rows, for sizing a sorted chunk: the column's entities, or the
    /// extents' rows before their live sets take the overwritten ones out.
    fn rows_hint(&self) -> u64 {
        match self {
            KeywordValues::Column(values) => values.len() as u64,
            KeywordValues::Extents(extents) => extents.blobs.iter().map(|blob| blob.rows()).sum(),
        }
    }
}

/// How one indexed keyword column's dictionary pass is sized: the rows a sorted chunk buffers, the
/// distinct keys a hashed chunk holds, and the characters either holds.
///
/// **The same allowance the text pass takes** ([`crate::residency::Sizing::term_index`]), because
/// both run in the same phase, one column at a time. The pass is one walk of the column in row
/// order, so one chunk is live and the allowance is that chunk, up to what its `u32` offsets
/// address ([`mosaica_filter_write::Allowance::chunk`]); its cascade and merge then read the runs
/// under the same allowance.
///
/// **A hashed chunk is bounded by its keys, not its rows**: its rows leave as ids the moment they
/// are read, so a column with few keys is one chunk at any row count. A sorted chunk holds its rows
/// until it sorts them, so its bound is rows. Either closes when what its keys or rows cost and its
/// characters together reach the chunk. Where the boundaries fall changes no byte of the output
/// ([`tests::chunking_the_keyword_column_does_not_change_its_bytes`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct KeywordDictPlan {
    chunk_rows: usize,
    chunk_keys: usize,
    chunk_bytes: usize,
    /// Rows a hashed chunk reads between two looks at its rate of new keys.
    window: u64,
    /// One way for every chunk, for the tests that must reach each; `None` chooses per chunk.
    forced: Option<ChunkMode>,
    /// Keys, and records, per batch the merge hands to its writers.
    merge_batch: usize,
    /// What the cascade and the merge read the runs under.
    allowance: mosaica_filter_write::Allowance,
    /// How the merge is divided into key ranges.
    ranges: mosaica_filter_write::Ranges,
}

/// The floor on a chunk, so a tiny `--memory-budget` cannot derive a plan that spills a run every
/// few rows and then cascades them all back together.
const KEYWORD_MIN_CHUNK_ROWS: usize = 1 << 12;

/// The floor on a chunk's characters, for the reason [`KEYWORD_MIN_CHUNK_ROWS`] is the floor on
/// its rows.
const KEYWORD_MIN_CHUNK_BYTES: usize = 1 << 20;

impl KeywordDictPlan {
    /// The plan an allowance derives.
    pub(crate) fn for_allowance(allowance: mosaica_filter_write::Allowance) -> KeywordDictPlan {
        let bytes = allowance.chunk(1);
        KeywordDictPlan {
            chunk_rows: (bytes / KEYWORD_ROW_BYTES).max(KEYWORD_MIN_CHUNK_ROWS),
            chunk_keys: (bytes / KEYWORD_KEY_BYTES).max(KEYWORD_MIN_CHUNK_ROWS),
            chunk_bytes: bytes.max(KEYWORD_MIN_CHUNK_BYTES),
            window: mosaica_filter_write::KEY_WINDOW_ROWS,
            forced: None,
            merge_batch: KEYWORD_MERGE_BATCH,
            allowance,
            ranges: mosaica_filter_write::Ranges::Planned,
        }
    }

    /// An explicit plan, for the tests that must force several chunks, a cascade and several merge
    /// batches out of a corpus small enough to assert over: `chunk` bounds both a sorted chunk's
    /// rows and a hashed chunk's keys.
    #[cfg(test)]
    fn explicit(chunk: usize, window: u64, forced: Option<ChunkMode>) -> KeywordDictPlan {
        KeywordDictPlan {
            chunk_rows: chunk.max(1),
            chunk_keys: chunk.max(1),
            chunk_bytes: usize::MAX,
            window: window.max(1),
            forced,
            merge_batch: 3,
            allowance: tests::TEST_MERGE,
            ranges: mosaica_filter_write::Ranges::Planned,
        }
    }

    /// This plan with its merge divided as `ranges` says.
    #[cfg(test)]
    fn in_ranges(self, ranges: mosaica_filter_write::Ranges) -> KeywordDictPlan {
        KeywordDictPlan { ranges, ..self }
    }
}

/// How one chunk ranks its rows ([`mosaica_filter_write::prefers_sort`] chooses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChunkMode {
    /// Each row is looked up in a table and leaves as a provisional id; the distinct keys are
    /// sorted at the end.
    Hash,
    /// Each row is buffered and sorted on its key; the ranks leave in row order at the end.
    Sort,
}

/// What one buffered row of a sorted chunk costs beyond its characters: its entry in the array the
/// chunk sorts, and its rank.
const KEYWORD_ROW_BYTES: usize =
    std::mem::size_of::<mosaica_filter_write::KeyRow>() + std::mem::size_of::<u32>();

/// What one distinct key of a hashed chunk costs beyond its characters at the table's peak, which
/// is while it grows: its entry, twice over because a vector that has just grown is half empty,
/// and six slots, the two of the table at half load and the four of the table it grows into.
/// Ranking releases the table before it sorts, and its 24 B row and 4 B rank are within this.
const KEYWORD_KEY_BYTES: usize =
    2 * std::mem::size_of::<mosaica_filter_write::KeyRef>() + 6 * std::mem::size_of::<u64>();

/// The partition the merge pushes `(chunk, ordinal)` into, one record per key of each chunk.
const KEYWORD_REMAP_PARTITION: &str = "keyword-remap";

/// One `(chunk, ordinal)` record of that partition: two little-endian `u32`s, the chunk first
/// because it is the key the partition routes on.
const KEYWORD_REMAP_RECORD: usize = 8;

/// Keys, and records, per batch the merge hands to the dictionary's and the remap's writers, at
/// [`KeywordDictPlan::for_allowance`].
const KEYWORD_MERGE_BATCH: usize = 1 << 14;

/// Batches in flight to each writer.
const KEYWORD_MERGE_DEPTH: usize = 4;

/// A scoped thread's result, with its panic resumed on the joining thread rather than lost.
fn joined<T>(handle: std::thread::ScopedJoinHandle<'_, T>) -> T {
    handle
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// Merge a keyword column's runs into `merged` in key ranges, as `ranges` says, on the allowance's
/// threads, each key's chunks drained into its remap records.
fn merge_keyword_runs(
    runs: &[SpillReceipt],
    merged: KeywordMerged,
    allowance: mosaica_filter_write::Allowance,
    ranges: mosaica_filter_write::Ranges,
) -> std::io::Result<KeywordMerged> {
    let how = mosaica_filter_write::RunMerge {
        payload: RunPayload::Rows,
        allowance,
        ranges,
        each: 0,
        beside: mosaica_filter_write::Beside::none(),
    };
    mosaica_filter_write::merge_runs(runs, merged, how, |range, readers, turn| {
        let mut merge = TermMerge::range(readers, range)?;
        let (mut carriers, mut bytes) = (Vec::<u32>::new(), Vec::<u8>::new());
        while merge.next_term()? {
            carriers.clear();
            merge.drain(std::convert::identity, &mut |chunk, _| {
                carriers.push(chunk);
                Ok(())
            })?;
            match turn.out()? {
                mosaica_filter_write::Out::Writer(merged) => {
                    merged.push(merge.term(), &carriers)?
                }
                mosaica_filter_write::Out::Held(held) => {
                    held.keys.push(merge.term());
                    bytes.clear();
                    bytes.extend(carriers.iter().flat_map(|chunk| chunk.to_le_bytes()));
                    held.chunks.push(&bytes);
                }
            }
            if turn.full() {
                turn.wait()?;
            }
        }
        Ok(merge.into_sources())
    })
    .map(|(merged, _)| merged)
}

/// The keyword merge's output as it is written: each key to the dictionary's writer, and a remap
/// record `(chunk, ordinal)` for each chunk holding it to the remap's.
struct KeywordMerged {
    keys: KeySender,
    records: Vec<u8>,
    queue: std::sync::mpsc::SyncSender<Vec<u8>>,
    /// Records a batch.
    batch: usize,
    /// Keys written, and remap records.
    written: u64,
    emitted: u64,
    chunks: usize,
}

impl KeywordMerged {
    /// Write `key`, the next distinct key, carried by `carriers`.
    fn push(&mut self, key: &[u8], carriers: &[u32]) -> std::io::Result<()> {
        let invalid = |detail: String| std::io::Error::new(std::io::ErrorKind::InvalidData, detail);
        let ordinal = u32::try_from(self.written).map_err(|_| {
            invalid("the column has more distinct keys than a u32 can number".to_string())
        })?;
        self.keys
            .push(key)
            .map_err(|_| std::io::Error::other("a keyword writer thread stopped"))?;
        for &chunk in carriers {
            if chunk as usize >= self.chunks {
                return Err(invalid(format!(
                    "a key is carried by chunk {chunk}, past the {} chunks the column has",
                    self.chunks
                )));
            }
            self.records.extend_from_slice(&chunk.to_le_bytes());
            self.records.extend_from_slice(&ordinal.to_le_bytes());
            self.emitted += 1;
        }
        self.written += 1;
        if self.records.len() >= self.batch * KEYWORD_REMAP_RECORD {
            self.queue
                .send(std::mem::take(&mut self.records))
                .map_err(|_| std::io::Error::other("a keyword writer thread stopped"))?;
        }
        Ok(())
    }
}

/// The keys a range of the keyword merge holds until its turn, and for each the chunks holding it
/// as little-endian `u32`s.
#[derive(Default)]
struct HeldKeywords {
    keys: HeldRecords,
    chunks: HeldRecords,
}

impl mosaica_filter_write::RangeWriter for KeywordMerged {
    type Held = HeldKeywords;

    fn held_bytes(held: &HeldKeywords) -> usize {
        held.keys.bytes() + held.chunks.bytes()
    }

    fn write_held(&mut self, held: &mut HeldKeywords) -> std::io::Result<()> {
        let mut carriers: Vec<u32> = Vec::new();
        for (key, chunks) in held.keys.records().zip(held.chunks.records()) {
            carriers.clear();
            carriers.extend(
                chunks
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|chunk| u32::from_le_bytes(*chunk)),
            );
            self.push(key, &carriers)?;
        }
        held.keys.clear();
        held.chunks.clear();
        Ok(())
    }
}

/// The sending half of a writer thread's queue of keys, `size` keys a batch. Dropping it closes the
/// queue, which is what ends the writer's loop.
struct KeySender {
    batch: HeldRecords,
    queue: std::sync::mpsc::SyncSender<HeldRecords>,
    size: usize,
}

impl KeySender {
    fn new(queue: std::sync::mpsc::SyncSender<HeldRecords>, size: usize) -> KeySender {
        KeySender {
            batch: HeldRecords::default(),
            queue,
            size,
        }
    }

    /// Add a key, sending the batch once it holds `size` keys or [`KEYWORD_BATCH_BYTES`] of
    /// characters.
    fn push(&mut self, key: &[u8]) -> Result<()> {
        self.batch.push(key);
        if self.batch.len() >= self.size || self.batch.bytes() >= KEYWORD_BATCH_BYTES {
            self.flush()?;
        }
        Ok(())
    }

    /// Send the keys held.
    fn flush(&mut self) -> Result<()> {
        if self.batch.is_empty() {
            return Ok(());
        }
        self.queue
            .send(std::mem::take(&mut self.batch))
            .map_err(|_| writer_stopped())
    }
}

/// The error a send to a writer thread that has stopped returns. The writer's own error is the one
/// reported, when it is joined.
fn writer_stopped() -> BuildError {
    BuildError::Invalid("a keyword writer thread stopped".to_string())
}

/// The characters a batch of keys holds before it is sent, whatever its key count.
const KEYWORD_BATCH_BYTES: usize = 1 << 20;

/// Ids per record of the id and perm streams. A record is a block rather than one id so the
/// stream's anchor and buffering are paid per block; the last block is padded with zeros.
const KEYWORD_ID_BLOCK: usize = 2048;

/// A stream of `u32`s spilled in [`KEYWORD_ID_BLOCK`]s.
struct IdWriter {
    spill: spill::SpillWriter,
    block: Vec<u8>,
    count: u64,
}

impl IdWriter {
    fn create(path: &Path) -> Result<IdWriter> {
        Ok(IdWriter {
            spill: spill::SpillWriter::create(path)?,
            block: Vec::with_capacity(KEYWORD_ID_BLOCK * 4),
            count: 0,
        })
    }

    fn push(&mut self, id: u32) -> Result<()> {
        self.block.extend_from_slice(&id.to_le_bytes());
        self.count += 1;
        if self.block.len() == KEYWORD_ID_BLOCK * 4 {
            self.spill.push_bytes(&self.block)?;
            self.block.clear();
        }
        Ok(())
    }

    /// The receipt and the ids pushed.
    fn finish(mut self) -> Result<(SpillReceipt, u64)> {
        if !self.block.is_empty() {
            self.block.resize(KEYWORD_ID_BLOCK * 4, 0);
            self.spill.push_bytes(&self.block)?;
        }
        Ok((self.spill.finish()?, self.count))
    }
}

/// An [`IdWriter`]'s stream read back, `count` ids, verified against its receipt at the end.
struct IdReader {
    records: spill::RecordReader,
    block: Vec<u32>,
    at: usize,
    left: u64,
}

impl IdReader {
    fn open(receipt: &SpillReceipt, count: u64) -> Result<IdReader> {
        if receipt.count != count.div_ceil(KEYWORD_ID_BLOCK as u64) {
            return Err(BuildError::Invalid(format!(
                "{}: {} blocks where {count} ids need {}",
                receipt.path.display(),
                receipt.count,
                count.div_ceil(KEYWORD_ID_BLOCK as u64)
            )));
        }
        Ok(IdReader {
            records: spill::RecordReader::open(receipt, KEYWORD_ID_BLOCK * 4)?,
            block: Vec::with_capacity(KEYWORD_ID_BLOCK),
            at: 0,
            left: count,
        })
    }

    /// The next ids, at most `want` and at least one, from the block in hand.
    fn take(&mut self, want: u64) -> Result<&[u32]> {
        if self.left == 0 || want == 0 {
            return Err(BuildError::Invalid(
                "a keyword id stream was read past the ids it holds".to_string(),
            ));
        }
        if self.at == self.block.len() {
            let record = self.records.next_record()?.ok_or_else(|| {
                BuildError::Invalid("a keyword id stream ends before its last block".to_string())
            })?;
            self.block.clear();
            self.block.extend(
                record
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|bytes| u32::from_le_bytes(*bytes)),
            );
            self.at = 0;
        }
        let n = (self.block.len() - self.at).min(want.min(self.left) as usize);
        self.left -= n as u64;
        self.at += n;
        Ok(&self.block[self.at - n..self.at])
    }

    /// Check that every id was read and the stream's anchor holds.
    fn finish(mut self) -> Result<()> {
        if self.left != 0 {
            return Err(BuildError::Invalid(format!(
                "a keyword id stream holds {} ids past the last chunk",
                self.left
            )));
        }
        if self.records.next_record()?.is_some() {
            return Err(BuildError::Invalid(
                "a keyword id stream holds a block past the ids it counted".to_string(),
            ));
        }
        Ok(())
    }
}

/// One closed chunk: how it ranked its rows, how many rows it read, and how many distinct keys
/// they carried.
struct ChunkMeta {
    mode: ChunkMode,
    rows: u64,
    keys: u32,
}

/// The chunk pass: rows in, an id per row and one sorted run of keys per chunk out.
///
/// **The id stream** holds one `u32` per row, in row order: a hashed chunk's provisional ids as
/// the rows arrive, a sorted chunk's ranks when it closes. **The perm stream** holds, for each
/// hashed chunk in turn, the rank of each provisional id. **A run** holds a chunk's distinct keys
/// ascending, each carrying the chunk's index as its one entity, so a merge of runs yields, per
/// key, the chunks that hold it.
struct KeywordChunks<'a> {
    dir: &'a Path,
    plan: KeywordDictPlan,
    mode: ChunkMode,
    /// The open chunk's characters; every [`mosaica_filter_write::KeyRef`] indexes into it.
    text: Vec<u8>,
    table: mosaica_filter_write::KeyTable,
    pairs: Vec<mosaica_filter_write::KeyRow>,
    rows: u64,
    /// Rows the column may still hold, so a sorted chunk is sized once and not grown.
    rows_left: u64,
    rule: mosaica_filter_write::WindowRule,
    ids: IdWriter,
    perms: IdWriter,
    runs: Vec<SpillReceipt>,
    chunks: Vec<ChunkMeta>,
    scratch: Vec<u32>,
}

/// The chunk's own characters for one of its keys.
fn chunk_key<'t>(text: &'t [u8], key: &mosaica_filter_write::KeyRef) -> &'t [u8] {
    &text[key.at as usize..key.at as usize + key.len as usize]
}

impl<'a> KeywordChunks<'a> {
    fn create(dir: &'a Path, plan: KeywordDictPlan, rows_hint: u64) -> Result<KeywordChunks<'a>> {
        Ok(KeywordChunks {
            dir,
            plan,
            mode: plan.forced.unwrap_or(ChunkMode::Hash),
            text: Vec::new(),
            table: mosaica_filter_write::KeyTable::default(),
            pairs: Vec::new(),
            rows: 0,
            rows_left: rows_hint,
            rule: mosaica_filter_write::WindowRule::default(),
            ids: IdWriter::create(&dir.join("keyword-ids.spill"))?,
            perms: IdWriter::create(&dir.join("keyword-perms.spill"))?,
            runs: Vec::new(),
            chunks: Vec::new(),
            scratch: Vec::new(),
        })
    }

    /// Append `key` to the chunk's characters and return the reference to it.
    fn buffer(&mut self, key: &str) -> Result<mosaica_filter_write::KeyRef> {
        let at = u32::try_from(self.text.len()).map_err(|_| {
            BuildError::Invalid(format!(
                "a keyword chunk buffered {} bytes of characters, past the u32 a buffered key's \
                 position is held in; give the build a smaller memory budget",
                self.text.len()
            ))
        })?;
        self.text.extend_from_slice(key.as_bytes());
        mosaica_filter_write::KeyRef::new(key.as_bytes(), at)
            .map_err(|e| BuildError::Invalid(e.to_string()))
    }

    fn push(&mut self, key: &str) -> Result<()> {
        self.rows += 1;
        match self.mode {
            ChunkMode::Hash => {
                let text = &self.text;
                let (id, new) = match self.table.find(key.as_bytes(), |k| chunk_key(text, k)) {
                    mosaica_filter_write::Lookup::Found(id) => (id, false),
                    mosaica_filter_write::Lookup::Vacant(vacant) => {
                        let at = self.buffer(key)?;
                        let id = self
                            .table
                            .insert(vacant, at)
                            .map_err(|e| BuildError::Invalid(e.to_string()))?;
                        (id, true)
                    }
                };
                self.ids.push(id)?;
                if self.table.len() >= self.plan.chunk_keys
                    || self.table.len() * KEYWORD_KEY_BYTES + self.text.len()
                        >= self.plan.chunk_bytes
                {
                    self.close(None)?;
                } else if self.plan.forced.is_none() && self.rule.row(new, self.plan.window) {
                    self.close(Some(ChunkMode::Sort))?;
                }
            }
            ChunkMode::Sort => {
                if self.pairs.len() == self.pairs.capacity() {
                    let room = self.plan.chunk_rows.saturating_sub(self.pairs.len());
                    let rows = (room as u64).min(self.rows_left.max(1)).max(1);
                    self.pairs.reserve_exact(rows as usize);
                }
                let row = u32::try_from(self.pairs.len()).map_err(|_| {
                    BuildError::Invalid(
                        "a sorted keyword chunk holds more rows than a u32 can number; give the \
                         build a smaller memory budget"
                            .to_string(),
                    )
                })?;
                let at = self.buffer(key)?;
                self.pairs
                    .push(mosaica_filter_write::KeyRow::new(key.as_bytes(), at, row));
                if self.pairs.len() >= self.plan.chunk_rows
                    || self.pairs.len() * KEYWORD_ROW_BYTES + self.text.len()
                        >= self.plan.chunk_bytes
                {
                    self.close(None)?;
                }
            }
        }
        Ok(())
    }

    /// Rank the open chunk, write its run and its ranks, and open the next one: as `switch` says,
    /// else hashed where this chunk's keys were at most one in ten of its rows and sorted where
    /// they were more.
    fn close(&mut self, switch: Option<ChunkMode>) -> Result<()> {
        let mut next = self.mode;
        if self.rows > 0 {
            let chunk = u32::try_from(self.chunks.len()).map_err(|_| {
                BuildError::Invalid(
                    "a keyword column spilled more chunks than a u32 can number".to_string(),
                )
            })?;
            let dir = &self.dir;
            let path = dir.join(format!("keyword-run-{chunk:05}.spill"));
            let (mode, size) = (self.mode, self.plan.merge_batch);
            let (text, table, pairs) = (&self.text, &mut self.table, &mut self.pairs);
            let (scratch, ids, perms) = (&mut self.scratch, &mut self.ids, &mut self.perms);
            // The run is written on its own thread while the chunk is ranked.
            let (keys, run) = std::thread::scope(|scope| -> Result<(u32, SpillReceipt)> {
                let (queue, batches) =
                    std::sync::mpsc::sync_channel::<HeldRecords>(KEYWORD_MERGE_DEPTH);
                let writer = scope.spawn(move || -> std::io::Result<SpillReceipt> {
                    let mut run = TextRunWriter::create(&path, RunPayload::Rows)?;
                    for batch in batches {
                        for key in batch.records() {
                            run.begin(key, 1)?;
                            run.push_entity(chunk, &[])?;
                        }
                    }
                    run.finish()
                });
                let mut sender = KeySender::new(queue, size);
                let mut emit =
                    |key: &mosaica_filter_write::KeyRef| sender.push(chunk_key(text, key));
                let ranked = match mode {
                    ChunkMode::Hash => mosaica_filter_write::rank_table(
                        table,
                        |k| chunk_key(text, k),
                        scratch,
                        mosaica_filter_write::SortOn::Pool,
                        &mut emit,
                    ),
                    ChunkMode::Sort => {
                        scratch.clear();
                        scratch.resize(pairs.len(), 0);
                        mosaica_filter_write::rank_rows(
                            pairs,
                            |k| chunk_key(text, k),
                            scratch,
                            mosaica_filter_write::SortOn::Pool,
                            &mut emit,
                        )
                    }
                };
                let sent = ranked.and_then(|keys| sender.flush().map(|()| keys));
                drop(sender);
                let run = joined(writer).map_err(|e| spill::run_error(dir, e))?;
                let keys = sent?;
                let ranks = if mode == ChunkMode::Hash { perms } else { ids };
                for &rank in scratch.iter() {
                    ranks.push(rank)?;
                }
                Ok((keys, run))
            })?;
            self.runs.push(run);
            next = if mosaica_filter_write::prefers_sort(keys as u64, self.rows) {
                ChunkMode::Sort
            } else {
                ChunkMode::Hash
            };
            self.chunks.push(ChunkMeta {
                mode: self.mode,
                rows: self.rows,
                keys,
            });
        }
        self.rows_left = self.rows_left.saturating_sub(self.rows);
        self.rows = 0;
        self.rule = mosaica_filter_write::WindowRule::default();
        let next = self.plan.forced.or(switch).unwrap_or(next);
        // A buffer keeps its capacity for a next chunk of its own mode and is released for one of
        // the other, so the two modes' buffers are never held at once.
        self.text.clear();
        self.scratch.clear();
        match next {
            ChunkMode::Hash if self.mode == ChunkMode::Hash => self.table.clear(),
            ChunkMode::Sort if self.mode == ChunkMode::Sort => self.pairs.clear(),
            _ => {
                self.table = mosaica_filter_write::KeyTable::default();
                self.pairs = Vec::new();
                self.scratch = Vec::new();
            }
        }
        self.mode = next;
        Ok(())
    }

    /// Close the last chunk and hand back the streams and runs, releasing the chunk's buffers.
    fn finish(mut self) -> Result<(IdWriter, IdWriter, Vec<SpillReceipt>, Vec<ChunkMeta>)> {
        self.close(None)?;
        Ok((self.ids, self.perms, self.runs, self.chunks))
    }
}

/// One indexed keyword column's dictionary and its ordinal values file. Returns the dictionary's
/// path and the cascade passes its merge took.
///
/// # The shape: chunk, merge the keys, translate the ids
///
/// 1. **Chunk.** The present rows are walked once in row order. Each chunk ranks its rows among
///    its own distinct keys, by a table or by a sort ([`KeywordChunks`]), and writes those keys,
///    sorted, as one run. A row leaves the chunk as a `u32` in the id stream and never enters the
///    merge.
/// 2. **Cascade**, where the chunks are more runs than one merge may hold file descriptors for
///    ([`cascade_sorted_runs`]).
/// 3. **Merge.** The runs are merged k-way on the key, in key ranges on the allowance's threads
///    and written in key order ([`merge_keyword_runs`]). Each distinct key is pushed once to
///    [`mosaica_filter::SortedDictWriter`], and the ordinal it returns is pushed to the remap
///    partition once for every chunk holding the key. A chunk's keys leave the merge ascending, so
///    its remap arrives in rank order: entry `r` is the ordinal of the chunk's `r`-th key.
/// 4. **Translate.** The id stream is read front to back. For each chunk, its remap (and, for a
///    hashed chunk, its perm) is loaded and each id becomes `remap[perm[id]]` or `remap[id]`,
///    pushed to the values writer in [`VALUE_CHUNK`] slices.
///
/// Disk is 4 B per row of ids, 8 B per chunk key of remap and 4 B per hashed chunk key of perm,
/// beside the runs. Memory is one chunk while chunking, and while translating one remap
/// partition bucket's remaps, 4 B a key, beside one hashed chunk's table, 4 B a key.
///
/// **The output is a function of the corpus alone, never of the plan.** The dictionary is the
/// sorted distinct key set and a row's ordinal is its key's position in it; neither depends on
/// where a chunk boundary fell or how a chunk ranked its rows
/// ([`tests::chunking_the_keyword_column_does_not_change_its_bytes`]).
///
/// # What is checked
///
/// The ordinal the dictionary writer returns is asserted against the count of keys pushed. Each
/// chunk's remap must hold exactly its key count and ascend strictly, since both its keys and the
/// dictionary are sorted. Every id and perm entry must fall inside its chunk's keys, every row the
/// walk visited must be translated, and every spill file verifies its count and anchor as it is
/// read. A wrong ordinal recolours a map with no other symptom (`mosaica_filter::dict`), so each of
/// these refuses the build rather than writing one.
#[allow(clippy::too_many_arguments)]
fn write_keyword_column(
    column_dir: &Path,
    values_path: &Path,
    attribute: &crate::config::Attribute,
    values: KeywordValues<'_>,
    presence: &mut Presence,
    reached: &mut Batched<'_>,
    writer: &mut ValueColumnWriter,
    plan: KeywordDictPlan,
) -> Result<(PathBuf, usize)> {
    let refuse =
        |detail: String| BuildError::Invalid(format!("attribute '{}': {detail}", attribute.name));

    // ---- 1. the chunk pass ---------------------------------------------------------------------
    let mut chunker = KeywordChunks::create(column_dir, plan, values.rows_hint())?;
    let rows = for_each_keyword(attribute, values, presence, reached, &mut |_row, text| {
        chunker.push(text)
    })?;
    let (ids, perms, runs, chunks) = chunker.finish()?;
    let (ids, id_count) = ids.finish()?;
    let (perms, perm_count) = perms.finish()?;
    let chunked: u64 = chunks.iter().map(|chunk| chunk.rows).sum();
    if chunked != rows || id_count != rows {
        return Err(refuse(format!(
            "the keyword chunks hold {chunked} rows and {id_count} ids where the column has \
             {rows}"
        )));
    }
    let keys_held: u64 = chunks.iter().map(|chunk| chunk.keys as u64).sum();

    // ---- 2. the cascade --------------------------------------------------------------------------
    let (runs, passes) = cascade_sorted_runs(
        column_dir,
        "keyword",
        RunPayload::Rows,
        runs,
        plan.allowance,
        0,
    )
    .map_err(|e| spill::run_error(column_dir, e))?;
    let spilled: u64 = runs.iter().map(|run| run.count).sum();
    if spilled != keys_held {
        return Err(refuse(format!(
            "the keyword runs hold {spilled} keys where the chunks counted {keys_held} — a run \
             file is missing or was not merged"
        )));
    }

    // ---- 3. the merge: the dictionary, and each chunk's remap ------------------------------------
    // The chunks' memory was freed on the pool's threads; hand it back before the merge's buffers.
    mosaica_types::process::trim_heap();
    let dict_path = column_dir.join(mosaica_filter::DICT_FILE);
    let mut partition = spill::Partition::create(
        column_dir,
        KEYWORD_REMAP_PARTITION,
        spill::boundaries_uniform(chunks.len() as u64),
        KEYWORD_REMAP_RECORD,
        keys_held,
    )?;
    let ranges: Vec<(u64, u64)> = (0..partition.buckets())
        .map(|k| {
            let (lo, hi) = partition.range(k);
            (lo as u64, hi.min(chunks.len() as u64))
        })
        .collect();
    // Three threads, joined by bounded queues of batches: the merge, the dictionary's writer and
    // the remap's.
    let (keys, emitted, partition) = std::thread::scope(|scope| {
        let (key_queue, key_batches) =
            std::sync::mpsc::sync_channel::<HeldRecords>(KEYWORD_MERGE_DEPTH);
        let (record_queue, record_batches) =
            std::sync::mpsc::sync_channel::<Vec<u8>>(KEYWORD_MERGE_DEPTH);
        let dict_writer = scope.spawn(|| -> Result<u64> {
            let dict_file =
                std::fs::File::create(&dict_path).map_err(|e| BuildError::io(&dict_path, e))?;
            let mut dict =
                mosaica_filter::SortedDictWriter::new(std::io::BufWriter::new(dict_file))
                    .map_err(|e| BuildError::io(&dict_path, std::io::Error::from(e)))?;
            let mut keys = 0u64;
            for batch in key_batches {
                for key in batch.records() {
                    let key = std::str::from_utf8(key).map_err(|e| {
                        refuse(format!(
                            "a merged key is not UTF-8 ({e}) — the column holds `&str`, so this \
                             is a corrupted run rather than a corpus value"
                        ))
                    })?;
                    let ordinal = dict
                        .push(key)
                        .map_err(|e| BuildError::io(&dict_path, std::io::Error::from(e)))?;
                    if ordinal as u64 != keys {
                        return Err(refuse(format!(
                            "the key {key:?} was written at dictionary ordinal {ordinal} and is \
                             the {keys}th distinct key of the merge — the values file would name \
                             another key's value"
                        )));
                    }
                    keys += 1;
                }
            }
            dict.finish()
                .map_err(|e| BuildError::io(&dict_path, std::io::Error::from(e)))?;
            Ok(keys)
        });
        let remap_writer = scope.spawn(move || -> Result<spill::Partition> {
            for records in record_batches {
                for record in records.as_chunks::<KEYWORD_REMAP_RECORD>().0 {
                    partition.push(record)?;
                }
            }
            Ok(partition)
        });

        let merged = merge_keyword_runs(
            &runs,
            KeywordMerged {
                keys: KeySender::new(key_queue, plan.merge_batch),
                records: Vec::new(),
                queue: record_queue,
                batch: plan.merge_batch,
                written: 0,
                emitted: 0,
                chunks: chunks.len(),
            },
            plan.allowance,
            plan.ranges,
        )
        .map_err(|e| spill::run_error(column_dir, e))
        .and_then(|mut merged| {
            merged.keys.flush()?;
            let records = std::mem::take(&mut merged.records);
            merged.queue.send(records).map_err(|_| writer_stopped())?;
            Ok((merged.written, merged.emitted))
        });
        let written = joined(dict_writer)?;
        let partition = joined(remap_writer)?;
        let (keys, emitted) = merged?;
        if written != keys {
            return Err(refuse(format!(
                "the keyword merge selected {keys} keys and the dictionary took {written}"
            )));
        }
        Ok((keys, emitted, partition))
    })?;
    if emitted != keys_held {
        return Err(refuse(format!(
            "the keyword merge yielded {emitted} chunk keys over {keys} keys where the chunks \
             hold {keys_held}"
        )));
    }
    let mut store = partition.finish()?;
    for run in &runs {
        std::fs::remove_file(&run.path).map_err(|e| BuildError::io(&run.path, e))?;
    }

    // ---- 4. the values file, in row order --------------------------------------------------------
    let mut id_reader = IdReader::open(&ids, id_count)?;
    let mut perm_reader = IdReader::open(&perms, perm_count)?;
    let mut out: Vec<u32> = Vec::with_capacity(VALUE_CHUNK);
    let mut translated = 0u64;
    for (k, &(lo, hi)) in ranges.iter().enumerate() {
        // Streamed into remaps sized to their chunks.
        let mut remaps: Vec<Vec<u32>> = (lo..hi)
            .map(|chunk| Vec::with_capacity(chunks[chunk as usize].keys as usize))
            .collect();
        // The closure answers in the store's error type, so a refusal is held here and returned
        // once the read, and the anchor check that ends it, are done.
        let mut refused: Option<BuildError> = None;
        store.read_each(k, |record| {
            if refused.is_some() {
                return Ok(());
            }
            let chunk = u32::from_le_bytes(record[..4].try_into().expect("a record is 8 bytes"));
            let ordinal = u32::from_le_bytes(record[4..].try_into().expect("a record is 8 bytes"));
            let Some(remap) = (chunk as u64)
                .checked_sub(lo)
                .and_then(|at| remaps.get_mut(at as usize))
            else {
                refused = Some(refuse(format!(
                    "bucket {k} of the keyword remap holds chunk {chunk}, outside the range \
                     [{lo}, {hi}) it addresses"
                )));
                return Ok(());
            };
            if remap.last().is_some_and(|&previous| ordinal <= previous) {
                refused = Some(refuse(format!(
                    "chunk {chunk}'s remap does not ascend: {ordinal} follows {:?} — both the \
                     chunk's keys and the dictionary are sorted, so a correct remap does",
                    remap.last()
                )));
                return Ok(());
            }
            remap.push(ordinal);
            Ok(())
        })?;
        if let Some(refusal) = refused {
            return Err(refusal);
        }
        store.delete(k)?;

        for (chunk, remap) in (lo..hi).zip(remaps) {
            let meta = &chunks[chunk as usize];
            if remap.len() != meta.keys as usize {
                return Err(refuse(format!(
                    "chunk {chunk} holds {} keys and the merge remapped {}",
                    meta.keys,
                    remap.len()
                )));
            }
            let table = match meta.mode {
                ChunkMode::Sort => remap,
                ChunkMode::Hash => {
                    let mut table = Vec::with_capacity(remap.len());
                    let mut left = meta.keys as u64;
                    while left > 0 {
                        let ranks = perm_reader.take(left)?;
                        left -= ranks.len() as u64;
                        for &rank in ranks {
                            table.push(*remap.get(rank as usize).ok_or_else(|| {
                                refuse(format!(
                                    "chunk {chunk}'s perm names rank {rank} of its {} keys",
                                    meta.keys
                                ))
                            })?);
                        }
                    }
                    drop(remap);
                    table
                }
            };
            let mut left = meta.rows;
            while left > 0 {
                let ids = id_reader.take(left)?;
                left -= ids.len() as u64;
                for &id in ids {
                    out.push(*table.get(id as usize).ok_or_else(|| {
                        refuse(format!(
                            "chunk {chunk} gives a row id {id} of its {} keys",
                            meta.keys
                        ))
                    })?);
                    if out.len() == VALUE_CHUNK {
                        writer
                            .push(&Codes::U32(std::mem::take(&mut out).into()))
                            .map_err(|e| BuildError::io(values_path, e))?;
                        out.reserve(VALUE_CHUNK);
                    }
                }
            }
            translated += meta.rows;
        }
    }
    if !out.is_empty() {
        writer
            .push(&Codes::U32(out.into()))
            .map_err(|e| BuildError::io(values_path, e))?;
    }
    id_reader.finish()?;
    perm_reader.finish()?;
    if translated != rows {
        return Err(refuse(format!(
            "the keyword pass translated {translated} rows where the column has {rows}"
        )));
    }
    for path in [&ids.path, &perms.path] {
        std::fs::remove_file(path).map_err(|e| BuildError::io(path, e))?;
    }
    Ok((dict_path, passes))
}

/// Values pushed to the column writer at a time. The writer writes each chunk as it arrives, so
/// this is the whole of what the emit holds beside the attribute tail it reads from.
const VALUE_CHUNK: usize = 1 << 16;

/// A category chunk at the declared width.
///
/// The declared width is the storage width. Narrowing here rather than storing `u32` throughout is
/// worth a match arm: the column is priced at 1 GB per byte of width per 10⁹ items (Appendix A), so
/// a `u8` category stored as `u32` would cost 3 GB it does not need.
fn category_chunk(ty: ScalarType, held: &[u32]) -> Codes {
    match ty {
        ScalarType::U8 => Codes::U8(held.iter().map(|&c| c as u8).collect::<Vec<_>>().into()),
        ScalarType::U16 => Codes::U16(held.iter().map(|&c| c as u16).collect::<Vec<_>>().into()),
        _ => Codes::U32(held.to_vec().into()),
    }
}

/// One numeric column's values, at the declared width, pushed in bounded chunks.
///
/// **`ScalarValue::Null` is skipped, not pushed**, which is what makes the slot addressing agree
/// with the presence bitmap [`write_column_values`] builds alongside: the *k*-th set bit's value is
/// at slot *k* (filter-index §2.1), so an absent entity must occupy no slot. Pushing a placeholder
/// instead would shift every later value along by one — every entity after the first absent one
/// reporting its neighbour's number, with no error anywhere.
fn push_numeric_chunks(
    writer: &mut ValueColumnWriter,
    values_path: &Path,
    attribute: &crate::config::Attribute,
    values: &EntityColumn,
    reached: &mut Batched<'_>,
) -> Result<()> {
    macro_rules! stream {
        ($variant:ident, $ctor:expr, $map:expr) => {{
            let mut out = Vec::with_capacity(VALUE_CHUNK);
            for v in values.iter() {
                reached.add(1);
                match v {
                    ScalarValue::$variant(x) => out.push($map(x)),
                    // No slot at all — see this function's doc.
                    ScalarValue::Null => continue,
                    other => {
                        return Err(BuildError::Invalid(format!(
                            "attribute '{}' is declared {:?} but carries {other:?}",
                            attribute.name, attribute.ty
                        )))
                    }
                }
                if out.len() >= VALUE_CHUNK {
                    writer
                        .push(&$ctor(std::mem::take(&mut out).into()))
                        .map_err(|e| BuildError::io(values_path, e))?;
                    out.reserve(VALUE_CHUNK);
                }
            }
            if !out.is_empty() {
                writer
                    .push(&$ctor(out.into()))
                    .map_err(|e| BuildError::io(values_path, e))?;
            }
        }};
        ($variant:ident, $ctor:expr) => {
            stream!($variant, $ctor, |x| x)
        };
    }
    match attribute.ty {
        ScalarType::Bool => stream!(Bool, Codes::U8, u8::from),
        ScalarType::U8 => stream!(U8, Codes::U8),
        ScalarType::U16 => stream!(U16, Codes::U16),
        ScalarType::U32 => stream!(U32, Codes::U32),
        ScalarType::U64 => stream!(U64, Codes::U64),
        ScalarType::I8 => stream!(I8, Codes::I8),
        ScalarType::I16 => stream!(I16, Codes::I16),
        ScalarType::I32 => stream!(I32, Codes::I32),
        ScalarType::I64 => stream!(I64, Codes::I64),
        ScalarType::F32 => stream!(F32, Codes::F32),
        ScalarType::F64 => stream!(F64, Codes::F64),
        ScalarType::TimestampUs => stream!(TimestampUs, Codes::I64),
        ScalarType::Utf8 => unreachable!("`utf8` is not a declarable type"),
        ScalarType::Keyword => unreachable!("the caller handles a keyword before reaching here"),
        ScalarType::Text => unreachable!("the caller handles text before reaching here"),
    }
    Ok(())
}

/// How the text index's pass is sized: how many windows the column is divided into, the bytes one
/// worker's accumulator may allocate before it spills a run, and the allowance its merge reads the
/// runs under.
///
/// **Both are needed, and neither would do alone.** The window count is what the pass parallelises
/// over, so it follows the machine; the byte budget is what bounds a worker's residency, and it
/// has to hold whatever the documents turn out to be. A column of short names and a column of
/// abstracts differ by two orders of magnitude in terms per entity, so a plan that sized chunks
/// alone would be a memory bound only for the corpus it was measured on.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TextIndexPlan {
    chunks: usize,
    worker_bytes: usize,
    merge_slice_cap: usize,
    allowance: mosaica_filter_write::Allowance,
    ranges: mosaica_filter_write::Ranges,
}

impl TextIndexPlan {
    /// The plan an allowance derives ([`crate::residency::Sizing::term_index`]): each of its
    /// threads' accumulators takes an equal share of it, up to what a chunk's offsets address, and
    /// the merge the whole of it once they are gone.
    pub(crate) fn for_allowance(allowance: mosaica_filter_write::Allowance) -> TextIndexPlan {
        let threads = allowance.threads;
        // **Eight chunks a thread, and the number is measured rather than chosen.** A
        // segmentation's cost per entity varies by an order of magnitude with the script — the
        // dictionary-backed scripts against the rule-based ones — and entity space is *sorted by
        // access term*, which for a geographic corpus means sorted by country. So the slow scripts
        // are contiguous, not spread. At two chunks a thread over 7.4×10⁷ Overture names, 23 of
        // the 24 chunks finished within 25 s of each other and the twenty-fourth ran alone for a
        // further 90 s: the stage's wall time was one chunk's. Splitting finer buys most of that
        // tail back, and it costs neither memory nor runs: a worker keeps one accumulator across
        // every chunk it takes and spills only when its budget is full, and only `threads` workers
        // are live, however many chunks there are. At eight the same column's chunk pass finished
        // in ~80 s against ~115 s, and the stage in 96.35 s against 148.00 s.
        //
        // ⊘ **The tail is smaller, not gone.** Four of the 96 chunks still ran ~60 s after the
        // other 92 had finished, so most of the chunk pass is still one region's segmentation.
        // Splitting finer again would shorten that tail further, and is not taken.
        //
        // **The chunks are arena windows now, not entity ranges** (`column.rs`), so the number is
        // a target and the arena's own record marks decide how close to it the split lands. Eight
        // a thread carries over unchanged: what it balances — a segmentation cost that varies by
        // an order of magnitude with the script — is a property of the documents, and the windows
        // hold the same documents the entity ranges did.
        TextIndexPlan {
            chunks: (threads * 8).max(1),
            worker_bytes: allowance.per_worker(),
            merge_slice_cap: mosaica_filter_write::TEXT_MERGE_SLICE_CAP,
            allowance,
            ranges: mosaica_filter_write::Ranges::Planned,
        }
    }

    /// An explicit plan, for the tests that must force several chunks and several runs a chunk out
    /// of a corpus small enough to assert over. Its merge has a fan-in of 32, so a plan of many
    /// runs cascades, several groups at once.
    #[cfg(test)]
    fn explicit(chunks: usize, worker_bytes: usize, merge_slice_cap: usize) -> TextIndexPlan {
        TextIndexPlan {
            chunks: chunks.max(1),
            worker_bytes: worker_bytes.max(1),
            merge_slice_cap,
            allowance: tests::TEST_MERGE,
            ranges: mosaica_filter_write::Ranges::Planned,
        }
    }

    /// This plan with its merge divided as `ranges` says.
    #[cfg(test)]
    fn in_ranges(self, ranges: mosaica_filter_write::Ranges) -> TextIndexPlan {
        TextIndexPlan { ranges, ..self }
    }
}

/// Where one text pass reads its prose from.
///
/// **Two producers, one pass.** A bundle-wide `text` column is read from the record-blob extents
/// the join spilled ([`crate::extents`]); a group-scoped one is read from the per-view
/// [`EntityColumn`] the scoped pass builds, which keeps its arena because it has no blob row to
/// be read from (`views.md` §5).
pub(crate) enum TextValues<'a> {
    /// A column's arena, divided into contiguous byte ranges.
    Arena(&'a EntityColumn),
    /// One column's extents, divided into contiguous block ranges.
    Extents(&'a crate::extents::OpenExtents),
}

/// One worker's share of a text pass: a byte range of an arena, or a block range of an extent.
#[derive(Debug, Clone, Copy)]
enum TextWindow {
    Arena(u64, u64),
    Extents(crate::extents::ExtentWindow),
}

impl TextValues<'_> {
    /// At most `target` windows, together covering every value the column holds.
    fn windows(&self, target: usize) -> Vec<TextWindow> {
        match self {
            TextValues::Arena(values) => values
                .arena_windows(target)
                .into_iter()
                .map(|(lo, hi)| TextWindow::Arena(lo, hi))
                .collect(),
            TextValues::Extents(extents) => extents
                .windows(target)
                .into_iter()
                .map(TextWindow::Extents)
                .collect(),
        }
    }

    /// The entities the column holds a value for, as [`Self::for_each_record_in`] yields them.
    fn present(&self) -> Result<croaring::Bitmap> {
        match self {
            TextValues::Arena(values) => Ok(values
                .present_entities()
                .map(|entity| entity as u32)
                .collect()),
            TextValues::Extents(extents) => extents.present(),
        }
    }

    /// Every value in one window, as `(entity, prose)`.
    fn for_each_record_in(
        &self,
        window: TextWindow,
        visit: &mut dyn FnMut(usize, &str) -> Result<()>,
    ) -> Result<()> {
        match (self, window) {
            (TextValues::Arena(values), TextWindow::Arena(lo, hi)) => {
                values.for_each_record_in(lo, hi, visit)
            }
            (TextValues::Extents(extents), TextWindow::Extents(window)) => {
                extents.for_each_record_in(window, visit)
            }
            _ => Err(BuildError::Invalid(
                "a text window was given to the other producer's reader".into(),
            )),
        }
    }
}

/// Write one text column's entity-space index: the per-layer token dictionary, the postings and
/// positions over it, and the presence. The open step counts each document tokenised out of the
/// documents the column holds.
///
/// **The dictionary's ordinals are positions in the sorted distinct term set**, exactly as a
/// keyword's are in its key set, and the postings are written in that same order, so posting *i*
/// belongs to the *i*-th key the dictionary holds. Both files are produced by one pass over one
/// sorted term stream, so an ordinal cannot drift between them: the ordinal a term gets is the one
/// [`mosaica_filter::SortedDictWriter::push`] returns, and the record encoded against it is
/// appended to the postings file before the next term is read.
///
/// # The shape: accumulate, spill, merge
///
/// 1. **Accumulate.** The column is divided into windows ([`TextIndexPlan`]), contiguous stretches
///    of the arena or of the extents, and the windows are indexed in parallel. A worker keeps one
///    [`mosaica_filter_write::TermAccumulator`] across every window it takes.
/// 2. **Spill.** An accumulator writes what it holds as a **sorted run** when what it has allocated
///    reaches the plan's per-worker budget. It holds each term's entities and positions in the
///    run's format, so a term whose documents arrived in entity order is copied into the run as it
///    stands, and only a term whose documents did not has its entities sorted. A worker's
///    residency is the budget whatever the documents are, and the run count grows instead of the
///    peak. What an accumulator holds when the pass ends is ranked and kept in memory, not spilled.
/// 3. **Merge.** [`mosaica_filter_write::write_text_index`] merges the runs and the accumulators'
///    held terms together, k-way on the term, through the one term merge the fold and the coalesce
///    write through too ([`mosaica_filter_write::TermMerge`]), and each term's entities by
///    comparing the sources' heads, because the windows do not partition entity space. The runs
///    are read through what the allowance leaves beside the held terms. Where that cannot pay for
///    a read buffer a run, the largest accumulators are spilled after all; and where every one is
///    spilled and the runs are more than the allowance's fan-in, which is what its read buffers
///    pay for under the descriptors a merge may hold, they are cascaded first
///    ([`cascade_sorted_runs`]): as many leading runs as bring the count to the fan-in are merged
///    in groups into intermediate runs, several groups at once. The build prints the passes it
///    forecasts before it starts, and the runs, held accumulators and passes taken as it ends.
///    The merge runs in term ranges on the plan's threads, each range's sources started at its
///    first term, and writes them in term order ([`mosaica_filter_write::merge_runs`]); a run's
///    records are given back to the file system as the ranges that read them are written.
///
/// **The output is a function of the corpus alone, never of the plan.** The dictionary is the
/// sorted distinct term set and a posting is the set of entities carrying its term; neither
/// depends on where a window boundary fell, how often a worker spilled, or whether a term was
/// read from a run or from memory. [`tests::chunking_the_text_index_does_not_change_its_bytes`]
/// is the assertion, over a corpus whose entity count straddles the chunk sizes it is emitted
/// under: a boundary that split a term's postings between two runs and lost one half would
/// otherwise be invisible.
///
/// # What each step costs, and what bounds it
///
/// * The accumulate pass holds one accumulator a thread, each of at most `worker_bytes` as it
///   counts what it has allocated: an equal share of the plan's allowance, which is what the index
///   phase leaves of the build's memory budget ([`crate::residency::Sizing::term_index`]). Nothing
///   in it scales with the corpus. A term costs about 40 bytes of state beyond its entities and
///   positions, and most terms in a run occur once, so on English-like prose a run is about a
///   quarter of the memory that wrote it at 32 MiB a worker, and more at larger allowances.
/// * The merge holds the accumulators it kept, and on each of its threads one open reader per
///   run: a read buffer, its share of what the allowance leaves beside them, and the head *term*,
///   never the head's entities, which is why a run's reader decodes a record's postings only when
///   they are asked for. The ranges not yet written each hold up to a buffer of their output, and
///   those buffers are charged to the allowance too ([`mosaica_filter_write::RangePlan`]). The
///   cascade holds the run count to the allowance's fan-in.
/// * One merged term's entity list is capped at the plan's slice cap, above which it accumulates
///   into a Roaring bitmap and encodes through the byte-identical bitmap encoder.
///
/// # Terms and postings
///
/// **A term is interned once.** The analyser hands back borrowed tokens
/// ([`mosaica_analyse::Analyser::for_each_token`]) and the accumulator looks them up by their
/// bytes, so a term it already holds costs no allocation.
///
/// **A term repeated within one document contributes one posting entry**, holding each position.
/// The analyser keeps duplicates and order because the positions need both; the accumulator groups
/// a document's tokens by term before it inserts them.
///
/// **The postings stream through [`mosaica_authz::RecordWriter`]** rather than being collected,
/// and the dictionary through [`mosaica_filter::SortedDictWriter`]: neither file is ever held
/// whole in memory.
///
/// The singleton encoding is `mosaica-authz`'s: a term carried by few enough entities is a bare
/// `u32` array rather than a serialised bitmap, which is what the string-storage campaign measured
/// at 4.4× smaller on the singleton-heavy vocabularies real prose produces.
fn write_text_index(
    column_dir: &Path,
    attribute: &crate::config::Attribute,
    values: TextValues<'_>,
    plan: TextIndexPlan,
    steps: Steps<'_>,
) -> Result<WrittenTextIndex> {
    steps.count(Unit::Items, || Ok(values.present()?.cardinality()))?;
    // The identity was resolved at the schema parse; the name is its first component. Resolving it
    // again here rather than threading an `Analyser` down keeps the build's contract with the
    // manifest one-directional: what is recorded is what indexed.
    let identity = attribute.analyser.as_deref().ok_or_else(|| {
        BuildError::Invalid(format!(
            "attribute '{}' is text but carries no resolved analyser — the schema parse is what \
             resolves one, so this is a compilation defect rather than a schema error",
            attribute.name
        ))
    })?;
    // The whole identity is compared. `Schema::parse` resolves it from this binary's analyser, but
    // a `Schema` built in code can carry a stale version, and a bundle recording one opens for no
    // reader.
    let analyser = mosaica_analyse::analyser_with_identity(identity)
        .ok_or_else(|| {
            BuildError::Invalid(format!(
                "attribute '{}' declares analyser '{identity}', which this build does not carry. \
                 Its terms cannot be reproduced, so an index written now would answer every \
                 `match` from a segmentation the manifest does not describe",
                attribute.name
            ))
        })?;

    // ---- 1. accumulate in parallel, spilling sorted runs -------------------------------------
    //
    // **Windows of the arena, not ranges of entity space.** Entity order is signature-then-Morton
    // order and the arena is in arrival order, so an entity range's documents are scattered
    // through the arena: at 1.02×10⁸ abstracts that is one major fault per document over a file
    // two and a half times the size of the box, and the pass did not finish in four hours. Each
    // worker reads one contiguous stretch of the arena front to back and releases it behind
    // itself, and what that costs is a sort at the spill of each term whose documents arrived out
    // of entity order, and a merge of the runs' entities at the fan-in.
    let chunks = values.windows(plan.chunks);
    if let TextValues::Arena(column) = &values {
        // The join wrote every one of those bytes through the mapping, so they are all in this
        // process's page tables and no `fadvise` would release them. See
        // `MappedArena::unmap_pages`.
        column.unmap_arena_pages();
    }
    // **One analyser, shared.** Its construction deserialises the segmenter's dictionary data,
    // the cost the type exists to amortise, and it holds no per-document state, so it is `Sync`
    // and the workers borrow it.
    //
    // **One accumulator a worker, kept across windows.** A window takes an idle accumulator, or
    // makes one where none is idle, and gives it back when it is done, so there are as many as
    // there are windows in flight and each spills only when its budget is full.
    let idle = std::sync::Mutex::new(Vec::<TextWorker>::new());
    let made = std::sync::atomic::AtomicUsize::new(0);
    let idle_workers = || idle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    chunks.par_iter().try_for_each(|&window| -> Result<()> {
        let mut worker = idle_workers().pop().unwrap_or_else(|| {
            let n = made.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            TextWorker {
                terms: TermAccumulator::new(
                    column_dir,
                    format!("text-run-{n:03}"),
                    plan.worker_bytes,
                ),
                scratch: mosaica_analyse::TokenScratch::default(),
            }
        });
        let indexed = index_text_window(column_dir, window, &values, &analyser, &mut worker, steps);
        idle_workers().push(worker);
        indexed
    })?;
    let workers = idle.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
    let gathered: Vec<mosaica_filter_write::Gathered> = workers
        .into_par_iter()
        .map(|worker| worker.terms.finish())
        .collect::<std::io::Result<_>>()
        .map_err(|e| spill::run_error(column_dir, e))?;

    // ---- 2. merge: one sorted term stream into the dictionary, postings, positions -----------
    let files = mosaica_filter::TextIndexFiles::in_dir(column_dir);
    let failed = |e: std::io::Error| BuildError::io(&files.dict, e);
    let mut writer = mosaica_filter_write::TextIndexWriter::create(files.clone()).map_err(failed)?;
    let merged = mosaica_filter_write::write_text_index(
        column_dir,
        "text",
        gathered,
        &mut writer,
        plan.merge_slice_cap,
        plan.allowance,
        plan.ranges,
    )
    .map_err(|e| spill::run_error(column_dir, e))?;
    eprintln!(
        "text index '{}': {} run(s) and {} accumulator(s) held in memory, {} cascade pass(es) \
         under a fan-in of {}, merged on {} thread(s)",
        attribute.name,
        merged.runs,
        merged.held,
        merged.passes,
        merged.fan_in,
        merged.threads
    );
    let terms = writer.finish().map_err(failed)?;
    mosaica_filter::write_presence(&files.presence, &values.present()?)
        .map_err(|e| BuildError::io(&files.presence, e))?;
    for path in files.paths() {
        fsync_file(path)?;
    }

    Ok(WrittenTextIndex {
        paths: files.paths().map(Path::to_path_buf).into(),
        terms,
        passes: merged.passes,
    })
}

/// One worker of a text pass: its accumulator, and the analyser's normalisation buffer.
struct TextWorker {
    terms: TermAccumulator,
    scratch: mosaica_analyse::TokenScratch,
}

/// Index one window into the accumulator of the worker that took it, which spills when its
/// budget is full.
///
/// A window yields `(entity, value)` pairs in arena order rather than ascending; the accumulator
/// does not rely on the order, and an entity appears in exactly one live arena record, so no two
/// documents carry the same entity.
fn index_text_window(
    column_dir: &Path,
    window: TextWindow,
    values: &TextValues<'_>,
    analyser: &mosaica_analyse::Analyser,
    worker: &mut TextWorker,
    steps: Steps<'_>,
) -> Result<()> {
    let mut documents = steps.batched();
    let TextWorker { terms, scratch } = worker;
    values.for_each_record_in(window, &mut |entity, prose| {
        documents.add(1);
        terms
            .add_document(entity as u32, |push| {
                analyser.for_each_token(prose, scratch, &mut |token| push(token))
            })
            .map_err(|e| spill::run_error(column_dir, e))
    })
}

/// One text column's index: the files written, and the term count [`BuildStage::TextIndex`]
/// reports.
struct WrittenTextIndex {
    paths: Vec<PathBuf>,
    terms: u64,
    /// The cascade passes its merge took.
    passes: usize,
}

/// Does this column owe a postings file?
///
/// Two independent reasons, and the second is the one a reader will not expect.
///
/// **`index = true`** is the obvious one: the column is declared filterable, and postings are
/// how a broad-coverage filter stays inside its latency budget (filter-index §2.3).
///
/// **`visibility = "derived"`** is the other, and it is *not* optional. That control gates the
/// existence of a value name, and the gate is membership-derived: a value is offered only if the
/// principal can see an item carrying it (per-point-attributes §3.3). Deriving that needs the
/// per-`(column, code)` member sets, which are exactly these postings. Without them `/v1/categories`
/// would have to derive membership by scanning the value column per request — which is inside a
/// *filter's* latency budget but not inside this endpoint's, and would make contracts §3.2's
/// compute-admission justification ("no mask composition, no projection, no file IO") false.
///
/// So a `derived` category gets postings whatever its `index` says. This is the one place the
/// postings stop being an optional accelerator: everywhere else a deployment that builds them and one
/// that does not answer identically and differ only in latency, but here a disclosure control depends
/// on them existing.
pub(crate) fn postings_are_owed(
    schema: &crate::config::Schema,
    attribute: &crate::config::Attribute,
) -> bool {
    if attribute.index {
        return true;
    }
    attribute
        .vocabulary
        .as_ref()
        .and_then(|name| schema.vocabularies.get(name))
        .is_some_and(|v| v.visibility() == crate::config::Visibility::Derived)
}

/// The vocabulary code a category column's value carries.
///
/// Reached only for a category — postings are derived for the vocabulary-bearing family alone —
/// so the three unsigned widths §3.6 allows are the whole domain; anything else reaching here is
/// a schema-compilation defect, and it fails loudly rather than filtering on a value it invented.
fn category_code(value: &ScalarValue, column: &str) -> Result<u32> {
    match value {
        ScalarValue::U8(c) => Ok(*c as u32),
        ScalarValue::U16(c) => Ok(*c as u32),
        ScalarValue::U32(c) => Ok(*c),
        // **An entity the column never reached is absent, which for this family is code 0.** The
        // presence bitmap is what a partially covered column spells absence with, and a category's
        // absence is the reserved code out of the value space (decision 0064,
        // `per-point-attributes.md` §3.6) — so the two spellings meet here, and both mean *no
        // posting and no value*. Reached by every column of a group-scoped family, where covering
        // fewer than every entity is the ordinary state rather than a symptom: a view holds its own
        // rows (`views.md` §5).
        ScalarValue::Null => Ok(mosaica_store::vocabulary::ABSENT_CODE),
        other => Err(BuildError::Invalid(format!(
            "attribute '{column}' is declared for filtering but carries {other:?}, which is not a \
             category code"
        ))),
    }
}

/// Which rows of one render column carry a value, from that column's values **in row order** —
/// `None` where every row does, which is the case that writes no file (decision 0064).
///
/// **`ScalarValue::Null` names exactly the columns that owe a bitmap, so this needs no schema.**
/// The two families with an in-band way to say "nothing" never produce one here: a category's
/// missing key resolves to the reserved code 0 its vocabulary keeps out of the value space, at the
/// points file (`BatchColumn::value`) and at the ingest plane alike, and `render` on `utf8` is
/// refused at schema parse. What is left is the numeric family, every bit pattern of which is a
/// legal value.
///
/// Shared by the streaming and linear builds because they hold their values in different shapes
/// but must write the same bytes — the property `tests/build_equivalence.rs` exists to hold them
/// to.
/// Takes presence per row rather than the values themselves: the entity-major columns are typed
/// now (see [`EntityColumn`]), so absence is a bit beside the value and never a variant of it.
pub(crate) fn render_presence_of(
    present_per_row: impl IntoIterator<Item = bool>,
) -> Option<Bitmap> {
    let mut present = Bitmap::new();
    let mut any_absent = false;
    for (row, is_present) in present_per_row.into_iter().enumerate() {
        if is_present {
            present.add(row as u32);
        } else {
            any_absent = true;
        }
    }
    any_absent.then_some(present)
}

/// One view's geometry in **ordinal** space, read once in pass one and permuted into entity
/// space in pass two (`views.md` §7).
struct ViewGeometry {
    x: spill::MappedU32,
    y: spill::MappedU32,
    /// Which ordinals this view holds a row for — the view's population, and what makes its
    /// permutation sentinel wherever it does not.
    present: Vec<u64>,
    rows: u64,
}

/// What [`build_dictionary`] establishes in its single pass over the pairs relation.
struct Dictionary {
    /// Source term ids, ascending — with `term_ids` in parallel, the source-term → term-id map
    /// as two flat arrays (12 bytes per term against an `FxHashMap`'s ~40 at T = 117M), looked
    /// up by merge sweep over term-sorted chunks, never by per-row probe.
    term_keys: Vec<u64>,
    /// `term_ids[i]` is the term id `term_keys[i]` interned to.
    term_ids: Vec<u32>,
    /// Per term id, how many pairs **rows** name it — counted before deduplication, so a bucket
    /// sized by this is wide enough even when the input repeats a `(entity, term)` row.
    row_counts: Vec<u64>,
    term_count: u64,
    /// Selected pairs rows, before deduplication.
    pair_rows: usize,
    /// Resolved pairs rows per ordinal range (`ordinal >> histogram_shift`), for the batch
    /// pre-flight: the worst batch's pairs are bounded by summing overlapping ranges.
    histogram: Vec<u64>,
    histogram_shift: u32,
    dict_paths: Vec<PathBuf>,
}

/// Assign term ids and write the dictionary extent.
///
/// The linear build interns descriptors while walking items in ascending source-id order, each
/// item's terms in ascending source-term order; a term's id is its rank in that stream of *first*
/// appearances. That rank is `(smallest ordinal carrying the term, source term id)` — computable
/// from one pass over the pairs relation, over a map with one entry per distinct term (~48k in
/// the probe corpus), rather than from a materialised item list.
///
/// Returns [`Dictionary`].
fn build_dictionary(
    args: &BuildArgs,
    access: &crate::AccessPlan,
    n: u64,
    dict_dir: &std::path::Path,
    numbering: &crate::ids::Numbering,
    steps: Steps<'_>,
) -> Result<Dictionary> {
    // Chunk-order insensitivity ([`join_chunk`]): per-term min-ordinal and row counts, and the
    // per-range histogram, are commutative aggregations — no arrival order is observable.
    let mut first_ordinal: FxHashMap<u64, (u64, u64)> = FxHashMap::default();
    // Ordinal-range histogram for batch sizing: 2^16 ranges regardless of n.
    let histogram_shift = (64 - n.max(1).leading_zeros()).saturating_sub(16);
    let mut histogram = vec![0u64; (n as usize >> histogram_shift) + 1];
    // Absent ids are reported by count and minimum, never collected: a mispaired input naming
    // billions of missing ids would otherwise accumulate a multi-gigabyte set — the exact
    // transient class this module exists to avoid — before producing its typed error.
    let mut absent_count = 0u64;
    let mut absent_min = u64::MAX;
    let mut pair_rows = 0usize;
    // Grows toward `JOIN_CHUNK_ROWS` only if the relation is actually that large; this pass
    // has no row count in hand yet.
    let mut chunk: Vec<(u64, u64)> = Vec::new();
    let mut resolve = |chunk: &mut Vec<(u64, u64)>| {
        join_chunk(chunk, n, |ordinal, source_id, source_term| {
            match ordinal {
                Some(ordinal) => {
                    let slot = first_ordinal.entry(source_term).or_insert((u64::MAX, 0));
                    slot.0 = slot.0.min(ordinal as u64);
                    slot.1 += 1;
                    histogram[(ordinal >> histogram_shift) as usize] += 1;
                }
                None => {
                    absent_count += 1;
                    absent_min = absent_min.min(source_id);
                }
            }
            Ok(())
        })
    };
    let mut failure: Option<BuildError> = None;
    let fill = crate::scan_access(
        args,
        access,
        numbering,
        steps,
        |_view, source_id, source_term| {
            pair_rows += 1;
            chunk.push((source_id, source_term));
            if chunk.len() == JOIN_CHUNK_ROWS {
                if let Err(e) = resolve(&mut chunk) {
                    failure = Some(e);
                    return ControlFlow::Break(());
                }
            }
            ControlFlow::Continue(())
        },
    )?;
    crate::report_access_fill(args, fill);
    if let Some(error) = failure {
        return Err(error);
    }
    resolve(&mut chunk)?;
    drop(chunk);
    if absent_count > 0 {
        return Err(BuildError::Invalid(format!(
            "pairs file references {absent_count} entity ids absent from the points file \
             (smallest: {absent_min})"
        )));
    }

    let mut order: Vec<(u64, u64)> = first_ordinal
        .iter()
        .map(|(&source_term, &(ordinal, _))| (ordinal, source_term))
        .collect();
    order.sort_unstable();

    // A source term's descriptor is its index key: the decimal for the exploded relation's
    // integer ids, and a key a label gives an item for a field-sourced view.
    // Streamed, not interned: the descriptors here are distinct by construction (one per
    // distinct source term) and arrive in term-id order, which is `DictStreamWriter`'s exact
    // contract — at T = 117M an interner is gigabytes of pointless ownership.
    let mut dict = mosaica_authz::DictStreamWriter::new(dict_dir);
    // **`public` is appended first, so it is term 0 in every bundle** and is minted for no other
    // descriptor — the streaming half of what `build_in_memory` does by interning it first. A
    // source term spelling `public` therefore maps to 0 rather than appending a second record,
    // which would put one descriptor in the dictionary twice.
    let public = dict
        .append(mosaica_authz::PUBLIC_LABEL)
        .map_err(|e| BuildError::io(dict_dir, e))?;
    debug_assert_eq!(public, mosaica_authz::PUBLIC_TERM);
    let mut pairs_of_term: Vec<(u64, u32)> = Vec::with_capacity(order.len());
    let mut row_counts: Vec<u64> = vec![0; 1];
    for &(_, source_term) in &order {
        let descriptor = access.descriptors.descriptor(source_term);
        let rows = first_ordinal[&source_term].1;
        if descriptor.as_bytes() == mosaica_authz::PUBLIC_LABEL {
            pairs_of_term.push((source_term, public.raw()));
            row_counts[public.raw() as usize] = rows;
            continue;
        }
        let term = dict
            .append(descriptor.as_bytes())
            .map_err(|e| BuildError::io(dict_dir, e))?;
        pairs_of_term.push((source_term, term.raw()));
        row_counts.push(rows);
    }
    drop(first_ordinal);
    drop(order);
    let term_count = dict.len() as u64;
    let dict_paths = dict.finish().map_err(|e| BuildError::io(dict_dir, e))?;
    for path in &dict_paths {
        fsync_file(path)?;
    }
    // The lookup arrays: source-term-ascending, consumed by merge sweeps over term-sorted
    // chunks. (Unique keys — one entry per distinct term — so the parallel unstable sort has
    // one output.)
    pairs_of_term.par_sort_unstable_by_key(|&(source_term, _)| source_term);
    let term_keys: Vec<u64> = pairs_of_term.iter().map(|&(st, _)| st).collect();
    let term_ids: Vec<u32> = pairs_of_term.iter().map(|&(_, tid)| tid).collect();
    Ok(Dictionary {
        term_keys,
        term_ids,
        row_counts,
        term_count,
        pair_rows,
        histogram,
        histogram_shift,
        dict_paths,
    })
}

/// Break the pre-sort's ties by full signature comparison.
///
/// `recs` arrives ordered by `(two-term key, ordinal)`. Items sharing a key share their first two
/// terms; where every one of them has a signature of at most two terms they are *identical*
/// signatures and the ordinal tiebreak already holds. Only a group containing a longer signature
/// needs refining — and inside such a group the work decomposes, because of two facts about the
/// stage-4 key:
///
/// * **Every member of a refined group has exactly two or more terms.** The key encodes "no
///   term at this position" as 0 and a real term `t` as `t + 1 ≥ 1` — and `t + 1` cannot wrap
///   to 0, because term ids are checked below 2³²−1 right after the dictionary is built. So a
///   zero-length signature keys as (0, 0) and a one-term signature as (t₀+1, 0), neither of
///   which a long (>2-term) signature — (t₀+1, t₁+1), both halves ≥ 1 — can collide with. A
///   refined group therefore holds only **short** members whose signature is *exactly* the
///   group's two-term prefix (all identical) and **long** members extending that prefix.
/// * **A prefix orders before every extension of it**, and identical signatures tie — so the
///   refined order is: all shorts first, in the `(morton, ordinal)` order the pre-sort already
///   established; then the longs, ordered by their signature *tails* (terms from index 2 on) with
///   `(morton, ordinal)` breaking exact-tail ties. That is precisely the reference build's
///   `(signature, morton, source_id)` order (decision 0073), with the tiebreak the current stable
///   sort left implicit made explicit.
///
///   **The shorts get the Morton tiebreak for free and the longs must be given it**, because the
///   pre-sort key stops at two terms: within a tie group the shorts are already fully ordered by
///   the key the parallel sort ran on, and the longs are re-sorted here from their tails up.
///
/// Mechanically, each long member's tail is gathered **once** into a scratch arena — its
/// location in `packed` comes from the batch's per-ordinal starts array in O(1), not from a
/// binary search — and the
/// long sort compares contiguous scratch, not the multi-gigabyte relation. The previous
/// implementation did two `partition_point` probes over `packed` *per comparison*; with 46% of
/// the probe corpus inside refined groups that was the single largest cost of the whole
/// build (measured: 52.5% of the 1e8 build, superlinear).
///
/// Groups are disjoint slices of `recs`, so refinement runs in parallel across groups; each
/// group's result is deterministic, so the whole pass is.
fn refine_signature_ties(
    recs: &mut [SortRec],
    packed: &[u64],
    starts: &[u32],
    base_ordinal: u64,
    long_sig: &[u64],
) {
    // Group boundaries first (cheap linear scan), keeping only groups that need refining.
    let mut refined: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    while start < recs.len() {
        let mut end = start + 1;
        while end < recs.len()
            && recs[end].key_hi == recs[start].key_hi
            && recs[end].key_lo == recs[start].key_lo
        {
            end += 1;
        }
        if end - start > 1
            && recs[start..end]
                .iter()
                .any(|r| bit_get(long_sig, (r.ordinal as u64 - base_ordinal) as usize))
        {
            refined.push((start, end));
        }
        start = end;
    }

    // Split `recs` into one disjoint `&mut` slice per refined group (safe: the ranges are
    // ascending and non-overlapping; `split_at_mut` walks them off the front).
    let mut groups: Vec<&mut [SortRec]> = Vec::with_capacity(refined.len());
    let mut rest = recs;
    let mut consumed = 0usize;
    for &(g_start, g_end) in &refined {
        let (_, tail) = rest.split_at_mut(g_start - consumed);
        let (group, tail) = tail.split_at_mut(g_end - g_start);
        groups.push(group);
        rest = tail;
        consumed = g_end;
    }

    groups
        .into_par_iter()
        .for_each_init(RefineScratch::default, |scratch, group| {
            refine_group(group, packed, starts, base_ordinal, long_sig, scratch)
        });
}

/// Per-worker buffers for [`refine_group`], reused across the groups a worker processes.
#[derive(Default)]
struct RefineScratch {
    shorts: Vec<SortRec>,
    /// `(tail start in `arena`, tail length, the rec)` per long member.
    longs: Vec<(usize, usize, SortRec)>,
    /// Worst case for one group is all long tails in the corpus sharing one two-term prefix —
    /// 4 bytes per tail term, approaching 4P in the fully degenerate one-group corpus. The
    /// probe corpus's shape stays in the tens of megabytes; a corpus pathological enough to matter
    /// here would already be pathological for `flat` (4P, resident in the same build).
    arena: Vec<u32>,
}

/// Refine one tie group: shorts keep their order at the front, longs sort by (tail, ordinal).
/// See [`refine_signature_ties`] for why this equals the full-signature stable sort.
fn refine_group(
    group: &mut [SortRec],
    packed: &[u64],
    starts: &[u32],
    base_ordinal: u64,
    long_sig: &[u64],
    s: &mut RefineScratch,
) {
    // A refined group's key has both halves non-zero (the disjointness argument above); a
    // violation would mean a short member with fewer than two terms slipped in, which the
    // partition below would order incorrectly. Loud in debug, impossible by construction.
    debug_assert!(
        group[0].key_hi != 0 && group[0].key_lo != 0,
        "a refined tie group must hold only signatures of length >= 2"
    );
    s.shorts.clear();
    s.longs.clear();
    s.arena.clear();
    for &rec in group.iter() {
        let local = (rec.ordinal as u64 - base_ordinal) as usize;
        if bit_get(long_sig, local) {
            let tail_start = s.arena.len();
            // Skip the two prefix terms every member shares; the batch's per-ordinal starts
            // array bounds the slice in O(1).
            for &value in &packed[starts[local] as usize + 2..starts[local + 1] as usize] {
                s.arena.push(term_of(value));
            }
            debug_assert!(
                s.arena.len() > tail_start,
                "a long signature has at least one tail term"
            );
            s.longs.push((tail_start, s.arena.len() - tail_start, rec));
        } else {
            s.shorts.push(rec);
        }
    }
    let arena = &s.arena;
    // (tail, ordinal) is a total order on unique keys — ordinals are unique — so this
    // unstable sort has exactly one output, identical to the stable full-signature sort's.
    // (tail, morton, ordinal): the same order the pre-sort gives the shorts, continued past the
    // two-term prefix the pre-sort key stops at (decision 0073).
    s.longs.sort_unstable_by(|a, b| {
        arena[a.0..a.0 + a.1]
            .cmp(&arena[b.0..b.0 + b.1])
            .then_with(|| a.2.morton.cmp(&b.2.morton))
            .then_with(|| a.2.ordinal.cmp(&b.2.ordinal))
    });
    group[..s.shorts.len()].copy_from_slice(&s.shorts);
    for (k, &(_, _, rec)) in s.longs.iter().enumerate() {
        group[s.shorts.len() + k] = rec;
    }
}

/// The distinct category-width codes a column of values carries — the input
/// [`crate::layers::predicate_artifact_keys`] reads a predicate layer's roster out of.
///
/// **Absence is not a value**, so a point with no value for the column is in none of the layer's
/// artifacts, exactly as a member row with a null key is in none of an enumerated layer's. A value
/// of any other width contributes nothing either: `compile_membership` refuses such a column at the
/// declaration, so one reaching here is a schema that never validated rather than a value to guess
/// at.
pub(crate) fn distinct_codes(
    values: impl Iterator<Item = ScalarValue>,
) -> std::collections::BTreeSet<u32> {
    let mut codes = std::collections::BTreeSet::new();
    for value in values {
        match value {
            ScalarValue::U8(v) => codes.insert(u32::from(v)),
            ScalarValue::U16(v) => codes.insert(u32::from(v)),
            ScalarValue::U32(v) => codes.insert(v),
            _ => false,
        };
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the explicit text and keyword plans read their runs under: a fan-in of 32 on four
    /// threads, so a plan of many runs cascades, several groups at once.
    pub(super) const TEST_MERGE: mosaica_filter_write::Allowance =
        mosaica_filter_write::Allowance {
            bytes: 32 * mosaica_filter_write::RUN_BUFFER_MIN as u64,
            threads: 4,
        };

    /// An accumulator's budget a little above what a spill holds beside it and its first pool
    /// page, so that it spills a run every few documents of the text fixture.
    const SMALL_WORKER: usize = mosaica_filter_write::RUN_BUFFER_MIN + (300 << 10);

    /// **The extent route is decided by the order a column's readers want, not by its type.**
    ///
    /// The entity-indexed offset array beside an arena answers `entity → value` at random, and
    /// the one pass that asks a string column that question is the hot row tail. Every other
    /// reader — the record blob's merge, a `text` column's token index, a keyword dictionary's
    /// chunk walk — reads the column once, ascending, which an extent merge answers. So every
    /// bundle-wide string column has two routes and `render` is what closes one.
    ///
    /// **And every column routed there has a reader for its extents.** Extents nothing reads
    /// would be a column stored nowhere: a blob-resident column's are merged into the blob, and
    /// an indexed one's are its dictionary pass's.
    #[test]
    fn the_extent_route_is_the_readers_and_not_the_type() {
        let declared = |ty, index, render| crate::config::Attribute {
            name: "column".to_string(),
            field: None,
            title: None,
            ty,
            analyser: None,
            vocabulary: None,
            value_set: None,
            index,
            render,
            unique: false,
        };
        let schema = |attribute| crate::config::Schema {
            attributes: vec![attribute],
            vocabularies: Default::default(),
        };
        let cases = [
            // A string column with no reader at an entity: the blob's, and spilled.
            (ScalarType::Keyword, false, false, true),
            (ScalarType::Utf8, false, false, true),
            (ScalarType::Text, false, false, true),
            // Indexed: `text` owes a token index over the extents, and the other two owe a value
            // column and a dictionary, whose one pass reads the extents in entity order.
            (ScalarType::Text, true, false, true),
            (ScalarType::Keyword, true, false, true),
            (ScalarType::Utf8, true, false, true),
            // Rendered: the row tail reads the column at an entity. ⊘ `render` on a string type
            // is refused at the declaration and `mosaica_store::write::columns_layout` refuses one
            // that arrives anyway, so this arm is reachable only from a `Schema` built
            // programmatically.
            (ScalarType::Keyword, false, true, false),
            // A fixed-width column is a slot and is never spilled, whatever its flags say.
            (ScalarType::U32, false, false, false),
            (ScalarType::U32, true, false, false),
            (ScalarType::F64, false, true, false),
        ];
        for (ty, index, render, expected) in cases {
            let schema = schema(declared(ty, index, render));
            let attribute = &schema.attributes[0];
            assert_eq!(
                may_take_extents(&schema, attribute),
                expected,
                "{ty:?} index={index} render={render}"
            );
            if may_take_extents(&schema, attribute) {
                assert!(
                    blob_resident(&schema, attribute) || value_column_is_owed(&schema, attribute),
                    "{ty:?} index={index} render={render}: spilled with no reader for the extents"
                );
                // The arena route is closed to `text` and open to the rest: the second decode a
                // `text` arena costs is the source permutation and not the arena's size
                // (`build-column-extents.md` §2).
                assert_eq!(
                    extents_are_forced(&schema, attribute),
                    ty == ScalarType::Text,
                    "{ty:?} index={index} render={render}: the wrong route is the unconditional one"
                );
            }
        }
    }

    /// **Every declared field lands in exactly one home, and the two placement passes must agree
    /// on which** (records §3). The blob takes a field the entity-space pass declines, so the
    /// question both ask is `postings_are_owed`: a `derived` category keeps its entity-space
    /// floor and gets no blob row, while a `public` category with neither flag — which that pass
    /// declines, having no `index` and no `derived` visibility — must land here rather than
    /// nowhere.
    ///
    /// The `public` half is a regression test. Excluding every category from the blob reads as
    /// records §4.2's rule and is not: §4.2's premise is the entity-space floor, and a `public`
    /// category with no `index` has none. The field was stored in no home at all, and nothing
    /// refused the declaration — the caller declared a column and the corpus silently dropped it.
    #[test]
    fn a_category_is_blob_resident_exactly_when_it_has_no_entity_space_home() {
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let category = crate::config::Attribute {
            name: "department".to_string(),
            field: None,
            title: None,
            ty: ScalarType::U16,
            analyser: None,
            vocabulary: Some("departments".to_string()),
            value_set: Some(crate::config::ValueSet::Closed),
            index: false,
            render: false,
            unique: false,
        };
        let note = crate::config::Attribute {
            name: "note".to_string(),
            field: None,
            title: None,
            ty: ScalarType::Keyword,
            analyser: None,
            vocabulary: None,
            value_set: None,
            index: false,
            render: false,
            unique: false,
        };

        // A `derived` visibility is what gives a category its entity-space floor.
        let vocabularies_at = |visibility| {
            let mut v = std::collections::HashMap::new();
            v.insert(
                "departments".to_string(),
                crate::config::Vocabulary {
                    name: "departments".to_string(),
                    title: None,
                    value_set: crate::config::ValueSet::Closed,
                    width: ScalarType::U16,
                    values: crate::config::VocabularyMinter::new(
                        "departments",
                        crate::config::VocabularyKind::Declared,
                        visibility,
                        ScalarType::U16,
                    ),
                    reserved: Vec::new(),
                },
            );
            v
        };

        let dir = tempfile::tempdir().expect("tempdir");
        let schema = crate::config::Schema {
            attributes: vec![category.clone(), note.clone()],
            vocabularies: vocabularies_at(crate::config::Visibility::Derived),
        };
        // One entity; values are per column, in declaration order. The keyword's value arrives
        // as an extent rather than as a column, [`ColumnRoutes`] having routed it there, and the
        // blob's answer is what this test is about either way.
        let by_entity = vec![
            EntityColumn::from_values(&scratch, ScalarType::U16, [ScalarValue::U16(7)], "colour")
                .expect("typed column"),
            EntityColumn::spilled(&scratch, ScalarType::Keyword, 1).expect("the note slot"),
        ];
        let mut notes = crate::extents::ExtentColumn::new(dir.path(), 1, "note");
        notes.push_extent(&[(0u32, "kept")]).expect("an extent");
        let open = [notes.open().expect("the extents open")];
        let written = write_record_blob(
            dir.path(),
            &schema,
            by_entity[0].len() as u64,
            &ColumnRoutes::every_available(&schema),
            &by_entity,
            &open,
        )
        .expect("blob stage writes");
        assert!(!written.is_empty());
        let blob = mosaica_filter::RecordBlob::open_dir(dir.path(), mosaica_filter::Access::Mapped)
            .expect("open");
        let fields = blob
            .fields_of(0)
            .expect("read")
            .expect("entity 0 has a row");
        assert_eq!(
            fields.len(),
            1,
            "only the utf8 column is blob-resident; the category's home is entity space"
        );
        assert_eq!(fields[0].tag, 1, "the surviving field is `note`, tag 1");

        // Alone, the `derived` category leaves the stage with nothing to write at all.
        let dir = tempfile::tempdir().expect("tempdir");
        let schema = crate::config::Schema {
            attributes: vec![category.clone()],
            vocabularies: vocabularies_at(crate::config::Visibility::Derived),
        };
        let only_category =
            [
                EntityColumn::from_values(
                    &scratch,
                    ScalarType::U16,
                    [ScalarValue::U16(7)],
                    "colour",
                )
                .expect("typed column"),
            ];
        let written = write_record_blob(
            dir.path(),
            &schema,
            only_category[0].len() as u64,
            &ColumnRoutes::every_available(&schema),
            &only_category,
            &[],
        )
        .expect("blob stage accepts");
        assert!(written.is_empty(), "no blob-resident column, no files");

        // But the same category under `public` owes no value column and no postings, so
        // the blob is its only home and must take it.
        let dir = tempfile::tempdir().expect("tempdir");
        let schema = crate::config::Schema {
            attributes: vec![category],
            vocabularies: vocabularies_at(crate::config::Visibility::Public),
        };
        let only_category =
            [
                EntityColumn::from_values(
                    &scratch,
                    ScalarType::U16,
                    [ScalarValue::U16(7)],
                    "colour",
                )
                .expect("typed column"),
            ];
        let written = write_record_blob(
            dir.path(),
            &schema,
            only_category[0].len() as u64,
            &ColumnRoutes::every_available(&schema),
            &only_category,
            &[],
        )
        .expect("blob stage writes");
        assert!(
            !written.is_empty(),
            "a public category with neither flag has no entity-space home; without a blob row \
             its values are stored nowhere at all"
        );
        let blob = mosaica_filter::RecordBlob::open_dir(dir.path(), mosaica_filter::Access::Mapped)
            .expect("open");
        assert_eq!(
            blob.fields_of(0)
                .expect("read")
                .expect("entity 0 has a row"),
            vec![mosaica_filter::RecordField {
                tag: 0,
                value: mosaica_filter::RecordValue::U16(7),
            }],
            "the public category's value is the blob row"
        );
    }

    /// **The cascade is not observable either.** A column that spilled more extents than one
    /// merge holds open folds them in groups first, and the fold has to leave the same relation:
    /// the same entities, each carrying the last value written for it, in the same order.
    #[test]
    fn folding_the_extents_leaves_the_same_rows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fan_in = 8usize;
        let extents = fan_in * 2 + 3;
        let mut spilled = crate::extents::ExtentColumn::new(dir.path(), 0, "abstract");
        // One row per extent, walking entity space in a stride so the extents interleave, plus a
        // second value for one entity in a later extent than the one that first carried it.
        for extent in 0..extents {
            let entity = ((extent * 7) % extents) as u32;
            let value = format!("value {extent}");
            spilled
                .push_extent(&[(entity, value.as_str())])
                .expect("an extent");
        }
        spilled
            .push_extent(&[(3u32, "the later value")])
            .expect("an extent");
        let before = prose_rows(&spilled);
        spilled.cascade(fan_in).expect("the cascade");
        assert_eq!(before, prose_rows(&spilled), "the fold moved a value");
        assert_eq!(before.get(&3).map(String::as_str), Some("the later value"));
    }

    /// Every live `(entity, value)` a spilled column holds, which is what both its readers see.
    fn prose_rows(
        column: &crate::extents::ExtentColumn,
    ) -> std::collections::BTreeMap<u32, String> {
        let open = column.open().expect("the extents open");
        let mut rows: std::collections::BTreeMap<u32, String> = Default::default();
        for window in open.windows(3) {
            open.for_each_record_in(window, &mut |entity, value| {
                rows.insert(entity as u32, value.to_string());
                Ok(())
            })
            .expect("the walk");
        }
        rows
    }

    /// **The join's chunking must not be observable in the record blob.** A chunk is one extent
    /// per spilled column, so a corpus joined in one chunk and the same corpus joined in chunks of
    /// three have to be the same three files. What that pins is the merge: an entity's fields
    /// gathered from two families of extents and an entity-ordered column, ordered by tag, with
    /// the last chunk that carried a value winning.
    ///
    /// **Two spilled families and a column**, because that is the shape [`ColumnRoutes`] routes
    /// on a real schema: a `keyword` column with neither `index` nor `render` takes the extents a
    /// `text` column takes, and a fixed-width column with the same flags stays where it was. A
    /// tag reaching the merge from two sources at one entity would be a value overwriting
    /// another's.
    ///
    /// The corpus straddles what the merge has to answer. Entities carry a count, a keyword,
    /// prose, several of those or none; one prose value is the empty string, which is a value and
    /// not absence; two entities are written twice, one of them across a chunk boundary at
    /// several of the chunkings and one within a chunk at all of them.
    #[test]
    fn the_extents_do_not_change_the_blobs_bytes() {
        const N: usize = 64;
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let string_column = |name: &str, ty: ScalarType| crate::config::Attribute {
            name: name.to_string(),
            field: None,
            title: None,
            ty,
            analyser: None,
            vocabulary: None,
            value_set: None,
            index: false,
            render: false,
            unique: false,
        };
        let schema = crate::config::Schema {
            attributes: vec![
                string_column("count", ScalarType::U32),
                string_column("note", ScalarType::Keyword),
                string_column("abstract", ScalarType::Text),
            ],
            vocabularies: Default::default(),
        };
        for attribute in &schema.attributes[1..] {
            assert!(
                may_take_extents(&schema, attribute),
                "'{}' is what this test is about",
                attribute.name
            );
        }
        assert!(
            !may_take_extents(&schema, &schema.attributes[0]),
            "a fixed-width column stays a column"
        );

        // The fixed-width column is entity-ordered as it always was: a third of the entities carry
        // a count.
        let counts: Vec<ScalarValue> = (0..N)
            .map(|entity| match entity % 3 {
                0 => ScalarValue::U32(entity as u32 * 10),
                _ => ScalarValue::Null,
            })
            .collect();

        // Each spilled column as its own source yields it: entity order is not source order, one
        // entity is written twice adjacently and one far apart, and one prose value is the empty
        // string. The two sources disagree about which entities they cover and about the order
        // they arrive in, so no chunking aligns their extents.
        let mut prose_source: Vec<(u32, String)> = (0..N)
            .filter(|entity| entity % 5 != 1)
            .map(|entity| {
                let value = if entity == 20 {
                    String::new()
                } else {
                    format!("prose for {entity}, long enough to be worth a block")
                };
                ((entity * 37 % N) as u32, value)
            })
            .collect();
        prose_source.push((prose_source[3].0, "the later value, adjacent".to_string()));
        prose_source.insert(
            2,
            (
                prose_source[9].0,
                "the earlier value, far apart".to_string(),
            ),
        );
        let mut note_source: Vec<(u32, String)> = (0..N)
            .filter(|entity| entity % 4 != 3)
            .map(|entity| ((entity * 11 % N) as u32, format!("note-{entity}")))
            .collect();
        note_source.push((note_source[7].0, "the later note, adjacent".to_string()));
        note_source.insert(
            1,
            (note_source[19].0, "the earlier note, far apart".to_string()),
        );

        let blob_of = |dir: &Path, chunk: usize| -> Vec<PathBuf> {
            let extent_dir = dir.join("extents");
            std::fs::create_dir_all(&extent_dir).expect("extent dir");
            let notes = spilled_column(&extent_dir, 1, "note", &note_source, chunk);
            let prose = spilled_column(&extent_dir, 2, "abstract", &prose_source, chunk);
            let open = [
                notes.open().expect("the extents open"),
                prose.open().expect("the extents open"),
            ];
            let by_entity = [
                EntityColumn::from_values(
                    &scratch,
                    ScalarType::U32,
                    counts.iter().cloned(),
                    "count",
                )
                .expect("typed column"),
                EntityColumn::spilled(&scratch, ScalarType::Keyword, N).expect("the note slot"),
                EntityColumn::spilled(&scratch, ScalarType::Text, N).expect("the prose slot"),
            ];
            write_record_blob(
                dir,
                &schema,
                N as u64,
                &ColumnRoutes::every_available(&schema),
                &by_entity,
                &open,
            )
            .expect("the blob merges")
        };

        let mut written: Vec<Vec<(PathBuf, Vec<u8>)>> = Vec::new();
        for chunk in [1usize, 2, 3, 5, 7, 64, 4_096] {
            let dir = tempfile::tempdir().expect("tempdir");
            let paths = blob_of(dir.path(), chunk);
            assert_eq!(paths.len(), 3, "the blob is three files");
            written.push(
                paths
                    .iter()
                    .map(|path| {
                        (
                            path.strip_prefix(dir.path()).expect("under the dir").into(),
                            std::fs::read(path).expect("read back"),
                        )
                    })
                    .collect(),
            );
        }
        let first = written.first().expect("at least one chunking").clone();
        for (k, emitted) in written.iter().enumerate().skip(1) {
            assert_eq!(emitted, &first, "chunking {k} wrote different bytes");
        }

        // And the merged blob says what the corpus says.
        let dir = tempfile::tempdir().expect("tempdir");
        blob_of(dir.path(), 3);
        let blob = mosaica_filter::RecordBlob::open_dir(dir.path(), mosaica_filter::Access::Read)
            .expect("open");
        // The last value written for an entity is the one the blob holds.
        let last_of = |source: &[(u32, String)]| {
            let mut held: std::collections::BTreeMap<u32, String> = Default::default();
            for (entity, value) in source {
                held.insert(*entity, value.clone());
            }
            held
        };
        let expected_prose = last_of(&prose_source);
        let expected_note = last_of(&note_source);
        for entity in 0..N as u32 {
            let fields = blob.fields_of(entity).expect("read");
            let utf8_at = |tag: u16| -> Option<String> {
                fields.as_ref().and_then(|fields| {
                    fields
                        .iter()
                        .find(|field| field.tag == tag)
                        .map(|field| match &field.value {
                            mosaica_filter::RecordValue::Utf8(value) => value.clone(),
                            other => panic!("entity {entity} carries {other:?} at tag {tag}"),
                        })
                })
            };
            assert_eq!(
                utf8_at(2),
                expected_prose.get(&entity).cloned(),
                "entity {entity}'s prose"
            );
            assert_eq!(
                utf8_at(1),
                expected_note.get(&entity).cloned(),
                "entity {entity}'s note"
            );
            let count = fields.as_ref().and_then(|fields| {
                fields
                    .iter()
                    .find(|field| field.tag == 0)
                    .map(|field| field.value.clone())
            });
            let expected = match entity % 3 {
                0 => Some(mosaica_filter::RecordValue::U32(entity * 10)),
                _ => None,
            };
            assert_eq!(count, expected, "entity {entity}'s count");
        }
    }

    /// One column's source rows spilled as extents of `chunk` rows each, the way the join spills
    /// them: sorted by entity within the chunk, stably, with the last row carrying a value
    /// winning. Spelt out rather than calling [`spill_extent_chunk`], so that the test states the
    /// rule it is asserting about.
    fn spilled_column(
        dir: &Path,
        column: usize,
        name: &str,
        source: &[(u32, String)],
        chunk: usize,
    ) -> crate::extents::ExtentColumn {
        let mut spilled = crate::extents::ExtentColumn::new(dir, column, name);
        for rows in source.chunks(chunk) {
            let mut held: Vec<(u32, &str)> = rows
                .iter()
                .map(|(entity, value)| (*entity, value.as_str()))
                .collect();
            held.sort_by_key(|&(entity, _)| entity);
            let mut kept: Vec<(u32, &str)> = Vec::new();
            for row in held {
                if kept.last().map(|&(entity, _)| entity) == Some(row.0) {
                    kept.pop();
                }
                kept.push(row);
            }
            spilled.push_extent(&kept).expect("an extent");
        }
        spilled
    }

    /// **The chunking must not be observable in the artefact.** A chunk boundary is a place one
    /// worker's accumulator ends and another's begins, and a run boundary is a place one worker
    /// spills mid-chunk — so a column emitted in one chunk and the same column emitted in chunks
    /// of seven have to be the same two files. That is what makes the plan a memory knob rather
    /// than a format decision, and it is the assertion an off-by-one at a boundary fails: a term
    /// whose postings split across two runs and lost half of them changes only the bytes.
    ///
    /// The plans below straddle deliberately. `1` and `7` do not divide the corpus and do not
    /// align to the 64-entity words presence is stored in; `64` and `128` align exactly; `4_096`
    /// is one chunk for the whole column. The byte budgets force between one and dozens of runs a
    /// chunk, and the merge-slice caps put the same corpus through both posting encoders — a term
    /// carried by every entity goes through the `u32` slice under the large cap and through
    /// Roaring under the small one, and the two must agree byte for byte.
    ///
    /// The merge's term ranges must not be observable either: several ranges over runs, over the
    /// accumulators held in memory and over a cascade's runs, on one thread and several, with a
    /// window as wide as the ranges and as narrow as the threads, and with a range buffer of a
    /// byte, so that every range but the head waits part way through its first term and goes on
    /// into the writer from there.
    #[test]
    fn chunking_the_text_index_does_not_change_its_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let attribute = text_fixture_attribute();
        let values = EntityColumn::from_values(
            &scratch,
            ScalarType::Text,
            (0..N_TEXT).map(text_fixture_prose),
            "abstract",
        )
        .expect("typed column");
        // The two producers over the same corpus: the scoped column's arena, and the extents the
        // join spills for a bundle-wide column. One, three and eleven of them, interleaved.
        let extent_dir = tempfile::tempdir().expect("tempdir");
        let spilled: Vec<crate::extents::ExtentColumn> = [1usize, 3, 11]
            .into_iter()
            .map(|extents| {
                let dir = extent_dir.path().join(format!("extents-{extents}"));
                std::fs::create_dir_all(&dir).expect("extent dir");
                text_fixture_extents(&dir, extents)
            })
            .collect();
        let open: Vec<crate::extents::OpenExtents> = spilled
            .iter()
            .map(|column| column.open().expect("the extents open"))
            .collect();
        let sources: Vec<TextValues<'_>> = std::iter::once(TextValues::Arena(&values))
            .chain(open.iter().map(TextValues::Extents))
            .collect();

        let mut files: Vec<[Vec<u8>; 4]> = Vec::new();
        for (k, plan) in [
            TextIndexPlan::explicit(4_096, 1 << 30, 1 << 22),
            TextIndexPlan::explicit(1, 1 << 30, 1 << 22),
            TextIndexPlan::explicit(7, 1 << 30, 1 << 22),
            TextIndexPlan::explicit(64, 1 << 30, 1 << 22),
            TextIndexPlan::explicit(128, 1 << 30, 1 << 22),
            TextIndexPlan::explicit(999, 1 << 30, 1 << 22),
            // The byte budget: one small enough that every document spills, and one a little
            // above a spill's writer buffer and the accumulator's first pool page, so that each
            // worker spills several runs of several documents, its terms gathered across the
            // windows it took.
            TextIndexPlan::explicit(4_096, 1, 1 << 22),
            TextIndexPlan::explicit(4_096, SMALL_WORKER, 1 << 22),
            TextIndexPlan::explicit(7, SMALL_WORKER, 1 << 22),
            // The merge's other encoder: a cap of 1 sends every term above one posting through
            // the Roaring arm.
            TextIndexPlan::explicit(4_096, 1 << 30, 1),
            TextIndexPlan::explicit(7, 1, 1),
            // The merge in term ranges.
            TextIndexPlan::explicit(7, SMALL_WORKER, 1 << 22).in_ranges(ranges(2, 2, 2, 1 << 30)),
            TextIndexPlan::explicit(7, SMALL_WORKER, 1 << 22).in_ranges(ranges(7, 3, 7, 1)),
            TextIndexPlan::explicit(64, 1 << 30, 1 << 22).in_ranges(ranges(16, 4, 4, 256)),
            TextIndexPlan::explicit(64, 1 << 30, 1).in_ranges(ranges(5, 1, 1, 1)),
            TextIndexPlan::explicit(4_096, 1, 1 << 22).in_ranges(ranges(6, 4, 8, 1 << 10)),
        ]
        .into_iter()
        .enumerate()
        {
            for (source, values) in sources.iter().enumerate() {
                let column_dir = dir.path().join(format!("plan-{k}-{source}"));
                std::fs::create_dir_all(&column_dir).expect("column dir");
                let values = match values {
                    TextValues::Arena(column) => TextValues::Arena(column),
                    TextValues::Extents(prose) => TextValues::Extents(prose),
                };
                let written =
                    write_text_index(&column_dir, &attribute, values, plan, Steps::default())
                        .expect("text index");
                assert_eq!(
                    written.terms,
                    expected_text_postings().len() as u64,
                    "plan {k} source {source} wrote the wrong term count"
                );
                let left: Vec<String> = std::fs::read_dir(&column_dir)
                    .expect("read dir")
                    .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                    .filter(|name| name.ends_with(".spill"))
                    .collect();
                assert!(left.is_empty(), "plan {k} source {source} left {left:?}");
                files.push(index_files(&column_dir));
            }
            let column_dir = dir.path().join(format!("plan-{k}"));
            std::fs::create_dir_all(&column_dir).expect("column dir");
            let written = write_text_index(
                &column_dir,
                &attribute,
                TextValues::Arena(&values),
                plan,
                Steps::default(),
            )
            .expect("text index");
            assert_eq!(
                written.terms,
                expected_text_postings().len() as u64,
                "plan {k} wrote the wrong term count"
            );
            // Nothing but the artefacts is left behind: every run this plan spilled is gone.
            let left: Vec<String> = std::fs::read_dir(&column_dir)
                .expect("read dir")
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".spill"))
                .collect();
            assert!(left.is_empty(), "plan {k} left {left:?} behind");
            files.push(index_files(&column_dir));
        }
        let [dict, postings, positions, presence] =
            files.first().expect("at least one plan").clone();
        for (k, [d, p, q, e]) in files.iter().enumerate().skip(1) {
            assert_eq!(d, &dict, "plan {k}'s dictionary differs");
            assert_eq!(p, &postings, "plan {k}'s postings differ");
            assert_eq!(q, &positions, "plan {k}'s positions differ");
            assert_eq!(e, &presence, "plan {k}'s presence differs");
        }
    }

    /// A merge in `count` term ranges on `threads` threads, `window` in flight, each holding up to
    /// `buffer` bytes before its turn.
    fn ranges(
        count: usize,
        threads: usize,
        window: usize,
        buffer: usize,
    ) -> mosaica_filter_write::Ranges {
        mosaica_filter_write::Ranges::Fixed {
            ranges: count,
            threads,
            window,
            buffer,
        }
    }

    /// A text index's dictionary, postings, positions and presence, as bytes.
    fn index_files(column_dir: &Path) -> [Vec<u8>; 4] {
        [
            mosaica_filter::DICT_FILE,
            mosaica_filter::POSTINGS_FILE,
            mosaica_filter::POSITIONS_FILE,
            mosaica_filter::PRESENCE_FILE,
        ]
        .map(|name| std::fs::read(column_dir.join(name)).expect(name))
    }

    /// And the index says what the column says: every term the analyser produces is a key, its
    /// posting is exactly the entities whose prose carries it, and its positions in each are where
    /// the analyser put it — read back through the readers that will serve it, over a plan of many
    /// chunks and many runs apiece, so what is asserted is the merge's output and not one worker's
    /// accumulator.
    #[test]
    fn the_merged_text_index_holds_every_term_and_the_entities_carrying_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let values = EntityColumn::from_values(
            &scratch,
            ScalarType::Text,
            (0..N_TEXT).map(text_fixture_prose),
            "abstract",
        )
        .expect("typed column");
        write_text_index(
            dir.path(),
            &text_fixture_attribute(),
            TextValues::Arena(&values),
            TextIndexPlan::explicit(97, 2_048, 1 << 22),
            Steps::default(),
        )
        .expect("text index");

        let dict = mosaica_filter::SortedDict::open(
            &dir.path().join(mosaica_filter::DICT_FILE),
            mosaica_filter::Access::Read,
        )
        .expect("the token dictionary opens");
        let postings = mosaica_authz::postings::PostingsReader::open(
            &dir.path().join(mosaica_filter::POSTINGS_FILE),
            false,
        )
        .expect("the postings open");
        let positions = mosaica_filter::ColumnPositions::open(
            &dir.path().join(mosaica_filter::POSITIONS_FILE),
            false,
        )
        .expect("the positions open");

        let expected = expected_text_postings();
        assert!(
            expected.iter().any(|(_, items)| items.len() > 3 * mosaica_filter::POSITIONS_BLOCK),
            "a term spans several blocks of positions"
        );
        assert_eq!(dict.len() as usize, expected.len());
        assert_eq!(postings.term_count() as usize, expected.len());
        for (ordinal, (term, items)) in expected.iter().enumerate() {
            let entities: Vec<u32> = items.iter().map(|(entity, _)| *entity).collect();
            assert_eq!(
                dict.resolve(term).expect("a readable dictionary"),
                Some(ordinal as u32),
                "term {term:?} is at the wrong ordinal"
            );
            let posting = postings
                .posting_at(ordinal as u32)
                .expect("a readable posting")
                .expect("every term in the dictionary has a posting");
            let got: Vec<u32> = posting.iter().collect();
            assert_eq!(got, entities, "term {term:?} carries the wrong entities");
            let mut cursor = positions
                .cursor(mosaica_types::AttrLocalId::new(ordinal as u32), items.len() as u64)
                .expect("every term has a positions record");
            for (rank, (entity, want)) in items.iter().enumerate() {
                assert_eq!(
                    cursor.positions(rank as u64).expect("a readable entry"),
                    want.as_slice(),
                    "term {term:?} in entity {entity}"
                );
            }
        }
    }

    /// The corpus both keyword tests read. Every shape the dictionary pass has to survive: keys
    /// shorter than the eight bytes [`mosaica_filter_write::key_prefix`] takes, keys agreeing in
    /// exactly those eight and differing after them, a key that is a proper prefix of another, a
    /// key carried by a seventh of the corpus, keys carried by exactly one entity, a non-ASCII
    /// key, and absent runs.
    const N_KEYWORD: usize = 300;

    fn keyword_fixture_key(entity: usize) -> ScalarValue {
        if entity.is_multiple_of(11) {
            // Non-ASCII, and its ordinal is decided by its bytes like every other key's.
            return ScalarValue::Utf8("ünïcode-ключ".to_string());
        }
        match entity % 7 {
            // Absence, every seventh entity and so never aligned to the 64-entity words
            // presence is stored in.
            3 => ScalarValue::Null,
            0 => ScalarValue::Utf8("a".to_string()),
            1 => ScalarValue::Utf8("ab".to_string()),
            // A proper prefix of the keys below it, and a key of exactly eight bytes.
            2 => ScalarValue::Utf8("prefix-".to_string()),
            4 => ScalarValue::Utf8(format!("prefix-{:04}", entity % 5)),
            5 => ScalarValue::Utf8("12345678".to_string()),
            // Distinct per entity, and all of them agreeing in their first eight characters.
            _ => ScalarValue::Utf8(format!("uuid-000{entity:05}")),
        }
    }

    fn keyword_fixture_attribute() -> crate::config::Attribute {
        crate::config::Attribute {
            name: "key".to_string(),
            field: None,
            title: None,
            ty: ScalarType::Keyword,
            analyser: None,
            vocabulary: None,
            value_set: None,
            index: true,
            render: false,
            unique: false,
        }
    }

    fn keyword_fixture_column(scratch: &crate::column::ColumnScratch) -> EntityColumn {
        EntityColumn::from_values(
            scratch,
            ScalarType::Keyword,
            (0..N_KEYWORD).map(keyword_fixture_key),
            "key",
        )
        .expect("typed column")
    }

    /// The distinct keys of the fixture, sorted — the dictionary the pass owes, derived here the
    /// way the pass no longer may: whole, in memory, from the corpus.
    fn keyword_fixture_keys() -> Vec<String> {
        let mut keys: Vec<String> = (0..N_KEYWORD)
            .filter_map(|entity| match keyword_fixture_key(entity) {
                ScalarValue::Utf8(key) => Some(key),
                _ => None,
            })
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// The chunk sizes the keyword tests run under. `1` spills a run per row and cascades, 257
    /// runs over the test plans' fan-in of 32, and `4_096` is one chunk for the whole column; the
    /// rest divide the corpus, and do not, and align to the 64-entity words presence is stored in,
    /// and do not.
    const KEYWORD_CHUNKS: [usize; 7] = [1, 2, 3, 7, 64, 300, 4_096];

    /// Every plan the keyword tests run under: each chunk size hashed throughout, sorted
    /// throughout, and choosing per chunk over windows short enough that the fixture's chunks
    /// switch between the two.
    fn keyword_plans() -> Vec<KeywordDictPlan> {
        let mut plans = Vec::new();
        for chunk in KEYWORD_CHUNKS {
            for mode in [ChunkMode::Hash, ChunkMode::Sort] {
                plans.push(KeywordDictPlan::explicit(chunk, 1 << 20, Some(mode)));
            }
            for window in [1, 5, 16] {
                plans.push(KeywordDictPlan::explicit(chunk, window, None));
            }
        }
        plans
    }

    /// **The route a keyword column's characters take is not in its artefacts.** Its dictionary,
    /// its ordinals and its presence bitmap are the same three files whether the pass reads an
    /// entity-ordered arena or the extents the join spilled, at every extent chunking.
    ///
    /// The source is what makes this worth asserting rather than assuming. Entity order is not
    /// source order; one entity is written twice next to itself and one far apart, so which value
    /// an entity ends with is the arena's last write on one route and the extents' live sets on
    /// the other; and the chunkings put those two rows in one extent and in different ones.
    /// `tests/extent_route.rs` holds the same property over a whole bundle; what is here is the
    /// case a corpus of 400 rows in one chunk cannot reach.
    #[test]
    fn the_keyword_column_is_the_same_files_from_an_arena_and_from_extents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let attribute = keyword_fixture_attribute();

        // The source rows, in an order that is not entity order, with two entities written twice.
        let mut source: Vec<(u32, String)> = (0..N_KEYWORD)
            .filter_map(|entity| match keyword_fixture_key(entity) {
                ScalarValue::Utf8(key) => Some(((entity * 37 % N_KEYWORD) as u32, key)),
                _ => None,
            })
            .collect();
        source.push((source[5].0, "the later key, adjacent".to_string()));
        source.insert(2, (source[40].0, "the earlier key, far apart".to_string()));

        // The arena route's column: the same rows scattered in order, so the last write for an
        // entity is the value, which is what `EntityColumn::set` does in the join.
        let mut column =
            EntityColumn::filled(&scratch, ScalarType::Keyword, N_KEYWORD).expect("typed column");
        for (entity, key) in &source {
            column
                .set(*entity as usize, ScalarValue::Utf8(key.clone()), "key")
                .expect("set");
        }

        let files_of = |column_dir: &Path,
                        values: &EntityColumn,
                        extents: Option<&crate::extents::OpenExtents>| {
            std::fs::create_dir_all(column_dir).expect("column dir");
            let values_path = column_dir.join("values.arrow");
            let presence_path = column_dir.join(mosaica_filter::PRESENCE_FILE);
            let written = write_column_values(
                column_dir,
                &attribute,
                values,
                extents,
                KeywordDictPlan::explicit(7, 5, None),
                Steps::default(),
            )
            .expect("keyword column");
            assert!(written.presence, "the fixture has absent entities");
            let dict_path = written.dict.expect("a keyword column owes a dictionary");
            [
                std::fs::read(&dict_path).expect("dict"),
                std::fs::read(&values_path).expect("values"),
                std::fs::read(&presence_path).expect("presence"),
            ]
        };

        let from_arena = files_of(&dir.path().join("arena"), &column, None);
        let empty =
            EntityColumn::spilled(&scratch, ScalarType::Keyword, N_KEYWORD).expect("the slot");
        for chunk in [1usize, 2, 3, 7, 64, 4_096] {
            let extent_dir = dir.path().join(format!("extents-{chunk}"));
            std::fs::create_dir_all(&extent_dir).expect("extent dir");
            let spilled = spilled_column(&extent_dir, 0, "key", &source, chunk);
            let open = spilled.open().expect("the extents open");
            let from_extents = files_of(
                &dir.path().join(format!("spilled-{chunk}")),
                &empty,
                Some(&open),
            );
            assert_eq!(
                from_extents, from_arena,
                "extents in chunks of {chunk} wrote different files"
            );
        }
    }

    /// **The chunking must not be observable in the artefacts.** A chunk boundary is a place one
    /// buffer's rows end and the next one's begin, so a column emitted in one chunk and the same
    /// column emitted in chunks of three have to be the same three files. That is what makes
    /// [`KeywordDictPlan`] a memory knob rather than a format decision, and it is what an
    /// off-by-one at a boundary fails: a key whose rows split across two runs and lost half of
    /// them changes only the bytes. Nor must the merge's key ranges be.
    #[test]
    fn chunking_the_keyword_column_does_not_change_its_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let attribute = keyword_fixture_attribute();
        let values = keyword_fixture_column(&scratch);

        let mut emitted: Vec<[Vec<u8>; 3]> = Vec::new();
        // And the merge in key ranges, with every range but the head held until its turn.
        let ranged = [
            KeywordDictPlan::explicit(3, 5, None).in_ranges(ranges(4, 3, 3, 1)),
            KeywordDictPlan::explicit(7, 1 << 20, Some(ChunkMode::Hash))
                .in_ranges(ranges(3, 2, 6, 64)),
        ];
        for (k, plan) in keyword_plans().into_iter().chain(ranged).enumerate() {
            let column_dir = dir.path().join(format!("plan-{k}"));
            std::fs::create_dir_all(&column_dir).expect("column dir");
            let values_path = column_dir.join("values.arrow");
            let presence_path = column_dir.join(mosaica_filter::PRESENCE_FILE);
            let written = write_column_values(
                &column_dir,
                &attribute,
                &values,
                None,
                plan,
                Steps::default(),
            )
            .expect("keyword column");
            assert!(written.presence, "the fixture has absent entities");
            let dict_path = written.dict.expect("a keyword column owes a dictionary");
            // The three artefacts and nothing else: every run this plan spilled, every
            // intermediate its cascade wrote, and the ordinal scratch are all gone.
            let mut left: Vec<String> = std::fs::read_dir(&column_dir)
                .expect("read dir")
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .filter(|name| {
                    name != "values.arrow"
                        && name != mosaica_filter::PRESENCE_FILE
                        && name != mosaica_filter::DICT_FILE
                })
                .collect();
            left.sort();
            assert!(left.is_empty(), "plan {plan:?} left {left:?} behind");
            emitted.push([
                std::fs::read(&dict_path).expect("dict"),
                std::fs::read(&values_path).expect("values"),
                std::fs::read(&presence_path).expect("presence"),
            ]);
        }
        let first = emitted.first().expect("at least one plan").clone();
        for (k, files) in emitted.iter().enumerate().skip(1) {
            assert_eq!(files[0], first[0], "chunk plan {k}'s dictionary differs");
            assert_eq!(files[1], first[1], "chunk plan {k}'s values differ");
            assert_eq!(files[2], first[2], "chunk plan {k}'s presence differs");
        }
    }

    /// And the column says what the corpus says: the dictionary is the sorted distinct key set,
    /// and every entity's ordinal names its own key — read back through the readers that will
    /// serve them, under every chunking, so what is asserted is the merge's output and not one
    /// chunk's buffer.
    #[test]
    fn the_keyword_column_names_every_entity_s_own_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let attribute = keyword_fixture_attribute();
        let values = keyword_fixture_column(&scratch);
        let expected = keyword_fixture_keys();

        for (k, plan) in keyword_plans().into_iter().enumerate() {
            let label = format!("{plan:?}");
            let column_dir = dir.path().join(format!("plan-{k}"));
            std::fs::create_dir_all(&column_dir).expect("column dir");
            let values_path = column_dir.join("values.arrow");
            let presence_path = column_dir.join(mosaica_filter::PRESENCE_FILE);
            write_column_values(
                &column_dir,
                &attribute,
                &values,
                None,
                plan,
                Steps::default(),
            )
            .expect("keyword column");

            let dict = mosaica_filter::SortedDict::open(
                &column_dir.join(mosaica_filter::DICT_FILE),
                mosaica_filter::Access::Read,
            )
            .expect("the dictionary opens");
            assert_eq!(dict.len() as usize, expected.len(), "plan {label}");
            let mut scratch_key = Vec::new();
            for (ordinal, key) in expected.iter().enumerate() {
                assert_eq!(
                    dict.key_of(ordinal as u32, &mut scratch_key)
                        .expect("the key resolves"),
                    key,
                    "plan {label}: ordinal {ordinal}"
                );
            }

            let column = mosaica_filter::ValueColumn::open(
                &values_path,
                Some(&presence_path),
                mosaica_filter::Access::Read,
            )
            .expect("the value column opens");
            for entity in 0..N_KEYWORD {
                let ordinal = column.value_of(entity as u32).map(|value| value.raw());
                match keyword_fixture_key(entity) {
                    ScalarValue::Utf8(key) => {
                        let ordinal = ordinal.expect("a present entity has a value");
                        assert_eq!(
                            dict.key_of(ordinal, &mut scratch_key)
                                .expect("the key resolves"),
                            key,
                            "plan {label}: entity {entity}"
                        );
                    }
                    _ => assert_eq!(ordinal, None, "plan {label}: entity {entity}"),
                }
            }
        }
    }

    /// The corpus both text tests read. 4,096 entities, and every shape the emit has to survive:
    /// a term carried by every entity (well past the Roaring threshold and past a 64-entity
    /// presence word), terms carried by exactly one, a term repeated inside one document, mixed
    /// scripts, whole absent runs longer than a presence word, and the empty string — which is a
    /// legal value that yields no term, and is not absence.
    const N_TEXT: usize = 4_096;

    fn text_fixture_attribute() -> crate::config::Attribute {
        crate::config::Attribute {
            name: "abstract".to_string(),
            field: None,
            title: None,
            ty: ScalarType::Text,
            analyser: Some(
                mosaica_analyse::analyser(mosaica_analyse::UNICODE)
                    .expect("the unicode analyser ships")
                    .identity(),
            ),
            vocabulary: None,
            value_set: None,
            index: true,
            render: false,
            unique: false,
        }
    }

    /// The fixture's prose spilled as `extents` blob extents, assigned round-robin by entity so
    /// that the extents interleave in entity space exactly as a join's chunks do.
    fn text_fixture_extents(dir: &Path, extents: usize) -> crate::extents::ExtentColumn {
        let mut column = crate::extents::ExtentColumn::new(dir, 0, "abstract");
        for extent in 0..extents {
            let held: Vec<(u32, String)> = (0..N_TEXT)
                .filter(|entity| entity % extents == extent)
                .filter_map(|entity| match text_fixture_prose(entity) {
                    ScalarValue::Utf8(prose) => Some((entity as u32, prose)),
                    _ => None,
                })
                .collect();
            let rows: Vec<(u32, &str)> = held
                .iter()
                .map(|(entity, prose)| (*entity, prose.as_str()))
                .collect();
            column.push_extent(&rows).expect("an extent");
        }
        column
    }

    fn text_fixture_prose(entity: usize) -> ScalarValue {
        // A 64-entity absent run, word-aligned, and a second one that is not.
        if (256..320).contains(&entity) || (1_001..1_101).contains(&entity) {
            return ScalarValue::Null;
        }
        if entity.is_multiple_of(313) {
            return ScalarValue::Utf8(String::new());
        }
        let mut prose = format!("common item{entity}");
        match entity % 5 {
            0 => prose.push_str(" quick brown fox"),
            1 => prose.push_str(" quick quick silver"),
            2 => prose.push_str(" 日本語のテキスト quick"),
            3 => prose.push_str(" Ω STRASSE ﬁle"),
            _ => prose.push_str(" brown bear"),
        }
        ScalarValue::Utf8(prose)
    }

    /// One entity holding a term, and the positions it holds it at.
    type Held = (u32, Vec<u32>);

    /// The expected terms, each with its entities and its positions in each, derived from the
    /// fixture through the analyser itself — the same route `mosaica tokenise` gives the
    /// conformance oracle, so this asserts the index against the analyser rather than against a
    /// second tokeniser that could drift.
    fn expected_text_postings() -> Vec<(String, Vec<Held>)> {
        let analyser = mosaica_analyse::analyser(mosaica_analyse::UNICODE).expect("the analyser");
        let mut terms: std::collections::BTreeMap<String, Vec<Held>> = Default::default();
        for entity in 0..N_TEXT {
            let ScalarValue::Utf8(prose) = text_fixture_prose(entity) else {
                continue;
            };
            for (position, token) in analyser.tokens(&prose).into_iter().enumerate() {
                let items = terms.entry(token).or_default();
                if items.last().map(|(e, _)| *e) != Some(entity as u32) {
                    items.push((entity as u32, Vec::new()));
                }
                items.last_mut().expect("just pushed").1.push(position as u32);
            }
        }
        terms.into_iter().collect()
    }

    /// **The band count must not be observable in the artefact.** A band boundary is a place the
    /// scatter restarts and the ascending-key check spans, so a column emitted in one band and the
    /// same column emitted in a band per code have to be the same file — which is also what makes
    /// the band budget a memory knob rather than a format decision.
    #[test]
    fn banding_the_postings_emit_does_not_change_its_bytes() {
        let scratch_dir = tempfile::tempdir().expect("tempdir");
        let scratch = crate::column::ColumnScratch::new(scratch_dir.path());
        let dir = tempfile::tempdir().expect("tempdir");
        // Codes scattered across a 32-bit space, as `vocabulary` mints them, with one code held
        // heavily enough to cross the Roaring threshold and code 0 (absent) carried too.
        let values = EntityColumn::from_values(
            &scratch,
            ScalarType::U32,
            (0..5_000u32).map(|e| {
                ScalarValue::U32(match e % 7 {
                    0 => mosaica_store::vocabulary::ABSENT_CODE,
                    1 => 3_999_999_999,
                    2 => 17,
                    _ => 1_000 + (e % 53),
                })
            }),
            "colour",
        )
        .expect("typed column");

        let mut files: Vec<Vec<u8>> = Vec::new();
        for band_rows in [1usize, 2, 100, 1_000, usize::MAX] {
            let path = dir.path().join(format!("postings-{band_rows}"));
            write_category_postings(&path, "colour", &values, band_rows).expect("emit");
            files.push(std::fs::read(&path).expect("read"));
        }
        for f in &files[1..] {
            assert_eq!(f, &files[0]);
        }

        // And the postings say what the column says: each code's posting is exactly the entities
        // carrying it, which is what makes one a derivative of the other.
        let tier =
            mosaica_authz::DeltaTier::open(&dir.path().join("postings-1")).expect("open");
        let mut codes: Vec<u32> = values
            .iter()
            .map(|v| category_code(&v, "colour").expect("code"))
            .collect::<Vec<_>>();
        codes.sort_unstable();
        codes.dedup();
        for code in codes {
            let posting = tier.posting_at(code).expect("read");
            let expected: Vec<u32> = values
                .iter()
                .enumerate()
                .filter(|(_, v)| category_code(v, "colour").expect("code") == code)
                .map(|(e, _)| e as u32)
                .collect();
            if code == mosaica_store::vocabulary::ABSENT_CODE {
                assert!(posting.is_none(), "the absent code earns no posting");
                continue;
            }
            let got: Vec<u32> = posting.expect("carried").iter().collect();
            assert_eq!(got, expected, "code {code}");
        }
    }

    /// `join_chunk` hands every row on in number order, rows of one number in the order they
    /// came, reports a number past the build's items as `None`, and drains the chunk.
    #[test]
    fn join_chunk_orders_by_number_and_reports_numbers_past_the_items() {
        let mut chunk: Vec<(u64, u32)> = vec![(3, 10), (0, 11), (3, 12), (9, 13), (1, 14)];
        let mut seen: Vec<(Option<u32>, u64, u32)> = Vec::new();
        join_chunk(&mut chunk, 4, |ordinal, number, payload| {
            seen.push((ordinal, number, payload));
            Ok(())
        })
        .unwrap();
        assert!(chunk.is_empty(), "the chunk must be drained");
        assert_eq!(
            seen,
            vec![
                (Some(0), 0, 11),
                (Some(1), 1, 14),
                (Some(3), 3, 10),
                (Some(3), 3, 12),
                (None, 9, 13),
            ]
        );
    }

    /// The first `Err` from `on_row` aborts the join and propagates.
    #[test]
    fn join_chunk_propagates_the_callbacks_error() {
        let mut chunk: Vec<(u64, ())> = vec![(1, ()), (3, ())];
        let result = join_chunk(&mut chunk, 2, |ordinal, number, ()| match ordinal {
            Some(_) => Ok(()),
            None => Err(input_changed(&format!("item {number} missing"))),
        });
        assert!(result.is_err());
    }
}
