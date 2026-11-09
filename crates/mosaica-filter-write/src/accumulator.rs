//! Where a text index's terms are gathered before they are written, and the merge of what was
//! spilled. The build's text pass and the flush both accumulate in a [`TermAccumulator`], spill
//! from it, and write through it or through [`merge_text_runs`].
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
//!   many entities it holds, and its share of the receipt.
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
//! The accumulator counts what it has allocated: the pool's pages, the capacity of its term state,
//! key bytes and table, and its per-document buffers. It spills when that reaches its budget, which
//! it checks between documents, so it may exceed the budget by one document and by the growth of a
//! vector that doubled within it. The ranking a spill does holds about forty bytes a term beside
//! it.

use std::io;
use std::path::{Path, PathBuf};

use croaring::Bitmap;
use mosaica_filter::{put_varint, take_varint};

use crate::chunk_dict::{rank_table, KeyRef, KeyTable, Lookup};
use crate::spill::{
    cascade_sorted_runs, encode_entity, fingerprint, RunPayload, SpillReceipt, TextRunMerge,
    TextRunWriter,
};
use crate::text::{Posting, TextIndexWriter};

/// Entities held as a `u32` slice for one merged term before the merge switches to a Roaring
/// bitmap.
///
/// **This is the merge's only term that grows with the corpus, and this is what bounds it.** A term
/// carried by a quarter of the corpus is 10⁸ entities at 10⁹ items, which as a `Vec<u32>` is 400 MB
/// for one record. Above the cap the merge accumulates into a `Bitmap` instead and encodes from
/// that; the two encoders are byte-identical over the same set (pinned by
/// `postings::the_bitmap_and_slice_encoders_agree_byte_for_byte`), so the cap is a memory knob and
/// not a format decision. 16 MB, which no vocabulary reaches by accident.
pub const TEXT_MERGE_SLICE_CAP: usize = 1 << 22;

/// Entities staged before each hand-off to croaring in the merge's bitmap arm: `add_many`
/// amortises over a run of values.
const TEXT_MERGE_STAGE_ENTITIES: usize = 1 << 16;

/// Bytes in one page of the pool. A slice never crosses a page.
const PAGE_BYTES: usize = 1 << 18;

/// The size of a term's slices, in the order its chain takes them, the last repeated.
const SLICE_BYTES: [usize; 9] = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

/// The bytes at the end of each slice that hold the address of the next.
const LINK_BYTES: usize = 8;

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
    terms: Vec<TermState>,
    pool: Pool,
    /// The open document's tokens, as the term's id above the position.
    tokens: Vec<u64>,
    positions: Vec<u32>,
    encoded: Vec<u8>,
}

/// One term's chain, and what it adds up to.
#[derive(Clone, Copy)]
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
            terms: Vec::new(),
            pool: Pool::default(),
            tokens: Vec::new(),
            positions: Vec::new(),
            encoded: Vec::new(),
        }
    }

    /// Add `entity`'s document, whose tokens `tokens` gives in order to the function it is passed,
    /// and spill if the budget is then full. An entity's document is added once.
    pub fn add_document(
        &mut self,
        entity: u32,
        tokens: impl FnOnce(&mut dyn FnMut(&str)),
    ) -> io::Result<()> {
        self.tokens.clear();
        let mut failed = None;
        let mut position = 0u32;
        tokens(&mut |token| {
            if failed.is_some() {
                return;
            }
            match self.intern(token.as_bytes()) {
                Ok(id) => self.tokens.push(u64::from(id) << 32 | u64::from(position)),
                Err(e) => failed = Some(e),
            }
            match position.checked_add(1) {
                Some(next) => position = next,
                None => {
                    failed = Some(invalid(format!(
                        "entity {entity}'s document holds more tokens than a u32 can number; \
                         split it into several items"
                    )))
                }
            }
        });
        if let Some(e) = failed {
            return Err(e);
        }
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
        if self.allocated() >= self.budget {
            self.spill()?;
        }
        Ok(())
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
        let state = &mut self.terms[id as usize];
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
            + self.terms.capacity() * std::mem::size_of::<TermState>()
            + self.keys.capacity()
            + self.table.allocated()
            + self.tokens.capacity() * std::mem::size_of::<u64>()
            + self.positions.capacity() * std::mem::size_of::<u32>()
            + self.encoded.capacity()
    }

    /// Write what the accumulator holds as one sorted run, if it holds anything, and empty it.
    pub fn spill(&mut self) -> io::Result<()> {
        if self.terms.is_empty() {
            return Ok(());
        }
        let path = self
            .dir
            .join(format!("{}-{:05}.spill", self.stem, self.runs.len()));
        let mut writer = TextRunWriter::create(&path, RunPayload::Positions)?;
        self.drain(|term, pairs, body, anchor| writer.push_body(term, pairs, body, anchor))?;
        self.runs.push(writer.finish()?);
        Ok(())
    }

    /// Spill what is left, and hand back every run spilled, for the caller to merge.
    pub fn into_runs(mut self) -> io::Result<Vec<SpillReceipt>> {
        self.spill()?;
        Ok(std::mem::take(&mut self.runs))
    }

    /// Write the index into `writer`: straight from memory where nothing was spilled, and
    /// otherwise by spilling the rest and merging the runs with [`merge_text_runs`].
    pub fn write_index(
        mut self,
        writer: &mut TextIndexWriter,
        merge_slice_cap: usize,
    ) -> io::Result<()> {
        if self.runs.is_empty() {
            let mut entities = Vec::new();
            return self.drain(|term, pairs, body, _| {
                writer.begin_term(text_term(term)?, u64::from(pairs))?;
                entities.clear();
                for_each_entity(body, |entity, positions| {
                    entities.push(entity);
                    writer.push_item(positions)
                })?;
                writer.end_term(Posting::Ascending(&entities))
            });
        }
        let family = format!("{}-merge", self.stem);
        let dir = self.dir.clone();
        merge_text_runs(&dir, &family, self.into_runs()?, writer, merge_slice_cap)
    }

    /// Visit every term in ascending order with its entity count, its body in the run format and
    /// its share of the anchor, and leave the accumulator empty.
    fn drain(
        &mut self,
        mut visit: impl FnMut(&[u8], u32, &[u8], u64) -> io::Result<()>,
    ) -> io::Result<()> {
        let keys = std::mem::take(&mut self.keys);
        let terms = std::mem::take(&mut self.terms);
        let pool = std::mem::take(&mut self.pool);
        let mut sorted: Vec<KeyRef> = Vec::with_capacity(terms.len());
        let mut ranks = Vec::new();
        rank_table(
            &mut self.table,
            |key| key_bytes(&keys, key),
            &mut ranks,
            |key| {
                sorted.push(*key);
                Ok::<(), io::Error>(())
            },
        )?;
        let mut order = vec![0u32; ranks.len()];
        for (id, &rank) in ranks.iter().enumerate() {
            order[rank as usize] = id as u32;
        }
        drop(ranks);
        let (mut chain, mut body, mut entries) = (Vec::new(), Vec::new(), Vec::new());
        for (key, &id) in sorted.iter().zip(&order) {
            let term = key_bytes(&keys, key);
            let state = &terms[id as usize];
            pool.gather(state, &mut chain);
            if state.segmented {
                sort_segments(term, &chain, &mut entries, &mut body)?;
                visit(term, state.pairs, &body, state.anchor)?;
            } else {
                visit(term, state.pairs, &chain, state.anchor)?;
            }
        }
        // The buffers keep their capacity for the next fill, and are counted in it.
        self.keys = keys;
        self.keys.clear();
        self.terms = terms;
        self.terms.clear();
        Ok(())
    }
}

impl Drop for TermAccumulator {
    /// Runs nobody took are removed, so a pass that failed leaves none behind.
    fn drop(&mut self) {
        for run in &self.runs {
            let _ = std::fs::remove_file(&run.path);
        }
    }
}

/// The bytes of `key` in `keys`.
fn key_bytes<'k>(keys: &'k [u8], key: &KeyRef) -> &'k [u8] {
    &keys[key.at as usize..][..key.len as usize]
}

/// A term as the index's dictionary holds it. The analyser gives `&str`, so a term that is not
/// UTF-8 is a damaged run.
fn text_term(term: &[u8]) -> io::Result<&str> {
    std::str::from_utf8(term).map_err(|e| {
        invalid(format!(
            "a text term is not UTF-8 ({e}); the analyser gives only UTF-8, so the run holding it \
             is damaged"
        ))
    })
}

/// Read one varint of a body this crate encoded.
fn next(body: &[u8], at: &mut usize) -> io::Result<u32> {
    take_varint(body, at).map_err(|e| invalid(format!("a text accumulator's chain {e}")))
}

/// Visit each entity of a term's body in the run format, with the term's positions in it.
fn for_each_entity(
    body: &[u8],
    mut visit: impl FnMut(u32, &[u32]) -> io::Result<()>,
) -> io::Result<()> {
    let (mut at, mut entity) = (0, 0u32);
    let mut positions = Vec::new();
    while at < body.len() {
        entity = entity.wrapping_add(next(body, &mut at)?);
        positions.clear();
        let mut position = 0u32;
        for _ in 0..next(body, &mut at)? {
            position = position.wrapping_add(next(body, &mut at)?);
            positions.push(position);
        }
        visit(entity, &positions)?;
    }
    Ok(())
}

/// Re-encode a chain of several ascending segments as one: its entities in ascending order, each
/// with its positions as they were encoded. `entries` is scratch.
fn sort_segments(
    term: &[u8],
    chain: &[u8],
    entries: &mut Vec<(u32, usize, usize)>,
    body: &mut Vec<u8>,
) -> io::Result<()> {
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
        let start = at;
        for _ in 0..next(chain, &mut at)? {
            next(chain, &mut at)?;
        }
        entries.push((entity, start, at));
        last = entity;
    }
    // A stable sort, which merges the ascending segments it finds.
    entries.sort_by_key(|&(entity, _, _)| entity);
    body.clear();
    let mut previous = None;
    for &(entity, start, end) in entries.iter() {
        let gap = match previous {
            None => entity,
            Some(previous) if entity > previous => entity - previous,
            Some(_) => {
                return Err(invalid(format!(
                    "entity {entity} holds term {:?} in two documents; add an entity's document \
                     once",
                    String::from_utf8_lossy(term)
                )))
            }
        };
        put_varint(gap, body);
        body.extend_from_slice(&chain[start..end]);
        previous = Some(entity);
    }
    Ok(())
}

/// Merge text runs into `writer`: cascaded first where there are more than one merge may hold
/// open, then merged k-way on the term. The runs are removed, whether or not the merge succeeds.
/// `family` names the cascade's files in `dir`.
pub fn merge_text_runs(
    dir: &Path,
    family: &str,
    runs: Vec<SpillReceipt>,
    writer: &mut TextIndexWriter,
    merge_slice_cap: usize,
) -> io::Result<()> {
    let mut left = runs.clone();
    let merged = cascade_sorted_runs(dir, family, RunPayload::Positions, runs).and_then(|runs| {
        left = runs;
        merge_runs(&left, writer, merge_slice_cap)
    });
    let removed = left
        .iter()
        .try_for_each(|run| std::fs::remove_file(&run.path));
    merged.and(removed)
}

fn merge_runs(
    runs: &[SpillReceipt],
    writer: &mut TextIndexWriter,
    merge_slice_cap: usize,
) -> io::Result<()> {
    let mut merge = TextRunMerge::open(runs, RunPayload::Positions)?;
    let mut entities: Vec<u32> = Vec::new();
    let mut staged: Vec<u32> = Vec::new();
    while merge.next_term()? {
        writer.begin_term(text_term(merge.term())?, merge.postings())?;
        if merge.postings() <= merge_slice_cap as u64 {
            entities.clear();
            merge.drain(std::convert::identity, &mut |entity, positions| {
                entities.push(entity);
                writer.push_item(positions)
            })?;
            writer.end_term(Posting::Ascending(&entities))?;
        } else {
            let mut bitmap = Bitmap::new();
            staged.clear();
            merge.drain(std::convert::identity, &mut |entity, positions| {
                staged.push(entity);
                if staged.len() >= TEXT_MERGE_STAGE_ENTITIES {
                    bitmap.add_many(&staged);
                    staged.clear();
                }
                writer.push_item(positions)
            })?;
            bitmap.add_many(&staged);
            writer.end_term(Posting::Set(&bitmap))?;
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
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

    /// `term`'s chain, its slices end to end, into `out`.
    fn gather(&self, term: &TermState, out: &mut Vec<u8>) {
        out.clear();
        let (mut at, mut level) = (term.head, 0);
        loop {
            let end = at + (SLICE_BYTES[level] - LINK_BYTES) as u64;
            if (at..=end).contains(&term.tail) {
                out.extend_from_slice(self.bytes(at, (term.tail - at) as usize));
                return;
            }
            out.extend_from_slice(self.bytes(at, (end - at) as usize));
            let link = self.bytes(end, LINK_BYTES);
            at = u64::from_le_bytes(link.try_into().expect("a link is eight bytes"));
            level = (level + 1).min(SLICE_BYTES.len() - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// **A run gives back what was added, whatever order the documents came in.** Documents arrive
    /// in a shuffled entity order with terms repeated within and across them, so terms gather
    /// several segments, under a budget that never spills before the end and one that spills after
    /// every document. The runs merged must give each term's entities ascending with their
    /// positions, as a naive reference has them, and every run's receipt must verify.
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

        for budget in [usize::MAX, 0] {
            let dir = tempfile::tempdir().unwrap();
            let mut accumulator = TermAccumulator::new(dir.path(), "text-run", budget);
            for (entity, tokens) in &documents {
                accumulator
                    .add_document(*entity, |push| tokens.iter().for_each(|t| push(t)))
                    .unwrap();
            }
            let runs = accumulator.into_runs().unwrap();
            assert_eq!(runs.len(), if budget == 0 { documents.len() } else { 1 });

            let mut merge = TextRunMerge::open(&runs, RunPayload::Positions).unwrap();
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
            assert_eq!(read, reference, "budget {budget}");
        }
    }
}
