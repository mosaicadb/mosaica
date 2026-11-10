pub(in crate::filter) mod keyword;
pub(in crate::filter) mod text;

use croaring::Bitmap;
use mosaica_filter::ValueColumn;

use self::keyword::scan_keyword;
use crate::cancel::{gone, CancelToken};
use crate::filter::columns::Layer;
use crate::filter::error::FilterError;
use crate::filter::expr::FilterOperand;

/// One operand against one layer, whichever family the layer belongs to, ended with
/// [`FilterError::Cancelled`] once the request holding `cancel` has gone.
///
/// The layer's dictionary picks the keyword route, not the column's declaration: a keyword layer
/// always carries one, so reading it off the layer keeps a resolve and its ordinals from the same
/// layer rather than from the column's family.
pub(in crate::filter) fn scan_layer(
    column: &str,
    layer: &Layer,
    operand: &FilterOperand,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
) -> Result<Bitmap, FilterError> {
    let scanned = match &layer.dict {
        Some(dict) => {
            scan_keyword(&layer.values, dict, operand, candidate, cancel).map_err(|e| {
                FilterError::DictionaryUnreadable {
                    column: column.to_string(),
                    detail: e.to_string(),
                }
            })?
        }
        None => in_spans(candidate, cancel, |part| scan(&layer.values, operand, part)),
    };
    scanned.ok_or(FilterError::Cancelled)
}

/// The fewest candidates one span of [`in_spans`] holds: a few milliseconds of a scan.
const SPAN_ENTITIES: u64 = 1 << 24;

/// The candidates a scan must be handed before it cuts as many pieces as it ever does for a pool
/// of `threads`: four a thread, of 2¹⁸ candidates at least.
fn fully_parallel(threads: u64) -> u64 {
    threads << 20
}

/// `scan` over `candidate`, or `None` once the request holding `cancel` has gone. A candidate of
/// more than one span is scanned a span at a time, with `cancel` read before each; a scan tests
/// each entity alone, so the spans' answers together are the whole's. A span holds as many
/// candidates as a scan needs to use the whole pool, so it is scanned as much in parallel as the
/// whole would be. Where nothing can cancel, `candidate` is scanned whole.
pub(in crate::filter) fn in_spans(
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
    scan: impl Fn(&Bitmap) -> Bitmap,
) -> Option<Bitmap> {
    let span = SPAN_ENTITIES.max(fully_parallel(rayon::current_num_threads() as u64));
    in_spans_of(span, candidate, cancel, scan)
}

/// [`in_spans`] with spans of `span` candidates. A span's bounds fall on 2¹⁶ blocks, which a scan
/// reads whole, so one may hold up to a block's candidates fewer or more.
fn in_spans_of(
    span: u64,
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
    scan: impl Fn(&Bitmap) -> Bitmap,
) -> Option<Bitmap> {
    if gone(cancel) {
        return None;
    }
    let card = candidate.cardinality();
    let Some(last) = candidate
        .maximum()
        .filter(|_| cancel.is_some() && card > span)
    else {
        return Some(scan(candidate));
    };
    // Where each span begins: the block holding the candidate at each multiple of a span.
    let mut starts = vec![0u32];
    for rank in (span..card).step_by(span as usize) {
        // Below `card`, which is at most 2³², so the rank fits.
        let at = candidate
            .select(rank as u32)
            .map_or(last, |at| at & !0xFFFF);
        if at > *starts.last().expect("starts begins with 0") {
            starts.push(at);
        }
    }
    let mut out = Bitmap::new();
    for (i, &start) in starts.iter().enumerate() {
        if gone(cancel) {
            return None;
        }
        let end = starts.get(i + 1).map_or(last, |next| next - 1);
        let mut part = Bitmap::from_range(start..=end);
        part.and_inplace(candidate);
        out.or_inplace(&scan(&part));
    }
    Some(out)
}

/// One operand against one layer's value column. The dispatch is on the operand, never the value.
pub(in crate::filter) fn scan(
    values: &ValueColumn,
    operand: &FilterOperand,
    candidate: &Bitmap,
) -> Bitmap {
    match operand {
        FilterOperand::Equals(v) => values.scan_eq(candidate, *v),
        FilterOperand::In(vs) => values.scan_in(candidate, vs),
        // A string operand against a layer with no dictionary answers empty: every string operand
        // is keyword, and a keyword layer always carries a dictionary, so this arm is unreachable
        // through the parse. Empty is the fail-safe reading.
        FilterOperand::TextEquals(_)
        | FilterOperand::TextIn(_)
        | FilterOperand::TextPrefix(_)
        | FilterOperand::TextContains(_)
        // `match` and `Phrase` reach no value column either: the text family has none.
        | FilterOperand::Match { .. }
        | FilterOperand::Phrase { .. } => Bitmap::new(),
        FilterOperand::NumEquals(n) => values.scan_num_eq(candidate, *n),
        FilterOperand::NumIn(ns) => values.scan_num_in(candidate, ns),
        FilterOperand::Range { lo, hi } => values.scan_range(candidate, *lo, *hi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// A scan taken a span at a time answers what the scan whole answers, reads its request
    /// before each span, and answers nothing once the request has gone. Spans are cut by how many
    /// candidates they hold, so a candidate spread thinly over entity space is cut as a dense one.
    #[test]
    fn a_scan_in_spans_answers_the_whole_scans_answer_and_stops_between_spans() {
        const SPAN: u64 = 1 << 17;
        // Three spans' worth: one dense from 0, two spread to the top of entity space.
        let mut candidate = Bitmap::from_range(0..SPAN as u32);
        let stride = (u32::MAX - SPAN as u32) / (2 * SPAN as u32);
        let spread: Vec<u32> = (1..=2 * SPAN as u32)
            .map(|i| SPAN as u32 + i * stride)
            .collect();
        candidate.add_many(&spread);
        candidate.add(u32::MAX);
        // Keeps the odd entities, as a scan keeps those whose value matches.
        let odd = |part: &Bitmap| part.iter().filter(|e| e % 2 == 1).collect::<Bitmap>();
        let whole = odd(&candidate);
        assert!(
            whole.contains(u32::MAX),
            "the last entity is a candidate and odd"
        );

        let sizes = RefCell::new(Vec::new());
        let scans = Cell::new(0u32);
        let counted = |part: &Bitmap| {
            scans.set(scans.get() + 1);
            sizes.borrow_mut().push(part.cardinality());
            odd(part)
        };
        let spans = |cancel: &Option<CancelToken>, scan: &dyn Fn(&Bitmap) -> Bitmap| {
            in_spans_of(SPAN, &candidate, cancel, scan)
        };
        assert_eq!(spans(&None, &counted), Some(whole.clone()));
        assert_eq!(
            scans.get(),
            1,
            "with nothing to cancel it the candidate is scanned whole"
        );

        scans.set(0);
        sizes.borrow_mut().clear();
        let live = Some(CancelToken::new());
        assert_eq!(spans(&live, &counted), Some(whole));
        assert_eq!(scans.get(), 4, "three spans and the candidates past them");
        assert_eq!(sizes.borrow().iter().sum::<u64>(), candidate.cardinality());
        assert!(
            sizes.borrow()[..3]
                .iter()
                .all(|held| held.abs_diff(SPAN) <= 1 << 16),
            "each span holds a span of candidates, give or take a block: {:?}",
            sizes.borrow()
        );

        scans.set(0);
        let token = CancelToken::new();
        let flipping = |part: &Bitmap| {
            token.cancel();
            counted(part)
        };
        assert_eq!(spans(&Some(token.clone()), &flipping), None);
        assert_eq!(
            scans.get(),
            1,
            "the span in hand finishes and no other begins"
        );
        assert_eq!(spans(&Some(token.clone()), &counted), None);
        assert_eq!(scans.get(), 1, "a request already gone scans nothing");
    }
}
