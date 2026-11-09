//! A text layer's writer, the one function that writes a merge's terms into it, and the text
//! family's two lifecycle transformations built on them: the fold's merge of every layer into one
//! base, and the coalesce's merge of a window of extents into one extent.
//!
//! # One merge for every route
//!
//! The build, the flush, the fold and the coalesce all write a text layer through
//! [`write_range`], one term range of a [`TermMerge`] at a time: the build and the flush over runs
//! and accumulators, the fold and the coalesce over finished layers ([`LayerTerms`]). So a layer's
//! bytes are a function of its term set alone, whichever route wrote it and however its terms were
//! divided. Each term's posting is held as an ascending slice up to the merge's slice cap and as a
//! bitmap above it, built from the entities as they arrive either way, and encoded through the one
//! canonical posting encoder. A range that is not yet being written encodes into [`HeldTerms`]
//! exactly what the writer would write, and the writer takes it over, a term it left part way
//! through included.
//!
//! What differs between the fold and the coalesce is small, and getting either wrong is silent:
//!
//! - The fold **retires**: `D₀`'s entities are subtracted from every posting and a term whose every
//!   carrier was blanked is not written at all, so the word leaves the corpus with the documents
//!   that used it. A coalesce **retires nothing**, since removal is the fold's alone, so it passes
//!   an empty tombstone set and there is no parameter that could spell anything else.
//!
//! Both write the presence of what they merged, less the tombstones, and both refuse inputs that
//! hold one entity twice: an entity's text is in exactly one layer of a column, and a union of
//! postings would hide a second copy where no later pass could find it.
//!
//! **A term only one layer holds, and none of whose carriers is tombstoned, is copied**: its
//! posting record and its positions record are the bytes that layer stores, so the pass costs a
//! copy for every term the changes since the last fold did not touch. The posting was checked when
//! the layer was opened, as every tag-1 record is, and the positions are checked as a reader
//! decodes them, in the copy as in the original. A term whose carriers are all tombstoned is not
//! copied either.
//!
//! # Why a text column may be coalesced where a keyword column may not
//!
//! Both families carry a **per-layer dictionary**, and a coalesce of either renumbers: the merged
//! dictionary is a new key set and every ordinal in the output means a position in it. For the
//! keyword family that is why the engine's coalesce declines the column — a coalesced `AttrExtent`
//! is composed as *values alone*, so the merged dictionary would have nowhere to be installed and
//! the extent's ordinals would resolve against dictionaries that no longer number them.
//!
//! A text layer has no such gap: its dictionary, its postings, its positions and its presence are
//! **one manifest record** (`TextExtent`), composed together and replaced together. So the
//! renumbering never escapes the layer, and no remap of anything outside it is owed — which is why
//! this merge carries no counterpart to the keyword module's [`crate::coalesce_keyword_extents`]
//! remap guard. There is nothing stored per entity to remap: a text column's index is postings and
//! positions over terms, and its values are record-blob rows the pass never touches.
//!
//! # What the merge checks
//!
//! **Posting *i* and positions record *i* are term *i*'s, and nothing else says so.** A layer whose
//! postings file is short of its dictionary answers "carried by nobody" for every ordinal past the
//! gap, so merging it would write that under-report into the output where no later pass could find
//! it. Each input's three counts are checked against each other before a term is read — the check
//! the reader makes before serving a layer, [`TextLayerCounts`].
//!
//! The dictionary writer refuses a key that does not ascend strictly, so a merge that lost its
//! order stops there rather than publishing a dictionary whose ordinals mean nothing.

use std::fs::File;
use std::io::{self, BufWriter};

use croaring::Bitmap;
use mosaica_authz::{PostingRef, RecordWriter, POSTINGS_COLUMN};
use mosaica_filter::{
    ColumnPositions, ColumnPostings, PositionsCursor, SortedDict, SortedDictWriter, TextIndexFiles,
    TextLayerCounts,
};
use mosaica_types::{AttrLocalId, SMALL_TERM_THRESHOLD_DEFAULT};

use crate::merge::{
    merge_ranges, split, DictCursor, Out, RangeWriter, Ranges, Records, TermMerge, TermSource, Turn,
};
use crate::positions::{HeldRecords, PositionsWriter, RecordSink};
use crate::spill::{Allowance, Sample};

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

/// One input layer: its dictionary, its postings and positions over that dictionary, and the
/// entities it holds a value for. They travel together because an ordinal is a position in *this*
/// dictionary.
pub struct TextLayerRef<'a> {
    pub dict: &'a SortedDict,
    pub postings: &'a ColumnPostings,
    pub positions: &'a ColumnPositions,
    pub present: &'a Bitmap,
}

/// A term's posting as its producer holds it.
pub enum Posting<'a> {
    /// Entities, ascending.
    Ascending(&'a [u32]),
    Set(&'a Bitmap),
}

/// Writes one text layer's dictionary, postings and positions a term at a time and in step, so
/// record *i* of each is term *i*'s. A writer dropped before [`Self::finish`] removes its postings
/// and positions files. The presence is the caller's to write.
pub struct TextIndexWriter {
    open: OpenFiles,
    items: u64,
    terms: u64,
}

struct OpenFiles {
    dict: SortedDictWriter<BufWriter<File>>,
    postings: RecordWriter,
    positions: PositionsWriter,
}

impl TextIndexWriter {
    pub fn create(files: TextIndexFiles) -> io::Result<TextIndexWriter> {
        Ok(TextIndexWriter {
            open: OpenFiles {
                postings: RecordWriter::create(&files.postings, POSTINGS_COLUMN)?,
                positions: PositionsWriter::create(&files.positions)?,
                dict: SortedDictWriter::new(BufWriter::new(File::create(&files.dict)?))?,
            },
            items: 0,
            terms: 0,
        })
    }

    /// Begin `term`, held by `items` items, and return its ordinal. Terms must ascend strictly.
    pub fn begin_term(&mut self, term: &str, items: u64) -> io::Result<u32> {
        let open = &mut self.open;
        let ordinal = open.dict.push(term)?;
        open.positions.begin(items)?;
        self.items = items;
        Ok(ordinal)
    }

    /// The positions of the term's next item, its items in ascending order.
    pub fn push_item(&mut self, positions: &[u32]) -> io::Result<()> {
        self.open.positions.push(positions)
    }

    /// End the term with its posting, which holds the items whose positions were pushed.
    pub fn end_term(&mut self, posting: Posting<'_>) -> io::Result<()> {
        let record = encode(posting, self.terms, self.items)?;
        let open = &mut self.open;
        open.positions.end()?;
        open.postings.append(&record)?;
        self.terms += 1;
        Ok(())
    }

    /// Write `term` whole from `records`, as a finished layer stores them. The posting was checked
    /// when that layer's postings were opened, and the positions are checked wherever a reader
    /// decodes them, so both are written as they are.
    fn copy_term(&mut self, term: &str, records: Records<'_>) -> io::Result<()> {
        let open = &mut self.open;
        open.dict.push(term)?;
        open.positions.copy(records.positions)?;
        open.postings.append(records.posting)?;
        self.terms += 1;
        Ok(())
    }

    /// Complete the three files, and return the number of terms.
    pub fn finish(self) -> io::Result<u64> {
        let OpenFiles {
            dict,
            postings,
            positions,
        } = self.open;
        dict.finish()?;
        postings.finish()?;
        positions.finish()?;
        Ok(self.terms)
    }
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// A term's posting record, checked to hold the `items` items its term was begun for. `term` is
/// its place in the layer, for the refusal.
fn encode(posting: Posting<'_>, term: u64, items: u64) -> io::Result<Vec<u8>> {
    let (held, record) = match posting {
        Posting::Ascending(entities) => (
            entities.len() as u64,
            mosaica_authz::encode_posting(term as usize, entities, SMALL_TERM_THRESHOLD_DEFAULT)?,
        ),
        Posting::Set(entities) => (
            entities.cardinality(),
            mosaica_authz::encode_posting_bitmap(entities, SMALL_TERM_THRESHOLD_DEFAULT)?,
        ),
    };
    if held != items {
        return Err(invalid(format!(
            "term {term} was begun for {items} items and its posting holds {held}"
        )));
    }
    Ok(record)
}

/// What a text layer's terms are written into: the writer, or what a range of a parallel merge
/// holds until its turn.
trait TermOut {
    fn begin_term(&mut self, term: &str, items: u64) -> io::Result<()>;
    fn push_item(&mut self, positions: &[u32]) -> io::Result<()>;
    fn end_term(&mut self, posting: Posting<'_>) -> io::Result<()>;
    fn copy_term(&mut self, term: &str, records: Records<'_>) -> io::Result<()>;
}

impl TermOut for TextIndexWriter {
    fn begin_term(&mut self, term: &str, items: u64) -> io::Result<()> {
        TextIndexWriter::begin_term(self, term, items).map(|_| ())
    }
    fn push_item(&mut self, positions: &[u32]) -> io::Result<()> {
        TextIndexWriter::push_item(self, positions)
    }
    fn end_term(&mut self, posting: Posting<'_>) -> io::Result<()> {
        TextIndexWriter::end_term(self, posting)
    }
    fn copy_term(&mut self, term: &str, records: Records<'_>) -> io::Result<()> {
        TextIndexWriter::copy_term(self, term, records)
    }
}

/// The terms a range of a parallel merge has encoded while ranges before it are still being
/// written: each term's key, posting record and positions record as the writer would write them,
/// and the term it is part way through, whose positions are encoded as far as they have come.
#[derive(Default)]
pub struct HeldTerms {
    keys: Vec<u8>,
    key_ends: Vec<usize>,
    postings: HeldRecords,
    positions: Option<PositionsWriter<HeldRecords>>,
    /// The open term's items, where one is open.
    items: Option<u64>,
}

impl HeldTerms {
    fn positions(&mut self) -> &mut PositionsWriter<HeldRecords> {
        self.positions.get_or_insert_with(PositionsWriter::held)
    }

    fn push_key(&mut self, term: &str) {
        self.keys.extend_from_slice(term.as_bytes());
        self.key_ends.push(self.keys.len());
    }

    fn key(&self, i: usize) -> &str {
        let start = i.checked_sub(1).map_or(0, |before| self.key_ends[before]);
        std::str::from_utf8(&self.keys[start..self.key_ends[i]]).expect("a held key was a str")
    }
}

impl TermOut for HeldTerms {
    fn begin_term(&mut self, term: &str, items: u64) -> io::Result<()> {
        self.positions().begin(items)?;
        self.push_key(term);
        self.items = Some(items);
        Ok(())
    }
    fn push_item(&mut self, positions: &[u32]) -> io::Result<()> {
        self.positions().push(positions)
    }
    fn end_term(&mut self, posting: Posting<'_>) -> io::Result<()> {
        let items = self.items.take().unwrap_or_default();
        let record = encode(posting, self.postings.len() as u64, items)?;
        self.positions().end()?;
        self.postings.append(&record)
    }
    fn copy_term(&mut self, term: &str, records: Records<'_>) -> io::Result<()> {
        self.positions().copy(records.positions)?;
        self.postings.append(records.posting)?;
        self.push_key(term);
        Ok(())
    }
}

impl RangeWriter for TextIndexWriter {
    type Held = HeldTerms;

    fn held_bytes(held: &HeldTerms) -> usize {
        held.keys.len()
            + held.postings.bytes()
            + held.positions.as_ref().map_or(0, |p| p.records().bytes())
    }

    fn write_held(&mut self, held: &mut HeldTerms) -> io::Result<()> {
        let open = &mut self.open;
        if let Some(positions) = &held.positions {
            for i in 0..held.postings.len() {
                open.dict.push(held.key(i))?;
                open.positions.copy(positions.records().record(i))?;
                open.postings.append(held.postings.record(i))?;
                self.terms += 1;
            }
        }
        if let Some(items) = held.items.take() {
            open.dict.push(held.key(held.postings.len()))?;
            open.positions.adopt(held.positions())?;
            self.items = items;
        }
        held.keys.clear();
        held.key_ends.clear();
        held.postings.clear();
        if let Some(positions) = &mut held.positions {
            positions.clear();
        }
        Ok(())
    }
}

/// Where a range's next term or item goes.
fn out<'t>(turn: &'t mut Turn<'_, &mut TextIndexWriter>) -> io::Result<&'t mut dyn TermOut> {
    Ok(match turn.out()? {
        Out::Writer(writer) => &mut **writer,
        Out::Held(held) => held,
    })
}

/// Write the terms `merge` yields into a range's place in the output, holding each term's entities
/// as a slice of at most `slice_cap` and as a bitmap above it. A term no source yields an entity
/// for is not written, and a term one finished layer holds whole is copied from it. Where what the
/// range holds reaches its buffer, it waits for its turn, part way through a term or between two.
pub(crate) fn write_range<S: TermSource>(
    merge: &mut TermMerge<S>,
    turn: &mut Turn<'_, &mut TextIndexWriter>,
    slice_cap: usize,
) -> io::Result<()> {
    let mut entities: Vec<u32> = Vec::new();
    let mut staged: Vec<u32> = Vec::new();
    let item = |turn: &mut Turn<'_, &mut TextIndexWriter>, positions: &[u32]| {
        out(turn)?.push_item(positions)?;
        if turn.full() {
            turn.wait()?;
        }
        Ok(())
    };
    while merge.next_term()? {
        // **The retention statement.** A term every one of whose carriers was blanked is not
        // written at all: no empty posting, no dictionary key and no positions record, so the word
        // leaves the corpus with the documents that used it. A copy is never of such a term.
        let items = merge.postings();
        if items == 0 {
            continue;
        }
        let term = text_term(merge.term())?;
        if let Some(records) = merge.records() {
            // A record copied whole is never held past the buffer: one that would pass it waits
            // for its turn and goes straight into the writer.
            if !turn.fits(records.posting.len() + records.positions.len()) {
                turn.wait()?;
            }
            out(turn)?.copy_term(term, records)?;
            continue;
        }
        out(turn)?.begin_term(term, items)?;
        if items <= slice_cap as u64 {
            entities.clear();
            merge.drain(std::convert::identity, &mut |entity, positions| {
                entities.push(entity);
                item(turn, positions)
            })?;
            out(turn)?.end_term(Posting::Ascending(&entities))?;
        } else {
            let mut bitmap = Bitmap::new();
            staged.clear();
            merge.drain(std::convert::identity, &mut |entity, positions| {
                staged.push(entity);
                if staged.len() >= TEXT_MERGE_STAGE_ENTITIES {
                    bitmap.add_many(&staged);
                    staged.clear();
                }
                item(turn, positions)
            })?;
            bitmap.add_many(&staged);
            out(turn)?.end_term(Posting::Set(&bitmap))?;
        }
    }
    Ok(())
}

/// A term as the index's dictionary holds it. The analyser gives `&str`, and a finished layer's
/// dictionary decodes to `&str`, so a term that is not UTF-8 is a damaged run.
pub(crate) fn text_term(term: &[u8]) -> io::Result<&str> {
    std::str::from_utf8(term).map_err(|e| {
        invalid(format!(
            "a text term is not UTF-8 ({e}); the analyser gives only UTF-8, so the run holding it \
             is damaged"
        ))
    })
}

/// The fold's pass: every layer merged into one base index, with `tombstones` subtracted, under
/// `allowance`.
pub fn merge_text_layers(
    inputs: &[TextLayerRef<'_>],
    tombstones: &Bitmap,
    out: TextIndexFiles,
    allowance: Allowance,
) -> io::Result<()> {
    merge(
        inputs,
        tombstones,
        out,
        "the fold's text pass",
        allowance,
        Ranges::Planned,
    )
}

/// The coalesce's pass: a window of a column's extents merged into one extent.
///
/// Retires nothing, and writes the presence union its replacement rule is checked against — the
/// engine's composition requires a coalesced layer's presence to **equal** the union of the layers
/// it replaces, or the column's coverage drifts and every later disjointness check tests against
/// the wrong set.
///
/// Refuses fewer than two inputs: this collapses a window into one extent and there is nothing to
/// collapse below two.
pub fn coalesce_text_extents(
    inputs: &[TextLayerRef<'_>],
    out: TextIndexFiles,
    allowance: Allowance,
) -> io::Result<()> {
    const PASS: &str = "the text coalesce";
    if inputs.len() < 2 {
        return Err(invalid(format!(
            "{PASS} was given {} extents; it collapses a window of a column's extents into one and \
             there is nothing to collapse below two",
            inputs.len()
        )));
    }
    merge(
        inputs,
        &Bitmap::new(),
        out,
        PASS,
        allowance,
        Ranges::Planned,
    )
}

/// Every term of every layer, in sorted order, with `tombstones` subtracted from each one's
/// postings and positions and an emptied term dropped, and the presence of every input less
/// `tombstones`. The terms are merged in ranges as `ranges` says, divided by the layers' positions
/// bytes, on the threads `allowance` gives.
fn merge(
    inputs: &[TextLayerRef<'_>],
    tombstones: &Bitmap,
    out: TextIndexFiles,
    pass: &str,
    allowance: Allowance,
    ranges: Ranges,
) -> io::Result<()> {
    let presence = crate::disjoint_union(inputs.iter().map(|input| input.present), pass)?;
    for (i, input) in inputs.iter().enumerate() {
        let counts = TextLayerCounts::of(input.dict, input.postings, input.positions);
        if !counts.agree() {
            return Err(invalid(format!(
                "text layer {i} holds {counts}; an ordinal names a record of each, so merging \
                 these would attribute terms to the wrong words"
            )));
        }
    }

    let presence_path = out.presence.clone();
    let mut writer = TextIndexWriter::create(out)?;
    let bytes = inputs
        .iter()
        .map(|input| input.positions.bytes_before(input.positions.record_count()))
        .sum();
    let plan = ranges.plan(allowance, bytes, 0);
    let mut samples: Vec<Sample> = Vec::new();
    if plan.ranges > 1 {
        for input in inputs {
            samples.extend(DictCursor::samples(
                input.dict,
                plan.samples(inputs.len()),
                |ordinal| input.positions.bytes_before(ordinal),
            )?);
        }
    }
    merge_ranges(
        &mut writer,
        &split(samples, plan.ranges),
        plan,
        |range, turn| {
            let sources = inputs
                .iter()
                .map(|input| LayerTerms::new(input, tombstones))
                .collect();
            write_range(
                &mut TermMerge::range(sources, range)?,
                turn,
                TEXT_MERGE_SLICE_CAP,
            )
        },
        |_| {},
    )?;
    writer.finish()?;
    mosaica_filter::write_presence(&presence_path, &presence.andnot(tombstones))
}

/// Entities a finished layer's term source decodes from its posting at a time.
const LAYER_BATCH: usize = 1 << 10;

/// A finished text layer as a term source: each term of its dictionary, with the entities its
/// posting holds that `tombstones` does not, each with its positions.
///
/// A term's count is taken before any entity is read, as the posting's cardinality less its
/// intersection with the tombstones, both read from the mapped posting without materialising it.
struct LayerTerms<'a> {
    dict: DictCursor<'a>,
    postings: &'a ColumnPostings,
    positions: &'a ColumnPositions,
    tombstones: &'a Bitmap,
    posting: Option<PostingRef<'a>>,
    cursor: Option<PositionsCursor<'a>>,
    records: Option<Records<'a>>,
    /// Entities the posting holds, and how many of them are tombstoned.
    held: u64,
    dead: u64,
    /// The next entity's rank in the posting, and the last entity decoded.
    rank: u64,
    last: Option<u32>,
    /// Entities decoded from the posting and not yet yielded, from `at`.
    batch: Vec<u32>,
    at: usize,
    current: Vec<u32>,
}

impl<'a> LayerTerms<'a> {
    fn new(layer: &TextLayerRef<'a>, tombstones: &'a Bitmap) -> LayerTerms<'a> {
        LayerTerms {
            dict: DictCursor::new(layer.dict, None),
            postings: layer.postings,
            positions: layer.positions,
            tombstones,
            posting: None,
            cursor: None,
            records: None,
            held: 0,
            dead: 0,
            rank: 0,
            last: None,
            batch: Vec::with_capacity(LAYER_BATCH),
            at: 0,
            current: Vec::new(),
        }
    }

    /// Decode the posting's next entities into the batch, returning `false` past its last.
    fn refill(&mut self) -> bool {
        self.batch.clear();
        self.at = 0;
        let Some(posting) = &self.posting else {
            return false;
        };
        match posting {
            PostingRef::Array(bytes) => {
                let from = (self.rank as usize).saturating_mul(4).min(bytes.len());
                self.batch.extend(
                    bytes[from..]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .take(LAYER_BATCH)
                        .map(|chunk| u32::from_le_bytes(*chunk)),
                );
            }
            PostingRef::Roaring(view) => {
                let mut entities = view.iter();
                match self.last.map(|last| last.checked_add(1)) {
                    Some(None) => return false,
                    Some(Some(next)) => entities.reset_at_or_after(next),
                    None => {}
                }
                self.batch.resize(LAYER_BATCH, 0);
                let taken = entities.next_many(&mut self.batch);
                self.batch.truncate(taken);
            }
        }
        self.last = self.batch.last().copied().or(self.last);
        !self.batch.is_empty()
    }
}

impl TermSource for LayerTerms<'_> {
    fn seek(&mut self, key: &[u8]) -> io::Result<()> {
        self.dict.seek(key)
    }

    fn next_term(&mut self) -> io::Result<bool> {
        self.posting = None;
        self.cursor = None;
        self.records = None;
        if !self.dict.next_term()? {
            return Ok(false);
        }
        let id = AttrLocalId::new(self.dict.at());
        let posting = self.postings.posting(id)?.ok_or_else(|| {
            invalid(format!(
                "term {} has no posting record, though the layer's counts agree",
                id.raw()
            ))
        })?;
        self.held = posting.cardinality();
        self.dead = match &posting {
            _ if self.tombstones.is_empty() => 0,
            PostingRef::Roaring(view) => self.tombstones.and_cardinality(view),
            array => array.iter().filter(|&e| self.tombstones.contains(e)).count() as u64,
        };
        if self.dead == 0 {
            if let Some(posting) = self.postings.record(id) {
                let positions = self.positions.record(id)?;
                self.records = Some(Records { posting, positions });
            }
        }
        self.cursor = Some(self.positions.cursor(id, self.held)?);
        self.posting = Some(posting);
        self.rank = 0;
        self.last = None;
        self.batch.clear();
        self.at = 0;
        Ok(true)
    }

    fn term(&self) -> &[u8] {
        self.dict.term()
    }

    fn count(&self) -> u64 {
        self.held - self.dead
    }

    fn next_entity(&mut self) -> io::Result<Option<u32>> {
        loop {
            if self.at == self.batch.len() && !self.refill() {
                return Ok(None);
            }
            let entity = self.batch[self.at];
            self.at += 1;
            let rank = self.rank;
            self.rank += 1;
            if self.dead > 0 && self.tombstones.contains(entity) {
                continue;
            }
            let cursor = self.cursor.as_mut().expect("a term has a positions cursor");
            self.current.clear();
            self.current.extend_from_slice(cursor.positions(rank)?);
            return Ok(Some(entity));
        }
    }

    fn positions(&self) -> &[u32] {
        &self.current
    }

    fn records(&self) -> Option<Records<'_>> {
        self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{write_text_index, Allowance, TermAccumulator};
    use std::path::Path;

    type Documents = Vec<(u32, Vec<&'static str>)>;

    fn files(dir: &Path, tag: &str) -> TextIndexFiles {
        TextIndexFiles {
            dict: dir.join(format!("{tag}-dict.bin")),
            postings: dir.join(format!("{tag}-postings")),
            positions: dir.join(format!("{tag}-positions")),
            presence: dir.join(format!("{tag}-presence.roaring")),
        }
    }

    /// A layer over `documents`, written as the build and the flush write one.
    fn build(dir: &Path, tag: &str, documents: &Documents) -> TextIndexFiles {
        let out = files(dir, tag);
        let mut terms = TermAccumulator::new(dir, tag, usize::MAX);
        for (entity, tokens) in documents {
            terms
                .add_document(*entity, |push| tokens.iter().for_each(|t| push(t)))
                .unwrap();
        }
        let mut writer = TextIndexWriter::create(out.clone()).unwrap();
        let gathered = vec![terms.finish().unwrap()];
        let allowance = Allowance::new(0, 0, 1);
        write_text_index(
            dir,
            tag,
            gathered,
            &mut writer,
            1 << 22,
            allowance,
            Ranges::Planned,
        )
        .unwrap();
        writer.finish().unwrap();
        let present: Bitmap = documents.iter().map(|(entity, _)| *entity).collect();
        mosaica_filter::write_presence(&out.presence, &present).unwrap();
        out
    }

    struct Layer {
        dict: SortedDict,
        postings: ColumnPostings,
        positions: ColumnPositions,
        present: Bitmap,
    }

    fn open(files: &TextIndexFiles) -> Layer {
        Layer {
            dict: SortedDict::open(&files.dict, mosaica_filter::Access::Read).unwrap(),
            postings: ColumnPostings::open(&files.postings, false).unwrap(),
            positions: ColumnPositions::open(&files.positions, false).unwrap(),
            present: mosaica_filter::read_presence(&files.presence).unwrap(),
        }
    }

    /// The bytes of each of a layer's files.
    fn bytes(files: &TextIndexFiles) -> Vec<Vec<u8>> {
        files
            .paths()
            .iter()
            .map(|path| std::fs::read(path).unwrap())
            .collect()
    }

    /// **A fold and a coalesce write the layer a build of what survives writes, byte for byte**, on
    /// each of their routes. In the base, `kept` is carried only by entities the fold keeps, so it
    /// is copied whole, and `partly` and `tie` lose a carrier each and are re-encoded. `tie`'s first
    /// container is `{0, 1, 2}` once entity 3 goes, which costs the same as a run and as an array,
    /// and the base held it as a run of four; the folded presence's first container is the same.
    /// `gone`'s only carrier is tombstoned, so it leaves the dictionary though only one layer holds
    /// it. The extent's `fresh` is copied, and `shared` is in both.
    #[test]
    fn a_fold_and_a_coalesce_write_what_a_build_of_the_survivors_writes() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        let base: Documents = (0..4u32)
            .map(|entity| match entity {
                1 => (entity, vec!["tie", "shared", "tie"]),
                3 => (entity, vec!["gone", "tie"]),
                _ => (entity, vec!["tie"]),
            })
            .chain((0..80u32).map(|k| {
                let tokens = match k % 2 {
                    0 => vec!["partly", "partly"],
                    _ => vec!["tie", "kept", "partly", "kept"],
                };
                ((1 << 16) + 2 * k, tokens)
            }))
            .collect();
        let extent: Documents = (0..10u32)
            .map(|k| (200_000 + k, vec!["fresh", "shared"]))
            .collect();
        let tombstones = Bitmap::of(&[3, (1 << 16) + 4]);

        let base_files = build(dir, "base", &base);
        let extent_files = build(dir, "extent", &extent);
        let layers = [open(&base_files), open(&extent_files)];
        let inputs: Vec<TextLayerRef<'_>> = layers
            .iter()
            .map(|layer| TextLayerRef {
                dict: &layer.dict,
                postings: &layer.postings,
                positions: &layer.positions,
                present: &layer.present,
            })
            .collect();

        let folded = files(dir, "folded");
        merge_text_layers(
            &inputs,
            &tombstones,
            folded.clone(),
            Allowance::new(0, 0, 1),
        )
        .unwrap();
        let survivors: Documents = base
            .iter()
            .chain(&extent)
            .filter(|(entity, _)| !tombstones.contains(*entity))
            .cloned()
            .collect();
        let built = bytes(&build(dir, "survivors", &survivors));
        assert_eq!(bytes(&folded), built);
        // In term ranges, each layer started at a range's first key, the same.
        let ranged = files(dir, "ranged");
        let fixed = Ranges::Fixed {
            ranges: 4,
            threads: 2,
            window: 3,
            buffer: 1,
        };
        merge(
            &inputs,
            &tombstones,
            ranged.clone(),
            "the test",
            Allowance::new(0, 0, 2),
            fixed,
        )
        .unwrap();
        assert_eq!(bytes(&ranged), built);

        let coalesced = files(dir, "coalesced");
        coalesce_text_extents(&inputs, coalesced.clone(), Allowance::new(0, 0, 1)).unwrap();
        let every: Documents = base.iter().chain(&extent).cloned().collect();
        assert_eq!(bytes(&coalesced), bytes(&build(dir, "every", &every)));
    }
}
