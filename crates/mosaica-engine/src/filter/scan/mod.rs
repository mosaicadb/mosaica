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

/// Entity ids one span of [`in_spans`] covers: a few milliseconds of a scan, and many times the
/// smallest piece a scan hands a thread, so a span is scanned as much in parallel as the whole.
const SPAN_ENTITIES: u64 = 1 << 24;

/// `scan` over `candidate`, or `None` once the request holding `cancel` has gone. A candidate
/// reaching past one span of entity ids is scanned a span at a time, with `cancel` read before
/// each; a scan tests each entity alone, so the spans' answers together are the whole's. Where
/// nothing can cancel, `candidate` is scanned whole.
pub(in crate::filter) fn in_spans(
    candidate: &Bitmap,
    cancel: &Option<CancelToken>,
    scan: impl Fn(&Bitmap) -> Bitmap,
) -> Option<Bitmap> {
    if gone(cancel) {
        return None;
    }
    let end = candidate.maximum().map_or(0, |last| u64::from(last) + 1);
    if cancel.is_none() || end <= SPAN_ENTITIES {
        return Some(scan(candidate));
    }
    let mut out = Bitmap::new();
    for lo in (0..end).step_by(SPAN_ENTITIES as usize) {
        if gone(cancel) {
            return None;
        }
        let hi = (lo + SPAN_ENTITIES).min(end);
        let mut part = Bitmap::from_range(lo as u32..=(hi - 1) as u32);
        part.and_inplace(candidate);
        if !part.is_empty() {
            out.or_inplace(&scan(&part));
        }
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
    use std::cell::Cell;

    /// A scan taken a span at a time answers what the scan whole answers, reads its request
    /// before each span, and answers nothing once the request has gone.
    #[test]
    fn a_scan_in_spans_answers_the_whole_scans_answer_and_stops_between_spans() {
        let span = SPAN_ENTITIES as u32;
        let candidate = Bitmap::of(&[5, span - 1, span, 2 * span + 9, 5 * span + 1]);
        // Keeps the odd entities, as a scan keeps those whose value matches.
        let odd = |part: &Bitmap| part.iter().filter(|e| e % 2 == 1).collect::<Bitmap>();
        let whole = odd(&candidate);

        let scans = Cell::new(0u32);
        let counted = |part: &Bitmap| {
            scans.set(scans.get() + 1);
            odd(part)
        };
        assert_eq!(in_spans(&candidate, &None, counted), Some(whole.clone()));
        assert_eq!(
            scans.get(),
            1,
            "with nothing to cancel it the candidate is scanned whole"
        );

        scans.set(0);
        let live = Some(CancelToken::new());
        assert_eq!(in_spans(&candidate, &live, counted), Some(whole));
        assert_eq!(
            scans.get(),
            4,
            "one scan for each span that holds a candidate"
        );

        scans.set(0);
        let token = CancelToken::new();
        let flipping = |part: &Bitmap| {
            token.cancel();
            counted(part)
        };
        assert_eq!(in_spans(&candidate, &Some(token.clone()), flipping), None);
        assert_eq!(
            scans.get(),
            1,
            "the span in hand finishes and no other begins"
        );
        assert_eq!(in_spans(&candidate, &Some(token.clone()), counted), None);
        assert_eq!(scans.get(), 1, "a request already gone scans nothing");
    }
}
