//! An items read in the order of one number, timestamp or bool field.
//!
//! No sorted structure is stored, so each pass reads the field over the whole set that page reads:
//! the view's visible rows, narrowed to the filter's matches unless every visible row is kept. The
//! pass keeps the keys after the cursor that come first, in parallel pieces of entity space, and
//! answers in that order. Its work depends on the visible set and its values alone, never on rows
//! the viewer cannot see.
//!
//! A key is `(absent, value, shard, entity)`: a value is encoded so that unsigned order is the
//! field's order, inverted for a descending read, and an item without a value comes after every
//! item with one. A pass may also take a histogram of the values after the cursor, from which the
//! cursor carries points: a value and how many keys lie between the cursor and it. A later pass
//! that can hold every key up to a point keeps only those, which needs no selection.

use std::collections::VecDeque;
use std::sync::Arc;

use croaring::Bitmap;
use rayon::prelude::*;

use mosaica_filter::{RecordValue, ValueColumn};
use mosaica_store::key_index::{signed_key, unsigned_key};
use mosaica_types::scalar::ScalarValue;

use super::walk::{segment_of, PageCx, Taken, WalkCx};
use crate::aggregate::ENTITY_PIECE as PIECE;
use crate::cancel::CancelToken;
use crate::error::Result;
use crate::figures::MaskIdentity;
use crate::filter::FilterExpr;
use crate::region::RegionVerdict;
use crate::Generation;

/// Points a cursor carries at most.
pub(super) const MOST_POINTS: usize = 32;
/// Below this many leading bits of distance from the cursor, a histogram bucket holds one value;
/// above, each power of two is divided into this many buckets.
const SUB_BITS: u32 = 6;
const SUB: u64 = 1 << SUB_BITS;
const BUCKETS: usize = (SUB + (64 - SUB_BITS as u64) * SUB) as usize;

/// A row's place in a value order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct SortKey {
    pub(super) absent: bool,
    /// The value, encoded so that unsigned order is the read's order; 0 where absent.
    pub(super) value: u64,
    pub(super) shard: u32,
    pub(super) entity: u32,
}

/// A value and how many keys lie after the position it was counted from, up to and including it.
pub(super) type Point = (u64, u64);

/// How many keys a pass holds: one that selects the first keys, and one that keeps every key up
/// to a point.
#[derive(Debug, Clone, Copy)]
pub(super) struct Caps {
    pub(super) select: usize,
    pub(super) window: usize,
}

/// The field a read is ordered by, as a page reads it.
pub(super) struct SortField {
    pub(super) spelling: String,
    pub(super) descending: bool,
    /// The entity-space value column's key, where the field has one.
    pub(super) held: Option<String>,
}

/// A number, timestamp or bool, encoded so that unsigned order is its order. A float takes its
/// total order, in which a NaN of positive sign is past infinity.
fn number_key(value: RecordValue) -> Option<u64> {
    Some(match value {
        RecordValue::Bool(v) => u64::from(v),
        RecordValue::U8(v) => unsigned_key(u64::from(v)),
        RecordValue::U16(v) => unsigned_key(u64::from(v)),
        RecordValue::U32(v) => unsigned_key(u64::from(v)),
        RecordValue::U64(v) => unsigned_key(v),
        RecordValue::I8(v) => signed_key(i64::from(v)),
        RecordValue::I16(v) => signed_key(i64::from(v)),
        RecordValue::I32(v) => signed_key(i64::from(v)),
        RecordValue::I64(v) | RecordValue::TimestampUs(v) => signed_key(v),
        RecordValue::F32(v) => float_key(f64::from(v)),
        RecordValue::F64(v) => float_key(v),
        _ => return None,
    })
}

fn drawn_key(value: ScalarValue) -> Option<u64> {
    number_key(match value {
        ScalarValue::Bool(v) => RecordValue::Bool(v),
        ScalarValue::U8(v) => RecordValue::U8(v),
        ScalarValue::U16(v) => RecordValue::U16(v),
        ScalarValue::U32(v) => RecordValue::U32(v),
        ScalarValue::U64(v) => RecordValue::U64(v),
        ScalarValue::I8(v) => RecordValue::I8(v),
        ScalarValue::I16(v) => RecordValue::I16(v),
        ScalarValue::I32(v) => RecordValue::I32(v),
        ScalarValue::I64(v) => RecordValue::I64(v),
        ScalarValue::F32(v) => RecordValue::F32(v),
        ScalarValue::F64(v) => RecordValue::F64(v),
        ScalarValue::TimestampUs(v) => RecordValue::TimestampUs(v),
        _ => return None,
    })
}

fn float_key(value: f64) -> u64 {
    let bits = value.to_bits();
    match bits >> 63 {
        1 => !bits,
        _ => bits | 1 << 63,
    }
}

/// The keys of a pass that come first, at most `cap` of them. Keys are offered unsorted; past
/// twice `cap` the buffer is cut back to the `cap` first, and a key after the last of those is
/// refused without being held.
struct Best {
    cap: usize,
    keys: Vec<SortKey>,
    bound: Option<SortKey>,
}

impl Best {
    fn new(cap: usize) -> Best {
        Best {
            cap,
            keys: Vec::new(),
            bound: None,
        }
    }

    fn offer(&mut self, key: SortKey) {
        if self.bound.is_some_and(|bound| key >= bound) {
            return;
        }
        self.keys.push(key);
        if self.keys.len() >= 2 * self.cap {
            self.cut();
        }
    }

    fn cut(&mut self) {
        if self.keys.len() > self.cap {
            self.keys.select_nth_unstable(self.cap - 1);
            self.keys.truncate(self.cap);
            self.bound = self.keys.iter().max().copied();
        }
    }

    fn merge(mut self, other: Best) -> Best {
        for key in other.keys {
            self.offer(key);
        }
        self
    }

    fn sorted(mut self) -> Vec<SortKey> {
        self.cut();
        self.keys.sort_unstable();
        self.keys
    }
}

/// Counts of the values after a cursor's value `lo`, by their distance from it: one bucket per
/// distance below [`SUB`], then [`SUB`] buckets for each power of two.
struct Histogram {
    lo: u64,
    counts: Vec<u64>,
}

impl Histogram {
    fn new(lo: u64) -> Histogram {
        Histogram {
            lo,
            counts: vec![0; BUCKETS],
        }
    }

    fn bucket(distance: u64) -> usize {
        if distance < SUB {
            return distance as usize;
        }
        let shift = 63 - distance.leading_zeros() - SUB_BITS;
        (SUB + u64::from(shift) * SUB + ((distance >> shift) - SUB)) as usize
    }

    /// The last distance bucket `at` holds.
    fn last_of(at: usize) -> u64 {
        let at = at as u64;
        if at < SUB {
            return at;
        }
        let shift = (at - SUB) / SUB;
        let lead = SUB + (at - SUB) % SUB;
        ((lead + 1) << shift).wrapping_sub(1)
    }

    fn add(&mut self, value: u64) {
        self.counts[Self::bucket(value - self.lo)] += 1;
    }

    fn merge(mut self, other: Histogram) -> Histogram {
        for (a, b) in self.counts.iter_mut().zip(other.counts) {
            *a += b;
        }
        self
    }

    /// Points about every `first` keys, each the end of a bucket, and last the end of every value
    /// with every key counted.
    fn points(&self, first: u64) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::new();
        let mut target = first.max(1);
        let mut held: u64 = 0;
        for (at, &count) in self.counts.iter().enumerate() {
            if count == 0 {
                continue;
            }
            if held + count > target && held > 0 && points.len() + 1 < MOST_POINTS {
                let end = self
                    .lo
                    .saturating_add(Self::last_of(at - 1).min(u64::MAX - self.lo));
                if points.last().is_none_or(|&(_, n)| n < held) {
                    points.push((end, held));
                }
                while target < held + count {
                    target = target.saturating_add(first.max(1));
                }
            }
            held += count;
        }
        points.push((u64::MAX, held));
        points
    }
}

/// What one side of a pass reads: its shard's items in the set.
struct Side {
    place: usize,
    shard: u32,
    entities: Bitmap,
}

/// One pass's answer.
struct Passed {
    keys: Vec<SortKey>,
    /// Valued keys past the start and within the bound, kept or not.
    seen: u64,
    histogram: Option<Histogram>,
}

/// The read's state between pages: the field, the keys passed and not yet sent, and the points.
pub(super) struct Sorted {
    pub(super) field: SortField,
    filter: Option<FilterExpr>,
    keep_unmatched: bool,
    /// The last key sent.
    pub(super) cursor: Option<SortKey>,
    /// Keys after the cursor, in order, under the publication and mask they were passed in.
    pending: VecDeque<SortKey>,
    made: Option<(Arc<Generation>, MaskIdentity)>,
    /// By place, the rows the filter matches, where every visible row is kept.
    matched: Vec<Option<Bitmap>>,
    /// Whether no key follows the last pending one.
    exhausted: bool,
    /// Counted from the last pending key, or from the cursor where none is pending.
    points: Vec<Point>,
    pub(super) region: Option<RegionVerdict>,
}

impl Sorted {
    pub(super) fn new(
        field: SortField,
        filter: Option<FilterExpr>,
        keep_unmatched: bool,
        cursor: Option<SortKey>,
        points: Vec<Point>,
    ) -> Sorted {
        Sorted {
            field,
            filter,
            keep_unmatched,
            cursor,
            pending: VecDeque::new(),
            made: None,
            matched: Vec::new(),
            exhausted: false,
            points,
            region: None,
        }
    }

    /// The points, counted from the cursor.
    pub(super) fn points(&self) -> Vec<Point> {
        let behind = self.pending.len() as u64;
        self.points.iter().map(|&(v, n)| (v, n + behind)).collect()
    }

    /// Up to `rows` rows in order under `cx`, after the cursor, and whether none follows them.
    /// Rows passed under another publication or mask are passed again. Keys a page does not keep
    /// go back with [`Sorted::unsend`].
    pub(super) fn take(
        &mut self,
        cx: &PageCx<'_>,
        rows: usize,
        caps: Caps,
        cancel: &Option<CancelToken>,
    ) -> Result<(Vec<Taken>, Vec<SortKey>, bool)> {
        let current = (Arc::clone(cx.generation()), cx.mask_identity());
        let same = self.made.as_ref().is_some_and(|(generation, mask)| {
            super::walk::same_publication(generation, &current.0) && *mask == current.1
        });
        if !same && !self.pending.is_empty() {
            let dropped = self.pending.len() as u64;
            self.pending.clear();
            for point in &mut self.points {
                point.1 += dropped;
            }
            self.exhausted = false;
        }
        while self.pending.len() < rows && !self.exhausted {
            self.pass(cx, caps, cancel)?;
            self.made = Some((Arc::clone(&current.0), current.1));
        }
        let mut taken = Vec::with_capacity(rows.min(self.pending.len()));
        let mut keys = Vec::with_capacity(taken.capacity());
        while taken.len() < rows {
            let Some(key) = self.pending.pop_front() else {
                break;
            };
            if let Some(row) = self.taken(cx, &key) {
                taken.push(row);
                keys.push(key);
            }
            self.cursor = Some(key);
        }
        let end = self.exhausted && self.pending.is_empty();
        Ok((taken, keys, end))
    }

    /// Return the keys a page did not keep, which follow `kept_last`, the last key it kept.
    pub(super) fn unsend(&mut self, kept_last: SortKey, unkept: &[SortKey]) {
        for &key in unkept.iter().rev() {
            self.pending.push_front(key);
        }
        self.cursor = Some(kept_last);
    }

    /// The row `key` names under `cx`, where the item still holds a visible row in the view.
    fn taken(&self, cx: &PageCx<'_>, key: &SortKey) -> Option<Taken> {
        let place = cx
            .reads()
            .iter()
            .copied()
            .find(|&place| cx.shard(place).raw() == key.shard)?;
        let row = cx.row_of(place, key.entity)?;
        if !cx.leaf(place).contains_row(row) {
            return None;
        }
        let (seg, local, mosaica_id) = segment_of(cx.segments(place), row)?;
        let matched = self
            .matched
            .get(place)
            .and_then(Option::as_ref)
            .is_none_or(|rows| rows.contains(row));
        Some(Taken {
            place,
            seg,
            local,
            mosaica_id,
            entity: key.entity,
            matched,
        })
    }

    /// The set each side reads, composed under `cx`'s mask.
    fn sides(&mut self, cx: &PageCx<'_>, cancel: &Option<CancelToken>) -> Result<Vec<Side>> {
        let mut sides = Vec::with_capacity(cx.reads().len());
        let mut matched = Vec::new();
        for &place in cx.reads() {
            let whole = 0..cx.total_rows(place);
            let visible = cx.leaf(place).rows_in_range(whole.clone());
            let filtered = match &self.filter {
                None => None,
                Some(expr) => {
                    let routed = cx.filter_rows(
                        place,
                        expr,
                        None,
                        std::slice::from_ref(&whole),
                        false,
                        cancel,
                    )?;
                    self.region = RegionVerdict::coarsest(self.region, routed.region);
                    Some(routed.rows.rows() & &visible)
                }
            };
            let rows = match (&filtered, self.keep_unmatched) {
                (Some(filtered), false) => filtered.clone(),
                _ => visible,
            };
            let entities = cx.engine.entities_of_rows(&cx.open.served, place, &rows)?;
            if matched.len() <= place {
                matched.resize(place + 1, None);
            }
            matched[place] = filtered.filter(|_| self.keep_unmatched);
            sides.push(Side {
                place,
                shard: cx.shard(place).raw(),
                entities,
            });
        }
        sides.sort_by_key(|side| side.shard);
        self.matched = matched;
        Ok(sides)
    }

    /// One pass after the last pending key, appending at most `cap` keys. Where a point holds no
    /// more than twice `cap`, it offers only the keys up to that point; otherwise it offers every
    /// key and, after a valued key, takes a histogram for new points.
    fn pass(&mut self, cx: &PageCx<'_>, caps: Caps, cancel: &Option<CancelToken>) -> Result<()> {
        let after = self.pending.back().copied().or(self.cursor);
        let sides = self.sides(cx, cancel)?;
        let window = self
            .points
            .iter()
            .rev()
            .find(|&&(_, n)| n <= 2 * caps.window as u64)
            .map(|&(value, _)| value);
        let cap = match window {
            Some(_) => caps.window,
            None => caps.select,
        };
        let valued_done = after.is_some_and(|key| key.absent);
        let mut keys = Vec::new();
        let mut seen = 0u64;
        if !valued_done {
            let lo = after.map(|key| key.value);
            let histogram = window.is_none().then_some(lo).flatten();
            let readers: Vec<Reader<'_>> = sides
                .iter()
                .map(|side| match &self.field.held {
                    Some(column) => Reader::Held(
                        cx.side_shard(side.place)
                            .filter_columns
                            .value_layers(column)
                            .map(|layers| {
                                layers.base().into_iter().chain(layers.extents()).collect()
                            })
                            .unwrap_or_default(),
                    ),
                    None => Reader::Drawn(cx.side_data(side.place), &self.field.spelling),
                })
                .collect();
            let bounds = Bounds {
                descending: self.field.descending,
                after,
                window,
            };
            let passed = cx
                .engine
                .pool
                .install(|| pass_values(&sides, &readers, bounds, cap, histogram, cancel));
            crate::cancel::check_cancelled(cancel)?;
            seen = passed.seen;
            keys = passed.keys;
            self.points = match passed.histogram {
                Some(histogram) => histogram.points(caps.window as u64),
                None => std::mem::take(&mut self.points),
            };
        }
        let through = window.is_none_or(|value| value == u64::MAX);
        let room = cap.saturating_sub(keys.len());
        let mut exhausted = false;
        if valued_done || (through && seen <= cap as u64) {
            let absent = self.absent(cx, &sides, after, room + 1);
            exhausted = absent.len() <= room;
            keys.extend(absent.into_iter().take(room));
        }
        let added = keys.len() as u64;
        let last = keys.last().copied();
        self.points.retain_mut(|point| {
            point.1 = point.1.saturating_sub(added);
            point.1 > 0 && last.is_none_or(|key| !key.absent && point.0 >= key.value)
        });
        if let Some(window) = window {
            self.points.retain(|&(value, _)| value > window);
        }
        self.exhausted = exhausted;
        self.pending.extend(keys);
        Ok(())
    }

    /// Up to `most` keys of the items without a value after `after`, side by side in shard order,
    /// each side's in entity order.
    fn absent(
        &self,
        cx: &PageCx<'_>,
        sides: &[Side],
        after: Option<SortKey>,
        most: usize,
    ) -> Vec<SortKey> {
        let mut out = Vec::new();
        for side in sides {
            if out.len() >= most {
                break;
            }
            let from = match after {
                Some(key) if key.absent && key.shard > side.shard => continue,
                Some(key) if key.absent && key.shard == side.shard => Some(key.entity),
                _ => None,
            };
            let valued = match &self.field.held {
                Some(column) => {
                    let mut valued = Bitmap::new();
                    if let Some(layers) = cx
                        .side_shard(side.place)
                        .filter_columns
                        .value_layers(column)
                    {
                        for layer in layers.base().into_iter().chain(layers.extents()) {
                            valued.or_inplace(&layer.present_in(&side.entities));
                        }
                    }
                    valued
                }
                None => {
                    let mut valued = Bitmap::new();
                    let _ = cx.side_data(side.place).for_each_rendered(
                        &self.field.spelling,
                        &side.entities,
                        &mut |entity, value| {
                            if !matches!(value, ScalarValue::Null) {
                                valued.add(entity);
                            }
                        },
                    );
                    valued
                }
            };
            let mut absent = &side.entities - &valued;
            if let Some(from) = from {
                absent.remove_range(..=from);
            }
            let room = most - out.len();
            out.extend(absent.iter().take(room).map(|entity| SortKey {
                absent: true,
                value: 0,
                shard: side.shard,
                entity,
            }));
        }
        out
    }
}

/// How a side's values are read: from its value column's layers, or from the view's own columns
/// under the field's name.
enum Reader<'a> {
    Held(Vec<&'a ValueColumn>),
    Drawn(&'a mosaica_store::read::ViewData, &'a str),
}

/// Which keys a pass offers: those after `after` whose value is at or before `window`, values
/// inverted for a descending read.
#[derive(Clone, Copy)]
struct Bounds {
    descending: bool,
    after: Option<SortKey>,
    window: Option<u64>,
}

/// The first `cap` valued keys within `bounds` in every side, how many keys there were, and with
/// `histogram`, the counts of their values from that value. A request that has gone stops the
/// pass between pieces.
fn pass_values(
    sides: &[Side],
    readers: &[Reader<'_>],
    bounds: Bounds,
    cap: usize,
    histogram: Option<u64>,
    cancel: &Option<CancelToken>,
) -> Passed {
    let Bounds {
        descending,
        after,
        window,
    } = bounds;
    let mut best = Best::new(cap);
    let mut seen = 0u64;
    let mut counts = histogram.map(Histogram::new);
    for (side, reader) in sides.iter().zip(readers) {
        let shard = side.shard;
        let offer = |best: &mut Best,
                     seen: &mut u64,
                     hist: &mut Option<Histogram>,
                     entity: u32,
                     value: u64| {
            let value = if descending { !value } else { value };
            let key = SortKey {
                absent: false,
                value,
                shard,
                entity,
            };
            if after.is_some_and(|after| key <= after) || window.is_some_and(|w| value > w) {
                return;
            }
            *seen += 1;
            if let Some(hist) = hist {
                hist.add(value);
            }
            best.offer(key);
        };
        let end = side
            .entities
            .maximum()
            .map_or(0, |last| u64::from(last) + 1);
        let fresh = || (Best::new(cap), 0u64, histogram.map(Histogram::new));
        let (b, n, h) = (0..end.div_ceil(PIECE))
            .into_par_iter()
            .fold(fresh, |(mut best, mut seen, mut hist), p| {
                if crate::cancel::gone(cancel) {
                    return (best, seen, hist);
                }
                let range = p * PIECE..((p + 1) * PIECE).min(end);
                let mut piece = Bitmap::from_range(range.start as u32..range.end as u32);
                piece.and_inplace(&side.entities);
                if piece.is_empty() {
                    return (best, seen, hist);
                }
                match reader {
                    Reader::Held(layers) => {
                        for layer in layers {
                            let _ = layer.for_each_record_value_in(&piece, |entity, value| {
                                if let Some(value) = number_key(value) {
                                    offer(&mut best, &mut seen, &mut hist, entity, value);
                                }
                                Ok::<(), ()>(())
                            });
                        }
                    }
                    Reader::Drawn(data, column) => {
                        let _ = data.for_each_rendered(column, &piece, &mut |entity, value| {
                            if let Some(value) = drawn_key(value) {
                                offer(&mut best, &mut seen, &mut hist, entity, value);
                            }
                        });
                    }
                }
                (best, seen, hist)
            })
            .reduce(fresh, merge_passes);
        best = best.merge(b);
        seen += n;
        counts = merge_histograms(counts, h);
    }
    Passed {
        keys: best.sorted(),
        seen,
        histogram: counts,
    }
}

type Partial = (Best, u64, Option<Histogram>);

fn merge_passes((a, m, x): Partial, (b, n, y): Partial) -> Partial {
    (a.merge(b), m + n, merge_histograms(x, y))
}

fn merge_histograms(a: Option<Histogram>, b: Option<Histogram>) -> Option<Histogram> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.merge(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_values_keep_their_order() {
        let floats = [
            f64::NEG_INFINITY,
            -2.5,
            -0.0,
            0.0,
            1e-300,
            3.0,
            f64::INFINITY,
            f64::NAN,
        ];
        let keys: Vec<u64> = floats.iter().map(|&f| float_key(f)).collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "{keys:?}");
        let ints = [i64::MIN, -1, 0, 1, i64::MAX];
        let keys: Vec<u64> = ints.iter().map(|&i| signed_key(i)).collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn every_distance_falls_in_a_bucket_that_ends_at_or_after_it() {
        for distance in (0..4096u64).chain([u64::MAX / 3, u64::MAX - 1, u64::MAX]) {
            let at = Histogram::bucket(distance);
            assert!(at < BUCKETS);
            assert!(Histogram::last_of(at) >= distance, "{distance} in {at}");
            if at > 0 {
                assert!(Histogram::last_of(at - 1) < distance, "{distance} in {at}");
            }
        }
    }

    #[test]
    fn points_count_exactly_the_values_up_to_them() {
        let mut histogram = Histogram::new(1000);
        let values: Vec<u64> = (0..10_000u64).map(|i| 1000 + i * 37 % 50_000).collect();
        for &value in &values {
            histogram.add(value);
        }
        let points = histogram.points(100);
        assert_eq!(points.last(), Some(&(u64::MAX, values.len() as u64)));
        for &(end, count) in &points {
            assert_eq!(values.iter().filter(|&&v| v <= end).count() as u64, count);
        }
        assert!(points
            .windows(2)
            .all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1));
    }

    #[test]
    fn the_best_keys_are_the_first_whatever_the_order_offered() {
        let key = |value| SortKey {
            absent: false,
            value,
            shard: 0,
            entity: 0,
        };
        let mut best = Best::new(10);
        for value in (0..1000u64).rev() {
            best.offer(key(value * 7919 % 1000));
        }
        let kept: Vec<u64> = best.sorted().iter().map(|k| k.value).collect();
        assert_eq!(kept, (0..10).collect::<Vec<_>>());
    }
}
