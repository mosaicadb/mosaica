use croaring::Bitmap;
use mosaica_authz::PostingBitmap;
use mosaica_filter::{ColumnPositions, ColumnPostings, PositionsCursor, SortedDict};
use mosaica_roaring::RankedRuns;
use mosaica_types::AttrLocalId;

use crate::cancel::CancelToken;
use crate::filter::error::FilterError;
use crate::filter::{check_cancelled, CANCEL_EVERY};

/// `match` over one layer of text column `column`: intersect the tokens' postings inside the
/// candidate, or count them where fewer than all are required.
///
/// Every posting is masked as it is read, before anything is unioned or counted, so no bitmap that
/// reaches the answer ever holds an entity outside the candidate. An unresolved token contributes
/// an empty posting rather than short-circuiting: under m-of-n it must still consume its place in
/// the count, or `match` of three tokens with `minimum = 2` would silently become a two-token
/// question when one is absent from the corpus.
pub(in crate::filter) fn text_match(
    column: &str,
    dict: &SortedDict,
    postings: &ColumnPostings,
    tokens: &[String],
    minimum: u32,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
) -> Result<Bitmap, FilterError> {
    let unreadable = |e: std::io::Error| FilterError::postings_unreadable(column, e);
    if tokens.is_empty() || minimum == 0 {
        // An empty `match` matches nothing rather than everything, the same reading `any_of([])`
        // takes.
        return Ok(Bitmap::new());
    }
    // `minimum` counts distinct tokens, the caller's query already deduplicated, so a request for
    // four of two words is one no item can meet.
    if minimum as usize > tokens.len() {
        return Ok(Bitmap::new());
    }
    // Each token's ordinal, resolved before any posting is read, so the conjunction below can
    // narrow token by token without a second dictionary pass.
    let mut ordinals: Vec<Option<u32>> = Vec::with_capacity(tokens.len());
    for token in tokens {
        ordinals.push(
            dict.resolve(token)
                .map_err(|e| FilterError::postings_unreadable(column, e))?,
        );
    }

    // Plain `match`: one running set, narrowed token by token, rather than one bitmap per token
    // intersected at the end. The set only shrinks and starts at the candidate, so the answer
    // stays inside it from the first step; each step after the first also works against an
    // already-narrowed set, cheaper by the same container-count reasoning as any bitmap operation.
    if minimum as usize == tokens.len() {
        // The candidate is borrowed for the first step, never cloned: a clone of a million-entity
        // mask costs ~800 ns, most of a one-word query's whole budget.
        let mut live: Option<Bitmap> = None;
        for ordinal in &ordinals {
            match live.as_mut() {
                None => {
                    live = Some(match ordinal {
                        Some(ordinal) => postings
                            .narrow(AttrLocalId::new(*ordinal), candidate)
                            .map_err(unreadable)?,
                        None => Bitmap::new(),
                    });
                }
                // Every token after it narrows the running set in place, which allocates nothing.
                Some(live) => match ordinal {
                    Some(ordinal) => postings
                        .narrow_inplace(AttrLocalId::new(*ordinal), live)
                        .map_err(unreadable)?,
                    None => live.clear(),
                },
            }
        }
        let mut out = live.unwrap_or_default();
        out.run_optimize();
        return Ok(out);
    }

    // m-of-n needs every token's answer at once, a count cannot be accumulated into one set, so it
    // holds n bitmaps, each bounded by the candidate rather than the posting.
    let mut per_token: Vec<Bitmap> = Vec::with_capacity(tokens.len());
    for ordinal in &ordinals {
        per_token.push(match ordinal {
            Some(ordinal) => postings
                .narrow(AttrLocalId::new(*ordinal), candidate)
                .map_err(unreadable)?,
            None => Bitmap::new(),
        });
    }

    // How many of the tokens each candidate entity carries, counted over the union rather than the
    // candidate, so the work is the postings' size and not the mask's.
    let mut union = Bitmap::new();
    for token in &per_token {
        union |= token;
    }
    let mut out = Bitmap::new();
    for (walked, entity) in union.iter().enumerate() {
        if walked % CANCEL_EVERY == 0 {
            check_cancelled(cancel)?;
        }
        let hits = per_token.iter().filter(|t| t.contains(entity)).count();
        if hits as u32 >= minimum {
            out.add(entity);
        }
    }
    Ok(out)
}

/// The survivors of a phrase's conjunction in one layer that hold its words adjacent and in order,
/// read from the layer's positions rather than from the text.
///
/// Survivors ascend and lie in every word's posting, so each distinct word walks its posting's
/// runs against the survivors once ([`RankedRuns`]) for the rank of each, and reads each survivor's
/// entry once however often the word occurs in the phrase. The rarest word is read first, and each
/// further word is read only while some start could still fit.
pub(in crate::filter) fn phrase_match(
    column: &str,
    dict: &SortedDict,
    postings: &ColumnPostings,
    positions: &ColumnPositions,
    phrase: &[String],
    survivors: &Bitmap,
    cancel: &Option<CancelToken>,
) -> Result<Bitmap, FilterError> {
    let unreadable = |e: std::io::Error| FilterError::postings_unreadable(column, e);
    struct Word<'a> {
        ordinal: u32,
        posting: PostingBitmap<'a>,
        /// Where the word stands in the phrase.
        offsets: Vec<u32>,
    }
    let mut words: Vec<Word<'_>> = Vec::new();
    for (offset, token) in (0u32..).zip(phrase) {
        let resolved = dict
            .resolve(token)
            .map_err(|e| FilterError::postings_unreadable(column, e))?;
        let Some(ordinal) = resolved else {
            return Ok(Bitmap::new());
        };
        if let Some(word) = words.iter_mut().find(|w| w.ordinal == ordinal) {
            word.offsets.push(offset);
            continue;
        }
        let Some(posting) = postings
            .posting(AttrLocalId::new(ordinal))
            .map_err(unreadable)?
        else {
            return Ok(Bitmap::new());
        };
        words.push(Word {
            ordinal,
            posting: posting.into_bitmap(),
            offsets: vec![offset],
        });
    }
    words.sort_by_key(|word| word.posting.cardinality());

    /// One word's walk: its runs against the survivors, the run the last survivor was in, and its
    /// positions.
    struct Cursor<'a, 'b> {
        runs: RankedRuns<'a, 'b>,
        run: Option<(u32, u32, u64)>,
        positions: PositionsCursor<'a>,
        offsets: &'a [u32],
    }
    impl Cursor<'_, '_> {
        /// The rank of `entity`, a survivor at or past the last one asked, in the word's posting.
        fn rank(&mut self, entity: u32) -> Option<u64> {
            loop {
                if let Some((start, last, rank)) = self.run {
                    if last >= entity {
                        return (start <= entity).then(|| rank + u64::from(entity - start));
                    }
                }
                self.run = Some(self.runs.next()?);
            }
        }
    }
    let mut cursors = Vec::with_capacity(words.len());
    for word in &words {
        let set: &Bitmap = &word.posting;
        cursors.push(Cursor {
            runs: RankedRuns::new(set, survivors),
            run: None,
            positions: positions
                .cursor(AttrLocalId::new(word.ordinal), set.cardinality())
                .map_err(unreadable)?,
            offsets: &word.offsets,
        });
    }

    let mut out = Bitmap::new();
    // Where the phrase could start in the current survivor's token sequence.
    let mut starts: Vec<u32> = Vec::new();
    for (walked, entity) in survivors.iter().enumerate() {
        if walked % CANCEL_EVERY == 0 {
            check_cancelled(cancel)?;
        }
        starts.clear();
        for (w, word) in cursors.iter_mut().enumerate() {
            let rank = word.rank(entity).ok_or_else(|| {
                FilterError::postings_unreadable(
                    column,
                    "a phrase survivor is missing from the posting of one of its words",
                )
            })?;
            let held = word.positions.positions(rank).map_err(unreadable)?;
            let mut offsets = word.offsets.iter().copied();
            if w == 0 {
                let first = offsets.next().expect("a word stands somewhere in the phrase");
                starts.extend(held.iter().filter_map(|p| p.checked_sub(first)));
            }
            for offset in offsets {
                starts.retain(|s| {
                    s.checked_add(offset)
                        .is_some_and(|p| held.binary_search(&p).is_ok())
                });
            }
            if starts.is_empty() {
                break;
            }
        }
        if !starts.is_empty() {
            out.add(entity);
        }
    }
    Ok(out)
}
