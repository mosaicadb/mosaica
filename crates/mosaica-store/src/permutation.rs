//! `Permutation`: the **only** legal EntityId→RowId path in the codebase (invariant I4).
//! Backed by `permutation.bin` (R4), the **two-level paged** entity→row map: a directory over
//! pages of 2¹⁶ consecutive entity ids, an absent page meaning every entity in it has no row in
//! this segment.
//!
//! ## The file
//!
//! ```text
//!   0   "MSPM"                        magic
//!   4   u16 version = 2
//!   6   u16 page_shift = 16           recorded, not assumed — any other value is refused
//!   8   u64 bound                     the entity ids this file covers, [0, bound)
//!  16   u32 page_count                = ceil(bound / 2^16)
//!  20   u32 present_count             how many of them carry slots
//!  24   u32 × page_count              the directory: each page's slot in the payload, or
//!                                     0xFFFF_FFFF for an absent page
//!       zero padding                  to the next 4096-byte boundary
//!  ...  u32 × 2^16 × present_count    the pages, in slot order, which is ascending page order
//! ```
//!
//! A slot is a row id, sentinel `0xFFFF_FFFF` for an entity with no row here — so a *present*
//! page may still hold holes, and an *absent* page is exactly one that holds nothing but holes.
//! **A dense view is the degenerate case with every page present** (`views.md` §8): the flat
//! array of earlier revisions plus an identity directory. A sparse one — a group's quarterly view
//! holding 8k of 21k entities — stores the pages it occupies and nothing else, which is the whole
//! reason the level exists: at 10⁹ entities a flat array is 4 GB per view, sentinel-dominated,
//! multiplied by the views of the group.
//!
//! Nothing is compressed and nothing is decoded (contracts §2.6 — mmap and slice). The payload
//! starts on a 4 KiB boundary and a page is 256 KiB, so every page is page-aligned in the mapping.
//!
//! **Canonical, so the bytes are a function of the mapping.** Slots ascend with page index and
//! number `0..present_count` exactly; the padding is zero; the tail of the last page above `bound`
//! is sentinel. A file departing from any of these is refused at load rather than read generously,
//! which is what lets the two producers — the build's planned writer and the fold's scatter
//! ([`crate::write::PermutationWriter`]) — be held to byte-for-byte agreement.
//!
//! **The flat array is gone, and the version number refuses it by construction.** Version 1 was
//! `bound` slots with no directory and no page count; this is version 3, so a file from the older
//! producer fails [`Permutation::load`] with an unsupported-version error rather than having its
//! first slots read as a directory. The artifacts are recreated rather than carried
//! ([decision 0048](../../../docs/decisions/0048-no-deployments-exist-so-delete-rather-than-support.md)),
//! and `bundle_format` does not move (owner direction) — the loud refusal a bump would have
//! supplied is delivered by the version field, which is the field that actually changed.
//!
//! ## Row space is that file plus an ordered extent list
//!
//! A build writes one segment per (partition, view) and one `permutation.bin` covering it. A
//! flush appends a segment beside it, and [`RowSpace`] is what makes the pair addressable as one
//! row space: the base file below the build bound, an ordered list of [`SegmentExtent`]s above it.
//!
//! **The dispatch lives here rather than in the engine, and that is I4 rather than tidiness.**
//! The claim this module makes about itself — that it is the only legal EntityId→RowId path — is
//! falsified by an engine that learns to select an extent and index a segment. Every caller still
//! sees `row_of` and `project`; which segment answered is this module's business, and so is
//! whether the answer came from a page or from an absent one.
//!
//! Two bounds hold by construction and are checked at the one place an extent enters
//! ([`RowSpace::with_extent`]): total rows per view stay under 2³², and the extent list is
//! bounded by the live segment count, which the merge policy bounds.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memmap2::Mmap;

use mosaica_roaring::{Container, Containers, Readable, Sink, BLOCK, WORDS};
use mosaica_types::{EntityId, RowId, ROW_ABSENT};
use rayon::prelude::*;

use crate::error::{Result, StoreError};
use crate::select;

/// A row-indexed "already claimed" set, one bit a row, for checking an extent's rows: a row whose
/// bit is already set is claimed by a second entity. [`Permutation::validate_rows`] claims the
/// base's rows a bucket at a time instead.
struct RowsSeen(Vec<u64>);

impl RowsSeen {
    fn new(rows: usize) -> RowsSeen {
        RowsSeen(vec![0u64; rows.div_ceil(64)])
    }

    /// Claim `row`, returning whether it was already claimed.
    fn claim(&mut self, row: usize) -> bool {
        let bit = 1u64 << (row & 63);
        let word = &mut self.0[row >> 6];
        let already = *word & bit != 0;
        *word |= bit;
        already
    }
}

const PERMUTATION_MAGIC: &[u8; 4] = b"MSPM";
const PERMUTATION_VERSION: u16 = 3;

/// A page covers `2^PAGE_SHIFT` consecutive entity ids.
///
/// 16 is the owner's ruling (2026-08-30) and is also the width that makes the two levels cost
/// what they should: a page is 256 KiB, so a directory entry costs 4 bytes per 256 KiB of covered
/// entity space (a 65 536-entry directory at the `u32` entity ceiling, 256 KiB in total), while a
/// view sparse at any coarser granularity than a page still pays only for the pages it lands in.
/// It is recorded in the header rather than assumed, so a file written at another width is a
/// refusal and not a misread.
pub const PAGE_SHIFT: u32 = 16;

/// Entity slots in one page.
pub const PAGE_ENTRIES: usize = 1 << PAGE_SHIFT;

/// Bytes in one page — `PAGE_ENTRIES` little-endian `u32` row ids.
pub const PAGE_BYTES: usize = PAGE_ENTRIES * 4;

/// The directory entry for a page that carries no slots: every entity in it is row-absent.
///
/// The same bit pattern as [`ROW_ABSENT`], and deliberately so — both mean *no row*, one page at a
/// time and one entity at a time.
pub const PAGE_ABSENT: u32 = 0xFFFF_FFFF;

/// magic, version, page_shift, bound, page_count, present_count.
pub(crate) const HEADER_LEN: usize = 4 + 2 + 2 + 8 + 4 + 4;

/// The payload's alignment. A page is 256 KiB, so aligning the first one aligns them all; 4 KiB is
/// the mapping granularity every platform this targets shares.
pub(crate) const PAGE_ALIGN: usize = 4096;

/// Where the pages begin, given the directory's length: the header and directory rounded up to
/// [`PAGE_ALIGN`]. The one arithmetic both the reader and the writer depend on, so it has one
/// definition site.
pub(crate) fn payload_start(page_count: usize) -> usize {
    (HEADER_LEN + page_count * 4).next_multiple_of(PAGE_ALIGN)
}

/// How many pages cover `[0, bound)`.
pub(crate) fn pages_for(bound: u64) -> u64 {
    bound.div_ceil(PAGE_ENTRIES as u64)
}

/// Rows per bucket in [`Permutation::project`], as a power of two — the one free parameter in that
/// pass, and the two constraints that fix it pull in opposite directions.
///
/// A bucket owns a contiguous range of row space. Too *wide* and the bit array it stamps falls out
/// of L2, making every row's stamp a last-level miss; too *narrow* and there are so many live write
/// cursors in the first pass that the append side misses instead. 2²² rows is a 512 KB bit array,
/// which is L2-resident on the machines this targets, and leaves 239 buckets at 10⁹ — whose cursors
/// and write tails together stay inside L1.
///
/// ⊘ **Measured against 2²⁴** on a Xeon with 2 MB of L2 a core, 10⁹ rows: the walk into the
/// buckets ran 10–30% faster with a quarter of the cursors, and the stamping 40–60% slower once a
/// bucket's bits filled L2, which left the pass slower overall on four threads.
///
/// It is also a whole number of Roaring containers (64 of them), which is not a coincidence to be
/// preserved by luck: the emit step hands each container's words straight to [`Sink`], so a bucket
/// boundary that fell inside a container would split a payload across two of them.
const BUCKET_SHIFT: u32 = 22;

/// `u64` words in one bucket's bit array — 2²² rows, 512 KB.
const STAMP_WORDS: usize = (1usize << BUCKET_SHIFT) / 64;

/// Roaring containers in one bucket. [`BUCKET_SHIFT`] is chosen so this is a whole number.
const CONTAINERS_PER_BUCKET: usize = (1usize << BUCKET_SHIFT) / BLOCK;

/// `u64` words in the mark array, one bit per word of the stamp.
const MARK_WORDS: usize = STAMP_WORDS / 64;

/// Mark words covering one container's 1,024 stamp words.
const MARKS_PER_CONTAINER: usize = WORDS / 64;

/// A page of entity space is one Roaring container of a mask, which is what lets the walk take a
/// mask a container at a time and find each container's page with one directory read.
const _: () = assert!(PAGE_ENTRIES == BLOCK);

/// Rows in a bucket below which the emit reads only the stamp words that hold something.
///
/// **The two emits produce the same containers and differ in what they cost.** Reading a
/// container's whole 1,024 words costs the same whether it holds 30,000 rows or three: a popcount
/// over 8 KB, a second pass to write the members out, and an 8 KB wipe. Following the marks instead
/// costs 16 words, plus one read and one clear of each word that holds a row, so it is cheaper
/// while a container's rows occupy under about a thousand of its words.
///
/// A bucket under this many rows averages fewer than one row per stamp word, so every container in
/// it is far inside that region. A session's mask is orders of magnitude above it — a quarter of
/// 10⁹ rows is a million rows per bucket — and takes the whole-container emit unchanged.
const SPARSE_BUCKET_ROWS: usize = STAMP_WORDS;

/// Row ids in one chunk of a worker's pool, at most. A bucket grows a chunk at a time.
///
/// **Why chunks and not a `Vec` per bucket.** A bucket's `Vec` kept the capacity of the largest
/// window share it ever took, and a principal whose entities are issued in clustered order hands
/// most of a window to a few dozen buckets and the next window to a different few dozen. Measured
/// at rung 6 (3.5×10⁹ rows, 834 buckets) over a 50% country principal: the buckets' capacity grew
/// from 80 MiB to 2,606 MiB across 105 windows while no window held more than 64 MiB, against a
/// result of 212 MiB. A shared pool bounds the buckets by one window whatever order the rows arrive in.
///
/// **Measured after, same principals at rung 6**:
/// anonymous peak 2,413 MiB to 351 MiB at the country principal and 4,501 MiB to 598 MiB at a 90%
/// species principal, each now the result as held plus 68 to 72 MiB, with the walk 9.5 s to 6.8 s
/// and 19.1 s to 14.1 s, medians of three. The walk is faster because no bucket is reallocated and
/// copied as it grows, and the pool's chunks are reused hot; that reason is inferred, not
/// separately measured.
///
/// 4,096 rows is 16 KB. The pool carries one partly filled chunk per bucket beyond the window, so
/// this sets that term (13 MiB at 834 buckets). Every row tests whether its bucket's chunk is full;
/// taking a chunk, the out-of-line call, runs once per chunk.
const CHUNK_ROWS_MAX: usize = 1 << 12;

/// Chunk size floor, for the small masks an artifact pass projects by the hundred thousand: the
/// pool is then a few kilobytes per bucket rather than [`CHUNK_ROWS_MAX`] rows.
const CHUNK_ROWS_MIN: usize = 1 << 6;

/// The buffers [`Permutation::project_with`] reuses between calls.
///
/// One worker's row buckets for each thread a walk runs on, and one stamp, mark array and member
/// run for each thread that settles buckets at once. They carry no information between calls — the
/// chains and cursors are reset where a round starts, the pool is written before it is read, and a
/// stamp and its marks are all-zero on both entry and exit — so a `Default` one and a reused one
/// give byte-identical results; what reuse saves is the allocation and the zeroing, which at one
/// projection per artifact is the dominant cost of the artifact pass rather than a rounding error.
#[derive(Default)]
pub struct ProjectScratch {
    workers: Vec<Worker>,
    /// Taken by a thread settling buckets and given back when it is done, so there are never more
    /// than the threads that settled at once.
    emitters: std::sync::Mutex<Vec<Emitter>>,
    serial: bool,
}

impl ProjectScratch {
    /// A scratch whose walks stay on the calling thread, for a caller that projects several masks
    /// at once on threads of its own, as the term-image derivation does.
    pub fn serial() -> Self {
        ProjectScratch {
            serial: true,
            ..ProjectScratch::default()
        }
    }

    fn take_emitter(&self) -> Emitter {
        self.emitters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop()
            .unwrap_or_else(Emitter::new)
    }

    fn give_emitter(&self, emitter: Emitter) {
        self.emitters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(emitter);
    }
}

/// The widest a [`ProjectScratch::serial`] grows while projecting masks of at most `rows` rows
/// through a permutation of `bound` entities, in bytes, split into the part that does not move with
/// the corpus, the chunk pool, and the bits a walk of more than one round keeps.
///
/// ⊘ **Modelled**, by the arithmetic [`Permutation::project_with`] sizes its pool with: the chunks
/// a round's rows fill plus a partial chunk a bucket, never above one window, the page that crossed
/// it and a full chunk for each bucket and one more. A mask under one window costs its own rows
/// instead, which is what a small corpus pays. A walk of more than one round keeps a container's
/// bits once its rows pass [`KEEP_ROWS`], 8 KiB each, so at most one container per `KEEP_ROWS` rows
/// of the mask and never more than four times what the result holds for those containers.
///
/// A walk spread over threads splits the same pool between them, and holds a stamp and its marks
/// for each thread that settles buckets at once.
///
/// Here so that a caller estimating a pass that projects, such as the build's residency model,
/// prices the scratch from the arithmetic that sizes it rather than from a figure copied into
/// another crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectScratchBound {
    /// The stamp, its mark array and the page of selected rows, the same size whatever the corpus.
    pub fixed: u64,
    /// The chunk pool the buckets fill.
    pub pool: u64,
    /// The container bits a walk of more than one round keeps until it ends.
    pub kept: u64,
}

impl ProjectScratchBound {
    /// All three.
    pub fn total(self) -> u64 {
        self.fixed
            .saturating_add(self.pool)
            .saturating_add(self.kept)
    }
}

/// [`ProjectScratchBound`] for one permutation and the widest mask projected through it.
pub fn project_scratch_bound(bound: u64, rows: u64) -> ProjectScratchBound {
    let window = PROJECT_WINDOW_ROWS as u64;
    let page = PAGE_ENTRIES as u64;
    let buckets = (bound >> BUCKET_SHIFT) + 1;
    let buffered_max = if rows <= window {
        rows
    } else {
        window.saturating_add(page)
    };
    let chunk_rows = (buffered_max / buckets)
        .next_power_of_two()
        .clamp(CHUNK_ROWS_MIN as u64, CHUNK_ROWS_MAX as u64);
    let chunks = buffered_max.div_ceil(chunk_rows).saturating_add(buckets);
    let ceiling = window
        .saturating_add(page)
        .saturating_add((buckets + 1).saturating_mul(CHUNK_ROWS_MAX as u64));
    let pool_rows = chunks.saturating_mul(chunk_rows).min(ceiling);
    let kept = if rows <= window {
        0
    } else {
        let containers = (rows / (u64::from(KEEP_ROWS) + 1)).min(bound.div_ceil(BLOCK as u64));
        containers.saturating_mul(8 * WORDS as u64)
    };
    ProjectScratchBound {
        fixed: 8 * (STAMP_WORDS + MARK_WORDS) as u64 + 4 * (page + select::SLACK as u64),
        pool: 4u64.saturating_mul(pool_rows),
        kept,
    }
}

/// A bucket's position in the pool: the index its next row is written at, and the end of its
/// current chunk. The two are equal when the bucket has no chunk with room, which is also how an
/// empty bucket starts. Kept side by side so a push reads one 16-byte entry.
#[derive(Clone, Copy, Default)]
struct Cursor {
    next: usize,
    end: usize,
}

/// One thread's row buckets: the chunks they fill, the cursors into them, and the page of rows a
/// bitset container selected before they are bucketed.
#[derive(Default)]
struct Worker {
    pool: Vec<u32>,
    /// Chunks not in any bucket's chain, taken from the end.
    free: Vec<u32>,
    /// Per bucket, its chunks in the order they were filled.
    chains: Vec<Vec<u32>>,
    /// Per bucket, where its next row is written.
    cursors: Vec<Cursor>,
    chunk_rows: usize,
    page: Vec<u32>,
}

impl Worker {
    /// Empty buckets for `nbuckets` buckets, with room for `rows` rows however they fall.
    fn prepare(&mut self, nbuckets: usize, rows: usize, chunk_rows: usize) {
        // Each bucket's full chunks hold its rows; its last chunk wastes under one chunk. So the
        // chunks in use never exceed the rows' own chunks plus one a bucket, and rows arriving in
        // any order fit.
        let chunks = rows.div_ceil(chunk_rows) + nbuckets;
        let pool_rows = chunks
            .checked_mul(chunk_rows)
            .expect("a round's chunks fit in the address space");
        if self.pool.len() < pool_rows {
            // **Grown geometrically, and capped at a round's pool.** An artifact pass projects
            // hundreds of thousands of masks through one scratch, and a pool sized exactly to each
            // new largest mask is replaced, and its pages faulted again, every time the largest
            // grows. Doubling replaces it a logarithmic number of times. The cap is the largest
            // pool any `chunk_rows` needs for these rows, so doubling never overshoots what a
            // round uses.
            //
            // A fresh zeroed allocation rather than `resize`, so the pages are the kernel's zero
            // pages until a row is written to them: a large pool is not paid for up front.
            let ceiling = rows.saturating_add((nbuckets + 1).saturating_mul(CHUNK_ROWS_MAX));
            let grown = self
                .pool
                .len()
                .saturating_mul(2)
                .min(ceiling)
                .max(pool_rows);
            self.pool = vec![0; grown];
        }
        self.free.clear();
        // Taken from the end, so the first chunks handed out are the pool's first.
        self.free
            .extend((0..u32::try_from(chunks).expect("chunk ids fit in u32")).rev());
        for chain in self.chains.iter_mut() {
            chain.clear();
        }
        self.chains.resize_with(nbuckets, Vec::new);
        self.chains.truncate(nbuckets);
        self.cursors.clear();
        self.cursors.resize(nbuckets, Cursor::default());
        self.chunk_rows = chunk_rows;
    }

    /// Give `bucket` a fresh chunk and write `row` as its first. Out of line: it runs once per
    /// `chunk_rows` rows a bucket takes.
    #[cold]
    #[inline(never)]
    fn push_into_new_chunk(&mut self, bucket: usize, row: u32) {
        // A cursor with room whose slot is outside the pool would abandon a partly filled chunk
        // that `len` still counts as full. Chunk ids are below the pool's chunk count, so this
        // cannot happen; the assertion says so where the tests will see it.
        let cursor = self.cursors[bucket];
        debug_assert_eq!(cursor.next, cursor.end, "a chunk with room was abandoned");
        let chunk = self.free.pop().expect(
            "the pool holds a round's rows and a partial chunk per bucket, which is all a walk \
             buffers",
        );
        self.chains[bucket].push(chunk);
        let at = chunk as usize * self.chunk_rows;
        self.pool[at] = row;
        self.cursors[bucket] = Cursor {
            next: at + 1,
            end: at + self.chunk_rows,
        };
    }

    /// The store into the pool is checked with `get_mut` and falls to the chunk-taking path when it
    /// is out of range, so the hot path carries no panic branch for the pool index. Measured on the
    /// 64-part smoke bundle's 185,418 species images through one scratch, an indexed store cost
    /// 9–12% more CPU than a `Vec` per bucket, and this form is within 2% of one.
    #[inline(always)]
    fn push(&mut self, bucket: usize, row: u32) {
        let cursor = &mut self.cursors[bucket];
        if cursor.next != cursor.end {
            if let Some(slot) = self.pool.get_mut(cursor.next) {
                *slot = row;
                cursor.next += 1;
                return;
            }
        }
        self.push_into_new_chunk(bucket, row);
    }

    #[inline(always)]
    fn push_row(&mut self, row: u32) {
        self.push((row >> BUCKET_SHIFT) as usize, row);
    }

    /// Rows buffered in `bucket`.
    fn len(&self, bucket: usize) -> usize {
        match self.chains[bucket].len() {
            0 => 0,
            chunks => {
                let cursor = self.cursors[bucket];
                chunks * self.chunk_rows - (cursor.end - cursor.next)
            }
        }
    }

    /// `bucket`'s rows, a chunk at a time, in the order they were pushed.
    fn slices(&self, bucket: usize) -> impl Iterator<Item = &[u32]> + '_ {
        let chain = &self.chains[bucket];
        let last = chain.len().saturating_sub(1);
        let chunk_rows = self.chunk_rows;
        let cursor = self.cursors[bucket];
        let tail = chunk_rows - (cursor.end - cursor.next);
        chain.iter().enumerate().map(move |(i, chunk)| {
            let at = *chunk as usize * chunk_rows;
            let rows = if i == last { tail } else { chunk_rows };
            &self.pool[at..at + rows]
        })
    }

    /// Bucket the rows of `pages`, each of which a container of the mask covers.
    fn walk(&mut self, containers: &Containers<'_>, pages: &[MaskPage<'_>]) {
        for page in pages {
            match containers.get(page.container) {
                Container::Bitset(words) => {
                    let mut selected = std::mem::take(&mut self.page);
                    select::select(page.slots, words, &mut selected);
                    for &row in &selected {
                        self.push_row(row);
                    }
                    self.page = selected;
                }
                Container::Array(lows) => {
                    for &low in lows {
                        let row = page.slots[usize::from(low)];
                        if row != ROW_ABSENT {
                            self.push_row(row);
                        }
                    }
                }
                Container::Run(runs) => {
                    for run in runs {
                        let start = usize::from(run.start);
                        for &row in &page.slots[start..=start + usize::from(run.length)] {
                            if row != ROW_ABSENT {
                                self.push_row(row);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One page of entity space the mask holds entities in and this permutation holds slots for.
struct MaskPage<'a> {
    /// The mask's container over the page, by its index among the mask's containers.
    container: usize,
    slots: &'a [u32; PAGE_ENTRIES],
    /// The entities the mask holds in the page, which bound the rows it reaches there.
    entities: usize,
}

/// One thread's stamp, the mark array over it and a member run: what settling a bucket takes.
/// The stamp and the marks are all-zero between uses, because every emit clears what it set.
struct Emitter {
    stamp: Vec<u64>,
    marks: Vec<u64>,
    members: Vec<u32>,
}

impl Emitter {
    fn new() -> Emitter {
        Emitter {
            stamp: vec![0; STAMP_WORDS],
            marks: vec![0; MARK_WORDS],
            members: Vec::new(),
        }
    }
}

/// One bucket's containers across the rounds of a walk that takes more than one: the rows each has
/// taken so far, and, for each that has taken more than [`KEEP_ROWS`], its bits.
///
/// **Why the bits are kept.** A round's rows are a fraction of each container's, so emitting every
/// round and unioning the parts reads and merges each container once per round: 15 rounds at a
/// quarter of 10⁹ rows. ⊘ **Measured** on a 4-core Xeon, 10⁹ rows, one thread: 4.7–5.9 s emitting
/// every round against 2.2 s for one round over the whole grant. A kept container is stamped by
/// each round's rows and emitted once, at the end.
struct KeptBucket {
    taken: [u32; CONTAINERS_PER_BUCKET],
    words: [Option<Box<[u64; WORDS]>>; CONTAINERS_PER_BUCKET],
}

impl KeptBucket {
    fn new() -> KeptBucket {
        KeptBucket {
            taken: [0; CONTAINERS_PER_BUCKET],
            words: std::array::from_fn(|_| None),
        }
    }
}

/// Rows a container takes before a walk of more than one round keeps its bits ([`KeptBucket`]).
///
/// A container's rows only grow, so a kept container holds more than this many in the result,
/// where Roaring stores it in at least 2 bytes a row: its 8 KiB of bits are never more than four
/// times what the result holds for it. ⊘ **Measured** on a 4-core Xeon, 10⁹ rows, a 25% grant:
/// keeping only past [`mosaica_roaring::ARRAY_MAX`], where the bits are no larger than the result's
/// bitset, took 3.7 s on one thread and 1.3–1.4 s on four against 3.0 s and 0.96 s here, because
/// its containers cross that count only in the fourth of fifteen rounds and are emitted and merged
/// every round until then.
const KEEP_ROWS: u32 = mosaica_roaring::ARRAY_MAX / 4;

/// Settle every bucket of `workers` for one round: emit its rows' containers, or stamp the rows of
/// a kept container into its bits, and leave the stamps zero for the next round. On the threads of
/// the current pool where `parallel`, a group of buckets a task, each with its own [`Sink`].
fn settle(
    workers: &[Worker],
    nbuckets: usize,
    kept: Option<&mut [KeptBucket]>,
    scratch: &ProjectScratch,
    parallel: bool,
) -> croaring::Bitmap {
    let settle_group = |first: usize, kept: Option<&mut [KeptBucket]>, len: usize| {
        let mut emitter = scratch.take_emitter();
        let mut sink = Sink::new();
        match kept {
            Some(kept) => {
                for (offset, kept) in kept.iter_mut().enumerate() {
                    settle_bucket(&mut sink, &mut emitter, workers, first + offset, Some(kept));
                }
            }
            None => {
                for bucket in first..first + len {
                    settle_bucket(&mut sink, &mut emitter, workers, bucket, None);
                }
            }
        }
        scratch.give_emitter(emitter);
        sink.finish()
    };
    if !parallel {
        return settle_group(0, kept, nbuckets);
    }
    let group = nbuckets.div_ceil(rayon::current_num_threads() * 4).max(1);
    let parts: Vec<croaring::Bitmap> = match kept {
        Some(kept) => kept
            .par_chunks_mut(group)
            .enumerate()
            .map(|(index, kept)| settle_group(index * group, Some(kept), 0))
            .collect(),
        None => (0..nbuckets.div_ceil(group))
            .into_par_iter()
            .map(|index| {
                let first = index * group;
                settle_group(first, None, group.min(nbuckets - first))
            })
            .collect(),
    };
    union_of_disjoint(parts)
}

/// The union of bitmaps whose containers' keys do not overlap, as [`settle`]'s groups' are.
fn union_of_disjoint(parts: Vec<croaring::Bitmap>) -> croaring::Bitmap {
    let mut parts = parts.into_iter().filter(|part| !part.is_empty());
    let Some(mut out) = parts.next() else {
        return croaring::Bitmap::new();
    };
    for part in parts {
        out.or_inplace(&part);
    }
    out
}

/// Settle one bucket's rows from every worker: see [`settle`].
fn settle_bucket(
    sink: &mut Sink,
    emitter: &mut Emitter,
    workers: &[Worker],
    bucket: usize,
    kept: Option<&mut KeptBucket>,
) {
    let len: usize = workers.iter().map(|worker| worker.len(bucket)).sum();
    if len == 0 {
        return;
    }
    let base = (bucket as u32) << BUCKET_SHIFT;
    let Some(kept) = kept else {
        emit_stamped(sink, emitter, workers, bucket, base, len, None);
        return;
    };
    let mut counts = [0u32; CONTAINERS_PER_BUCKET];
    for worker in workers {
        for rows in worker.slices(bucket) {
            for &row in rows {
                counts[((row - base) >> 16) as usize] += 1;
            }
        }
    }
    let mut rest = 0usize;
    for (container, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        // A bijection gives each row to one entity, so the rows a container has taken are its
        // cardinality in the result so far.
        kept.taken[container] += count;
        if kept.words[container].is_none() && kept.taken[container] > KEEP_ROWS {
            kept.words[container] = Some(
                vec![0u64; WORDS]
                    .into_boxed_slice()
                    .try_into()
                    .expect("a container's words"),
            );
        }
        if kept.words[container].is_none() {
            rest += count as usize;
        }
    }
    if rest < len {
        for worker in workers {
            for rows in worker.slices(bucket) {
                for &row in rows {
                    let offset = row - base;
                    if let Some(words) = &mut kept.words[(offset >> 16) as usize] {
                        let low = offset & 0xFFFF;
                        words[(low >> 6) as usize] |= 1u64 << (low & 63);
                    }
                }
            }
        }
    }
    if rest > 0 {
        emit_stamped(sink, emitter, workers, bucket, base, rest, Some(kept));
    }
}

/// Stamp one bucket's rows from every worker, less those of `kept`'s kept containers, and emit the
/// containers they landed in. `len` is how many rows that leaves.
fn emit_stamped(
    sink: &mut Sink,
    emitter: &mut Emitter,
    workers: &[Worker],
    bucket: usize,
    base: u32,
    len: usize,
    kept: Option<&KeptBucket>,
) {
    let skipped =
        |offset: u32| kept.is_some_and(|kept| kept.words[(offset >> 16) as usize].is_some());
    let Emitter {
        stamp,
        marks,
        members,
    } = emitter;
    if len < SPARSE_BUCKET_ROWS {
        // Following the marks: see [`SPARSE_BUCKET_ROWS`].
        let mut occupied: u64 = 0;
        for worker in workers {
            for rows in worker.slices(bucket) {
                for &row in rows {
                    let offset = row - base;
                    if skipped(offset) {
                        continue;
                    }
                    let word = (offset >> 6) as usize;
                    stamp[word] |= 1u64 << (offset & 63);
                    marks[word >> 6] |= 1u64 << (word & 63);
                    occupied |= 1u64 << (offset >> 16);
                }
            }
        }
        emit_marked(sink, stamp, marks, members, occupied, base);
        return;
    }
    // Which of the bucket's 64 containers hold anything. One `u64` covers them exactly, which is
    // what lets the emit below skip the empty ones without scanning their words.
    let mut occupied: u64 = 0;
    for worker in workers {
        for rows in worker.slices(bucket) {
            for &row in rows {
                let offset = row - base;
                if skipped(offset) {
                    continue;
                }
                stamp[(offset >> 6) as usize] |= 1u64 << (offset & 63);
                occupied |= 1u64 << (offset >> 16);
            }
        }
    }
    // Emitted and cleared in the same pass, container by container: the container's words are in
    // cache because the popcount just read them, and only the occupied ones are touched at all, so
    // the bucket pays a sequential 8 KB wipe per container it filled.
    for (container, words) in stamp.as_chunks_mut::<WORDS>().0.iter_mut().enumerate() {
        if occupied & (1u64 << container) == 0 {
            continue;
        }
        let cardinality: u32 = words.iter().map(|word| word.count_ones()).sum();
        let key = u16::try_from((base >> 16) + container as u32)
            .expect("a row below 2^32 has a container key below 2^16");
        sink.push_block(key, cardinality, words);
        words.fill(0);
    }
}

/// Emit the `occupied` containers of a stamp whose set words the marks name, reading only those
/// words — the emit for a bucket under [`SPARSE_BUCKET_ROWS`] rows.
///
/// The mark array carries one bit per stamp word, so a container's members are read in one pass
/// over the words that hold them rather than three over all 1,024. Both arrays are cleared as they
/// are read, which is what leaves them zero for the next call.
///
/// The containers this stages are the ones the whole-container emit would have staged, with the
/// same keys, the same cardinalities and the same ascending members: [`Sink::push_members`] writes
/// the array payload [`Sink::push_block`] writes below the array threshold, and stamps the words
/// itself above it.
fn emit_marked(
    sink: &mut Sink,
    stamp: &mut [u64],
    marks: &mut [u64],
    members: &mut Vec<u32>,
    occupied: u64,
    base: u32,
) {
    for container in 0..CONTAINERS_PER_BUCKET {
        if occupied & (1u64 << container) == 0 {
            continue;
        }
        members.clear();
        let words_at = container * WORDS;
        let marks_at = container * MARKS_PER_CONTAINER;
        for slot in 0..MARKS_PER_CONTAINER {
            let mut marked = std::mem::replace(&mut marks[marks_at + slot], 0);
            while marked != 0 {
                let word = slot * 64 + marked.trailing_zeros() as usize;
                marked &= marked - 1;
                let mut bits = std::mem::replace(&mut stamp[words_at + word], 0);
                let low = (word as u32) * 64;
                while bits != 0 {
                    members.push(low + bits.trailing_zeros());
                    bits &= bits - 1;
                }
            }
        }
        let key = u16::try_from((base >> 16) + container as u32)
            .expect("a row below 2^32 has a container key below 2^16");
        sink.push_members(key, members);
    }
}

/// The kept containers' bits as one bitmap, each emitted once, and the buckets left without them.
fn emit_kept(kept: &mut [KeptBucket], parallel: bool) -> croaring::Bitmap {
    let emit = |first: usize, buckets: &mut [KeptBucket]| {
        let mut sink = Sink::new();
        for (offset, bucket) in buckets.iter_mut().enumerate() {
            let base_key = ((first + offset) * CONTAINERS_PER_BUCKET) as u32;
            for (container, words) in bucket.words.iter_mut().enumerate() {
                let Some(words) = words.take() else {
                    continue;
                };
                let cardinality: u32 = words.iter().map(|word| word.count_ones()).sum();
                let key = u16::try_from(base_key + container as u32)
                    .expect("a row below 2^32 has a container key below 2^16");
                sink.push_block(key, cardinality, &words);
            }
        }
        sink.finish()
    };
    if !parallel {
        return emit(0, kept);
    }
    let group = kept.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    let parts: Vec<croaring::Bitmap> = kept
        .par_chunks_mut(group)
        .enumerate()
        .map(|(index, buckets)| emit(index * group, buckets))
        .collect();
    union_of_disjoint(parts)
}

/// Rows a round of [`Permutation::project_with`]'s walk buckets before they are settled — the bound
/// on that pass's transient memory, beside the result.
///
/// Bucketing the whole result before settling any of it costs four bytes a projected row: 14 GB at
/// 3.5×10⁹ rows over a whole-corpus grant, a transient no configured budget bounds. A round at a
/// time makes the bucket transient a constant — 64 MB of row ids, plus a page's that overshot it
/// and one partly filled chunk per bucket ([`CHUNK_ROWS_MAX`]) — and leaves the result itself, and
/// the bits a walk of more than one round keeps for its denser containers ([`KEEP_ROWS`]), as the
/// only terms that grow with the grant. The constant holds only because the buckets share one pool
/// a worker: see [`CHUNK_ROWS_MAX`] for what a `Vec` per bucket kept instead. A walk spread over
/// threads splits a round between them, so the pools together hold the same.
const PROJECT_WINDOW_ROWS: usize = (64 << 20) / 4;

/// Entities below which a walk stays on the calling thread: splitting the mask, fanning the second
/// pass out and joining the parts cost more than they save on a mask this small, and an artifact
/// pass projecting hundreds of thousands of small masks runs each of them serially.
const PARALLEL_MIN_ENTITIES: usize = 1 << 20;

/// Add to `out` the entities of `page` whose slot holds a row. A page's tail past the bound holds
/// only [`ROW_ABSENT`], which `validate_directory` checks, so no bound test is needed here.
fn add_page_entities(
    out: &mut croaring::Bitmap,
    page: usize,
    slots: &[u32],
    scratch: &mut Vec<u32>,
) {
    let first = (page << PAGE_SHIFT) as u32;
    scratch.clear();
    scratch.extend(
        slots
            .iter()
            .enumerate()
            .filter(|(_, &slot)| slot != ROW_ABSENT)
            .map(|(offset, _)| first + offset as u32),
    );
    if scratch.len() == PAGE_ENTRIES {
        out.add_range(first..=first + (PAGE_ENTRIES as u32 - 1));
    } else {
        out.add_many(scratch);
    }
}

/// A memory-mapped `permutation.bin`. `row_of` and `project` are the only ways to cross from
/// entity space to row space anywhere in the codebase (I4) — no other module may open this
/// file or otherwise derive a row ID from an entity ID.
#[derive(Debug)]
pub struct Permutation {
    mmap: Mmap,
    bound: u64,
    bound_usize: usize,
    page_count: usize,
    present_count: usize,
    payload_start: usize,
    path: PathBuf,
    /// The row count this file's slots claim exactly, where that is established.
    ///
    /// **What it is for:** a mask holding every entity in `[0, bound)` projects to every row this
    /// mapping has, and this value is what says those rows are the contiguous range
    /// `[0, dense_rows)` rather than some subset of it. With it, [`Self::project_with`] answers a
    /// whole-domain mask as that range and reads no page, and [`Self::project_complement_with`]
    /// answers any mask by walking the entities outside it; without it, every mask walks. Two
    /// routes set it, and both are the same claim from a different source:
    /// [`Self::validate_rows`] counts the slots and finds they number `row_count`, and
    /// [`Self::declare_dense_rows`] takes the writer's word for the same count where the caller's
    /// premise is that this process wrote the file.
    /// Unset means neither has spoken, and the walk answers everything.
    dense_rows: std::sync::OnceLock<u32>,
    /// Every entity holding a row here, filled by [`Self::validate_rows`] where it runs and on
    /// first use elsewhere.
    present_entities: std::sync::OnceLock<croaring::Bitmap>,
}

impl Permutation {
    /// The permutation of a view that has no row space at all — **a view created while the
    /// service runs, before its first flush** (`views.md` §3.2).
    ///
    /// `bound = 0`, no pages, no present slots: [`Self::row_of`] answers `None` for every entity
    /// and [`Self::project`] answers the empty bitmap, which is what an empty view must answer.
    /// It is an anonymous mapping carrying the same header a `bound = 0` file would, rather than
    /// a second representation with its own arithmetic — every accessor below reads it exactly as
    /// it reads a mapped file, so there is one decode path and not two.
    ///
    /// **No file is written for it, and that is the point.** A created view owns nothing on disc
    /// until the flush that gives it rows; writing an empty `permutation.bin` into the live prefix
    /// at creation would put a file into a bundle the create does not otherwise publish, and a
    /// prefix rotation would then have to carry it.
    pub fn empty() -> Result<Self> {
        let len = payload_start(0);
        let mut map = memmap2::MmapOptions::new()
            .len(len)
            .map_anon()
            .map_err(|source| StoreError::Io {
                path: PathBuf::from("<empty permutation>"),
                source,
            })?;
        map[0..4].copy_from_slice(PERMUTATION_MAGIC);
        map[4..6].copy_from_slice(&PERMUTATION_VERSION.to_le_bytes());
        map[6..8].copy_from_slice(&(PAGE_SHIFT as u16).to_le_bytes());
        // bound, page_count and present_count are all zero, which the zeroed mapping already
        // holds; they are not written back so that the header's shape is stated once, above.
        let mmap = map.make_read_only().map_err(|source| StoreError::Io {
            path: PathBuf::from("<empty permutation>"),
            source,
        })?;
        Ok(Permutation {
            mmap,
            bound: 0,
            bound_usize: 0,
            page_count: 0,
            present_count: 0,
            payload_start: len,
            path: PathBuf::from("<empty permutation>"),
            dense_rows: std::sync::OnceLock::new(),
            present_entities: std::sync::OnceLock::new(),
        })
    }

    /// Open and validate `path`: magic, version, page width, that the directory is canonical, and
    /// that the file is exactly as long as its own header says (a truncated or padded file is a
    /// corrupt bundle, not a partial one to silently accept).
    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        // SAFETY: the mapped file is treated as read-only for the lifetime of this struct;
        // nothing in this process writes to it concurrently. A backing file that another
        // process truncates while mapped is an operational hazard shared with every other
        // mmap-based reader in this codebase (`mosaica-authz`'s postings reader), not one
        // introduced here.
        let mmap = unsafe { Mmap::map(&file) }.map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        let invalid = |detail: String| StoreError::InvalidPermutation {
            path: path.to_path_buf(),
            detail,
        };

        if mmap.len() < HEADER_LEN {
            return Err(invalid(format!(
                "file is {} bytes, shorter than the {HEADER_LEN}-byte header",
                mmap.len()
            )));
        }
        if &mmap[0..4] != PERMUTATION_MAGIC {
            return Err(invalid("bad magic (expected 'MSPM')".to_string()));
        }
        let version = u16::from_le_bytes(mmap[4..6].try_into().expect("2-byte view"));
        if version != PERMUTATION_VERSION {
            // Version 1 is the flat array this representation replaced; it lands here, which is
            // the loud refusal that stands in for a `bundle_format` bump.
            return Err(invalid(format!(
                "unsupported version {version} (expected {PERMUTATION_VERSION})"
            )));
        }
        let page_shift = u16::from_le_bytes(mmap[6..8].try_into().expect("2-byte view"));
        if u32::from(page_shift) != PAGE_SHIFT {
            return Err(invalid(format!(
                "page shift {page_shift} (expected {PAGE_SHIFT})"
            )));
        }
        let bound = u64::from_le_bytes(mmap[8..16].try_into().expect("8-byte view"));
        // Checked, not `as usize`: on a 32-bit target (or an adversarial 64-bit `bound` value)
        // a truncating cast would silently shrink `bound` instead of failing closed.
        let bound_usize = usize::try_from(bound).map_err(|_| {
            invalid(format!(
                "bound {bound} does not fit in usize on this platform"
            ))
        })?;
        let page_count = u32::from_le_bytes(mmap[16..20].try_into().expect("4-byte view")) as usize;
        let present_count =
            u32::from_le_bytes(mmap[20..24].try_into().expect("4-byte view")) as usize;
        if page_count as u64 != pages_for(bound) {
            return Err(invalid(format!(
                "page count {page_count} does not cover bound {bound} (expected {})",
                pages_for(bound)
            )));
        }
        if present_count > page_count {
            return Err(invalid(format!(
                "{present_count} pages present of {page_count} covered"
            )));
        }

        let payload_start = payload_start(page_count);
        let expected_len = present_count
            .checked_mul(PAGE_BYTES)
            .and_then(|payload| payload.checked_add(payload_start))
            .ok_or_else(|| {
                invalid(format!(
                    "{present_count} pages of {PAGE_BYTES} bytes overflow the expected file length"
                ))
            })?;
        if mmap.len() != expected_len {
            return Err(invalid(format!(
                "file is {} bytes, expected {expected_len} for {present_count} present pages of \
                 {page_count}",
                mmap.len()
            )));
        }

        let permutation = Permutation {
            mmap,
            bound,
            bound_usize,
            page_count,
            present_count,
            payload_start,
            path: path.to_path_buf(),
            dense_rows: std::sync::OnceLock::new(),
            present_entities: std::sync::OnceLock::new(),
        };
        permutation.validate_directory()?;
        Ok(permutation)
    }

    /// The directory is canonical: slots number `0..present_count` in ascending page order, the
    /// padding between it and the payload is zero, and the last page holds nothing above `bound`.
    ///
    /// **Checked because the encoding claims to be a function of the mapping.** A file whose slots
    /// were permuted would serve every page under some other page's rows — every lookup wrong, no
    /// lookup out of range — and two producers of the same mapping could disagree byte for byte
    /// while both being "valid". `O(page_count)`, which is 65 536 iterations at the entity ceiling.
    fn validate_directory(&self) -> Result<()> {
        let invalid = |detail: String| StoreError::InvalidPermutation {
            path: self.path.clone(),
            detail,
        };
        let mut next_slot: u32 = 0;
        for (page, &slot) in self.directory().iter().enumerate() {
            if slot == PAGE_ABSENT {
                continue;
            }
            if slot != next_slot {
                return Err(invalid(format!(
                    "page {page} holds slot {slot} where the canonical order gives {next_slot}"
                )));
            }
            next_slot += 1;
        }
        if next_slot as usize != self.present_count {
            return Err(invalid(format!(
                "the directory names {next_slot} pages, the header {}",
                self.present_count
            )));
        }
        let padding = &self.mmap[HEADER_LEN + self.page_count * 4..self.payload_start];
        if padding.iter().any(|&b| b != 0) {
            return Err(invalid(
                "the padding before the payload is not zero".to_string(),
            ));
        }
        // The last page runs past `bound` whenever the bound is not a whole number of pages. Those
        // slots name entities that cannot exist, so they must be sentinel: a row id there would be
        // reachable through nothing and would break the bijection `validate_rows` checks.
        let tail = (self.page_count * PAGE_ENTRIES) - self.bound_usize;
        if tail > 0 {
            if let Some(page) = self.page_of(self.page_count - 1) {
                if page[PAGE_ENTRIES - tail..].iter().any(|&r| r != ROW_ABSENT) {
                    return Err(invalid(format!(
                        "the last page holds a row above bound {}",
                        self.bound
                    )));
                }
            }
        }
        Ok(())
    }

    /// The number of entity-ID slots this permutation covers, `[0, bound)`.
    pub fn bound(&self) -> u64 {
        self.bound
    }

    /// How many pages carry slots — the sparsity of the view, and what the file's size is
    /// proportional to. For diagnostics and for the tests that assert a sparse view does not pay
    /// for entity space it does not occupy.
    pub fn present_pages(&self) -> usize {
        self.present_count
    }

    /// How many pages `bound` spans, present or not.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    fn directory(&self) -> &[u32] {
        let bytes = &self.mmap[HEADER_LEN..HEADER_LEN + self.page_count * 4];
        // SAFETY: `bytes` starts at a fixed offset (HEADER_LEN = 24) into a page-aligned mmap
        // base, and 24 is a multiple of 4, so the cast is aligned regardless of file content — no
        // adversarial input can misalign it. The length is `page_count * 4` bytes, and `load`
        // checked that the file holds at least the header, the directory and the payload.
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, self.page_count) }
    }

    /// The `2^16` slots of `page`, or `None` where the page is absent.
    #[inline]
    fn page_of(&self, page: usize) -> Option<&[u32]> {
        let slot = *self.directory().get(page)?;
        if slot == PAGE_ABSENT {
            return None;
        }
        let at = self.payload_start + slot as usize * PAGE_BYTES;
        let bytes = &self.mmap[at..at + PAGE_BYTES];
        // SAFETY: `payload_start` is a multiple of 4096 and `PAGE_BYTES` a multiple of 4, so the
        // cast is aligned; `validate_directory` bounded `slot` by `present_count` and `load`
        // checked the file holds exactly that many pages.
        Some(unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, PAGE_ENTRIES) })
    }

    /// [`Self::page_of`] as a whole page, so a slot indexed by an entity's low 16 bits needs no
    /// bounds check.
    #[inline]
    fn page_slots(&self, page: usize) -> Option<&[u32; PAGE_ENTRIES]> {
        self.page_of(page)
            .map(|slots| slots.try_into().expect("a page holds PAGE_ENTRIES slots"))
    }

    /// Validate that every non-sentinel slot addresses a row within `row_count`, and that no
    /// two entities claim the same row (a permutation is a bijection onto `[0, row_count)`,
    /// not merely a function into it). Called once per view at bundle open, against the row
    /// count of the single build segment this permutation addresses (R4) — **not** on any
    /// per-viewport path (this is an `O(present pages)` scan, same cost class as [`Self::project`]).
    /// A corrupt or hand-edited `permutation.bin` that points rows out of range, or that
    /// aliases two entities onto one row, must fail bundle open rather than let `row_of` or
    /// `project` later hand out a `RowId` that indexes `columns.arrow` out of bounds (I4/I11).
    ///
    /// **It also records whether the mapping is onto `[0, row_count)` and not merely into it.**
    /// The checks below establish injectivity; counting the claims is what turns that into
    /// surjectivity, since `row_count` distinct claims below `row_count` are all of them. Only the
    /// surjective case is recorded, and only a recorded one enables [`Self::project_with`]'s
    /// whole-domain answer and [`Self::project_complement_with`] at all. A file claiming fewer
    /// rows than the descriptor declares takes the general path, where the image is read rather
    /// than assumed.
    pub fn validate_rows(&self, row_count: u32) -> Result<()> {
        let invalid = |detail: String| StoreError::InvalidPermutation {
            path: self.path.clone(),
            detail,
        };
        // **Claimed a bucket at a time, not a row at a time.** A claim per slot into one bit array
        // over the whole row space is a random read and write of 125 MB at 10⁹, a miss for nearly
        // every row. Here the slots of a round of pages are first bucketed by row range on as many
        // threads as the pool has, as the projection's walk buckets them, and each bucket is then
        // claimed by one thread into its own 512 KB of the array, which stays in L2 while it does.
        // No claim is shared, so none is atomic. ⊘ **Measured** on a 4-core Xeon at 10⁹ rows:
        // 19.0 s for a claim per slot on one thread, against 11.0 s bucketed on one and 2.8–3.1 s
        // bucketed on four.
        let nbuckets = (row_count as usize >> BUCKET_SHIFT) + 1;
        let threads = rayon::current_num_threads().max(1);
        let pages: Vec<usize> = (0..self.page_count)
            .filter(|&page| self.page_of(page).is_some())
            .collect();
        let pages_per_round = (PROJECT_WINDOW_ROWS / PAGE_ENTRIES).max(1);
        let share = pages_per_round.div_ceil(threads);
        let rows_per_worker = share * PAGE_ENTRIES;
        let chunk_rows = (rows_per_worker / nbuckets)
            .next_power_of_two()
            .clamp(CHUNK_ROWS_MIN, CHUNK_ROWS_MAX);
        let mut workers: Vec<Worker> = (0..threads).map(|_| Worker::default()).collect();
        let mut seen = vec![0u64; nbuckets * STAMP_WORDS];
        let mut claimed: u64 = 0;
        let mut present = croaring::Bitmap::new();
        for round in pages.chunks(pages_per_round) {
            let shares: Vec<&[usize]> = round.chunks(share).collect();
            let parts: Vec<Result<(u64, croaring::Bitmap)>> = workers
                .par_iter_mut()
                .zip(shares.par_iter())
                .map(|(worker, pages)| {
                    worker.prepare(nbuckets, rows_per_worker, chunk_rows);
                    let mut claimed = 0u64;
                    let mut present = croaring::Bitmap::new();
                    let mut scratch: Vec<u32> = Vec::new();
                    for &page in pages.iter() {
                        let slots = self.page_of(page).expect("only present pages are listed");
                        add_page_entities(&mut present, page, slots, &mut scratch);
                        for (offset, &slot) in slots.iter().enumerate() {
                            if slot == ROW_ABSENT {
                                continue;
                            }
                            if slot >= row_count {
                                let entity = page * PAGE_ENTRIES + offset;
                                return Err(invalid(format!(
                                    "entity {entity} maps to row {slot}, out of bound for \
                                     row_count {row_count}"
                                )));
                            }
                            worker.push_row(slot);
                            claimed += 1;
                        }
                    }
                    Ok((claimed, present))
                })
                .collect();
            for part in parts {
                let (count, part) = part?;
                claimed += count;
                present.or_inplace(&part);
            }
            let filled = &workers[..shares.len()];
            seen.par_chunks_mut(STAMP_WORDS)
                .enumerate()
                .try_for_each(|(bucket, words)| {
                    let base = (bucket as u32) << BUCKET_SHIFT;
                    for worker in filled {
                        for rows in worker.slices(bucket) {
                            for &row in rows {
                                let offset = row - base;
                                let word = &mut words[(offset >> 6) as usize];
                                let bit = 1u64 << (offset & 63);
                                if *word & bit != 0 {
                                    return Err(invalid(format!(
                                        "row {row} is claimed by more than one entity (not a \
                                         bijection)"
                                    )));
                                }
                                *word |= bit;
                            }
                        }
                    }
                    Ok(())
                })?;
        }
        if claimed == u64::from(row_count) {
            let _ = self.dense_rows.set(row_count);
        }
        present.run_optimize();
        let _ = self.present_entities.set(present);
        Ok(())
    }

    /// Record that this mapping's slots are a bijection onto `[0, row_count)` on the **writer's**
    /// word rather than on a scan of the file.
    ///
    /// [`Self::validate_rows`] establishes the same fact by reading every present page, which is
    /// `O(bound)` and is the check a bundle arriving from storage must pay. A caller that wrote
    /// these bytes itself, in this process, has the fact already: the writer emits one slot per row
    /// of the segment it is writing, so the mapping is onto its own row count by construction. That
    /// caller is `read::open_written_prefix`, whose doc carries the premise and the obligation it
    /// puts on the one call site allowed to hold it.
    ///
    /// The first value recorded wins, so this cannot contradict a validation that already ran. It
    /// records nothing else: a caller with no such premise omits the call and the projection walks,
    /// which is correct at any cost.
    pub fn declare_dense_rows(&self, row_count: u32) {
        let _ = self.dense_rows.set(row_count);
    }

    /// The row count this mapping is known to be a bijection onto, or `None` where nothing has
    /// established it.
    ///
    /// `Some` says the slots are onto `[0, rows)`, which is what [`Self::project_with`]'s
    /// whole-domain answer rests on and what [`Self::project_complement_with`] requires; it says
    /// nothing about any particular mask. A caller choosing a route reads it here: it is the whole
    /// of the complement route's validity.
    pub fn dense_rows(&self) -> Option<u32> {
        self.dense_rows.get().copied()
    }

    /// `Some(rows)` where `mask` holds every entity in `[0, bound)` **and** this mapping is known
    /// to be a bijection onto `[0, rows)`, which are the two premises under
    /// [`Self::project_with`]'s whole-domain answer.
    ///
    /// A caller that has to know *which* route a projection took asks here first and reads the
    /// answer rather than inferring it from a cardinality: the two premises are exactly what
    /// `project_with` checks, so a caller that agrees with this agrees with it.
    /// `O(containers)` over entity space, the cost of [`Self::covers_domain`].
    pub fn whole_domain_rows(&self, mask: &croaring::Bitmap) -> Option<u32> {
        let rows = self.dense_rows()?;
        self.covers_domain(mask).then_some(rows)
    }

    /// Every entity that holds a row here, as a bitmap.
    ///
    /// Held for the life of the mapping. Where the mapping is known to be onto `[0, bound)` it is
    /// that range and reads no page; otherwise it is one walk of the present pages, which
    /// [`Self::validate_rows`] makes anyway and fills this from.
    pub fn present_entities(&self) -> &croaring::Bitmap {
        self.present_entities.get_or_init(|| {
            if let Some(rows) = self
                .dense_rows()
                .filter(|&rows| u64::from(rows) == self.bound)
            {
                return croaring::Bitmap::from_range(0..rows);
            }
            let mut present = croaring::Bitmap::new();
            let mut scratch: Vec<u32> = Vec::new();
            for page in 0..self.page_count {
                if let Some(slots) = self.page_of(page) {
                    add_page_entities(&mut present, page, slots, &mut scratch);
                }
            }
            present.run_optimize();
            present
        })
    }

    /// Every entity that holds a row here, ascending, with the row it holds.
    ///
    /// **The route across a whole permutation**, where [`Self::row_of`] is the route to one
    /// entity. A sweep by `row_of` repeats the directory lookup at every slot and visits each
    /// entity of an absent page one at a time; this reads the directory once a page and each
    /// present page end to end, so a sparse view costs its own slots and not its entity span.
    /// `mosaica verify` crosses entity space this way.
    pub fn try_for_each_slot<E>(
        &self,
        mut f: impl FnMut(u64, RowId) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        for page in 0..self.page_count {
            let Some(slots) = self.page_of(page) else {
                continue;
            };
            for (offset, &slot) in slots.iter().enumerate() {
                let entity = ((page as u64) << PAGE_SHIFT) | offset as u64;
                // The last page covers `bound` rounded up, so its tail slots address entities
                // this permutation does not have. `row_of` refuses them and so does this.
                if entity >= self.bound {
                    return Ok(());
                }
                if slot == ROW_ABSENT {
                    continue;
                }
                f(entity, RowId::new(slot))?;
            }
        }
        Ok(())
    }

    /// Row ID currently occupied by `e` in this segment, or `None` if `e` is out of bound, falls
    /// in an absent page, or holds the row-absent sentinel (never allocated a row here — including
    /// entities that exist but live in a different segment, or don't exist at all).
    pub fn row_of(&self, e: EntityId) -> Option<RowId> {
        let raw = e.raw();
        if raw >= self.bound {
            return None;
        }
        let page = self.page_of((raw >> PAGE_SHIFT) as usize)?;
        let slot = page[(raw as usize) & (PAGE_ENTRIES - 1)];
        if slot == ROW_ABSENT {
            None
        } else {
            Some(RowId::new(slot))
        }
    }

    /// Project an entity-space bitmap into row space: for every entity ID set in `mask`
    /// (ascending order, as `croaring::Bitmap` iterates), look up its row via this
    /// permutation, skip entities with no row here (out of bound, absent page, or sentinel), and
    /// return the resulting row IDs as a bitmap.
    ///
    /// **Cost (shared-context constraint 8):** this reads a slot for every member of `mask` and the
    /// pages they land in, which at 10⁹ is most of a second. ⊘ **Measured** on a 4-core Xeon, 10⁹
    /// entities, a 25% grant: 3.0 s on one thread and 0.96 s on four, where the windowed walk this
    /// replaced took 5.1 s whatever the pool; 9.0 s and 2.5 s for a 90% grant against 16.7 s. Never call it on the per-viewport path; the engine caches the
    /// result per `(token, view, pin)` and reuses it across viewports within a session.
    ///
    /// # One pass, and why the shape is not the obvious one
    ///
    /// §10.4 asks for "gather, radix sort, and bulk-construct from the sorted array". The gather
    /// and the sort are the same pass here, and the sort is not a sort.
    ///
    /// A row is written **once**, into a bucket, straight out of the slot lookup — the entity list
    /// never exists, and neither does a row array to be sorted and re-read. The earlier form wrote
    /// it four times over (decode to a `Vec`, gather, sort in place, read back through `add_many`)
    /// and spent 5 272 ms of its 8 267 ms in `par_sort_unstable` alone.
    ///
    /// **A bucket is a row range, not a container key**, and that is the load-bearing choice rather
    /// than an arbitrary one. Partitioning on the high 16 bits — which *is* the Roaring container
    /// key, so it looks like the natural unit — leaves 15 259 live write cursors at 10⁹, far more
    /// than the cache holds, and was **measured to get worse with scale**: 3.22× over the stages it
    /// replaces at 10⁸ but only 2.06× at 10⁹. [`BUCKET_SHIFT`] instead fixes the two things that
    /// decide the cost, and they pull in opposite directions: few enough buckets that the write
    /// cursors stay in L1, wide enough that the bit array each one stamps stays in L2.
    ///
    /// **Containers are emitted, not inserted.** The bit array a bucket stamps *is* 64 Roaring
    /// container payloads laid end to end, so [`mosaica_roaring::Sink`] takes them as they are —
    /// a popcount and a memcpy each. Expanding them back to `u32` for `add_many` instead costs
    /// 1 077 ms more at 10⁹ (2 306 ms against 1 229 ms), which is the larger half of the primitive.
    ///
    /// **The mask is read a container at a time, never decoded.** A container covers exactly one
    /// page, so its page is one directory read, a page the permutation does not hold is skipped
    /// whole, and the walk ends at the last page rather than at the mask's last entity. A bitset
    /// container selects its page's slots sixteen or eight at a time (`crate::select`), where
    /// croaring's cursor decodes a bitset a bit at a time at 2.4 ns a member; an array container is
    /// read by index and a run container as a span of slots.
    ///
    /// **Two passes a round, each on the current pool's threads.** A round is
    /// [`PROJECT_WINDOW_ROWS`] rows' worth of the mask. Its entity range is cut into one piece a
    /// thread, and each thread buckets its piece's rows into buckets of its own; then each bucket,
    /// a row range, is settled by one thread from every thread's share of it. No row is written by
    /// two threads and the settled parts have disjoint keys, so neither pass shares a write and
    /// joining the parts copies containers rather than merging them. On a pool whose other threads
    /// are busy, rayon runs the pieces on the calling thread one after another, which costs what
    /// one thread walking the mask costs: ⊘ **measured** 1.2–1.9× less time than the windowed walk
    /// this replaced, so a saturated server pays less for a projection than it did, not more. A
    /// mask under [`PARALLEL_MIN_ENTITIES`] stays on the calling thread.
    ///
    /// **Transient memory is a round's row ids, whatever the grant**, plus a page's that overshot
    /// it and a partly filled chunk per bucket a thread. A walk of more than one round also keeps
    /// the bits of each container past [`KEEP_ROWS`] rows ([`KeptBucket`]), never more than four
    /// times what the result holds for those containers. What else scales with the grant is the result, which is the
    /// answer. The mmap-backed pages are never copied, only read.
    ///
    /// **A mask holding every entity in `[0, bound)` is answered without reading a page.** Its
    /// image is every row this mapping has, and [`Self::dense_rows`] is what establishes that those
    /// are exactly `[0, row_count)` — so the answer is that range, built directly.
    ///
    /// **The served set is identical and the time taken is not, which is C4/C14's shape rather than
    /// a new channel.** A mask over the whole domain maps onto the whole range under every
    /// permutation, so the general path returns this same set from this same mask and no response
    /// differs anywhere. What a principal can observe is that its own first viewport was faster,
    /// and the quantity that decided it is whether its own grant covers the whole domain — a fact
    /// the principal holds already, which is exactly C14's accepted reasoning ("the principal
    /// already knows their own clearances, so this reveals nothing about data"). Nothing about the
    /// corpus enters the branch: the mapping is not read on this path, and `bound` is the view's
    /// entity-space width rather than anything a grant selects. So the residual sits inside C19's
    /// accepted shape — work correlating with the principal's own coverage — and adds no quantity
    /// to C4's open row, the same reading `mosaica_engine`'s `DecodeSource` records for the
    /// decode-source choice. Appendix C's **C19** names this branch as one of its accepted
    /// widenings rather than giving it a row of its own (owner ruling, 2026-09-14).
    ///
    /// **[`Self::project_complement_with`] is the same shape.** Whether a session takes it depends
    /// on how large the principal's own grant is against the domain, which is a fact the principal
    /// holds already, and the set it returns is the set this walk returns from the same mask. It
    /// reads the slots of entities the principal does not hold, and what it does with them is
    /// subtract their rows from the row range. Nothing read there is aggregated or served. C19
    /// covers it on the same ruling.
    ///
    /// The invariant this file carries is I4 — it is the only EntityId→RowId path — and both
    /// routes keep it, neither handing an entity id or a `mosaica_id` to a caller, so I10 stands
    /// where it stood.
    pub fn project(&self, mask: &croaring::Bitmap) -> croaring::Bitmap {
        self.project_with(mask, &mut ProjectScratch::default())
    }

    /// [`Self::project`], reusing a caller's scratch buffers.
    ///
    /// **For a caller that projects many masks in a row**, which the artifact pass does — once per
    /// artifact, three times over per level. The scratch this reuses is a 512 KB stamp plus the
    /// buckets' chunk pool, and allocating it per call put an `mmap`/`munmap` pair and 128 minor
    /// faults on every projection: at 6×10⁵ artifacts that is ~10⁸ page faults and hundreds of
    /// gigabytes of zeroing, for buffers whose contents never outlive the call. The session path
    /// projects once and keeps [`Self::project`], which allocates as it always did.
    ///
    /// Behaviour is identical — the buffers are cleared rather than carried, exactly as a fresh
    /// allocation would leave them.
    pub fn project_with(
        &self,
        mask: &croaring::Bitmap,
        scratch: &mut ProjectScratch,
    ) -> croaring::Bitmap {
        if mask.is_empty() {
            return croaring::Bitmap::new();
        }
        if let Some(&rows) = self.dense_rows.get() {
            if self.covers_domain(mask) {
                return Self::row_range(rows);
            }
        }
        self.project_windowed(mask, scratch, PROJECT_WINDOW_ROWS)
    }

    /// The image of `mask`, computed from the entities **outside** it:
    /// `[0, rows) \ image([0, bound) \ mask)`. `None` where that route cannot answer, and then the
    /// caller walks.
    ///
    /// **The same set [`Self::project_with`] returns**, reached by walking `bound − |mask|` slots
    /// instead of `|mask|` of them. It is the route for a principal holding most of the domain: a
    /// grant over 99% of it walks 1% of the entities. A session picks between this and the other
    /// routes by cost, before any of them runs (`crate::term_images::choose`); this function
    /// decides nothing but whether it has an answer at all.
    ///
    /// # Why it is the same set
    ///
    /// The route needs [`Self::dense_rows`], and that is the whole of its validity: with it the
    /// slots are a bijection onto `[0, rows)`, so **every row is claimed by exactly one entity in
    /// `[0, bound)`**, and that entity is either in the mask or in the complement. The rows of the
    /// complement are therefore exactly the rows the mask does not reach, and subtracting them from
    /// the whole range leaves the mask's own image. Without it nothing says the rows are `[0, rows)`
    /// rather than some subset, the subtraction would remove rows no entity claims, and the answer
    /// is `None`.
    ///
    /// An entity in the complement with no row, from an absent slot or an absent page, projects to
    /// nothing and subtracts nothing. That is right: it claims no row the mask could be denied.
    /// An entity at or above `bound` in the mask is not this permutation's. The walk skips it, and
    /// the complement is taken over `[0, bound)`, so neither route lets it reach a row.
    ///
    /// # Transient memory
    ///
    /// ⊘ **Modelled.** The peak is two bitmaps: the complement of the mask, at most one bitset
    /// container per 65 536 entities of `bound`, and the image of it the walk produces, at most one
    /// per 65 536 rows. At `bound = rows = 3.5×10⁹` each is 53 407 containers of 8 KiB, **437 MB
    /// each and 875 MB together**, on top of the walk's own transient, which is as
    /// [`Self::project`] states. The complement is
    /// built in place, so that step holds one bitmap and the mask's own containers rather than a
    /// second copy of the range, and it is dropped as soon as the walk over it returns. What
    /// replaces it is the answer rather than a transient. A grant close to the whole domain, which
    /// is what this route is for, holds far less than that on both counts, because its complement
    /// is small.
    pub fn project_complement_with(
        &self,
        mask: &croaring::Bitmap,
        scratch: &mut ProjectScratch,
    ) -> Option<croaring::Bitmap> {
        let &rows = self.dense_rows.get()?;
        // A bound of zero holds no entity, and one above the `u32` entity ceiling (I9) names
        // entities no mask can hold, as `covers_domain` reads it. Neither has a complement to walk.
        let hi = self
            .bound
            .checked_sub(1)
            .and_then(|hi| u32::try_from(hi).ok())?;
        let outside = {
            let mut complement = croaring::Bitmap::from_range(0..=hi);
            complement.andnot_inplace(mask);
            self.project_windowed(&complement, scratch, PROJECT_WINDOW_ROWS)
        };
        let mut image = Self::row_range(rows);
        image.andnot_inplace(&outside);
        Some(image)
    }

    /// `[0, rows)` as a bitmap. Zero rows is the empty bitmap.
    fn row_range(rows: u32) -> croaring::Bitmap {
        match rows {
            0 => croaring::Bitmap::new(),
            rows => croaring::Bitmap::from_range(0..rows),
        }
    }

    /// [`Self::project_with`]'s general path: the bucket walk, a round of `window_rows` rows at a
    /// time.
    ///
    /// **The window is a parameter because the forms have to be compared.** A walk of many rounds
    /// and one of a single round must produce the same set, and the production window is 16.7
    /// million rows — larger than any fixture a test builds. Passing `usize::MAX` gives the
    /// single round, which is what the equality is checked against; [`Self::project_with`] passes
    /// [`PROJECT_WINDOW_ROWS`].
    fn project_windowed(
        &self,
        mask: &croaring::Bitmap,
        scratch: &mut ProjectScratch,
        window_rows: usize,
    ) -> croaring::Bitmap {
        self.walk(mask, scratch, window_rows, PARALLEL_MIN_ENTITIES)
    }

    /// [`Self::project_windowed`], spreading over the current pool's threads a mask holding at
    /// least `parallel_min` entities, which a test sets to zero to spread a small one.
    fn walk(
        &self,
        mask: &croaring::Bitmap,
        scratch: &mut ProjectScratch,
        window_rows: usize,
        parallel_min: usize,
    ) -> croaring::Bitmap {
        let readable = Readable::of(mask);
        let containers = readable.containers();
        // The pages the mask holds entities in that this permutation holds slots for, in entity
        // order. A container keyed at or past `page_count` is entities at or above `bound`, which
        // have no row *here* — ordinarily entities living in a different segment — and the last
        // page's slots past `bound` hold no row, as `validate_directory` checks.
        let mut pages: Vec<MaskPage<'_>> = Vec::with_capacity(containers.len());
        let mut held = 0usize;
        for (index, &key) in containers.keys().iter().enumerate() {
            let Some(slots) = self.page_slots(usize::from(key)) else {
                continue;
            };
            let entities = containers.cardinality(index) as usize;
            held += entities;
            pages.push(MaskPage {
                container: index,
                slots,
                entities,
            });
        }
        if held == 0 {
            return croaring::Bitmap::new();
        }
        let threads = if scratch.serial || held < parallel_min {
            1
        } else {
            rayon::current_num_threads().max(1)
        };
        // Pieces of whole pages, each about a round's share for one thread. A piece overshoots its
        // share by less than one page.
        let rounds = held.div_ceil(window_rows).max(1);
        let share = held.div_ceil(rounds.saturating_mul(threads)).max(1);
        let mut pieces: Vec<std::ops::Range<usize>> = Vec::new();
        let (mut start, mut taken, mut widest) = (0usize, 0usize, 0usize);
        for (at, page) in pages.iter().enumerate() {
            taken += page.entities;
            if taken >= share {
                pieces.push(start..at + 1);
                widest = widest.max(taken);
                (start, taken) = (at + 1, 0);
            }
        }
        if start < pages.len() {
            pieces.push(start..pages.len());
            widest = widest.max(taken);
        }
        // Every row this permutation can yield is below `bound`: `validate_rows` establishes that
        // it is a bijection *onto* `[0, row_count)`, so each row is claimed by a distinct in-bound
        // entity and `row_count <= bound`. Sizing the buckets from `bound` therefore cannot
        // under-count them, whatever the mask contains.
        let nbuckets = (self.bound_usize >> BUCKET_SHIFT) + 1;
        // A chunk near the mean a bucket takes from a piece, so a mask spread evenly over row
        // space fills about one chunk a bucket, and one concentrated in a few buckets takes many.
        let chunk_rows = (widest / nbuckets)
            .next_power_of_two()
            .clamp(CHUNK_ROWS_MIN, CHUNK_ROWS_MAX);
        let used = threads.min(pieces.len());
        if scratch.workers.len() < used {
            scratch.workers.resize_with(used, Worker::default);
        }
        let parallel = used > 1;
        let mut kept: Option<Vec<KeptBucket>> =
            (pieces.len() > used).then(|| (0..nbuckets).map(|_| KeptBucket::new()).collect());

        let mut rows = croaring::Bitmap::new();
        for round in pieces.chunks(used) {
            let workers = &mut scratch.workers[..round.len()];
            let walk_piece = |worker: &mut Worker, piece: &std::ops::Range<usize>| {
                worker.prepare(nbuckets, widest, chunk_rows);
                worker.walk(&containers, &pages[piece.clone()]);
            };
            if round.len() == 1 {
                walk_piece(&mut workers[0], &round[0]);
            } else {
                workers
                    .par_iter_mut()
                    .zip(round.par_iter())
                    .for_each(|(worker, piece)| walk_piece(worker, piece));
            }
            let part = settle(
                &scratch.workers[..round.len()],
                nbuckets,
                kept.as_deref_mut(),
                scratch,
                parallel,
            );
            if rows.is_empty() {
                rows = part;
            } else {
                rows.or_inplace(&part);
            }
        }
        if let Some(kept) = kept.as_deref_mut() {
            let part = emit_kept(kept, parallel);
            if rows.is_empty() {
                rows = part;
            } else {
                rows.or_inplace(&part);
            }
        }
        rows
    }

    /// Whether `mask` holds every entity this permutation covers, `[0, bound)`.
    ///
    /// `O(containers)` over entity space, and the first gap answers it — 65 536 containers at the
    /// entity ceiling, against the pass it decides whether to run.
    fn covers_domain(&self, mask: &croaring::Bitmap) -> bool {
        let Some(hi) = self.bound.checked_sub(1) else {
            // No entities, so every mask holds all of them.
            return true;
        };
        match u32::try_from(hi) {
            Ok(hi) => mask.contains_range(0..=hi),
            // An entity id is a `u32` (I9), so a bound above the ceiling names entities no mask
            // can hold. The general path answers it.
            Err(_) => false,
        }
    }

    /// The path this permutation was loaded from (for diagnostics only).
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// One flush or merge segment's slice of row space: the entity range it covers, and where in
/// row space its rows live.
///
/// `rows` is **dense over `[entity_lo, entity_hi]`** and holds each entity's row *relative to*
/// `row_base`, or [`ROW_ABSENT`] for an entity in the span with no row here: one deleted before
/// its flush, or one whose row is in another extent's `below`. Dense rather than keyed because
/// ids above everything a view holds are issued in ascending runs from the high-water, and one
/// `u32` per entity is smaller than any keyed form over such a span.
///
/// `below` holds the rows of entities under `entity_lo`, as `(entity, row)` pairs ascending by
/// entity. An existing item added to a view keeps its entity, and the allocator issues ids a fold
/// freed before new ones, so a flush can carry an entity older than rows the view already holds.
/// Those rows cannot join the dense span without widening it across the whole view, so they are
/// listed. `entity_hi + 1 == entity_lo` for an extent whose every row is listed.
///
/// It is not a `permutation.bin`. That file's length is the *bundle's* whole entity space, which
/// is the wrong shape for a segment covering a few thousand ids at the top of it; an extent's rows
/// live in the side-manifest's file set instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentExtent {
    pub entity_lo: u64,
    /// Inclusive.
    pub entity_hi: u64,
    pub seg_id: String,
    pub row_base: u32,
    /// `rows[e - entity_lo]`, relative to `row_base`; [`ROW_ABSENT`] where the entity has no row.
    pub rows: Vec<u32>,
    /// `(entity, row)` for each entity below `entity_lo` with a row here, ascending by entity,
    /// the row relative to `row_base`.
    pub below: Vec<(u32, u32)>,
}

impl SegmentExtent {
    /// An extent over `entities`, where `entities[i]` holds row `i` relative to `row_base`: an
    /// entity in `[entity_lo, entity_hi]` takes its slot in the dense span, one below it is
    /// listed. `None` where an entity appears twice or lies above `entity_hi`, or where the span
    /// is inverted or too wide to address.
    pub fn from_rows(
        seg_id: &str,
        row_base: u32,
        (entity_lo, entity_hi): (u64, u64),
        entities: impl IntoIterator<Item = u64>,
    ) -> Option<Self> {
        let span = usize::try_from((entity_hi + 1).checked_sub(entity_lo)?).ok()?;
        let mut rows = vec![ROW_ABSENT; span];
        let mut below: Vec<(u32, u32)> = Vec::new();
        for (row, entity) in entities.into_iter().enumerate() {
            let row = u32::try_from(row).ok()?;
            if entity < entity_lo {
                below.push((u32::try_from(entity).ok()?, row));
                continue;
            }
            let slot = rows.get_mut(usize::try_from(entity - entity_lo).ok()?)?;
            if *slot != ROW_ABSENT {
                return None;
            }
            *slot = row;
        }
        below.sort_unstable();
        let extent = SegmentExtent {
            entity_lo,
            entity_hi,
            seg_id: seg_id.to_string(),
            row_base,
            rows,
            below,
        };
        extent.is_well_formed().then_some(extent)
    }

    /// The dense span a flush gives `entities` in a view whose rows reach up to `floor`: from the
    /// lowest entity at or above `floor` to the highest, or the empty span at `floor` where every
    /// entity is below it.
    pub fn flush_span(floor: u64, entities: impl IntoIterator<Item = u64>) -> (u64, u64) {
        let mut span: Option<(u64, u64)> = None;
        for entity in entities.into_iter().filter(|&e| e >= floor) {
            span = Some(match span {
                Some((lo, hi)) => (lo.min(entity), hi.max(entity)),
                None => (entity, entity),
            });
        }
        // An empty span exists only where some entity lies below `floor`, so `floor` is above 0.
        span.unwrap_or((floor, floor.saturating_sub(1)))
    }

    /// Recover a flush or merge segment's extent from the segment itself, at open.
    ///
    /// **Why this exists rather than a file.** §2.1 describes an extent as four scalars —
    /// `{entity_lo, entity_hi, seg_id, row_base}` — which is only a mapping if the segment's rows
    /// are in entity order. The same section requires every segment to be Morton-sorted, and that
    /// sort *is* the tile index, so the mapping is an arbitrary permutation of the entity range and
    /// four scalars cannot express it. Something has to carry it across a restart.
    ///
    /// The alternative was to write `rows` beside `morton.u32` (4 bytes × entities in the segment,
    /// one more file, one more manifest field, a contracts §2.1 change). This construction stores
    /// only the rows an edit moved: `columns.arrow` already carries `mosaica_id` at the row, the
    /// identity is a bijection over 2⁶⁴ ([`mosaica_types::IdentityKey`]) from an item's number,
    /// and `MANIFEST.json` already carries the key, so a row's entity is its number's unless the
    /// segment's edited rows list another ([`crate::edited`]). An entity under `entity_lo` goes to
    /// [`Self::below`], as the flush that wrote the segment put it.
    ///
    /// **The invariant it spends, stated so it is not spent again silently.** Row space above the
    /// build bound is now recoverable *only* while the identity permutation is invertible at open.
    /// A future construction that made `mosaica_id` one-way — a keyed hash, a key held outside the
    /// bundle, a per-session blinding — would silently strand every flushed entity's row, exactly
    /// the failure this replaces. I10 is unaffected: nothing here leaves the engine, and the
    /// inversion is the same one `Engine::item` already performs per request.
    ///
    /// **Cost.** One Feistel inversion per row of each flush segment, at every open, bounded by
    /// what merge leaves unmerged rather than by the corpus. Deliberately not parallelised: it runs
    /// once at open, inside a loop that is already mapping and digest-verifying files.
    pub fn rebuild(
        segment: &crate::read::SegmentData,
        entity_lo: u64,
        entity_hi: u64,
        row_base: u32,
        key: &mosaica_types::IdentityKey,
        shard: mosaica_types::ShardId,
    ) -> Result<Self> {
        let (seg_id, mosaica_ids) = (segment.seg_id.as_str(), segment.columns.mosaica_id());
        let mut entities = Vec::with_capacity(mosaica_ids.len());
        for (local, &raw) in mosaica_ids.iter().enumerate() {
            // A wrong shard means this segment was written under a different identity
            // configuration than the manifest declares — corruption, not a row to skip. Serving
            // past it would put a row under an entity id that names a different item.
            let entity = segment
                .entities
                .entity_of(local as u32, raw, key, shard, seg_id)?;
            entities.push(entity.raw());
        }
        // The same check `with_extent` applies to an extent arriving from a flush, applied here
        // too so a malformed extent names the segment it was read from.
        SegmentExtent::from_rows(seg_id, row_base, (entity_lo, entity_hi), entities).ok_or_else(
            || StoreError::MalformedBundle {
                detail: format!(
                    "segment '{seg_id}': the extent rebuilt from its mosaica_id column is not a \
                     bijection onto its own rows at or below the descriptor's entity \
                     {entity_hi}"
                ),
            },
        )
    }

    /// How many rows this extent actually owns: the non-absent slots and the listed rows.
    pub fn row_count(&self) -> u32 {
        (self.rows.iter().filter(|&&r| r != ROW_ABSENT).count() + self.below.len()) as u32
    }

    /// The lowest entity holding a row here, where any does.
    pub fn lowest(&self) -> Option<u64> {
        self.below.first().map(|&(e, _)| u64::from(e)).or_else(|| {
            self.rows
                .iter()
                .position(|&r| r != ROW_ABSENT)
                .map(|i| self.entity_lo + i as u64)
        })
    }

    /// Every `(entity, row)` this extent holds, the row relative to `row_base`: the listed rows,
    /// then the dense span's, each ascending by entity.
    pub fn pairs(&self) -> impl Iterator<Item = (u64, u32)> + '_ {
        let dense = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, &row)| row != ROW_ABSENT)
            .map(|(i, &row)| (self.entity_lo + i as u64, row));
        self.below
            .iter()
            .map(|&(e, row)| (u64::from(e), row))
            .chain(dense)
    }

    /// Every entity holding a row here, listed or in the dense span.
    pub fn entities(&self) -> croaring::Bitmap {
        let mut out = croaring::Bitmap::of(&self.below.iter().map(|&(e, _)| e).collect::<Vec<_>>());
        for (i, _) in self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, &row)| row != ROW_ABSENT)
        {
            out.add((self.entity_lo + i as u64) as u32);
        }
        out.run_optimize();
        out
    }

    /// Whether this extent is internally well-formed: its `rows` cover its entity span exactly,
    /// `below` ascends strictly under `entity_lo`, and the rows of both are a bijection onto
    /// `[0, row_count)`.
    ///
    /// The same property [`Permutation::validate_rows`] enforces for the base, and for the same
    /// reason: a slot outside the range, or two entities aliased onto one row, would let `row_of`
    /// hand out a `RowId` that indexes the segment's `columns.arrow` out of bounds (I4/I11).
    fn is_well_formed(&self) -> bool {
        let Some(span) = (self.entity_hi + 1)
            .checked_sub(self.entity_lo)
            .and_then(|d| usize::try_from(d).ok())
        else {
            return false;
        };
        if self.rows.len() != span {
            return false;
        }
        if !self.below.windows(2).all(|w| w[0].0 < w[1].0)
            || self
                .below
                .last()
                .is_some_and(|&(e, _)| u64::from(e) >= self.entity_lo)
        {
            return false;
        }
        let count = self.row_count();
        let mut seen = RowsSeen::new(count as usize);
        for row in self
            .rows
            .iter()
            .copied()
            .filter(|&r| r != ROW_ABSENT)
            .chain(self.below.iter().map(|&(_, r)| r))
        {
            if row >= count || seen.claim(row as usize) {
                return false;
            }
        }
        true
    }

    /// This extent's contribution to a projection: the rows of every entity of `mask` that falls
    /// inside it. Nothing else in `mask` can be answered here, so nothing else is looked at.
    ///
    /// **It seeks to `entity_lo` rather than skipping up to it, and the difference is the whole
    /// cost of a flush's patch.** An extent's dense entities are the newest in the partition, so
    /// they sit at the top of the mask; a walk from the mask's start pays O(mask cardinality) to
    /// reach them — *measured* 79 ms per extent against a 25 M-entity grant at 10⁸,
    /// which is the patch's cost being a function of the
    /// grant's width rather than of the flush's size. `reset_at_or_after` costs O(containers
    /// skipped), which is the cost model this index is designed against
    /// (`CLAUDE.md`: bitmap operations cost O(containers touched), not O(cardinality)).
    ///
    /// The listed rows are reached from whichever side is smaller: the mask's entities within the
    /// listed ones' range, each looked up, or the listed rows, each tested against the mask. A
    /// stored level projects each of its artifacts through the extent a flush wrote, and most
    /// artifacts are far smaller than the flush.
    fn project(&self, mask: &croaring::Bitmap) -> croaring::Bitmap {
        let mut rows: Vec<u32> = Vec::new();
        if let (Some(&(first, _)), Some(&(last, _))) = (self.below.first(), self.below.last()) {
            if mask.range_cardinality(first..=last) <= self.below.len() as u64 {
                let mut iter = mask.iter();
                iter.reset_at_or_after(first);
                for entity in iter.take_while(|&entity| entity <= last) {
                    if let Ok(at) = self.below.binary_search_by_key(&entity, |&(e, _)| e) {
                        rows.push(self.row_base + self.below[at].1);
                    }
                }
            } else {
                rows.extend(
                    self.below
                        .iter()
                        .filter(|&&(entity, _)| mask.contains(entity))
                        .map(|&(_, row)| self.row_base + row),
                );
            }
        }
        // `entity_hi` is inclusive and the range end is exclusive; both ends are already inside
        // `u32` because a `mask` is entity-space and entity ids are capped at `u32::MAX` (I9).
        if !self.rows.is_empty() {
            let lo = u32::try_from(self.entity_lo).unwrap_or(u32::MAX);
            let hi = u32::try_from(self.entity_hi).unwrap_or(u32::MAX);
            let mut iter = mask.iter();
            iter.reset_at_or_after(lo);
            for entity in iter {
                if entity > hi {
                    break;
                }
                let slot = self.rows[(entity - lo) as usize];
                if slot != ROW_ABSENT {
                    rows.push(self.row_base + slot);
                }
            }
        }
        croaring::Bitmap::of(&rows)
    }

    /// The row `entity` holds in the dense span, where it holds one there.
    fn dense_row_of(&self, entity: u64) -> Option<RowId> {
        if entity < self.entity_lo || entity > self.entity_hi {
            return None;
        }
        let slot = self.rows[(entity - self.entity_lo) as usize];
        (slot != ROW_ABSENT).then(|| RowId::new(self.row_base + slot))
    }

    /// The row `entity` holds among the listed rows, where it holds one there.
    fn below_row_of(&self, entity: u64) -> Option<RowId> {
        let entity = u32::try_from(entity).ok()?;
        let at = self.below.binary_search_by_key(&entity, |&(e, _)| e).ok()?;
        Some(RowId::new(self.row_base + self.below[at].1))
    }
}

/// A row set's entities grouped by their high 16 bits, from [`RowSpace::entity_buckets`].
pub struct EntityBuckets {
    pieces: Vec<Buckets16>,
    keys: Vec<u16>,
}

impl EntityBuckets {
    /// Every key holding an entity, ascending.
    pub fn keys(&self) -> &[u16] {
        &self.keys
    }

    /// How many entities `key` holds.
    pub fn count(&self, key: u16) -> u32 {
        self.pieces.iter().map(|p| p.lows(key).len() as u32).sum()
    }

    /// The low 16 bits of `key`'s entities, a piece's bucket at a time, in no particular order.
    pub fn for_each_bucket(&self, key: u16, mut f: impl FnMut(&[u16])) {
        for piece in &self.pieces {
            let lows = piece.lows(key);
            if !lows.is_empty() {
                f(lows);
            }
        }
    }
}

/// One piece of a row set's entities, bucketed by their high 16 bits: each key present, ascending,
/// with where its low 16 bits start in `lows`.
struct Buckets16 {
    keys: Vec<(u16, u32)>,
    lows: Vec<u16>,
}

impl Buckets16 {
    /// Count each key, then place each entity's low bits in its key's bucket, reading the rows'
    /// entities twice rather than holding them.
    fn of(
        rows: &croaring::Bitmap,
        piece: std::ops::Range<u32>,
        tables: RowEntities<'_>,
        keys_below: usize,
    ) -> Self {
        let mut at = vec![0u32; keys_below.min(1 << 16)];
        mosaica_roaring::for_each_run_in(rows, piece.clone(), &mut |run| {
            tables.for_each_slice(run, |slice| {
                for &e in slice {
                    at[(e >> 16) as usize] += 1;
                }
            });
        });
        let mut keys = Vec::new();
        let mut total = 0u32;
        for (key, slot) in at.iter_mut().enumerate() {
            if *slot > 0 {
                keys.push((key as u16, total));
                let n = *slot;
                *slot = total;
                total += n;
            }
        }
        let mut lows = vec![0u16; total as usize];
        mosaica_roaring::for_each_run_in(rows, piece, &mut |run| {
            tables.for_each_slice(run, |slice| {
                for &e in slice {
                    let slot = &mut at[(e >> 16) as usize];
                    lows[*slot as usize] = e as u16;
                    *slot += 1;
                }
            });
        });
        Buckets16 { keys, lows }
    }

    /// The low bits bucketed under `key`, empty where the piece has none.
    fn lows(&self, key: u16) -> &[u16] {
        match self.keys.binary_search_by_key(&key, |&(k, _)| k) {
            Ok(i) => {
                let start = self.keys[i].1 as usize;
                let end = self
                    .keys
                    .get(i + 1)
                    .map_or(self.lows.len(), |&(_, next)| next as usize);
                &self.lows[start..end]
            }
            Err(_) => &[],
        }
    }
}

/// A view's row-to-entity direction: the base's `row-entity.u32` for rows below the base's row
/// count, and the extents' inverse above it.
#[derive(Debug, Clone, Copy)]
pub struct RowEntities<'a> {
    base: &'a [u32],
    tail: &'a [u32],
}

impl RowEntities<'_> {
    /// How many rows the tables cover.
    pub fn len(&self) -> u32 {
        (self.base.len() + self.tail.len()) as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entity at `row`. Panics where `row` is past [`Self::len`].
    #[inline]
    pub fn entity_of(&self, row: u32) -> u32 {
        match self.base.get(row as usize) {
            Some(&entity) => entity,
            None => self.tail[row as usize - self.base.len()],
        }
    }

    /// The entities of `rows` as at most two slices, in row order: the base's part, then the
    /// extents'. Panics where `rows` runs past [`Self::len`].
    #[inline]
    pub fn for_each_slice(&self, rows: std::ops::Range<u32>, mut f: impl FnMut(&[u32])) {
        let (start, end, split) = (rows.start as usize, rows.end as usize, self.base.len());
        if start < split {
            f(&self.base[start..end.min(split)]);
        }
        if end > split {
            f(&self.tail[start.max(split) - split..end - split]);
        }
    }
}

/// One view's whole entity→row mapping: the built base permutation, plus the extents flush has
/// appended and merge has collapsed since.
///
/// **Constructed incrementally, never rebuilt.** [`Self::with_extent`] and [`Self::collapsing`]
/// return a new value sharing the base by `Arc`, because re-opening it would re-pay
/// `Permutation::load`'s `O(bound)` `validate_rows` — more than the flush that prompted it.
///
/// The extents' dense spans are ordered, ascending and disjoint, and each extent begins exactly
/// where row space currently ends. That is what makes `total_rows` a running sum rather than a
/// scan, and it is what a merge preserves: a merge emits exactly as many rows as it consumed, so
/// no later extent's `row_base` ever moves. An entity below an extent's span is listed in its
/// `below` and has a row nowhere else in the view.
#[derive(Debug, Clone)]
pub struct RowSpace {
    base: Arc<Permutation>,
    /// `row-entity.u32` for the base, when the view published one — the row→entity direction, for
    /// the filtered viewport's per-tile route ([`crate::row_entity`]). `None` where a view has no
    /// table, in which case [`Self::entity_of`] answers `None` and the caller falls back to the
    /// projecting route rather than to a wrong answer.
    base_inverse: Option<Arc<crate::row_entity::RowToEntity>>,
    /// The same direction for the rows *above* the base, derived from the extents' own mappings on
    /// first use and thrown away whenever an extent is added. Materialised rather than searched
    /// because the caller is a per-row path; bounded by the flushed tail, which the merge ladder
    /// bounds in turn.
    extent_inverse: std::sync::OnceLock<Vec<u32>>,
    /// The build segment's row count — the base owns `[0, base_rows)`. Not derivable from the
    /// permutation, whose `bound` is an entity-space width and may exceed its row count.
    base_rows: u32,
    /// Dense spans ordered, ascending, disjoint.
    extents: Vec<SegmentExtent>,
    /// `extents[i].entities()`, built when the extent joins.
    extent_entities: Vec<Arc<croaring::Bitmap>>,
    /// Every entity an extent lists below its span, so a lookup that finds no row in the base or
    /// a span answers from one bitmap probe rather than a search of every extent's list.
    listed: Arc<croaring::Bitmap>,
    /// `base_rows` plus every extent's `row_count`, maintained rather than recomputed.
    total_rows: u64,
}

impl RowSpace {
    pub fn new(base: Arc<Permutation>, base_rows: u32) -> Self {
        RowSpace {
            base,
            base_inverse: None,
            extent_inverse: std::sync::OnceLock::new(),
            base_rows,
            extents: Vec::new(),
            extent_entities: Vec::new(),
            listed: Arc::new(croaring::Bitmap::new()),
            total_rows: base_rows as u64,
        }
    }

    /// This row space with the base's `row-entity.u32` attached.
    ///
    /// Additive rather than a constructor parameter: a row space is correct without it — every
    /// caller that only crosses entity→row is unaffected — and the table is an optimisation for
    /// the one path that crosses the other way.
    pub fn with_row_entity(mut self, table: Arc<crate::row_entity::RowToEntity>) -> Self {
        self.base_inverse = Some(table);
        self
    }

    /// The base's row-to-entity table, where it published one.
    pub fn row_entity(&self) -> Option<&Arc<crate::row_entity::RowToEntity>> {
        self.base_inverse.as_ref()
    }

    /// Can this row space cross row→entity at all?
    ///
    /// A caller choosing between the per-tile and projecting routes asks this once, before
    /// committing to a route, rather than discovering row by row that [`Self::entity_of`] cannot
    /// answer. True when the base published a `row-entity.u32` — or when there is no base to
    /// invert, every row belonging to an extent, whose mapping is always recoverable.
    pub fn can_invert(&self) -> bool {
        self.base_inverse.is_some() || self.base_rows == 0
    }

    /// The entity occupying `row`, or `None` when this row space cannot answer — either `row` is
    /// out of range, or the view published no `row-entity.u32` and the base cannot be inverted
    /// without one.
    ///
    /// **`None` is "ask another way", not "no entity".** Every row has an entity by construction;
    /// a `None` here means the caller must fall back to the projecting route, and treating it as an
    /// absence would silently drop rows from a filtered viewport.
    pub fn entity_of(&self, row: RowId) -> Option<EntityId> {
        let raw = row.raw();
        if raw >= self.total_rows as u32 {
            return None;
        }
        if raw < self.base_rows {
            return self.base_inverse.as_ref()?.entity_of(row);
        }
        let table = self
            .extent_inverse
            .get_or_init(|| self.build_extent_inverse());
        table
            .get((raw - self.base_rows) as usize)
            .map(|&e| EntityId::new(e as u64))
    }

    /// Invert every extent's `rows` into one flat table covering `[base_rows, total_rows)`.
    ///
    /// Extents are disjoint and their `row_base`s continue row space exactly (`with_extent`
    /// enforces both), so the flat table is dense and each extent writes only its own rows.
    fn build_extent_inverse(&self) -> Vec<u32> {
        let span = (self.total_rows - self.base_rows as u64) as usize;
        let mut out = vec![0u32; span];
        for extent in &self.extents {
            for (entity, row) in extent.pairs() {
                let absolute = extent.row_base as u64 + row as u64;
                let at = (absolute - self.base_rows as u64) as usize;
                out[at] = entity as u32;
            }
        }
        out
    }

    /// Where the next extent's dense span may begin: one past the newest extent's span, or the
    /// base's bound. A flush lists the rows of entities below it in the extent's `below`.
    pub fn entity_floor(&self) -> u64 {
        self.extents
            .last()
            .map_or(self.base.bound(), |extent| extent.entity_hi + 1)
    }

    /// This row space plus one more segment, sharing the base.
    ///
    /// `None` if `extent` is malformed, its dense span does not begin strictly above the last
    /// extent's `entity_hi` and at or above the base's bound, one of its listed entities already
    /// has a row in this view, or it does not continue row space exactly
    /// (`row_base == total_rows()`). Every one of those is corruption rather than a state to
    /// tolerate: a gap or an overlap makes some other segment's rows unreachable or aliased.
    pub fn with_extent(&self, extent: SegmentExtent) -> Option<Self> {
        if !extent.is_well_formed() {
            return None;
        }
        if extent.entity_lo < self.entity_floor() {
            return None;
        }
        // An item is added to a view only where it has no row there, and a fold frees an id only
        // once it has removed every row of the id's previous holder, so a listed entity with a row
        // here is corruption rather than a race.
        if extent
            .below
            .iter()
            .any(|&(entity, _)| self.row_of(EntityId::new(u64::from(entity))).is_some())
        {
            return None;
        }
        if u64::from(extent.row_base) != self.total_rows {
            return None;
        }
        let total_rows = self.total_rows + u64::from(extent.row_count());
        // Row ids are `u32` (bundle_format 1), so a view that would cross 2^32 rows must fail
        // here rather than at the first `row_base + slot` that wraps.
        if total_rows > u64::from(u32::MAX) {
            return None;
        }
        let listed = if extent.below.is_empty() {
            Arc::clone(&self.listed)
        } else {
            let mut listed = (*self.listed).clone();
            listed.add_many(&extent.below.iter().map(|&(e, _)| e).collect::<Vec<_>>());
            Arc::new(listed)
        };
        let mut extent_entities = self.extent_entities.clone();
        extent_entities.push(Arc::new(extent.entities()));
        let mut extents = self.extents.clone();
        extents.push(extent);
        Some(RowSpace {
            base: Arc::clone(&self.base),
            // The base table survives — the base's rows are exactly what an extent does not touch.
            // The derived tail does not: it covers the extents, and this call changes them.
            base_inverse: self.base_inverse.clone(),
            extent_inverse: std::sync::OnceLock::new(),
            base_rows: self.base_rows,
            extents,
            extent_entities,
            listed,
            total_rows,
        })
    }

    /// This row space with the adjacent run `seg_ids` replaced by the single extent `merged`.
    ///
    /// `None` if any input `seg_id` is absent from the current generation — which is how a merge
    /// planned against a superseded generation is discarded rather than published. It is ABA-safe
    /// because `seg_id`s are never reused, across merges or prefixes (contracts §2.1), so a
    /// present `seg_id` is the same segment the merge consumed.
    ///
    /// Also `None` unless the inputs form a *contiguous run* and `merged` covers exactly their
    /// entity range at exactly their `row_base` and row count. A merge is row-count preserving, so
    /// anything else would move a later extent's rows.
    pub fn collapsing(&self, seg_ids: &[String], merged: SegmentExtent) -> Option<Self> {
        if seg_ids.is_empty() || !merged.is_well_formed() {
            return None;
        }
        let start = self
            .extents
            .iter()
            .position(|e| e.seg_id == seg_ids[0])
            .filter(|&i| i + seg_ids.len() <= self.extents.len())?;
        let run = &self.extents[start..start + seg_ids.len()];
        if run.iter().zip(seg_ids).any(|(e, id)| &e.seg_id != id) {
            return None;
        }
        let consumed_rows: u32 = run.iter().map(SegmentExtent::row_count).sum();
        if merged.entity_lo != run[0].entity_lo
            || merged.entity_hi != run[run.len() - 1].entity_hi
            || merged.row_base != run[0].row_base
            || merged.row_count() != consumed_rows
        {
            return None;
        }
        let mut extent_entities = Vec::with_capacity(self.extents.len() - seg_ids.len() + 1);
        extent_entities.extend_from_slice(&self.extent_entities[..start]);
        extent_entities.push(Arc::new(merged.entities()));
        extent_entities.extend_from_slice(&self.extent_entities[start + seg_ids.len()..]);
        let mut extents = Vec::with_capacity(self.extents.len() - seg_ids.len() + 1);
        extents.extend_from_slice(&self.extents[..start]);
        extents.push(merged);
        extents.extend_from_slice(&self.extents[start + seg_ids.len()..]);
        // A listed row the merged span now holds leaves the lists.
        let mut listed = croaring::Bitmap::new();
        for extent in &extents {
            listed.add_many(&extent.below.iter().map(|&(e, _)| e).collect::<Vec<_>>());
        }
        Some(RowSpace {
            base: Arc::clone(&self.base),
            base_inverse: self.base_inverse.clone(),
            // Row-count preserving, but not extent-preserving: a merge replaces a run of extents
            // with one, so the tail this derived is stale even though its length is not.
            extent_inverse: std::sync::OnceLock::new(),
            base_rows: self.base_rows,
            extents,
            extent_entities,
            listed: Arc::new(listed),
            total_rows: self.total_rows,
        })
    }

    /// The members of `set` holding a row in the base or in an extent up to and including the one
    /// whose `seg_id` is `through`; the base alone where `through` is `None`.
    ///
    /// `through` is the last extent a caller's rows were projected over, so the answer is the
    /// entities of those rows and not of rows a flush has added since. Segment ids are never
    /// reused. `None` where no extent here has that id: a merge has collapsed it, and which of the
    /// merged extent's entities the caller's rows cover is not known here. `O(containers of set)`
    /// per part.
    pub fn restrict_to_view(
        &self,
        set: &croaring::Bitmap,
        through: Option<&str>,
    ) -> Option<croaring::Bitmap> {
        let covered = match through {
            None => 0,
            Some(seg_id) => self.extents.iter().position(|e| e.seg_id == seg_id)? + 1,
        };
        let mut out = set.and(self.base.present_entities());
        for entities in &self.extent_entities[..covered] {
            out.or_inplace(&set.and(entities));
        }
        Some(out)
    }

    /// The row-to-entity tables of this row space, or `None` where the base published no
    /// `row-entity.u32` and so cannot be inverted.
    pub fn row_entities(&self) -> Option<RowEntities<'_>> {
        let base: &[u32] = match &self.base_inverse {
            Some(table) if table.row_count() >= self.base_rows => table.slice(0..self.base_rows),
            None if self.base_rows == 0 => &[],
            _ => return None,
        };
        let tail: &[u32] = if self.total_rows > u64::from(self.base_rows) {
            self.extent_inverse
                .get_or_init(|| self.build_extent_inverse())
                .as_slice()
        } else {
            &[]
        };
        Some(RowEntities { base, tail })
    }

    /// The entities of `rows` grouped by their high 16 bits, the Roaring container key, keeping
    /// the low 16; `None` where the base published no `row-entity.u32` and so cannot be inverted.
    /// Rows at or past [`Self::total_rows`] have no entity and contribute nothing.
    ///
    /// The rows are cut into pieces of equal cardinality on the current rayon pool, and each
    /// piece reads its entities a run at a time as slices of the row-to-entity tables, twice: once
    /// to count each key and once to place each entity in its key's bucket. Two bytes per row are
    /// held, and nothing is sorted. The entities of a view's rows are distinct.
    pub fn entity_buckets(&self, rows: &croaring::Bitmap) -> Option<EntityBuckets> {
        use rayon::prelude::*;
        let tables = self.row_entities()?;
        // Every entity with a row here is below the floor, so its key is below this.
        let keys_below = (self.entity_floor().min(1 << 32) >> 16) as usize + 1;
        let pieces = mosaica_roaring::split_by_cardinality(
            rows,
            0..tables.len(),
            rayon::current_num_threads() * 4,
        );
        let pieces: Vec<Buckets16> = pieces
            .par_iter()
            .map(|piece| Buckets16::of(rows, piece.clone(), tables, keys_below))
            .collect();
        let mut keys: Vec<u16> = pieces
            .iter()
            .flat_map(|b| b.keys.iter().map(|&(key, _)| key))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        Some(EntityBuckets { pieces, keys })
    }

    /// The entities of `rows`, or `None` where the base published no `row-entity.u32` and so
    /// cannot be inverted. Rows at or past [`Self::total_rows`] have no entity and contribute
    /// nothing.
    ///
    /// Built from [`Self::entity_buckets`] on the current rayon pool, one container per key: a
    /// bitset stamped directly from the key's buckets, or a sorted array where it holds
    /// [`mosaica_roaring::ARRAY_MAX`] members or fewer.
    pub fn entities_of_rows(&self, rows: &croaring::Bitmap) -> Option<croaring::Bitmap> {
        use rayon::prelude::*;
        let buckets = self.entity_buckets(rows)?;
        let keys = buckets.keys();
        let groups = (rayon::current_num_threads() * 4).clamp(1, keys.len().max(1));
        let parts: Vec<croaring::Bitmap> = keys
            .par_chunks(keys.len().div_ceil(groups).max(1))
            .map(|keys| {
                let mut sink = mosaica_roaring::Sink::new();
                let mut words = [0u64; mosaica_roaring::WORDS];
                let mut members: Vec<u32> = Vec::new();
                for &key in keys {
                    let card = buckets.count(key);
                    if card > mosaica_roaring::ARRAY_MAX {
                        words.fill(0);
                        buckets.for_each_bucket(key, |lows| {
                            for &low in lows {
                                words[usize::from(low >> 6)] |= 1 << (low & 63);
                            }
                        });
                        debug_assert_eq!(
                            words.iter().map(|w| w.count_ones()).sum::<u32>(),
                            card,
                            "the entities of a view's rows are distinct"
                        );
                        sink.push_block(key, card, &words);
                    } else {
                        members.clear();
                        buckets.for_each_bucket(key, |lows| {
                            members.extend(lows.iter().map(|&low| u32::from(low)));
                        });
                        members.sort_unstable();
                        debug_assert!(
                            members.windows(2).all(|pair| pair[0] < pair[1]),
                            "the entities of a view's rows are distinct"
                        );
                        sink.push_members(key, &members);
                    }
                }
                sink.finish()
            })
            .collect();
        let refs: Vec<&croaring::Bitmap> = parts.iter().collect();
        Some(croaring::Bitmap::fast_or(&refs))
    }

    /// How many rows the base permutation covers — the boundary below which no flush and no merge
    /// moves a row, which is what makes an extents-only re-projection exact
    /// (`mosaica_engine::projection::RowProjection::rebase_extents`).
    pub fn base_rows(&self) -> u32 {
        self.base_rows
    }

    /// Row ID currently occupied by `e`, or `None` if it has none: the base lookup below the
    /// build bound, otherwise a binary search over the extents' dense spans, `O(log k)`, and
    /// where neither holds it and it is listed, the extent that lists it.
    pub fn row_of(&self, e: EntityId) -> Option<RowId> {
        let raw = e.raw();
        let found = if raw < self.base.bound() {
            self.base.row_of(e)
        } else {
            self.extents
                .partition_point(|extent| extent.entity_lo <= raw)
                .checked_sub(1)
                .and_then(|i| self.extents[i].dense_row_of(raw))
        };
        found.or_else(|| {
            let listed = u32::try_from(raw).is_ok_and(|e| self.listed.contains(e));
            listed.then(|| {
                self.extents
                    .iter()
                    .find_map(|extent| extent.below_row_of(raw))
            })?
        })
    }

    /// Project an entity-space bitmap into this view's row space: the base projection unioned
    /// with each extent's own. The parts are disjoint — an extent's rows lie at or above
    /// `row_base`, which is where every earlier part ended — so the union is exact rather than
    /// merely a superset, and that is what lets a flush patch a cached projection instead of
    /// rebuilding it.
    ///
    /// Inherits [`Permutation::project`]'s cost note in full: never call this on a per-viewport
    /// path.
    pub fn project(&self, mask: &croaring::Bitmap) -> croaring::Bitmap {
        self.project_with(mask, &mut ProjectScratch::default())
    }

    /// [`Self::project`], reusing a caller's scratch buffers ([`Permutation::project_with`]).
    pub fn project_with(
        &self,
        mask: &croaring::Bitmap,
        scratch: &mut ProjectScratch,
    ) -> croaring::Bitmap {
        let mut rows = self.base.project_with(mask, scratch);
        rows.or_inplace(&self.project_extents_from(mask, 0));
        rows
    }

    /// Project into the **base** rows alone, ignoring every extent above them.
    ///
    /// **What the fold writes.** A durable derived structure — a row-major column, a tile
    /// index's extents — is written over the rows the fold folded and describes nothing above them,
    /// so this is the projection that produces one and the projection a reader must compare it
    /// against.
    ///
    /// **Not the artifact row forms' projection any more.** A form covers the whole row space and
    /// is extended by each flush in place (`mosaica_engine::artifacts`); what base-only bought was
    /// that a form survived a flush and a merge untouched, and what it cost was that a member
    /// ingested since the last fold contributed nothing to its artifact's masked count — fail-closed
    /// and hours wide under the nightly compaction gate. A generating set is projected through the whole
    /// row space too.
    pub fn project_base(&self, mask: &croaring::Bitmap) -> croaring::Bitmap {
        self.base.project(mask)
    }

    /// [`Self::project_base`] by the complement route: the base's contribution derived from the
    /// entities outside `mask` ([`Permutation::project_complement_with`]). `None` where that route
    /// has no answer, and then the caller projects as it otherwise would.
    ///
    /// **The extents are not in this**, as they are not in [`Self::project_base`]. A caller
    /// building a whole-view projection adds `project_extents_from(mask, 0)` above it, which is the
    /// term [`Self::project`] adds above the base's own contribution by any route.
    pub fn project_complement_base(&self, mask: &croaring::Bitmap) -> Option<croaring::Bitmap> {
        self.base
            .project_complement_with(mask, &mut ProjectScratch::default())
    }

    /// [`Self::project_base`], reusing a caller's scratch — see [`Permutation::project_with`].
    pub fn project_base_with(
        &self,
        mask: &croaring::Bitmap,
        scratch: &mut ProjectScratch,
    ) -> croaring::Bitmap {
        self.base.project_with(mask, scratch)
    }

    /// The rows contributed by the extents at or after `from` — the only part a flush recomputes.
    pub fn project_extents_from(&self, mask: &croaring::Bitmap, from: usize) -> croaring::Bitmap {
        let extents = &self.extents[from.min(self.extents.len())..];
        // An extent maps only the entities it holds, so a mask whose highest entity is below the
        // lowest of them projects to nothing at all. One `maximum` against an iterator, a `Vec`
        // and a bitmap per extent: a stored level's held form asks this of every artifact it
        // holds at every flush, and on a corpus whose artifacts are hundreds of thousands of admin
        // divisions almost every one of them has nothing in the extent a flush just wrote.
        let floor = extents.iter().filter_map(SegmentExtent::lowest).min();
        if let (Some(floor), Some(highest)) = (floor, mask.maximum()) {
            if u64::from(highest) < floor {
                return croaring::Bitmap::new();
            }
        }
        let mut rows = croaring::Bitmap::new();
        for extent in extents {
            rows.or_inplace(&extent.project(mask));
        }
        rows
    }

    /// The rows contributed by the one extent at `index` — what a merge's publication re-projects
    /// into a held row form for the span it renumbered (`mosaica_engine::artifacts`). Empty where
    /// `index` names no extent.
    pub fn project_extent(&self, mask: &croaring::Bitmap, index: usize) -> croaring::Bitmap {
        match self.extents.get(index) {
            Some(extent) => extent.project(mask),
            None => croaring::Bitmap::new(),
        }
    }

    pub fn extent_count(&self) -> usize {
        self.extents.len()
    }

    pub fn extents(&self) -> &[SegmentExtent] {
        &self.extents
    }

    /// Every row this view holds, across the base and every extent.
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// The base permutation, for the callers that legitimately need the built segment alone —
    /// `mosaica build`'s own verification, and the sharing assertion incremental generation
    /// construction rests on.
    pub fn base(&self) -> &Arc<Permutation> {
        &self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mosaica_types::EntityId;
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};

    /// A permutation over `bound` entity slots in which each entity holds a row with probability
    /// `density`, in an order the seed decides — written through the real writer and read back
    /// through the real mmap path, so the slots the walk reads are the slots a bundle holds.
    fn fixture(dir: &Path, bound: u64, density: f64, rng: &mut StdRng) -> Permutation {
        let mut entities: Vec<EntityId> = (0..bound)
            .filter(|_| rng.gen_bool(density))
            .map(EntityId::new)
            .collect();
        entities.shuffle(rng);
        let path = dir.join("permutation.bin");
        crate::write::write_permutation(&path, &entities, bound).expect("write_permutation");
        Permutation::load(&path).expect("load permutation")
    }

    /// A base over `bound` entities at `density`, its `row-entity.u32`, and two extents: the first
    /// holding ids above the bound and one listed entity below it that the base has no row for,
    /// the second above the first. Returns the row space and each extent in the order it joined.
    fn row_space_fixture(
        dir: &Path,
        bound: u64,
        density: f64,
        rng: &mut StdRng,
    ) -> (RowSpace, Vec<SegmentExtent>) {
        let mut order: Vec<u32> = (0..bound as u32)
            .filter(|_| rng.gen_bool(density))
            .collect();
        order.shuffle(rng);
        let entities: Vec<EntityId> = order.iter().map(|&e| EntityId::new(u64::from(e))).collect();
        let perm_path = dir.join("permutation.bin");
        crate::write::write_permutation(&perm_path, &entities, bound).expect("write_permutation");
        let table_path = dir.join(crate::row_entity::ROW_ENTITY_FILE);
        crate::row_entity::write_row_entity(&table_path, &order).expect("write_row_entity");
        let base_rows = order.len() as u32;
        let space = RowSpace::new(
            Arc::new(Permutation::load(&perm_path).expect("load")),
            base_rows,
        )
        .with_row_entity(Arc::new(
            crate::row_entity::RowToEntity::load(&table_path).expect("load table"),
        ));
        let absent = (0..bound)
            .find(|&e| space.row_of(EntityId::new(e)).is_none())
            .expect("the base leaves some entity without a row");

        let mut first: Vec<u64> = (bound..bound + 300).filter(|_| rng.gen_bool(0.8)).collect();
        first.push(absent);
        first.shuffle(rng);
        let span = SegmentExtent::flush_span(space.entity_floor(), first.iter().copied());
        let one = SegmentExtent::from_rows("s1", base_rows, span, first).expect("extent one");
        let space = space.with_extent(one.clone()).expect("extent one joins");

        let floor = space.entity_floor();
        let mut second: Vec<u64> = (floor..floor + 200).filter(|_| rng.gen_bool(0.5)).collect();
        second.shuffle(rng);
        let span = SegmentExtent::flush_span(floor, second.iter().copied());
        let row_base = space.total_rows() as u32;
        let two = SegmentExtent::from_rows("s2", row_base, span, second).expect("extent two");
        let space = space.with_extent(two.clone()).expect("extent two joins");
        (space, vec![one, two])
    }

    /// `present_entities` holds exactly the entities `row_of` finds a row for, whether it was
    /// filled by `validate_rows`, on first use, or from a declared row count.
    #[test]
    fn present_entities_are_the_entities_with_a_row() {
        const BOUND: u64 = (3 << PAGE_SHIFT) + 1_000;
        for (seed, density) in [(1u64, 0.6), (2, 1.0)] {
            let dir = tempfile::tempdir().expect("tempdir");
            let mut rng = StdRng::seed_from_u64(seed);
            let perm = fixture(dir.path(), BOUND, density, &mut rng);
            let want: croaring::Bitmap = (0..BOUND)
                .filter(|&e| perm.row_of(EntityId::new(e)).is_some())
                .map(|e| e as u32)
                .collect();
            assert_eq!(
                perm.present_entities(),
                &want,
                "density {density}, on first use"
            );

            let validated = Permutation::load(&dir.path().join("permutation.bin")).expect("load");
            validated
                .validate_rows(want.cardinality() as u32)
                .expect("the fixture is a bijection");
            assert_eq!(
                validated.present_entities(),
                &want,
                "density {density}, validated"
            );

            let declared = Permutation::load(&dir.path().join("permutation.bin")).expect("load");
            declared.declare_dense_rows(want.cardinality() as u32);
            assert_eq!(
                declared.present_entities(),
                &want,
                "density {density}, declared"
            );
        }
    }

    /// `restrict_to_view(set, through)` keeps the members of `set` with a row in the base or in the
    /// extents up to `through`, and nothing else; after a merge it keeps them up to the merged
    /// extent, and it has no answer for an extent the merge collapsed.
    #[test]
    fn restricting_to_a_view_keeps_the_entities_with_a_row_in_the_covered_extents() {
        const BOUND: u64 = (2 << PAGE_SHIFT) + 500;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(3);
        let (space, extents) = row_space_fixture(dir.path(), BOUND, 0.7, &mut rng);
        let top = space.entity_floor() + 50;
        let set: croaring::Bitmap = (0..top)
            .filter(|_| rng.gen_bool(0.5))
            .map(|e| e as u32)
            .collect();
        let want = |covered: &[SegmentExtent]| -> croaring::Bitmap {
            set.iter()
                .filter(|&e| {
                    let e = u64::from(e);
                    space.base().row_of(EntityId::new(e)).is_some()
                        || covered
                            .iter()
                            .any(|extent| extent.pairs().any(|(held, _)| held == e))
                })
                .collect()
        };
        assert_eq!(
            space.restrict_to_view(&set, None),
            Some(want(&[])),
            "the base"
        );
        for (i, extent) in extents.iter().enumerate() {
            assert_eq!(
                space.restrict_to_view(&set, Some(&extent.seg_id)),
                Some(want(&extents[..=i])),
                "through {}",
                extent.seg_id
            );
        }
        assert_eq!(space.restrict_to_view(&set, Some("no-such-segment")), None);

        // A merge of both extents into one: rows projected through the first no longer say which
        // of the merged extent's entities they hold.
        let mut by_row: Vec<(u32, u64)> = extents
            .iter()
            .flat_map(|extent| {
                extent
                    .pairs()
                    .map(move |(entity, row)| (extent.row_base + row, entity))
            })
            .collect();
        by_row.sort_unstable();
        let merged = SegmentExtent::from_rows(
            "m",
            extents[0].row_base,
            (extents[0].entity_lo, extents[1].entity_hi),
            by_row.into_iter().map(|(_, entity)| entity),
        )
        .expect("the merged extent is well formed");
        let seg_ids: Vec<String> = extents.iter().map(|e| e.seg_id.clone()).collect();
        let merged_space = space
            .collapsing(&seg_ids, merged)
            .expect("the merge applies");
        assert_eq!(
            merged_space.restrict_to_view(&set, Some(&extents[0].seg_id)),
            None
        );
        assert_eq!(
            merged_space.restrict_to_view(&set, Some(&extents[1].seg_id)),
            None
        );
        assert_eq!(
            merged_space.restrict_to_view(&set, Some("m")),
            Some(want(&extents))
        );
        assert_eq!(merged_space.restrict_to_view(&set, None), Some(want(&[])));
    }

    /// `entities_of_rows` is `entity_of` over every row of the set: across the base and the
    /// extents, over sets dense, scattered and in runs, and ignoring rows past row space. One
    /// key holding exactly 4,096 entities is an array container and one holding 4,097 a bitset,
    /// and both come back whole.
    #[test]
    fn the_entities_of_rows_are_each_rows_entity() {
        const BOUND: u64 = (1 << 20) + (3 << PAGE_SHIFT);
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(4);
        let (space, _) = row_space_fixture(dir.path(), BOUND, 0.98, &mut rng);
        let total = space.total_rows() as u32;
        let base = space.base_rows();
        let mut sets = vec![
            croaring::Bitmap::new(),
            croaring::Bitmap::from_range(0..total),
            croaring::Bitmap::from_range(base - 10..total + 10),
        ];
        let rows_of_key_zero: Vec<u32> = (0..total)
            .filter(|&row| {
                space
                    .entity_of(RowId::new(row))
                    .is_some_and(|e| e.raw() < 1 << 16)
            })
            .collect();
        for members in [4_096usize, 4_097] {
            sets.push(croaring::Bitmap::of(&rows_of_key_zero[..members]));
        }
        sets.push((0..total + 5).filter(|_| rng.gen_bool(0.01)).collect());
        let mut runs = croaring::Bitmap::new();
        for _ in 0..40 {
            let start = rng.gen_range(0..total);
            runs.add_range(start..(start + rng.gen_range(1..5_000)).min(total));
        }
        sets.push(runs);
        for rows in &sets {
            let want: croaring::Bitmap = rows
                .iter()
                .filter_map(|row| space.entity_of(RowId::new(row)))
                .map(|e| e.raw() as u32)
                .collect();
            assert_eq!(
                space.entities_of_rows(rows),
                Some(want),
                "{} rows",
                rows.cardinality()
            );
        }
    }

    /// Without a base `row-entity.u32` the crossing has no answer, and says so.
    #[test]
    fn the_entities_of_rows_need_the_base_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(5);
        let perm = fixture(dir.path(), 1_000, 0.5, &mut rng);
        let rows = perm.present_entities().cardinality() as u32;
        let space = RowSpace::new(Arc::new(perm), rows);
        assert_eq!(
            space.entities_of_rows(&croaring::Bitmap::from_range(0..rows)),
            None
        );
    }

    /// `count` entities drawn from `[0, bound)`, plus a handful above it — which a real mask holds
    /// whenever a flush has issued ids the base does not cover, and which the walk skips.
    fn mask_of(bound: u64, count: usize, rng: &mut StdRng) -> croaring::Bitmap {
        let mut mask = croaring::Bitmap::new();
        for _ in 0..count {
            mask.add(rng.gen_range(0..bound) as u32);
        }
        for above in 0..8u64 {
            mask.add((bound + above).min(u64::from(u32::MAX)) as u32);
        }
        mask
    }

    /// Windowing changes when containers are emitted, not which rows reach them. Several seeds
    /// rather than one fixture, because the property is over permutations and masks and a single
    /// pair of them checks one point of that space.
    #[test]
    fn a_windowed_pass_projects_what_a_single_pass_projects() {
        // Two buckets, the second a sliver, so the union runs across a bucket boundary; 5% of the
        // entities hold no row, so the walk skips sentinels as well as the ids above `bound` the
        // mask carries.
        const BOUND: u64 = (1 << 22) + 600_000;
        for seed in 0..3u64 {
            let dir = tempfile::tempdir().expect("tempdir");
            let mut rng = StdRng::seed_from_u64(seed);
            let perm = fixture(dir.path(), BOUND, 0.95, &mut rng);
            for count in [5_000usize, 1_000_000] {
                let mask = mask_of(BOUND, count, &mut rng);
                let mut scratch = ProjectScratch::default();
                let single = perm.project_windowed(&mask, &mut scratch, usize::MAX);
                for window in [1usize, 997, 100_000] {
                    let windowed = perm.project_windowed(&mask, &mut scratch, window);
                    assert_eq!(
                        windowed.iter().collect::<Vec<u32>>(),
                        single.iter().collect::<Vec<u32>>(),
                        "seed {seed}, {count} entities, window {window}: a windowed pass must \
                         project exactly what a single pass projects"
                    );
                }
            }
        }
    }

    /// The buckets hold one round and not the whole result, which is the bound the rounds exist
    /// for. Measured on each worker's pool rather than on process memory: a window's worth of
    /// `u32`s plus the page that overshot it and a partial chunk per bucket, and nothing
    /// proportional to the mask.
    #[test]
    fn the_buckets_never_hold_more_than_a_window() {
        const BOUND: u64 = (1 << 22) + 100_000;
        const WINDOW: usize = 10_000;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(7);
        let perm = fixture(dir.path(), BOUND, 1.0, &mut rng);
        let mask = croaring::Bitmap::from_range(0..BOUND as u32);
        let mut scratch = ProjectScratch::default();
        let rows = perm.project_windowed(&mask, &mut scratch, WINDOW);
        assert_eq!(rows.cardinality(), BOUND, "every entity holds a row here");
        for worker in &scratch.workers {
            let held = worker.pool.len();
            let ceiling = WINDOW + PAGE_ENTRIES + (CHUNK_ROWS_MAX + 1) * (worker.chains.len() + 1);
            assert!(
                held <= ceiling,
                "a worker's buckets kept {held} slots against a ceiling of {ceiling} for a \
                 {WINDOW}-row window over {BOUND} rows"
            );
        }
    }

    /// **The case the pool exists for:** rows arriving in order, so each window lands in one
    /// bucket and the next window in the next. A `Vec` per bucket kept each bucket's largest share,
    /// which summed to the whole of row space over enough windows; the pool is sized once, before
    /// the walk, and a walk that needed more would have panicked taking a chunk. Checked against a
    /// single pass, and the reused scratch against a fresh one, for masks both dense and sparse in
    /// each bucket.
    #[test]
    fn rows_arriving_bucket_by_bucket_stay_within_one_window() {
        const BUCKETS: u64 = 4;
        const BOUND: u64 = BUCKETS << BUCKET_SHIFT;
        const WINDOW: usize = 50_000;
        let dir = tempfile::tempdir().expect("tempdir");
        // Entity e holds row e: row order is entity order, the most clustered a walk can see.
        let entities: Vec<EntityId> = (0..BOUND).map(EntityId::new).collect();
        let path = dir.path().join("permutation.bin");
        crate::write::write_permutation(&path, &entities, BOUND).expect("write_permutation");
        let perm = Permutation::load(&path).expect("load permutation");
        let mut rng = StdRng::seed_from_u64(11);
        let mut scratch = ProjectScratch::default();
        for density in [1.0, 0.5, 0.01] {
            let mask: croaring::Bitmap = (0..BOUND as u32)
                .filter(|_| density == 1.0 || rng.gen_bool(density))
                .collect();
            let single = perm.project_windowed(&mask, &mut ProjectScratch::default(), usize::MAX);
            assert_eq!(single, mask, "row e is entity e's row");
            let windowed = perm.project_windowed(&mask, &mut scratch, WINDOW);
            assert_eq!(
                windowed, single,
                "density {density}: a windowed pass over clustered rows projects what a single \
                 pass projects"
            );
            let ceiling = WINDOW + PAGE_ENTRIES + CHUNK_ROWS_MAX * (BUCKETS as usize + 2);
            for worker in &scratch.workers {
                assert!(
                    worker.pool.len() <= ceiling,
                    "density {density}: a pool holds {} rows against a ceiling of {ceiling}, \
                     where the mask projects to {} rows",
                    worker.pool.len(),
                    single.cardinality()
                );
            }
        }
    }

    /// **The pool's worst case, driven directly.** Every bucket stops one row into a fresh chunk,
    /// `k·c + 1` rows, which is the layout that uses the most chunks for the rows buffered. A pool
    /// of exactly that many chunks must be drained to zero without the `expect` in
    /// `push_into_new_chunk` firing, each bucket's chunks must read back as the rows pushed into it
    /// in order, and the sizing `Worker::prepare` does for the same rows must be at least that
    /// pool.
    #[test]
    fn the_worst_chunk_layout_drains_the_pool_exactly() {
        const NBUCKETS: usize = 37;
        const CHUNK: usize = 4;
        const K: usize = 3;
        let per_bucket = K * CHUNK + 1;
        let chunks = NBUCKETS * (K + 1);
        let mut buckets = Worker {
            pool: vec![0u32; chunks * CHUNK],
            free: (0..chunks as u32).rev().collect(),
            chains: vec![Vec::new(); NBUCKETS],
            cursors: vec![Cursor::default(); NBUCKETS],
            chunk_rows: CHUNK,
            page: Vec::new(),
        };
        let mut pushed = vec![Vec::new(); NBUCKETS];
        // Interleaved across buckets, so each bucket's chunks are scattered through the pool
        // rather than contiguous, which is what `slices` has to read back in order.
        let mut row = 0u32;
        for _ in 0..per_bucket {
            for (bucket, rows) in pushed.iter_mut().enumerate() {
                buckets.push(bucket, row);
                rows.push(row);
                row += 1;
            }
        }
        assert_eq!(
            buckets.free.len(),
            0,
            "the worst layout uses every chunk of an exact pool"
        );
        for (bucket, rows) in pushed.iter().enumerate() {
            assert_eq!(buckets.len(bucket), per_bucket);
            let read: Vec<u32> = buckets.slices(bucket).flatten().copied().collect();
            assert_eq!(
                &read, rows,
                "bucket {bucket} reads back what was pushed, in order"
            );
        }
        // The sizing rule in `Worker::prepare`, applied to the same buffered rows, and what
        // preparing a worker that has buffered rows leaves of them.
        let buffered = NBUCKETS * per_bucket;
        buckets.prepare(NBUCKETS, buffered, CHUNK);
        assert!(
            buckets.free.len() >= chunks,
            "prepare sizes at least the exact pool"
        );
        for bucket in 0..NBUCKETS {
            assert_eq!(buckets.len(bucket), 0, "prepare empties every bucket");
        }
    }

    /// One scratch carried across permutations with different bucket counts, and a mask whose
    /// cardinality is exactly the window, each checked against a single pass through a fresh
    /// scratch.
    #[test]
    fn a_reused_scratch_and_an_exact_window_project_what_a_single_pass_projects() {
        let mut scratch = ProjectScratch::default();
        let mut rng = StdRng::seed_from_u64(23);
        // Five buckets, then two, then five again: the chains and cursors shrink and regrow, and
        // the pool is reused at a different chunk size.
        for (index, bound) in [
            (5u64 << BUCKET_SHIFT) - 1_000,
            (1u64 << BUCKET_SHIFT) + 7,
            5 << BUCKET_SHIFT,
        ]
        .into_iter()
        .enumerate()
        {
            let dir = tempfile::tempdir().expect("tempdir");
            let perm = fixture(dir.path(), bound, 0.9, &mut rng);
            for count in [300usize, 200_000] {
                let mask = mask_of(bound, count, &mut rng);
                let single =
                    perm.project_windowed(&mask, &mut ProjectScratch::default(), usize::MAX);
                for window in [5_000usize, usize::MAX] {
                    let reused = perm.project_windowed(&mask, &mut scratch, window);
                    assert_eq!(
                        reused, single,
                        "permutation {index}, {count} entities, window {window}: a reused scratch \
                         projects what a fresh single pass projects"
                    );
                }
            }
            // Cardinality exactly the window, every entity in bound.
            const WINDOW: usize = 65_536;
            let mut exact = croaring::Bitmap::new();
            while exact.cardinality() < WINDOW as u64 {
                exact.add(rng.gen_range(0..bound) as u32);
            }
            let single = perm.project_windowed(&exact, &mut ProjectScratch::default(), usize::MAX);
            let windowed = perm.project_windowed(&exact, &mut scratch, WINDOW);
            assert_eq!(
                windowed, single,
                "permutation {index}: a mask of exactly one window"
            );
        }
    }

    /// A mask over the whole of `[0, bound)` is answered as the row range, and that answer is the
    /// one the walk produces from the same mask.
    #[test]
    fn a_whole_domain_mask_is_the_row_range() {
        const BOUND: u64 = 200_000;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(11);
        let perm = fixture(dir.path(), BOUND, 0.6, &mut rng);
        let rows = u32::try_from(
            (0..BOUND)
                .filter(|&e| perm.row_of(EntityId::new(e)).is_some())
                .count(),
        )
        .expect("fixture rows fit u32");
        perm.validate_rows(rows)
            .expect("the fixture is a bijection");
        assert_eq!(perm.dense_rows.get(), Some(&rows));

        let mask = croaring::Bitmap::from_range(0..BOUND as u32);
        let mut scratch = ProjectScratch::default();
        let walked = perm.project_windowed(&mask, &mut scratch, usize::MAX);
        let answered = perm.project(&mask);
        assert_eq!(
            answered.iter().collect::<Vec<u32>>(),
            walked.iter().collect::<Vec<u32>>(),
            "the whole-domain answer must be the set the walk produces"
        );
        assert_eq!(answered, croaring::Bitmap::from_range(0..rows));

        // **The shape a flush leaves**: the grant covers this base's whole domain and also holds
        // entities above `bound`, which a later segment issued and this mapping knows nothing
        // about. Those are skipped, here as everywhere, so the base's answer is still the row
        // range — and this is the case a domain test written as "the mask is exactly `[0, bound)`"
        // would send down the walk for no reason.
        let mut with_flushed = mask.clone();
        for above in 0..64u64 {
            with_flushed.add((BOUND + above) as u32);
        }
        assert!(perm.covers_domain(&with_flushed));
        assert_eq!(
            perm.project(&with_flushed).iter().collect::<Vec<u32>>(),
            perm.project_windowed(&with_flushed, &mut scratch, usize::MAX)
                .iter()
                .collect::<Vec<u32>>(),
            "entities above `bound` have no row here and must not change either route's answer"
        );
        assert_eq!(
            perm.project(&with_flushed),
            croaring::Bitmap::from_range(0..rows)
        );

        // One entity short of the whole domain is not the whole domain, and takes the walk.
        let mut short = mask.clone();
        short.remove(rng.gen_range(0..BOUND) as u32);
        assert!(!perm.covers_domain(&short));
        assert_eq!(
            perm.project(&short).iter().collect::<Vec<u32>>(),
            perm.project_windowed(&short, &mut scratch, usize::MAX)
                .iter()
                .collect::<Vec<u32>>()
        );
    }

    /// The writer's word does what the scan's does, and the first word recorded is the one kept.
    ///
    /// `read::open_written_prefix` is the caller — a compaction opening a prefix this process just
    /// wrote, where the scan is skipped and its result is declared instead.
    #[test]
    fn a_declared_row_count_answers_a_whole_domain_mask() {
        const BOUND: u64 = 100_000;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(17);
        let perm = fixture(dir.path(), BOUND, 1.0, &mut rng);
        let mask = croaring::Bitmap::from_range(0..BOUND as u32);
        let mut scratch = ProjectScratch::default();
        let walked = perm.project_windowed(&mask, &mut scratch, usize::MAX);

        assert_eq!(perm.dense_rows.get(), None, "nothing has spoken yet");
        perm.declare_dense_rows(BOUND as u32);
        assert_eq!(perm.dense_rows.get(), Some(&(BOUND as u32)));
        assert_eq!(
            perm.project(&mask).iter().collect::<Vec<u32>>(),
            walked.iter().collect::<Vec<u32>>(),
            "a declared row count must answer what the walk answers"
        );

        // A second declaration cannot overwrite the first, so a later scan and an earlier
        // declaration cannot disagree about what this mapping is.
        perm.declare_dense_rows(1);
        assert_eq!(perm.dense_rows.get(), Some(&(BOUND as u32)));
    }

    /// A mapping that claims fewer rows than the descriptor declares is injective into
    /// `[0, row_count)` but not onto it, so the whole-domain answer must not be offered for it.
    ///
    /// **The fixture's image happens to be `[0, claimed)`, and that is not what makes the case.**
    /// What `validate_rows` can conclude from a short count is only that the image is *some* subset
    /// of `[0, row_count)` of size `claimed`; nothing in the file says which. So the count is not
    /// recorded, and the walk — which reads the image rather than assuming it — answers.
    #[test]
    fn a_mapping_that_claims_fewer_rows_than_declared_takes_the_walk() {
        const BOUND: u64 = 50_000;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(13);
        let perm = fixture(dir.path(), BOUND, 0.5, &mut rng);
        let claimed = u32::try_from(
            (0..BOUND)
                .filter(|&e| perm.row_of(EntityId::new(e)).is_some())
                .count(),
        )
        .expect("fixture rows fit u32");
        perm.validate_rows(claimed + 100)
            .expect("every slot is still in bound and distinct");
        assert_eq!(perm.dense_rows.get(), None);

        let mask = croaring::Bitmap::from_range(0..BOUND as u32);
        let mut scratch = ProjectScratch::default();
        assert_eq!(
            perm.project(&mask),
            perm.project_windowed(&mask, &mut scratch, usize::MAX)
        );
    }

    // ----------------------------------------------------------------------------------------
    // The complement route
    // ----------------------------------------------------------------------------------------

    /// Each entity in `[0, bound)` with probability `share`, and nothing above `bound`.
    fn share_of(bound: u64, share: f64, rng: &mut StdRng) -> croaring::Bitmap {
        let mut mask = croaring::Bitmap::new();
        for entity in 0..bound {
            if rng.gen_bool(share) {
                mask.add(entity as u32);
            }
        }
        mask
    }

    /// How many entities in `[0, bound)` hold a row.
    fn rows_held(perm: &Permutation, bound: u64) -> u32 {
        u32::try_from(
            (0..bound)
                .filter(|&e| perm.row_of(EntityId::new(e)).is_some())
                .count(),
        )
        .expect("fixture rows fit u32")
    }

    /// The projection as one `row_of` per entity of the mask, sorted: the definition every route
    /// is checked against.
    fn by_row_of(perm: &Permutation, mask: &croaring::Bitmap) -> Vec<u32> {
        let mut rows: Vec<u32> = mask
            .iter()
            .filter_map(|e| perm.row_of(EntityId::new(u64::from(e))))
            .map(RowId::raw)
            .collect();
        rows.sort_unstable();
        rows
    }

    /// The complement route answers what the single-pass walk, a windowed walk and one `row_of`
    /// per entity answer. One scratch across all of them, so each route leaves it as the next
    /// expects.
    fn assert_the_complement_agrees(perm: &Permutation, mask: &croaring::Bitmap, what: &str) {
        let mut scratch = ProjectScratch::default();
        let complement = perm
            .project_complement_with(mask, &mut scratch)
            .unwrap_or_else(|| panic!("{what}: the complement route has an answer here"))
            .iter()
            .collect::<Vec<u32>>();
        let single = perm
            .project_windowed(mask, &mut scratch, usize::MAX)
            .iter()
            .collect::<Vec<u32>>();
        let windowed = perm
            .project_windowed(mask, &mut scratch, 997)
            .iter()
            .collect::<Vec<u32>>();
        assert_eq!(
            complement, single,
            "{what}: the complement against the single pass"
        );
        assert_eq!(
            complement, windowed,
            "{what}: the complement against a windowed pass"
        );
        assert_eq!(
            complement,
            by_row_of(perm, mask),
            "{what}: the complement against row_of"
        );
    }

    /// A mask over most of a bijection is answered through its complement, and that answer is the
    /// one the walk produces from the mask itself.
    #[test]
    fn a_mask_over_a_bijection_is_answered_through_its_complement() {
        const BOUND: u64 = 6_000;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(19);
        let perm = fixture(dir.path(), BOUND, 1.0, &mut rng);
        perm.validate_rows(BOUND as u32)
            .expect("the fixture is a bijection onto [0, bound)");
        assert_eq!(perm.dense_rows.get(), Some(&(BOUND as u32)));

        let mask = share_of(BOUND, 0.9, &mut rng);
        assert!(!perm.covers_domain(&mask));
        assert_the_complement_agrees(&perm, &mask, "a 90% mask over a bijection");

        // The whole domain, where the complement is empty and nothing is subtracted.
        let whole = croaring::Bitmap::from_range(0..BOUND as u32);
        assert_eq!(
            perm.project_complement_with(&whole, &mut ProjectScratch::default()),
            Some(croaring::Bitmap::from_range(0..BOUND as u32)),
            "a whole-domain mask reaches every row by either route"
        );
        assert_the_complement_agrees(&perm, &whole, "a whole-domain mask");
    }

    /// Slack between `bound` and the row count: entities with no row, and a whole page with none,
    /// sit in the complement and project to nothing, so the subtraction still lands on the rows
    /// the mask reaches.
    #[test]
    fn the_complement_route_holds_with_absent_slots_and_an_absent_page() {
        const BOUND: u64 = 3 * PAGE_ENTRIES as u64 - 1_000;
        let absent_page = (PAGE_ENTRIES as u64)..(2 * PAGE_ENTRIES as u64);
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(23);
        let mut entities: Vec<EntityId> = (0..BOUND)
            .filter(|e| !absent_page.contains(e))
            .filter(|_| rng.gen_bool(0.7))
            .map(EntityId::new)
            .collect();
        entities.shuffle(&mut rng);
        let path = dir.path().join("permutation.bin");
        crate::write::write_permutation(&path, &entities, BOUND).expect("write_permutation");
        let perm = Permutation::load(&path).expect("load permutation");
        assert_eq!(perm.present_pages(), 2, "the middle page is absent");

        let rows = rows_held(&perm, BOUND);
        assert!(u64::from(rows) < BOUND, "the mapping has slack");
        perm.validate_rows(rows)
            .expect("the fixture is a bijection onto [0, rows)");
        assert_eq!(perm.dense_rows.get(), Some(&rows));

        let mask = share_of(BOUND, 0.75, &mut rng);
        assert_the_complement_agrees(&perm, &mask, "a 75% mask over a mapping with slack");
    }

    /// Entities at or above `bound` are not this permutation's. Both routes skip them, whether the
    /// mask is mostly inside the domain or mostly outside it.
    #[test]
    fn entities_above_bound_reach_no_row_by_either_route() {
        const BOUND: u64 = 8_000;
        const ABOVE: u32 = 5_000;
        let bound = BOUND as u32;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(29);
        let perm = fixture(dir.path(), BOUND, 1.0, &mut rng);
        perm.validate_rows(bound)
            .expect("the fixture is a bijection onto [0, bound)");

        // Most of the domain, plus a run of ids above `bound` that a later segment issued.
        let mut mask = share_of(BOUND, 0.8, &mut rng);
        mask.add_range(bound..bound + ABOVE);
        assert_the_complement_agrees(&perm, &mask, "an 80% mask with ids above bound");

        // A small share of the domain, with enough ids above `bound` that the mask's cardinality
        // is the larger number. The complement is still taken over `[0, bound)` alone.
        let mut mask = share_of(BOUND, 0.3, &mut rng);
        mask.add_range(bound..bound + ABOVE);
        assert!(mask.cardinality() > BOUND / 2);
        assert_the_complement_agrees(&perm, &mask, "a 30% mask padded above bound");
    }

    /// Without a recorded row count nothing says the slots are onto `[0, rows)`, so the complement
    /// route has no answer and the caller walks.
    #[test]
    fn the_complement_has_no_answer_where_no_row_count_is_recorded() {
        const BOUND: u64 = 6_000;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut rng = StdRng::seed_from_u64(31);
        let perm = fixture(dir.path(), BOUND, 0.9, &mut rng);
        assert_eq!(perm.dense_rows.get(), None, "nothing has spoken");

        let mut scratch = ProjectScratch::default();
        for (mask, what) in [
            (share_of(BOUND, 0.9, &mut rng), "a 90% mask"),
            (
                croaring::Bitmap::from_range(0..BOUND as u32),
                "a whole-domain mask",
            ),
        ] {
            assert!(
                perm.project_complement_with(&mask, &mut scratch).is_none(),
                "{what} must have no complement answer without a recorded row count"
            );
            assert_eq!(
                perm.project(&mask),
                perm.project_windowed(&mask, &mut scratch, usize::MAX),
                "{what} is answered by the walk"
            );
        }
    }

    /// **The arithmetic does not turn on how much of the domain the mask holds.** The share decides
    /// only which route costs less, and that decision is the session chooser's
    /// (`crate::term_images::choose`); here the complement must agree with the walk at every share,
    /// from an empty mask to one entity short of the domain, and at the three shares around half.
    /// An even and an odd `bound`, so nothing rests on the halving being exact.
    #[test]
    fn the_complement_agrees_with_the_walk_at_every_share() {
        for (seed, bound) in [(37u64, 4_096u64), (41, 4_097)] {
            let dir = tempfile::tempdir().expect("tempdir");
            let mut rng = StdRng::seed_from_u64(seed);
            let perm = fixture(dir.path(), bound, 1.0, &mut rng);
            perm.validate_rows(bound as u32)
                .expect("the fixture is a bijection onto [0, bound)");

            let mut ids: Vec<u32> = (0..bound as u32).collect();
            ids.shuffle(&mut rng);
            let half = (bound / 2) as usize;
            for (count, what) in [
                (0, "an empty mask"),
                (1, "one entity"),
                (half - 1, "one below half"),
                (half, "exactly half"),
                (half + 1, "one above half"),
                (bound as usize - 1, "one short of the domain"),
            ] {
                let mask = croaring::Bitmap::of(&ids[..count]);
                assert_the_complement_agrees(&perm, &mask, &format!("bound {bound}, {what}"));
            }
        }
    }

    fn pool(threads: usize) -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("a pool")
    }

    /// Masks holding each container kind: scattered arrays and bitsets, runs, a full page, and
    /// entities above the bound.
    fn masks_of(bound: u64, rng: &mut StdRng) -> Vec<(&'static str, croaring::Bitmap)> {
        let mut runs = croaring::Bitmap::new();
        let mut at = 0u32;
        while u64::from(at) < bound {
            let len = rng.gen_range(1..20_000u32);
            runs.add_range(at..(at + len).min(bound as u32));
            at += len + rng.gen_range(1..40_000u32);
        }
        runs.add_range(bound as u32..bound as u32 + 5);
        runs.run_optimize();
        vec![
            ("sparse", mask_of(bound, 300, rng)),
            ("arrays", mask_of(bound, 40_000, rng)),
            // Past `ARRAY_MAX` a container, so a walk of many rounds keeps bits.
            ("bitsets", mask_of(bound, 900_000, rng)),
            ("runs", runs),
            (
                "whole pages",
                croaring::Bitmap::from_range(65_536..3 * 65_536),
            ),
        ]
    }

    /// The walk spread over threads, in rounds of any size, projects what one thread projects in
    /// one round: over a permutation with absent slots, and over one with an absent page, for
    /// masks of every container kind. A reused scratch carries no answer between masks.
    #[test]
    fn a_walk_spread_over_threads_projects_what_one_thread_projects_in_one_round() {
        // Two buckets, the second a sliver.
        const BOUND: u64 = (1 << BUCKET_SHIFT) + 70_000;
        let mut rng = StdRng::seed_from_u64(31);
        let dense_dir = tempfile::tempdir().expect("tempdir");
        let dense = fixture(dense_dir.path(), BOUND, 0.95, &mut rng);
        // Pages 1, 2 and every fifth page hold no slots at all.
        let paged_dir = tempfile::tempdir().expect("tempdir");
        let paged = {
            let mut entities: Vec<EntityId> = (0..BOUND)
                .filter(|e| {
                    let page = e >> PAGE_SHIFT;
                    page != 1 && page != 2 && !page.is_multiple_of(5)
                })
                .map(EntityId::new)
                .collect();
            entities.shuffle(&mut rng);
            let path = paged_dir.path().join("permutation.bin");
            crate::write::write_permutation(&path, &entities, BOUND).expect("write_permutation");
            Permutation::load(&path).expect("load permutation")
        };
        assert!(
            paged.present_pages() < paged.page_count(),
            "some page is absent"
        );
        for (which, perm) in [("dense", &dense), ("paged", &paged)] {
            for (kind, mask) in masks_of(BOUND, &mut rng) {
                let want = by_row_of(perm, &mask);
                let one = perm.walk(&mask, &mut ProjectScratch::serial(), usize::MAX, usize::MAX);
                assert_eq!(
                    one.iter().collect::<Vec<u32>>(),
                    want,
                    "{which} {kind}: one round"
                );
                for threads in [1usize, 4] {
                    let mut scratch = ProjectScratch::default();
                    for window in [usize::MAX, 20_000] {
                        let spread =
                            pool(threads).install(|| perm.walk(&mask, &mut scratch, window, 0));
                        assert_eq!(
                            spread, one,
                            "{which} {kind}: {threads} threads, window {window}"
                        );
                    }
                }
            }
        }
    }

    /// The rows of `perm` as a fresh file, then with bytes changed: `entity`'s slot set to `row`.
    fn with_slot(dir: &Path, perm: &Permutation, entity: u64, row: u32) -> Permutation {
        let mut bytes = std::fs::read(perm.path()).expect("read permutation");
        let page = (entity >> PAGE_SHIFT) as usize;
        let slot = perm.directory()[page] as usize;
        let at =
            perm.payload_start + slot * PAGE_BYTES + (entity as usize & (PAGE_ENTRIES - 1)) * 4;
        bytes[at..at + 4].copy_from_slice(&row.to_le_bytes());
        let path = dir.join("changed.bin");
        std::fs::write(&path, &bytes).expect("write permutation");
        Permutation::load(&path).expect("load permutation")
    }

    /// On a pool of any size, `validate_rows` accepts a bijection and records it as one, refuses a
    /// row claimed twice from far apart in entity and row space and a row past the count, and
    /// records nothing for a mapping onto fewer rows than declared.
    #[test]
    fn validation_on_any_pool_refuses_a_row_claimed_twice_or_out_of_range() {
        const BOUND: u64 = (2 << BUCKET_SHIFT) + 70_000;
        let mut rng = StdRng::seed_from_u64(37);
        let dir = tempfile::tempdir().expect("tempdir");
        let perm = fixture(dir.path(), BOUND, 1.0, &mut rng);
        let rows = BOUND as u32;
        let first = perm.row_of(EntityId::new(3)).expect("a row").raw();
        let last = BOUND - 2;
        for threads in [1usize, 4] {
            let on =
                |perm: &Permutation, rows: u32| pool(threads).install(|| perm.validate_rows(rows));
            let fresh = Permutation::load(perm.path()).expect("load permutation");
            on(&fresh, rows).expect("a bijection validates");
            assert_eq!(
                fresh.dense_rows(),
                Some(rows),
                "{threads} threads: recorded as onto"
            );
            assert_eq!(fresh.present_entities().cardinality(), BOUND);

            let twice = with_slot(dir.path(), &perm, last, first);
            assert!(
                on(&twice, rows).is_err(),
                "{threads} threads: a row claimed twice"
            );
            let past = with_slot(dir.path(), &perm, last, rows);
            assert!(
                on(&past, rows).is_err(),
                "{threads} threads: a row past the count"
            );
            let short = with_slot(dir.path(), &perm, last, ROW_ABSENT);
            on(&short, rows).expect("a mapping onto fewer rows validates");
            assert_eq!(
                short.dense_rows(),
                None,
                "{threads} threads: not recorded as onto"
            );
            assert_eq!(short.present_entities().cardinality(), BOUND - 1);
        }
    }
}
