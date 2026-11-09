//! Where a text index's terms are gathered before they are written, and how what was gathered is
//! written. The build's text pass and the flush both accumulate in a [`TermAccumulator`], spill
//! from it, and write the index with [`write_text_index`], each under an [`Allowance`]: an
//! accumulator's budget is its share of it, and the merge's read buffers are what it pays for
//! beside what the accumulators still hold.
//!
//! # What the accumulator holds
//!
//! - **The terms**, interned by a [`KeyTable`] over one buffer of their bytes.
//! - **Each term's entities and positions, already in the run format.** They are held in a pool of
//!   pages, as a chain of slices that double in size up to a cap, each slice ending in the address
//!   of the next. The bytes are what [`encode_entity`] writes, so a spill copies a term's chain
//!   into its run as it stands, and the term's share of the run's receipt is summed as it is
//!   inserted.
//! - **About forty bytes of state a term**: where its chain starts and ends, its last entity, how
//!   many entities it holds, and its share of the receipt. The states are held in pages, so they
//!   grow without moving.
//!
//! A document's tokens are grouped by term before they are inserted, because the run format writes
//! how often an entity holds a term before the positions.
//!
//! # Ascending segments
//!
//! A run's entities ascend within a term, and the documents need not arrive in that order: a build
//! worker takes several windows in the order work-stealing gives it, a group-scoped column's arena
//! is in arrival order, and the flush is ascending without this relying on it. So a term's chain is
//! a list of segments, each ascending. A document whose entity does not exceed the term's last
//! starts a new segment: a zero byte, which no gap within a segment can be, then the entity itself.
//! The first segment starts with its entity and no marker, so a term of one segment is a run's body
//! as it is. A term of several has its entities sorted at the spill and re-encoded, and keeps the
//! receipt share it was given at insert, which depends on its entities and positions and not on
//! their gaps.
//!
//! # Memory
//!
//! The accumulator counts what it has allocated: the pool's pages, its term-state pages, the
//! capacity of its key bytes and table, and its per-document buffers. Before each document it
//! spills if that, plus what its term state, key bytes and table would allocate were every token
//! of the document a new term, plus what a spill of all those terms would hold beside them, would
//! reach its budget; a growing array's old copy is live while it moves, so the new copy is counted
//! whole. The table's entries can still grow past that estimate
//! by up to the size of one document, when a spill has released them and one document then adds
//! more than twice their capacity.
//!
//! Within a document the accumulator can exceed its budget by that document's encoded bytes and
//! per-document buffers, plus one partly filled slice for each distinct term and one new pool
//! page. The per-document buffers return to a small size at each spill, so one long document does
//! not shrink every run after it.
//!
//! A spill holds beside the accumulator forty bytes a term for the ranking and its run writer's
//! buffer, a sixteenth of the budget between [`RUN_BUFFER_MIN`] and [`SPILL_BUF_BYTES`]
//! ([`SPILL_TERM_BYTES`]), and these are counted. For each term whose
//! documents arrived out of entity order it also holds a copy of its chain and eight bytes an
//! entity, kept at the size of the largest such term, which is not counted: at most about four
//! times that one term's chain. Every other term is written from the pool as it stands.
//!
//! # The end of the pass
//!
//! What an accumulator holds when its pass ends is not spilled. [`TermAccumulator::finish`] ranks
//! it once, as a spill would, and hands it back as a [`Held`] source beside its runs, and
//! [`write_text_index`] merges the runs and the held sources together into the index. The merge's
//! read buffers are what the allowance leaves beside the held sources, so where they do not leave
//! enough for the runs, the largest held sources are spilled after all, one at a time, until they
//! do. A pass that never spilled writes no run at all.

use std::io;
use std::path::{Path, PathBuf};

use mosaica_filter::{put_varint, take_varint, VarintError};

use crate::chunk_dict::{rank_table, KeyRef, KeyRow, KeyTable, Lookup, SortOn};
use crate::merge::{TermMerge, TermSource};
use crate::spill::{
    cascade_sorted_runs, encode_entity, fingerprint, merge_descriptors, remove_runs, Allowance,
    RunPayload, SpillReceipt, TextRunReader, TextRunWriter, RUN_BUFFER_MIN, SPILL_BUF_BYTES,
};
use crate::text::{write_terms, TextIndexWriter};

/// Bytes in one page of the pool. A slice never crosses a page.
const PAGE_BYTES: usize = 1 << 18;

/// The size of a term's slices, in the order its chain takes them, the last repeated.
const SLICE_BYTES: [usize; 9] = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

/// The bytes at the end of each slice that hold the address of the next.
const LINK_BYTES: usize = 8;

/// What a spill's ranking holds a term: the row it sorts and its rank, then the key in order and
/// the id at that rank.
const SPILL_TERM_BYTES: usize =
    std::mem::size_of::<KeyRow>() + std::mem::size_of::<KeyRef>() + 2 * std::mem::size_of::<u32>();

/// Items each per-document buffer is returned to at a spill.
const DOCUMENT_BUFFER_ITEMS: usize = 1 << 12;

/// A text column's terms, the entities holding each and the positions at which they do, gathered a
/// document at a time under a byte budget, and spilled as sorted runs whenever the budget fills.
pub struct TermAccumulator {
    budget: usize,
    /// Where the runs go, and the start of their file names.
    dir: PathBuf,
    stem: String,
    runs: Vec<SpillReceipt>,
    /// Every term's bytes, end to end, which the table's keys point into.
    keys: Vec<u8>,
    table: KeyTable,
    /// Each term's state, by its id in the table.
    terms: States,
    pool: Pool,
    /// The open document's tokens, end to end, and where each ends.
    text: Vec<u8>,
    ends: Vec<u32>,
    /// The open document's tokens, as the term's id above the position.
    tokens: Vec<u64>,
    positions: Vec<u32>,
    encoded: Vec<u8>,
}

/// One term's chain, and what it adds up to.
#[derive(Clone, Copy, Default)]
struct TermState {
    /// The first slice, and where the next byte goes in the last.
    head: u64,
    tail: u64,
    /// The term's share of the receipt's anchor.
    anchor: u64,
    last: u32,
    pairs: u32,
    /// Bytes left in the last slice before its link.
    room: u16,
    /// Which of [`SLICE_BYTES`] the last slice is.
    level: u8,
    /// Whether the chain holds more than one segment.
    segmented: bool,
}

impl TermAccumulator {
    /// An empty accumulator whose runs are written in `dir`, named `{stem}-{n}.spill`, and which
    /// spills when it has allocated `budget` bytes.
    pub fn new(dir: &Path, stem: impl Into<String>, budget: usize) -> TermAccumulator {
        TermAccumulator {
            budget,
            dir: dir.to_path_buf(),
            stem: stem.into(),
            runs: Vec::new(),
            keys: Vec::new(),
            table: KeyTable::default(),
            terms: States::default(),
            pool: Pool::default(),
            text: Vec::new(),
            ends: Vec::new(),
            tokens: Vec::new(),
            positions: Vec::new(),
            encoded: Vec::new(),
        }
    }

    /// Add `entity`'s document, whose tokens `tokens` gives in order to the function it is passed,
    /// spilling first if the document could take the accumulator past its budget. An entity's
    /// document is added once.
    pub fn add_document(
        &mut self,
        entity: u32,
        tokens: impl FnOnce(&mut dyn FnMut(&str)),
    ) -> io::Result<()> {
        self.text.clear();
        self.ends.clear();
        let mut failed = None;
        tokens(&mut |token| {
            self.text.extend_from_slice(token.as_bytes());
            match u32::try_from(self.text.len()) {
                Ok(end) => self.ends.push(end),
                Err(_) => {
                    failed.get_or_insert_with(|| {
                        invalid(format!(
                            "entity {entity}'s document holds more than 4 GiB of tokens; split it \
                             into several items"
                        ))
                    });
                }
            }
        });
        if let Some(e) = failed {
            return Err(e);
        }
        if self.allocated() + self.growth(self.ends.len(), self.text.len()) >= self.budget {
            // On this thread: a sort on the pool would let a worker waiting for it take another
            // window and make another accumulator.
            self.spill(SortOn::ThisThread)?;
        }
        let (text, ends) = (
            std::mem::take(&mut self.text),
            std::mem::take(&mut self.ends),
        );
        self.tokens.clear();
        let mut start = 0;
        for (position, &end) in (0u32..).zip(&ends) {
            let id = self.intern(&text[start..end as usize])?;
            self.tokens.push(u64::from(id) << 32 | u64::from(position));
            start = end as usize;
        }
        (self.text, self.ends) = (text, ends);
        let mut tokens = std::mem::take(&mut self.tokens);
        tokens.sort_unstable();
        let mut from = 0;
        while from < tokens.len() {
            let id = (tokens[from] >> 32) as u32;
            let to = from + tokens[from..].partition_point(|t| (t >> 32) as u32 == id);
            self.positions.clear();
            self.positions
                .extend(tokens[from..to].iter().map(|&t| t as u32));
            self.insert(id, entity)?;
            from = to;
        }
        self.tokens = tokens;
        Ok(())
    }

    /// What the term state, key bytes and table would allocate, beyond what they hold, were
    /// `terms` new terms of `bytes` bytes in all added: each array that would grow, at its new
    /// size, and each page of state. And what a spill of every term then held would hold beside
    /// them: its ranking and its writer's buffer.
    fn growth(&self, terms: usize, bytes: usize) -> usize {
        let grown = |len: usize, capacity: usize, more: usize| {
            if len + more > capacity {
                (len + more).max(capacity * 2)
            } else {
                0
            }
        };
        self.terms.growth(terms)
            + grown(self.keys.len(), self.keys.capacity(), bytes)
            + self.table.growth(terms)
            + (self.terms.len + terms) * SPILL_TERM_BYTES
            + self.writer_buffer()
    }

    /// The buffer a spill writes its run through: a sixteenth of the budget, so that a small one
    /// is not mostly buffer, between [`RUN_BUFFER_MIN`] and [`SPILL_BUF_BYTES`].
    fn writer_buffer(&self) -> usize {
        (self.budget / 16).clamp(RUN_BUFFER_MIN, SPILL_BUF_BYTES)
    }

    /// The id of `term`, given one if it is new.
    fn intern(&mut self, term: &[u8]) -> io::Result<u32> {
        let keys = &self.keys;
        let vacant = match self.table.find(term, |key| key_bytes(keys, key)) {
            Lookup::Found(id) => return Ok(id),
            Lookup::Vacant(vacant) => vacant,
        };
        let at = u32::try_from(self.keys.len())
            .ok()
            .filter(|at| at.checked_add(term.len() as u32).is_some())
            .ok_or_else(|| {
                invalid(
                    "a text accumulator holds more than 4 GiB of distinct terms; give it a \
                     smaller budget",
                )
            })?;
        let key = KeyRef::new(term, at)?;
        self.keys.extend_from_slice(term);
        let id = self.table.insert(vacant, key)?;
        let head = self.pool.alloc(SLICE_BYTES[0]);
        self.terms.push(TermState {
            head,
            tail: head,
            anchor: 0,
            last: 0,
            pairs: 0,
            room: (SLICE_BYTES[0] - LINK_BYTES) as u16,
            level: 0,
            segmented: false,
        });
        Ok(id)
    }

    /// Append `entity`, holding term `id` at `self.positions`, to the term's chain.
    fn insert(&mut self, id: u32, entity: u32) -> io::Result<()> {
        let term = key_bytes(&self.keys, &self.table.key(id));
        let state = self.terms.get_mut(id);
        self.encoded.clear();
        let gap = if state.pairs == 0 {
            entity
        } else if entity > state.last {
            entity - state.last
        } else {
            self.encoded.push(0);
            state.segmented = true;
            entity
        };
        let anchor = encode_entity(
            &mut self.encoded,
            RunPayload::Positions,
            fingerprint(term),
            gap,
            entity,
            &self.positions,
        );
        state.pairs = state.pairs.checked_add(1).ok_or_else(|| {
            invalid(format!(
                "term {:?} is held by more entities than a u32 can count",
                String::from_utf8_lossy(term)
            ))
        })?;
        state.anchor = state.anchor.wrapping_add(anchor);
        state.last = entity;
        self.pool.append(state, &self.encoded);
        Ok(())
    }

    /// The bytes this accumulator has allocated.
    pub fn allocated(&self) -> usize {
        self.pool.pages.len() * PAGE_BYTES
            + self.terms.allocated()
            + self.keys.capacity()
            + self.table.allocated()
            + self.text.capacity()
            + self.ends.capacity() * std::mem::size_of::<u32>()
            + self.tokens.capacity() * std::mem::size_of::<u64>()
            + self.positions.capacity() * std::mem::size_of::<u32>()
            + self.encoded.capacity()
    }

    /// Write what the accumulator holds as one sorted run, if it holds anything, and empty it,
    /// ranking its terms where `sort` says. A run that fails part-written is removed.
    fn spill(&mut self, sort: SortOn) -> io::Result<()> {
        let Some(mut held) = self.rank(sort)? else {
            return Ok(());
        };
        let path = self
            .dir
            .join(format!("{}-{:05}.spill", self.stem, self.runs.len()));
        self.runs.push(held.write_run(&path, self.writer_buffer())?);
        // The key bytes keep their capacity for the next fill, and are counted in it.
        self.keys = held.keys;
        self.keys.clear();
        self.text.shrink_to(DOCUMENT_BUFFER_ITEMS);
        self.ends.shrink_to(DOCUMENT_BUFFER_ITEMS);
        self.tokens.shrink_to(DOCUMENT_BUFFER_ITEMS);
        self.positions.shrink_to(DOCUMENT_BUFFER_ITEMS);
        self.encoded.shrink_to(DOCUMENT_BUFFER_ITEMS);
        Ok(())
    }

    /// Rank every term the accumulator holds, ascending, where `sort` says, and hand them back as
    /// a source, leaving the accumulator empty. `None` where it holds none.
    fn rank(&mut self, sort: SortOn) -> io::Result<Option<Held>> {
        if self.terms.is_empty() {
            return Ok(None);
        }
        let keys = std::mem::take(&mut self.keys);
        let terms = std::mem::take(&mut self.terms);
        let pool = std::mem::take(&mut self.pool);
        let mut sorted: Vec<KeyRef> = Vec::with_capacity(terms.len);
        let mut ranks = Vec::new();
        rank_table(
            &mut self.table,
            |key| key_bytes(&keys, key),
            &mut ranks,
            sort,
            |key| {
                sorted.push(*key);
                Ok::<(), io::Error>(())
            },
        )?;
        let mut order = vec![0u32; ranks.len()];
        for (id, &rank) in ranks.iter().enumerate() {
            order[rank as usize] = id as u32;
        }
        Ok(Some(Held {
            keys,
            terms,
            pool,
            sorted,
            order,
            next: 0,
            state: TermState::default(),
            chain: Vec::new(),
            entries: Vec::new(),
            reading: ChainAt::default(),
            taken: 0,
            last: 0,
            positions: Vec::new(),
        }))
    }

    /// End the pass: rank what the accumulator holds and hand it back beside the runs it spilled,
    /// for [`write_text_index`].
    pub fn finish(mut self) -> io::Result<Gathered> {
        let held = self.rank(SortOn::Pool)?;
        Ok(Gathered {
            dir: std::mem::take(&mut self.dir),
            stem: std::mem::take(&mut self.stem),
            runs: std::mem::take(&mut self.runs),
            buffer: self.writer_buffer(),
            held,
        })
    }
}

/// What a [`TermAccumulator`] gathered over its pass: the runs it spilled, and what it held at the
/// end, ranked. Runs nobody took are removed when it is dropped.
pub struct Gathered {
    dir: PathBuf,
    stem: String,
    runs: Vec<SpillReceipt>,
    /// The buffer a spill of the held terms writes through.
    buffer: usize,
    held: Option<Held>,
}

impl Gathered {
    /// The bytes the held terms take.
    fn held_bytes(&self) -> u64 {
        self.held.as_ref().map_or(0, |held| held.allocated() as u64)
    }

    /// Spill the held terms as one more run.
    fn spill(&mut self) -> io::Result<()> {
        if let Some(mut held) = self.held.take() {
            let path = self
                .dir
                .join(format!("{}-{:05}.spill", self.stem, self.runs.len()));
            self.runs.push(held.write_run(&path, self.buffer)?);
        }
        Ok(())
    }
}

impl Drop for Gathered {
    fn drop(&mut self) {
        let _ = remove_runs(&self.runs);
    }
}

/// How a text index was written: the runs its merge read, the sources it held in memory beside
/// them, and the cascade passes the runs took first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMerge {
    pub runs: usize,
    pub held: usize,
    pub passes: usize,
}

/// Write a text index into `writer` from what the accumulators gathered, under `allowance`, with
/// each term's entities held as a slice of at most `merge_slice_cap`. Every run is removed,
/// whether or not the merge succeeds. `family` names the cascade's files in `dir`.
///
/// The held sources stay in memory, and the runs are read through what the allowance leaves
/// beside them, at least [`RUN_BUFFER_MIN`] a run. Where that is too little, the largest held
/// source is spilled, and the next, until the runs fit; where every one is spilled and the runs
/// are more than the allowance's fan-in, they are cascaded first.
pub fn write_text_index(
    dir: &Path,
    family: &str,
    mut gathered: Vec<Gathered>,
    writer: &mut TextIndexWriter,
    merge_slice_cap: usize,
    allowance: Allowance,
) -> io::Result<TextMerge> {
    gathered.sort_by_key(Gathered::held_bytes);
    let fits = |gathered: &[Gathered]| {
        let runs: usize = gathered.iter().map(|g| g.runs.len()).sum();
        let held: u64 = gathered.iter().map(Gathered::held_bytes).sum();
        runs <= merge_descriptors()
            && (runs as u64).saturating_mul(RUN_BUFFER_MIN as u64)
                <= allowance.bytes.saturating_sub(held)
    };
    while !fits(&gathered) {
        match gathered.iter_mut().rev().find(|g| g.held.is_some()) {
            Some(largest) => largest.spill()?,
            None => break,
        }
    }
    let held: Vec<Held> = gathered.iter_mut().filter_map(|g| g.held.take()).collect();
    let runs: Vec<SpillReceipt> = gathered
        .iter_mut()
        .flat_map(|g| std::mem::take(&mut g.runs))
        .collect();
    drop(gathered);
    let held_bytes: u64 = held.iter().map(|h| h.allocated() as u64).sum();
    let figures = TextMerge {
        runs: runs.len(),
        held: held.len(),
        passes: 0,
    };

    let cascaded = match held.is_empty() {
        true => cascade_sorted_runs(dir, family, RunPayload::Positions, runs.clone(), allowance),
        false => Ok((runs.clone(), 0)),
    };
    // What the accumulators or the cascade's threads freed in their own arenas goes back before
    // the merge's buffers are taken.
    mosaica_types::process::trim_heap();
    let merged = match &cascaded {
        Ok((left, _)) => {
            let buffer = Allowance {
                bytes: allowance.bytes.saturating_sub(held_bytes),
                threads: allowance.threads,
            }
            .buffer(left.len());
            left.iter()
                .map(|run| {
                    TextRunReader::open(run, RunPayload::Positions, buffer)
                        .map(|reader| Box::new(reader) as Box<dyn TermSource>)
                })
                .chain(held.into_iter().map(|h| Ok(Box::new(h) as Box<dyn TermSource>)))
                .collect::<io::Result<Vec<_>>>()
                .and_then(TermMerge::open)
                .and_then(|mut merge| write_terms(&mut merge, writer, merge_slice_cap))
        }
        Err(_) => Ok(()),
    };
    let removed = remove_runs(
        runs.iter()
            .chain(cascaded.iter().flat_map(|(left, _)| left)),
    );
    let passes = cascaded.map(|(_, passes)| passes);
    merged.and(removed).and(passes).map(|passes| TextMerge { passes, ..figures })
}

/// What an accumulator held when its pass ended, ranked: a [`TermSource`] over its terms in order,
/// each term's entities read from its chain in the pool.
pub struct Held {
    keys: Vec<u8>,
    terms: States,
    pool: Pool,
    /// The terms in order, and each one's id in `terms`.
    sorted: Vec<KeyRef>,
    order: Vec<u32>,
    /// The rank of the next term, and the state of the one the source is at.
    next: usize,
    state: TermState,
    /// A term of several segments: its chain, and its entities sorted, each with where its
    /// positions start in the chain.
    chain: Vec<u8>,
    entries: Vec<(u32, u32)>,
    /// Where a term of one segment is read from in the pool.
    reading: ChainAt,
    /// Entities of the term read, and the last of them.
    taken: u32,
    last: u32,
    positions: Vec<u32>,
}

impl Held {
    /// The bytes it holds.
    fn allocated(&self) -> usize {
        self.pool.pages.len() * PAGE_BYTES
            + self.terms.allocated()
            + self.keys.capacity()
            + self.sorted.capacity() * std::mem::size_of::<KeyRef>()
            + self.order.capacity() * std::mem::size_of::<u32>()
            + self.chain.capacity()
            + self.entries.capacity() * std::mem::size_of::<(u32, u32)>()
    }

    /// Write every term as one run at `path`, through a buffer of `buffer` bytes: a term of one
    /// segment copied from the pool as it stands, and one of several re-encoded from its sorted
    /// entities. A run that fails part-written is removed.
    fn write_run(&mut self, path: &Path, buffer: usize) -> io::Result<SpillReceipt> {
        let written = (|| {
            let mut writer = TextRunWriter::buffered(path, RunPayload::Positions, buffer)?;
            while self.next_term()? {
                let term = key_bytes(&self.keys, &self.sorted[self.next - 1]);
                let state = &self.state;
                let body = match state.segmented {
                    true => Body::Sorted(&self.chain, &self.entries),
                    false => Body::Chain(&self.pool, state),
                };
                writer.push_body(term, state.pairs, state.anchor, |emit| body.write(emit))?;
            }
            writer.finish()
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(path);
        }
        written
    }
}

impl TermSource for Held {
    fn next_term(&mut self) -> io::Result<bool> {
        let Some(&id) = self.order.get(self.next) else {
            return Ok(false);
        };
        let term = key_bytes(&self.keys, &self.sorted[self.next]);
        self.next += 1;
        self.state = *self.terms.get(id);
        self.taken = 0;
        if self.state.segmented {
            self.chain.clear();
            let chain = &mut self.chain;
            self.pool
                .slices(&self.state, |slice| chain.extend_from_slice(slice));
            sort_segments(term, &self.chain, &mut self.entries)?;
        } else {
            self.reading = self.pool.chain(&self.state);
        }
        Ok(true)
    }

    fn term(&self) -> &[u8] {
        key_bytes(&self.keys, &self.sorted[self.next - 1])
    }

    fn count(&self) -> u64 {
        u64::from(self.state.pairs)
    }

    fn next_entity(&mut self) -> io::Result<Option<u32>> {
        if self.taken == self.state.pairs {
            return Ok(None);
        }
        self.positions.clear();
        let entity = if self.state.segmented {
            let (entity, start) = self.entries[self.taken as usize];
            let mut at = start as usize;
            let chain = &self.chain;
            read_positions(|| next(chain, &mut at), &mut self.positions)?;
            entity
        } else {
            let (pool, reading) = (&self.pool, &mut self.reading);
            let gap = pool.varint(reading)?;
            let entity = match self.taken {
                0 => gap,
                _ => self.last.wrapping_add(gap),
            };
            read_positions(|| pool.varint(reading), &mut self.positions)?;
            entity
        };
        self.taken += 1;
        self.last = entity;
        Ok(Some(entity))
    }

    fn positions(&self) -> &[u32] {
        &self.positions
    }
}

/// Read one entity's occurrence count and positions in the run format, each varint from `next`.
fn read_positions(
    mut next: impl FnMut() -> io::Result<u32>,
    out: &mut Vec<u32>,
) -> io::Result<()> {
    let mut position = 0u32;
    for _ in 0..next()? {
        position = position.wrapping_add(next()?);
        out.push(position);
    }
    Ok(())
}

/// A term's body in the run format, as a spill writes it.
enum Body<'a> {
    /// A chain of one ascending segment, which is the body as it stands.
    Chain(&'a Pool, &'a TermState),
    /// A chain of several, and its entities sorted, each with where its positions start.
    Sorted(&'a [u8], &'a [(u32, u32)]),
}

impl Body<'_> {
    /// Hand the body to `emit`, in order, in pieces.
    fn write(&self, emit: &mut dyn FnMut(&[u8]) -> io::Result<()>) -> io::Result<()> {
        match self {
            Body::Chain(pool, term) => {
                let mut written = Ok(());
                pool.slices(term, |slice| {
                    if written.is_ok() {
                        written = emit(slice);
                    }
                });
                written
            }
            Body::Sorted(chain, entries) => {
                let mut previous = None;
                let mut gap = Vec::with_capacity(5);
                for &(entity, start) in entries.iter() {
                    gap.clear();
                    put_varint(entity - previous.unwrap_or(0), &mut gap);
                    emit(&gap)?;
                    let mut end = start as usize;
                    for _ in 0..next(chain, &mut end)? {
                        next(chain, &mut end)?;
                    }
                    emit(&chain[start as usize..end])?;
                    previous = Some(entity);
                }
                Ok(())
            }
        }
    }
}

impl Drop for TermAccumulator {
    /// Runs nobody took are removed, so a pass that failed leaves none behind.
    fn drop(&mut self) {
        let _ = remove_runs(&self.runs);
    }
}

/// The bytes of `key` in `keys`.
fn key_bytes<'k>(keys: &'k [u8], key: &KeyRef) -> &'k [u8] {
    &keys[key.at as usize..][..key.len as usize]
}

/// Read one varint of a body this crate encoded.
fn next(body: &[u8], at: &mut usize) -> io::Result<u32> {
    take_varint(body, at).map_err(|e| invalid(format!("a text accumulator's chain {e}")))
}

/// The entities of a chain of several ascending segments into `entries`, ascending, each with
/// where its positions start in the chain. An entity that holds the term twice is refused.
fn sort_segments(term: &[u8], chain: &[u8], entries: &mut Vec<(u32, u32)>) -> io::Result<()> {
    if u32::try_from(chain.len()).is_err() {
        return Err(invalid(format!(
            "term {:?} holds more than 4 GiB of entities and positions in one accumulator; give \
             it a smaller budget",
            String::from_utf8_lossy(term)
        )));
    }
    entries.clear();
    let (mut at, mut last) = (0, 0u32);
    while at < chain.len() {
        let gap = next(chain, &mut at)?;
        let entity = if entries.is_empty() {
            gap
        } else if gap == 0 {
            next(chain, &mut at)?
        } else {
            last.wrapping_add(gap)
        };
        entries.push((entity, at as u32));
        for _ in 0..next(chain, &mut at)? {
            next(chain, &mut at)?;
        }
        last = entity;
    }
    // A stable sort, which merges the ascending segments it finds.
    entries.sort_by_key(|&(entity, _)| entity);
    if let Some(pair) = entries.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(invalid(format!(
            "entity {} holds term {:?} in two documents; add an entity's document once",
            pair[0].0,
            String::from_utf8_lossy(term)
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Term states by id, in pages of [`STATE_PAGE`], so they grow a page at a time and never move.
#[derive(Default)]
struct States {
    pages: Vec<Box<[TermState]>>,
    len: usize,
}

/// Term states in one page of [`States`].
const STATE_PAGE: usize = 1 << 12;

impl States {
    fn push(&mut self, state: TermState) {
        if self.len == self.pages.len() * STATE_PAGE {
            self.pages
                .push(vec![TermState::default(); STATE_PAGE].into_boxed_slice());
        }
        self.pages[self.len / STATE_PAGE][self.len % STATE_PAGE] = state;
        self.len += 1;
    }

    fn get(&self, id: u32) -> &TermState {
        let id = id as usize;
        &self.pages[id / STATE_PAGE][id % STATE_PAGE]
    }

    fn get_mut(&mut self, id: u32) -> &mut TermState {
        let id = id as usize;
        &mut self.pages[id / STATE_PAGE][id % STATE_PAGE]
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn allocated(&self) -> usize {
        self.pages.len() * STATE_PAGE * std::mem::size_of::<TermState>()
    }

    /// The bytes the pages `terms` more states would need take.
    fn growth(&self, terms: usize) -> usize {
        ((self.len + terms).div_ceil(STATE_PAGE)).saturating_sub(self.pages.len())
            * STATE_PAGE
            * std::mem::size_of::<TermState>()
    }
}

/// A place in a term's chain: the slice it is in and how far through it, and where the chain ends.
#[derive(Clone, Copy, Default)]
struct ChainAt {
    at: u64,
    /// Where the slice's bytes end: its link, or the chain's tail in its last slice.
    end: u64,
    level: usize,
    tail: u64,
}

impl ChainAt {
    /// The start of the slice at `head`, the chain's `level`th, in a chain ending at `tail`.
    fn of(head: u64, level: usize, tail: u64) -> ChainAt {
        let link = head + (SLICE_BYTES[level] - LINK_BYTES) as u64;
        ChainAt {
            at: head,
            end: if (head..=link).contains(&tail) { tail } else { link },
            level,
            tail,
        }
    }
}

/// Pages that terms' slices are carved from, addressed by a `u64`: the page above the offset.
#[derive(Default)]
struct Pool {
    pages: Vec<Box<[u8]>>,
    /// Bytes taken from the last page.
    used: usize,
}

impl Pool {
    /// A new slice of `bytes`, which must be at most a page.
    fn alloc(&mut self, bytes: usize) -> u64 {
        if self.pages.is_empty() || self.used + bytes > PAGE_BYTES {
            self.pages.push(vec![0; PAGE_BYTES].into_boxed_slice());
            self.used = 0;
        }
        let at = (self.pages.len() - 1) * PAGE_BYTES + self.used;
        self.used += bytes;
        at as u64
    }

    fn bytes(&self, at: u64, len: usize) -> &[u8] {
        let at = at as usize;
        &self.pages[at / PAGE_BYTES][at % PAGE_BYTES..][..len]
    }

    fn bytes_mut(&mut self, at: u64, len: usize) -> &mut [u8] {
        let at = at as usize;
        &mut self.pages[at / PAGE_BYTES][at % PAGE_BYTES..][..len]
    }

    /// Append `bytes` to `term`'s chain, linking a new slice on wherever the last is full.
    fn append(&mut self, term: &mut TermState, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            if term.room == 0 {
                let level = (term.level as usize + 1).min(SLICE_BYTES.len() - 1);
                let next = self.alloc(SLICE_BYTES[level]);
                self.bytes_mut(term.tail, LINK_BYTES)
                    .copy_from_slice(&next.to_le_bytes());
                term.tail = next;
                term.room = (SLICE_BYTES[level] - LINK_BYTES) as u16;
                term.level = level as u8;
            }
            let n = bytes.len().min(term.room as usize);
            self.bytes_mut(term.tail, n).copy_from_slice(&bytes[..n]);
            term.tail += n as u64;
            term.room -= n as u16;
            bytes = &bytes[n..];
        }
    }

    /// Where `term`'s chain starts.
    fn chain(&self, term: &TermState) -> ChainAt {
        ChainAt::of(term.head, 0, term.tail)
    }

    /// The next varint of the chain `at` reads, which may cross from one slice into the next.
    fn varint(&self, at: &mut ChainAt) -> io::Result<u32> {
        let mut read = 0;
        match take_varint(self.bytes(at.at, (at.end - at.at) as usize), &mut read) {
            Ok(value) => {
                at.at += read as u64;
                return Ok(value);
            }
            Err(VarintError::Truncated) => {}
            Err(e) => return Err(invalid(format!("a text accumulator's chain {e}"))),
        }
        let (mut bytes, mut len) = ([0u8; 5], 0);
        while len < bytes.len() {
            if at.at == at.end {
                if at.end == at.tail {
                    break;
                }
                *at = self.following(at);
                continue;
            }
            let byte = self.bytes(at.at, 1)[0];
            at.at += 1;
            bytes[len] = byte;
            len += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
        next(&bytes[..len], &mut 0)
    }

    /// Each slice of `term`'s chain, in order.
    fn slices(&self, term: &TermState, mut visit: impl FnMut(&[u8])) {
        let mut at = self.chain(term);
        loop {
            visit(self.bytes(at.at, (at.end - at.at) as usize));
            if at.end == at.tail {
                return;
            }
            at = self.following(&at);
        }
    }

    /// The start of the slice after the one `at` is in, which must not be the chain's last.
    fn following(&self, at: &ChainAt) -> ChainAt {
        let link = self.bytes(at.end, LINK_BYTES);
        let next = u64::from_le_bytes(link.try_into().expect("a link is eight bytes"));
        ChainAt::of(next, (at.level + 1).min(SLICE_BYTES.len() - 1), at.tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// **The runs and the held terms give back what was added, whatever order the documents came
    /// in.** Documents arrive in a shuffled entity order with terms repeated within and across
    /// them, so terms gather several segments, under a budget that never spills before the end and
    /// one that spills before every document, with what is held at the end read in memory and
    /// spilled. Merged, they must give each term's entities ascending with their positions, as a
    /// naive reference has them, and every run's receipt must verify.
    #[test]
    fn runs_give_back_what_was_added_in_any_order() {
        let words = [
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "lm", "nop",
        ];
        let mut entities: Vec<u32> = (0..200u32).map(|i| i * 7 + (i % 3)).collect();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut draw = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for i in (1..entities.len()).rev() {
            entities.swap(i, draw(i + 1));
        }
        let documents: Vec<(u32, Vec<&str>)> = entities
            .iter()
            .map(|&entity| {
                let len = 1 + draw(40);
                (
                    entity,
                    (0..len)
                        .map(|_| words[draw(words.len()) % (1 + draw(words.len()))])
                        .collect(),
                )
            })
            .collect();

        let mut reference: BTreeMap<Vec<u8>, BTreeMap<u32, Vec<u32>>> = BTreeMap::new();
        for (entity, tokens) in &documents {
            for (position, token) in tokens.iter().enumerate() {
                reference
                    .entry(token.as_bytes().to_vec())
                    .or_default()
                    .entry(*entity)
                    .or_default()
                    .push(position as u32);
            }
        }

        for (budget, spill) in [(usize::MAX, false), (usize::MAX, true), (0, false), (0, true)] {
            let dir = tempfile::tempdir().unwrap();
            let mut accumulator = TermAccumulator::new(dir.path(), "text-run", budget);
            for (entity, tokens) in &documents {
                accumulator
                    .add_document(*entity, |push| tokens.iter().for_each(|t| push(t)))
                    .unwrap();
            }
            let mut gathered = accumulator.finish().unwrap();
            if spill {
                gathered.spill().unwrap();
            }
            let mut sources: Vec<Box<dyn TermSource>> = gathered
                .runs
                .iter()
                .map(|run| {
                    let reader =
                        TextRunReader::open(run, RunPayload::Positions, RUN_BUFFER_MIN).unwrap();
                    Box::new(reader) as Box<dyn TermSource>
                })
                .collect();
            sources.extend(
                gathered
                    .held
                    .take()
                    .map(|held| Box::new(held) as Box<dyn TermSource>),
            );

            let mut merge = TermMerge::open(sources).unwrap();
            let mut read: BTreeMap<Vec<u8>, BTreeMap<u32, Vec<u32>>> = BTreeMap::new();
            while merge.next_term().unwrap() {
                let term = read.entry(merge.term().to_vec()).or_default();
                merge
                    .drain(std::convert::identity, &mut |entity, positions| {
                        term.insert(entity, positions.to_vec());
                        Ok(())
                    })
                    .unwrap();
            }
            assert_eq!(read, reference, "budget {budget}, spilled {spill}");
        }
    }
}
