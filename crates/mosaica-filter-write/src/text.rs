//! The text family's two lifecycle transformations: the fold's merge of every layer into one base,
//! and the coalesce's merge of a window of extents into one extent (`records-and-search.md` §7;
//! `filter-index.md` §5.2 for the coalesce, §6.2 for the fold).
//!
//! **One merge, two obligations**, exactly as the value and keyword families are arranged here. The
//! k-way walk below is the whole of both passes: every term of every layer in sorted order, its
//! posting the union of the layers that hold it and its positions theirs in item order, written
//! into one dictionary, one postings file and one positions file. What differs is small, and
//! getting either wrong is silent:
//!
//! - The fold **retires**: `D₀`'s entities are subtracted from every posting and a term whose every
//!   carrier was blanked is not written at all, so the word leaves the corpus with the documents
//!   that used it. A coalesce **retires nothing** — removal is Rule F's alone (write-path §5.4) —
//!   so it passes an empty tombstone set and there is no parameter that could spell anything else.
//!
//! Both write the presence of what they merged, less the tombstones, and both refuse inputs that
//! hold one entity twice: an entity's text is in exactly one layer of a column, and a union of
//! postings would hide a second copy where no later pass could find it.
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
use mosaica_authz::{RecordWriter, POSTINGS_COLUMN};
use mosaica_filter::{
    ColumnPositions, ColumnPostings, PositionsCursor, SortedDict, SortedDictWriter, TextIndexFiles,
    TextLayerCounts,
};
use mosaica_types::{AttrLocalId, SMALL_TERM_THRESHOLD_DEFAULT};

use crate::positions::PositionsWriter;

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
        let (held, record) = match posting {
            Posting::Ascending(entities) => (
                entities.len() as u64,
                mosaica_authz::encode_posting(
                    self.terms as usize,
                    entities,
                    SMALL_TERM_THRESHOLD_DEFAULT,
                )?,
            ),
            Posting::Set(entities) => (
                entities.cardinality(),
                mosaica_authz::encode_posting_bitmap(entities, SMALL_TERM_THRESHOLD_DEFAULT)?,
            ),
        };
        if held != self.items {
            return Err(invalid(format!(
                "term {} was begun for {} items and its posting holds {held}",
                self.terms, self.items
            )));
        }
        let open = &mut self.open;
        open.positions.end()?;
        open.postings.append(&record)?;
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

/// The fold's pass: every layer merged into one base index, with `tombstones` subtracted.
pub fn merge_text_layers(
    inputs: &[TextLayerRef<'_>],
    tombstones: &Bitmap,
    out: TextIndexFiles,
) -> io::Result<()> {
    merge(inputs, tombstones, out, "the fold's text pass")
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
pub fn coalesce_text_extents(inputs: &[TextLayerRef<'_>], out: TextIndexFiles) -> io::Result<()> {
    const PASS: &str = "the text coalesce";
    if inputs.len() < 2 {
        return Err(invalid(format!(
            "{PASS} was given {} extents; it collapses a window of a column's extents into one and \
             there is nothing to collapse below two",
            inputs.len()
        )));
    }
    merge(inputs, &Bitmap::new(), out, PASS)
}

/// One input's share of the term being merged: the entities its posting holds and a cursor over
/// their positions, read in step.
struct Share<'a> {
    entities: croaring::bitmap::BitmapIterator<'a>,
    head: Option<u32>,
    rank: u64,
    positions: PositionsCursor<'a>,
}

/// The k-way merge itself: every term of every layer, in sorted order, with `tombstones` subtracted
/// from each one's postings and positions and an emptied term dropped, and the presence of every
/// input less `tombstones`.
fn merge(
    inputs: &[TextLayerRef<'_>],
    tombstones: &Bitmap,
    out: TextIndexFiles,
    pass: &str,
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

    // One decode buffer for the whole merge, and one owned key per layer: `key_of` borrows the
    // scratch it decodes into, and a cursor has to hold its key across the comparisons that pick
    // the least. `None` is an exhausted layer.
    let mut scratch: Vec<u8> = Vec::new();
    let mut at: Vec<u32> = vec![0; inputs.len()];
    let mut current: Vec<Option<String>> = Vec::with_capacity(inputs.len());
    for input in inputs {
        current.push(cursor_key(input.dict, 0, &mut scratch)?);
    }

    let mut least = String::new();
    let mut holding: Vec<(usize, AttrLocalId, Bitmap)> = Vec::with_capacity(inputs.len());
    loop {
        // Linear over the layers rather than through a heap: the layer count is one base plus the
        // flush extents published since the last fold, so it is small, and a heap would cost a
        // clone per term to save a comparison per term.
        let mut chosen: Option<usize> = None;
        for (i, key) in current.iter().enumerate() {
            let Some(key) = key else { continue };
            if chosen.is_none_or(|c| key < current[c].as_ref().expect("a chosen layer has a key")) {
                chosen = Some(i);
            }
        }
        let Some(chosen) = chosen else { break };
        least.clear();
        least.push_str(
            current[chosen]
                .as_ref()
                .expect("the chosen layer has a key"),
        );

        // **Every layer holding this term, not the first**: a word in two layers is one term of the
        // merged index, and its posting is the union of theirs.
        holding.clear();
        let mut entities = Bitmap::new();
        for i in 0..inputs.len() {
            if current[i].as_deref() != Some(least.as_str()) {
                continue;
            }
            let ordinal = AttrLocalId::new(at[i]);
            let held = inputs[i].postings.entities(ordinal)?;
            entities |= &held;
            holding.push((i, ordinal, held));
            at[i] += 1;
            current[i] = cursor_key(inputs[i].dict, at[i], &mut scratch)?;
        }
        entities.andnot_inplace(tombstones);
        // **The retention statement.** A term every one of whose carriers was blanked is not
        // written — not as an empty posting, not as a dictionary key, not as a positions record —
        // so the word leaves the corpus with the documents that used it. With an empty tombstone
        // set, as a coalesce passes, nothing here can fire: no input posting is empty, the writers
        // never emit one.
        if entities.is_empty() {
            continue;
        }
        // The writer refuses a key that does not ascend strictly, so a merge that lost its order
        // stops here rather than publishing a dictionary whose ordinals mean nothing.
        writer.begin_term(&least, entities.cardinality())?;
        let mut shares: Vec<Share<'_>> = Vec::with_capacity(holding.len());
        for (i, ordinal, held) in &holding {
            let mut iter = held.iter();
            shares.push(Share {
                head: iter.next(),
                entities: iter,
                rank: 0,
                positions: inputs[*i].positions.cursor(*ordinal, held.cardinality())?,
            });
        }
        // The inputs' items in item order, each kept item's positions copied across.
        loop {
            let mut next: Option<usize> = None;
            for (s, share) in shares.iter().enumerate() {
                let Some(head) = share.head else { continue };
                if next.is_none_or(|n| head < shares[n].head.expect("a chosen share has a head")) {
                    next = Some(s);
                }
            }
            let Some(next) = next else { break };
            let share = &mut shares[next];
            let entity = share.head.expect("the chosen share has a head");
            if !tombstones.contains(entity) {
                writer.push_item(share.positions.positions(share.rank)?)?;
            }
            share.rank += 1;
            share.head = share.entities.next();
        }
        drop(shares);
        writer.end_term(Posting::Set(&entities))?;
    }

    writer.finish()?;
    mosaica_filter::write_presence(&presence_path, &presence.andnot(tombstones))
}

/// The key at `ordinal`, owned, or `None` where the ordinal is past the dictionary's end.
fn cursor_key(dict: &SortedDict, ordinal: u32, scratch: &mut Vec<u8>) -> io::Result<Option<String>> {
    if ordinal >= dict.len() {
        return Ok(None);
    }
    let key = dict.key_of(ordinal, scratch)?;
    Ok(Some(key.to_string()))
}
