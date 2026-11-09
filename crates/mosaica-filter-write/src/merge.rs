//! The one merge over term-sorted sources, which writes every term index and keyword dictionary
//! that is made by merging: the build's runs and its accumulators' remainders, the flush's, and
//! the fold's and the coalesce's finished layers.
//!
//! A [`TermSource`] yields its terms ascending, each with how many entities it holds for the term
//! and then those entities ascending, each with the term's positions in it, and can start at the
//! first term at or after a key. Three kinds implement it: a spilled run ([`crate::TextRunWriter`]'s
//! file, read by [`crate::TextRunReader`], which starts at a key through the run's sparse index),
//! what an accumulator held when its pass ended (which [`crate::TermAccumulator::finish`] hands
//! back, ranked, so that it starts at a key by binary search), and a finished layer, read through
//! the one [`DictCursor`] both families share. [`TermMerge`] merges any number of them, of any
//! kind, k-way on the term, and each selected term's entities by comparing the sources' heads,
//! because two sources need not partition entity space.
//!
//! # In parallel, by term range
//!
//! A merge divides its term space into [`TermRange`]s of about equal bytes, [`split`] from keys
//! sampled from its sources ([`Sample`]), several to a thread ([`RangePlan`]). Each range is a
//! [`TermMerge`] of its own over sources started at its first key, and the ranges run on the plan's
//! threads, at most its window of them issued and not yet written ([`Ordered`]). The output is
//! written in term order by [`merge_ranges`]: the earliest range not yet written writes straight
//! into the writer, and a later one holds what it encodes, up to the plan's buffer, then waits for
//! its turn. A term is never divided between the two: one that passes the buffer while it is held
//! waits for the ranges before it and then goes on into the writer. So what a merge writes is the
//! same bytes however many ranges and threads it took.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io;
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};

use croaring::Bitmap;
use mosaica_filter::SortedDict;

use crate::ordered::{FailOnPanic, Ordered};
use crate::spill::{Allowance, Sample, RUN_BUFFER_MIN, SPILL_BUF_BYTES};

/// One term-sorted input to a [`TermMerge`].
pub trait TermSource {
    /// Move to the next term, returning `false` at the end, which a run verifies against its
    /// receipt. A source that has not been moved is before its first term.
    fn next_term(&mut self) -> io::Result<bool>;

    /// Start before the first term at or after `key`, so that [`Self::next_term`] moves to it.
    /// Called before the source has been moved.
    fn seek(&mut self, key: &[u8]) -> io::Result<()>;

    /// The term the source is at.
    fn term(&self) -> &[u8];

    /// How many entities [`Self::next_entity`] yields for the term, known before any is read.
    fn count(&self) -> u64;

    /// The term's next entity, ascending, or `None` once it has yielded [`Self::count`].
    fn next_entity(&mut self) -> io::Result<Option<u32>>;

    /// The term's positions in the entity [`Self::next_entity`] last yielded: empty where the
    /// source carries none.
    fn positions(&self) -> &[u32];

    /// The term's posting and positions records as a finished layer stores them, where this source
    /// yields every entity they hold: what the term is written as when no other source holds it.
    fn records(&self) -> Option<Records<'_>> {
        None
    }
}

/// A term's posting and positions records in a finished layer. The posting was checked when the
/// layer's postings were opened; the positions are checked as a reader decodes them.
#[derive(Clone, Copy)]
pub struct Records<'a> {
    pub posting: &'a [u8],
    pub positions: &'a [u8],
}

/// A k-way merge over term sources, yielding each distinct term once with its entities ascending.
///
/// The heap holds **source indices**, and the comparison reaches into the sources, so a term is
/// never copied into the heap and the merge allocates nothing per term.
pub struct TermMerge<S> {
    sources: Vec<S>,
    heap: Vec<usize>,
    /// The selected term, held because the caller needs it after its sources have moved on.
    term: Vec<u8>,
    /// The sources at the selected term, ascending.
    selected: Vec<usize>,
    /// Entities the selected term carries across them.
    postings: u64,
    /// The selected sources' unconsumed heads, `(entity, source)`, smallest first.
    heads: BinaryHeap<Reverse<(u32, usize)>>,
    /// The first term past the merge's range, where it has one.
    hi: Option<Vec<u8>>,
}

impl<S: TermSource> TermMerge<S> {
    /// A merge over the terms of `sources` in `range`.
    pub fn range(mut sources: Vec<S>, range: &TermRange) -> io::Result<TermMerge<S>> {
        if let Some(lo) = &range.lo {
            for source in &mut sources {
                source.seek(lo)?;
            }
        }
        let mut merge = TermMerge::open(sources)?;
        merge.hi = range.hi.clone();
        Ok(merge)
    }

    /// The sources, wherever the merge left them.
    pub fn into_sources(self) -> Vec<S> {
        self.sources
    }

    /// A merge over `sources`, each moved to its first term.
    pub fn open(mut sources: Vec<S>) -> io::Result<TermMerge<S>> {
        let mut heap = Vec::with_capacity(sources.len());
        for (i, source) in sources.iter_mut().enumerate() {
            if source.next_term()? {
                heap.push(i);
            }
        }
        for root in (0..heap.len() / 2).rev() {
            sift_down(&mut heap, &sources, root);
        }
        Ok(TermMerge {
            sources,
            heap,
            term: Vec::new(),
            selected: Vec::new(),
            postings: 0,
            heads: BinaryHeap::new(),
            hi: None,
        })
    }

    /// Select the next distinct term, or `false` when every source is exhausted. The previous
    /// term's sources are moved on here rather than by the caller, so a caller that took fewer
    /// entities than the term carried still leaves each source where its next term starts.
    pub fn next_term(&mut self) -> io::Result<bool> {
        for &i in &self.selected {
            if self.sources[i].next_term()? {
                push(&mut self.heap, &self.sources, i);
            }
        }
        self.selected.clear();
        self.postings = 0;
        let Some(&first) = self.heap.first() else {
            return Ok(false);
        };
        if self
            .hi
            .as_deref()
            .is_some_and(|hi| self.sources[first].term() >= hi)
        {
            return Ok(false);
        }
        self.term.clear();
        self.term.extend_from_slice(self.sources[first].term());
        while self
            .heap
            .first()
            .is_some_and(|&top| self.sources[top].term() == self.term.as_slice())
        {
            let top = pop(&mut self.heap, &self.sources);
            self.postings += self.sources[top].count();
            self.selected.push(top);
        }
        Ok(true)
    }

    pub fn term(&self) -> &[u8] {
        &self.term
    }

    /// How many entities the selected term carries, known before any is read, which is what lets
    /// the caller choose how to hold them.
    pub fn postings(&self) -> u64 {
        self.postings
    }

    /// The sources at the selected term, with their indices in the order they were given.
    pub fn selected(&self) -> impl Iterator<Item = (usize, &S)> {
        self.selected.iter().map(|&i| (i, &self.sources[i]))
    }

    /// The selected term's records as one finished layer stores them, where that layer is the only
    /// source holding the term and yields every entity they hold.
    pub fn records(&self) -> Option<Records<'_>> {
        match self.selected.as_slice() {
            [only] => self.sources[*only].records(),
            _ => None,
        }
    }

    /// Feed the selected term's entities to `sink`, ascending, each with the term's positions in
    /// it, checking the ascent as it goes. A failure of the sources reaches the caller through
    /// `lift`, in the sink's error type.
    ///
    /// The check is over the *merged* list, not each source's: it is what catches an entity two
    /// sources carry for one term, which no caller produces (a text entity has one document and one
    /// layer holds it, and a keyword row is visited once) and which a union would otherwise hide.
    pub fn drain<E>(
        &mut self,
        lift: impl Fn(io::Error) -> E,
        sink: &mut impl FnMut(u32, &[u32]) -> Result<(), E>,
    ) -> Result<(), E> {
        self.heads.clear();
        for &i in &self.selected {
            if let Some(entity) = self.sources[i].next_entity().map_err(&lift)? {
                self.heads.push(Reverse((entity, i)));
            }
        }
        let mut last: Option<u32> = None;
        while let Some(Reverse((entity, i))) = self.heads.pop() {
            if last.is_some_and(|previous| entity <= previous) {
                return Err(lift(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "term merge: term {:?}'s entity {entity} does not ascend past {}: two \
                         sources carry the same entity for one term",
                        String::from_utf8_lossy(&self.term),
                        last.unwrap_or_default()
                    ),
                )));
            }
            last = Some(entity);
            sink(entity, self.sources[i].positions())?;
            if let Some(next) = self.sources[i].next_entity().map_err(&lift)? {
                self.heads.push(Reverse((next, i)));
            }
        }
        Ok(())
    }
}

/// Order two sources by their term, ties broken by index so equal terms leave the heap in the
/// order the sources were given.
fn before<S: TermSource>(sources: &[S], a: usize, b: usize) -> bool {
    match sources[a].term().cmp(sources[b].term()) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => a < b,
    }
}

fn sift_down<S: TermSource>(heap: &mut [usize], sources: &[S], mut node: usize) {
    loop {
        let left = node * 2 + 1;
        if left >= heap.len() {
            return;
        }
        let right = left + 1;
        let child = if right < heap.len() && before(sources, heap[right], heap[left]) {
            right
        } else {
            left
        };
        if !before(sources, heap[child], heap[node]) {
            return;
        }
        heap.swap(node, child);
        node = child;
    }
}

fn push<S: TermSource>(heap: &mut Vec<usize>, sources: &[S], i: usize) {
    heap.push(i);
    let mut node = heap.len() - 1;
    while node > 0 {
        let parent = (node - 1) / 2;
        if !before(sources, heap[node], heap[parent]) {
            return;
        }
        heap.swap(node, parent);
        node = parent;
    }
}

fn pop<S: TermSource>(heap: &mut Vec<usize>, sources: &[S]) -> usize {
    let top = heap[0];
    let last = heap.pop().expect("the heap is not empty");
    if !heap.is_empty() {
        heap[0] = last;
        sift_down(heap, sources, 0);
    }
    top
}

/// A finished layer's dictionary as a term source, stepping with [`SortedDict::key_of`]: its
/// keys, or only those whose ordinals `live` holds. A dictionary alone carries no entities, so
/// each key's count is zero; a text layer's source reads the entities from its postings.
///
/// `key_of` re-decodes its block from the restart each time, about half a block of entries a key,
/// and that is what lets several cursors coexist where the push-driven [`SortedDict::walk`] cannot.
/// At the shipped restart interval of 16 that is 8.5 entry decodes a key, which at a measured 11.0
/// to 18.8 ns a decode models to 0.1 to 0.16 µs a key.
pub struct DictCursor<'a> {
    dict: &'a SortedDict,
    live: Option<&'a Bitmap>,
    /// The ordinal whose key `scratch` holds, and the next to look at.
    at: u32,
    next: u32,
    scratch: Vec<u8>,
}

impl<'a> DictCursor<'a> {
    pub fn new(dict: &'a SortedDict, live: Option<&'a Bitmap>) -> DictCursor<'a> {
        DictCursor {
            dict,
            live,
            at: 0,
            next: 0,
            scratch: Vec::new(),
        }
    }

    /// The ordinal of the key the cursor is at.
    pub fn at(&self) -> u32 {
        self.at
    }
}

/// About `samples` of `dict`'s keys, evenly spaced by ordinal, each with the bytes from it to the
/// next sample, `bytes` giving the bytes before an ordinal and the whole at the dictionary's length.
pub(crate) fn samples(
    dict: &SortedDict,
    samples: usize,
    bytes: impl Fn(u32) -> io::Result<u64>,
) -> io::Result<Vec<Sample>> {
    let n = dict.len() as usize;
    let taken = samples.min(n);
    let ordinals: Vec<u32> = (0..taken).map(|i| (i * n / taken) as u32).collect();
    let mut scratch = Vec::new();
    ordinals
        .iter()
        .enumerate()
        .map(|(i, &ordinal)| {
            let end = ordinals.get(i + 1).copied().unwrap_or(dict.len());
            Ok(Sample {
                key: dict.key_of(ordinal, &mut scratch)?.as_bytes().to_vec(),
                bytes: bytes(end)?.saturating_sub(bytes(ordinal)?),
            })
        })
        .collect()
}

/// The least input a merge gives each range it divides its terms into, so that a small merge is
/// one range.
pub const RANGE_BYTES_MIN: u64 = 8 << 20;

/// Ranges a merge divides its terms into for each thread, at the most, so that a thread that
/// finishes early takes another.
pub const RANGES_PER_THREAD: usize = 4;

/// The most a range that is not yet written holds of its output before it waits.
pub const RANGE_BUFFER_MAX: usize = 64 << 20;

/// Keys a merge samples from all its sources together to place its ranges, at the most.
const SAMPLES_MAX: usize = 1 << 16;

/// One range of a merge's terms: from `lo`, or the first term, up to but not including `hi`, or
/// past the last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TermRange {
    pub lo: Option<Vec<u8>>,
    pub hi: Option<Vec<u8>>,
}

/// How a merge runs: how many ranges its terms are divided into and on how many threads, how many
/// ranges may be issued and not yet written, what one of them holds of its output before it
/// waits, and the read buffer each run's reader is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangePlan {
    pub ranges: usize,
    pub threads: usize,
    pub window: usize,
    pub buffer: usize,
    pub read: usize,
}

impl RangePlan {
    /// The plan for a merge over `bytes` of input, `runs` of its sources runs, under `allowance`,
    /// where each thread holds `each` bytes beside its readers.
    ///
    /// It runs on [`Allowance::merge_threads`], the threads the cascade sized the runs for, or
    /// fewer where its input is too small for that many ranges. The ranges are sized by bytes,
    /// [`RANGES_PER_THREAD`] a thread at most, and the window is two ranges a thread, one on one
    /// thread.
    pub fn new(allowance: Allowance, bytes: u64, runs: usize, each: u64) -> RangePlan {
        let most = usize::try_from(bytes / RANGE_BYTES_MIN)
            .unwrap_or(usize::MAX)
            .max(1);
        let threads = allowance.merge_threads(each).min(most);
        let ranges = most.min(RANGES_PER_THREAD * threads);
        RangePlan::with(allowance, ranges, threads, 2 * threads, runs, each)
    }

    /// A plan of `ranges` ranges on `threads` threads, `window` of them in flight (one on one
    /// thread). Each thread holds its `each` bytes and a reader on each of `runs` runs, and each
    /// range in the window but the one being written holds up to an equal share of half what they
    /// leave, from [`RUN_BUFFER_MIN`] to [`RANGE_BUFFER_MAX`]; the readers' buffers are what is
    /// left after that.
    pub fn with(
        allowance: Allowance,
        ranges: usize,
        threads: usize,
        window: usize,
        runs: usize,
        each: u64,
    ) -> RangePlan {
        let ranges = ranges.max(1);
        let threads = threads.clamp(1, ranges);
        let window = match threads {
            1 => 1,
            _ => window.clamp(threads, ranges.max(threads)),
        };
        let left = allowance.less(threads as u64 * each);
        let readers = (threads * runs) as u64 * RUN_BUFFER_MIN as u64;
        let buffer = usize::try_from(left.less(readers).bytes / 2 / (window as u64 - 1).max(1))
            .unwrap_or(usize::MAX)
            .clamp(RUN_BUFFER_MIN, RANGE_BUFFER_MAX);
        let read = left
            .less((window - 1) as u64 * buffer as u64)
            .buffer(threads * runs.max(1))
            .clamp(RUN_BUFFER_MIN, SPILL_BUF_BYTES);
        RangePlan {
            ranges,
            threads,
            window,
            buffer,
            read,
        }
    }

    /// How many keys each of `sources` samples to place the ranges.
    pub fn samples(&self, sources: usize) -> usize {
        (8 * self.ranges).min(SAMPLES_MAX / sources.max(1)).max(2)
    }
}

/// How a merge is divided: as its allowance pays for, or as fixed, for a test that must divide a
/// small merge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ranges {
    #[default]
    Planned,
    Fixed {
        ranges: usize,
        threads: usize,
        window: usize,
        buffer: usize,
    },
}

impl Ranges {
    /// The plan for a merge over `bytes` of input, `runs` of its sources runs, under `allowance`,
    /// where each thread holds `each` bytes beside its readers.
    pub fn plan(self, allowance: Allowance, bytes: u64, runs: usize, each: u64) -> RangePlan {
        match self {
            Ranges::Planned => RangePlan::new(allowance, bytes, runs, each),
            Ranges::Fixed {
                ranges,
                threads,
                window,
                buffer,
            } => RangePlan {
                buffer: buffer.max(1),
                ..RangePlan::with(allowance, ranges, threads, window, runs, each)
            },
        }
    }
}

/// Divide the terms `samples` describe, each standing for the bytes from its key to its source's
/// next, into at most `ranges` ranges of about equal bytes, ascending and together covering every
/// term.
pub fn split(mut samples: Vec<Sample>, ranges: usize) -> Vec<TermRange> {
    samples.sort_unstable_by(|a, b| a.key.cmp(&b.key));
    let total: u128 = samples.iter().map(|sample| u128::from(sample.bytes)).sum();
    let mut cuts: Vec<Vec<u8>> = Vec::new();
    let mut before = 0u128;
    for sample in samples {
        let share = (cuts.len() as u128 + 1) * total / ranges.max(1) as u128;
        if cuts.len() + 1 < ranges
            && before > 0
            && before >= share
            && cuts.last().is_none_or(|last| *last < sample.key)
        {
            cuts.push(sample.key.clone());
        }
        before += u128::from(sample.bytes);
    }
    let mut bounds: Vec<Option<Vec<u8>>> = vec![None];
    bounds.extend(cuts.into_iter().map(Some));
    bounds.push(None);
    bounds
        .windows(2)
        .map(|pair| TermRange {
            lo: pair[0].clone(),
            hi: pair[1].clone(),
        })
        .collect()
}

/// What a parallel merge writes its terms into, in term order, and what a range that is not yet
/// being written holds of its output.
pub trait RangeWriter: Send {
    type Held: Default + Send;

    /// The bytes `held` holds.
    fn held_bytes(held: &Self::Held) -> usize;

    /// Write what a range held, leaving it empty, and go on with a term it left open.
    fn write_held(&mut self, held: &mut Self::Held) -> io::Result<()>;
}

impl<W: RangeWriter> RangeWriter for &mut W {
    type Held = W::Held;

    fn held_bytes(held: &Self::Held) -> usize {
        W::held_bytes(held)
    }

    fn write_held(&mut self, held: &mut Self::Held) -> io::Result<()> {
        (**self).write_held(held)
    }
}

/// Run `work` over each of `ranges` as `plan` says, its output into `writer` in the ranges' order,
/// and call `written` with each range's index once it and every range before it are written.
pub fn merge_ranges<W: RangeWriter>(
    writer: W,
    ranges: &[TermRange],
    plan: RangePlan,
    work: impl Fn(&TermRange, &mut Turn<'_, W>) -> io::Result<()> + Sync,
    written: impl Fn(usize) + Sync,
) -> io::Result<W> {
    let ordered = Ordered::<W::Held>::new(plan.window);
    let sink = Mutex::new(writer);
    let failure: Mutex<Option<io::Error>> = Mutex::new(None);
    let worker = || {
        let _stop = FailOnPanic(&ordered);
        let result = (|| -> io::Result<()> {
            while let Some(seq) = ordered.issue(ranges.len() as u64)? {
                let mut turn = Turn {
                    seq,
                    ordered: &ordered,
                    sink: &sink,
                    direct: None,
                    held: W::Held::default(),
                    limit: plan.buffer,
                    written: &written,
                };
                work(&ranges[seq as usize], &mut turn)?;
                turn.finish()?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_or_insert(e);
            ordered.fail();
        }
    };
    match plan.threads.min(ranges.len()) {
        0 | 1 => worker(),
        threads => std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(worker);
            }
        }),
    }
    if let Some(e) = failure.into_inner().unwrap_or_else(PoisonError::into_inner) {
        return Err(e);
    }
    Ok(sink.into_inner().unwrap_or_else(PoisonError::into_inner))
}

/// One range's place in a parallel merge's output: the writer, once every range before it is
/// written, and until then what it holds.
pub struct Turn<'a, W: RangeWriter> {
    seq: u64,
    ordered: &'a Ordered<W::Held>,
    sink: &'a Mutex<W>,
    direct: Option<MutexGuard<'a, W>>,
    held: W::Held,
    limit: usize,
    written: &'a (dyn Fn(usize) + Sync),
}

/// Where a range's next term goes.
pub enum Out<'t, W: RangeWriter> {
    Writer(&'t mut W),
    Held(&'t mut W::Held),
}

impl<W: RangeWriter> Turn<'_, W> {
    /// Where the next term goes: the writer, once every range before this one is written, and what
    /// the range holds until then. Called between terms.
    pub fn out(&mut self) -> io::Result<Out<'_, W>> {
        if self.direct.is_none() && self.ordered.is_head(self.seq) {
            self.take_writer()?;
        }
        Ok(match &mut self.direct {
            Some(writer) => Out::Writer(&mut **writer),
            None => Out::Held(&mut self.held),
        })
    }

    /// Whether what the range holds has reached the plan's buffer, so that it waits for its turn.
    pub fn full(&self) -> bool {
        !self.fits(0)
    }

    /// Whether `bytes` more go where the next term goes without passing the plan's buffer.
    pub fn fits(&self, bytes: usize) -> bool {
        self.direct.is_some() || W::held_bytes(&self.held).saturating_add(bytes) < self.limit
    }

    /// Wait until every range before this one is written, write what it holds, and hand over the
    /// writer, which the rest of the range goes into.
    pub fn wait(&mut self) -> io::Result<&mut W> {
        if self.direct.is_none() {
            self.ordered.wait_head(self.seq)?;
            self.take_writer()?;
        }
        Ok(&mut **self.direct.as_mut().expect("the writer was taken"))
    }

    fn take_writer(&mut self) -> io::Result<()> {
        let mut writer = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        writer.write_held(&mut self.held)?;
        self.direct = Some(writer);
        Ok(())
    }

    /// The range is done: hand in what it holds, and write it and the ranges after it that are
    /// done, in order, where the writer is free. A writer held by another thread is released by a
    /// range that writes, which then looks again.
    fn finish(mut self) -> io::Result<()> {
        self.ordered.put(self.seq, std::mem::take(&mut self.held));
        let mut writer = self.direct.take();
        loop {
            let mut sink = match writer.take() {
                Some(sink) => sink,
                None => match self.sink.try_lock() {
                    Ok(sink) => sink,
                    Err(TryLockError::WouldBlock) => return Ok(()),
                    Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                },
            };
            while let Some((seq, mut held)) = self.ordered.take(false)? {
                sink.write_held(&mut held)?;
                (self.written)(seq as usize);
            }
            drop(sink);
            if !self.ordered.head_ready() {
                return Ok(());
            }
        }
    }
}

impl TermSource for DictCursor<'_> {
    fn seek(&mut self, key: &[u8]) -> io::Result<()> {
        self.next = self.dict.rank(key)?;
        Ok(())
    }

    fn next_term(&mut self) -> io::Result<bool> {
        while self.next < self.dict.len() {
            let ordinal = self.next;
            self.next += 1;
            if self.live.is_none_or(|live| live.contains(ordinal)) {
                self.dict.key_of(ordinal, &mut self.scratch)?;
                self.at = ordinal;
                return Ok(true);
            }
        }
        self.scratch.clear();
        Ok(false)
    }

    fn term(&self) -> &[u8] {
        &self.scratch
    }

    fn count(&self) -> u64 {
        0
    }

    fn next_entity(&mut self) -> io::Result<Option<u32>> {
        Ok(None)
    }

    fn positions(&self) -> &[u32] {
        &[]
    }
}
