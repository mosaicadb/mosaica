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
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use croaring::Bitmap;
use rayon::prelude::*;

use mosaica_filter::{RecordValue, ValueColumn};
use mosaica_roaring::for_each_run_in;
use mosaica_store::key_index::{signed_key, unsigned_key};
use mosaica_store::read::SegmentData;
use mosaica_types::{IdentityKey, ShardId};

use super::walk::{entity_in, same_publication, segment_of, PageCx, Taken, WalkCx};
use crate::aggregate::{for_each_drawn_number, pieces, ENTITY_PIECE as PIECE};
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
    /// The entity-space value column's key, where a page reads the field from one; `None` where
    /// it reads the view's own columns.
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

/// The digest of what a read's points were counted under: its filter and whether it keeps the
/// unmatched. A request that changes either is answered without the points.
pub(super) fn scope_of(filter: Option<&FilterExpr>, keep_unmatched: bool) -> u64 {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"mosaica-items-order-scope-v1");
    hasher.update([u8::from(keep_unmatched)]);
    hasher.update(format!("{filter:?}").as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
}

/// What one side of a pass reads: its shard's items in the set, as entities where the field has a
/// value column and as the view's rows where it is drawn only.
struct Side {
    place: usize,
    shard: ShardId,
    set: Bitmap,
}

/// The sets a response's pages read, composed once for a publication and a mask.
struct Composed {
    generation: Arc<Generation>,
    mask: MaskIdentity,
    /// In shard order.
    sides: Vec<Side>,
    /// By place, the rows the filter matches, where every visible row is kept.
    matched: Vec<Option<Bitmap>>,
    /// The visible rows the filter matches, over every side.
    matches: u64,
}

/// One pass's answer.
struct Passed {
    keys: Vec<SortKey>,
    /// Valued keys past the start and within the bound, kept or not.
    seen: u64,
    histogram: Option<Histogram>,
    /// The window held more keys than the pass may hold, so it was abandoned.
    overfull: bool,
}

/// The read's state between pages: the field, the sets composed, the keys passed and not yet sent,
/// and the points.
pub(super) struct Sorted {
    pub(super) field: SortField,
    filter: Option<FilterExpr>,
    keep_unmatched: bool,
    /// The digest the points are bound to.
    pub(super) scope: u64,
    /// The last key sent.
    pub(super) cursor: Option<SortKey>,
    composed: Option<Composed>,
    /// Keys after the cursor, in order, passed under `composed`.
    pending: VecDeque<SortKey>,
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
        let scope = scope_of(filter.as_ref(), keep_unmatched);
        Sorted {
            field,
            filter,
            keep_unmatched,
            scope,
            cursor,
            composed: None,
            pending: VecDeque::new(),
            exhausted: false,
            points,
            region: None,
        }
    }

    /// Drop the points, which were counted under another filter.
    pub(super) fn forget_points(&mut self) {
        self.points.clear();
    }

    /// The points, counted from the cursor.
    pub(super) fn points(&self) -> Vec<Point> {
        let behind = self.pending.len() as u64;
        self.points.iter().map(|&(v, n)| (v, n + behind)).collect()
    }

    /// How many visible items the filter matches, from the sets the first page reads.
    pub(super) fn count(
        &mut self,
        cx: &PageCx<'_>,
        cancel: &Option<CancelToken>,
    ) -> Result<(u64, Option<RegionVerdict>)> {
        self.compose(cx, cancel)?;
        let matches = self.composed.as_ref().map_or(0, |composed| composed.matches);
        Ok((matches, self.region))
    }

    /// Up to `rows` rows in order under `cx`, after the cursor, and whether none follows them.
    /// The rows fall short of `rows` only where the read ends. Keys a page does not keep go back
    /// with [`Sorted::unsend`].
    pub(super) fn take(
        &mut self,
        cx: &PageCx<'_>,
        rows: usize,
        caps: Caps,
        cancel: &Option<CancelToken>,
    ) -> Result<(Vec<Taken>, Vec<SortKey>, bool)> {
        self.compose(cx, cancel)?;
        let (mut taken, mut keys) = (Vec::new(), Vec::new());
        while taken.len() < rows {
            let Some(key) = self.pending.pop_front() else {
                if self.exhausted {
                    break;
                }
                self.pass(cx, caps, cancel)?;
                continue;
            };
            if let Some(row) = self.taken(cx, &key) {
                taken.push(row);
                keys.push(key);
            }
            self.cursor = Some(key);
        }
        Ok((taken, keys, self.exhausted && self.pending.is_empty()))
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
            .composed
            .as_ref()
            .and_then(|composed| composed.matched.get(place)?.as_ref())
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

    /// The sets each side reads, composed under `cx`'s mask unless they already are. Keys passed
    /// under another publication or mask are dropped, to be passed again.
    fn compose(&mut self, cx: &PageCx<'_>, cancel: &Option<CancelToken>) -> Result<()> {
        let mask = cx.mask_identity();
        let held = self.composed.as_ref().is_some_and(|composed| {
            composed.mask == mask && same_publication(&composed.generation, cx.generation())
        });
        if held {
            return Ok(());
        }
        if !self.pending.is_empty() {
            let dropped = self.pending.len() as u64;
            self.pending.clear();
            for point in &mut self.points {
                point.1 += dropped;
            }
            self.exhausted = false;
        }
        let (mut sides, mut matched, mut matches) = (Vec::new(), Vec::new(), 0u64);
        for &place in cx.reads() {
            let whole = 0..cx.total_rows(place);
            let visible = cx.leaf(place).rows_in_range(whole.clone());
            let filtered = match &self.filter {
                None => None,
                Some(expr) => {
                    let domain = std::slice::from_ref(&whole);
                    let routed = cx.filter_rows(place, expr, None, domain, false, cancel)?;
                    self.region = RegionVerdict::coarsest(self.region, routed.region);
                    Some(routed.rows.rows() & &visible)
                }
            };
            matches += filtered.as_ref().unwrap_or(&visible).cardinality();
            let rows = match (&filtered, self.keep_unmatched) {
                (Some(filtered), false) => filtered.clone(),
                _ => visible,
            };
            let set = match self.field.held {
                Some(_) => cx.engine.entities_of_rows(&cx.open.served, place, &rows)?,
                None => rows,
            };
            if matched.len() <= place {
                matched.resize(place + 1, None);
            }
            matched[place] = filtered.filter(|_| self.keep_unmatched);
            sides.push(Side {
                place,
                shard: cx.shard(place),
                set,
            });
        }
        sides.sort_by_key(|side| side.shard.raw());
        self.composed = Some(Composed {
            generation: Arc::clone(cx.generation()),
            mask,
            sides,
            matched,
            matches,
        });
        Ok(())
    }

    /// One pass after the last pending key, appending its keys. Where a point holds no more than
    /// twice the window's cap, it offers only the keys up to that point, and abandons the window
    /// for a selecting pass once more keys than that arrive. A selecting pass offers every key,
    /// keeps the first `caps.select` and, after a valued key, takes a histogram for new points.
    fn pass(&mut self, cx: &PageCx<'_>, caps: Caps, cancel: &Option<CancelToken>) -> Result<()> {
        let composed = self.composed.take().expect("the sets are composed before a pass");
        let passed = self.pass_over(cx, &composed, caps, cancel);
        self.composed = Some(composed);
        passed
    }

    fn pass_over(
        &mut self,
        cx: &PageCx<'_>,
        composed: &Composed,
        caps: Caps,
        cancel: &Option<CancelToken>,
    ) -> Result<()> {
        let after = self.pending.back().copied().or(self.cursor);
        let most = 2 * caps.window as u64;
        let mut window = self
            .points
            .iter()
            .rev()
            .find(|&&(_, n)| n <= most)
            .map(|&(value, _)| value);
        let valued_done = after.is_some_and(|key| key.absent);
        let (mut keys, mut seen) = (Vec::new(), 0u64);
        if !valued_done {
            let identity_key = &cx.engine.identity_key;
            let readers: Vec<Reader<'_>> = composed
                .sides
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
                    None => Reader::Drawn {
                        segments: cx.segments(side.place),
                        column: &self.field.spelling,
                        identity_key,
                    },
                })
                .collect();
            loop {
                let bounds = Bounds {
                    descending: self.field.descending,
                    after,
                    window,
                    most: window.map(|_| most),
                };
                let cap = window.map_or(caps.select, |_| caps.window);
                let histogram = window.is_none().then(|| after.map(|key| key.value)).flatten();
                let sides = &composed.sides;
                let passed = cx
                    .engine
                    .pool
                    .install(|| pass_values(sides, &readers, bounds, cap, histogram, cancel));
                crate::cancel::check_cancelled(cancel)?;
                if passed.overfull {
                    self.points.clear();
                    window = None;
                    continue;
                }
                seen = passed.seen;
                keys = passed.keys;
                if let Some(histogram) = passed.histogram {
                    self.points = histogram.points(caps.window as u64);
                }
                break;
            }
        }
        let cap = window.map_or(caps.select, |_| caps.window);
        let through = window.is_none_or(|value| value == u64::MAX);
        let room = cap.saturating_sub(keys.len());
        let mut exhausted = false;
        if valued_done || (through && seen <= cap as u64) {
            let absent = self.absent(cx, composed, after, room + 1)?;
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
        composed: &Composed,
        after: Option<SortKey>,
        most: usize,
    ) -> Result<Vec<SortKey>> {
        let mut out = Vec::new();
        for side in &composed.sides {
            if out.len() >= most {
                break;
            }
            let shard = side.shard.raw();
            let from = match after {
                Some(key) if key.absent && key.shard > shard => continue,
                Some(key) if key.absent && key.shard == shard => Some(key.entity),
                _ => None,
            };
            let mut absent = match &self.field.held {
                Some(column) => {
                    let mut valued = Bitmap::new();
                    let layers = cx.side_shard(side.place).filter_columns.value_layers(column);
                    if let Some(layers) = layers {
                        for layer in layers.base().into_iter().chain(layers.extents()) {
                            valued.or_inplace(&layer.present_in(&side.set));
                        }
                    }
                    &side.set - &valued
                }
                None => {
                    let segments = cx.segments(side.place);
                    let rows = absent_rows(segments, &self.field.spelling, &side.set);
                    cx.engine.entities_of_rows(&cx.open.served, side.place, &rows)?
                }
            };
            if let Some(from) = from {
                absent.remove_range(..=from);
            }
            let room = most - out.len();
            out.extend(absent.iter().take(room).map(|entity| SortKey {
                absent: true,
                value: 0,
                shard,
                entity,
            }));
        }
        Ok(out)
    }
}

/// The rows of `set` where the drawn column `column` holds no value.
fn absent_rows(segments: &[(&SegmentData, u32)], column: &str, set: &Bitmap) -> Bitmap {
    let mut absent = Bitmap::new();
    for &(segment, row_base) in segments {
        let range = row_base..row_base + segment.row_count;
        let mut runs =
            |f: &mut dyn FnMut(Range<u32>)| for_each_run_in(set, range.clone(), &mut |r| f(r));
        let drawn = for_each_drawn_number(segment, row_base, column, &mut runs, |row, value| {
            if value.is_none() {
                absent.add(row);
            }
        });
        if !drawn {
            for_each_run_in(set, range, &mut |run| absent.add_range(run));
        }
    }
    absent
}

/// How a side's values are read: from its value column's layers, or from the view's own columns
/// under the field's name, row by row, with the item a row holds found only for a key a pass
/// keeps.
enum Reader<'a> {
    Held(Vec<&'a ValueColumn>),
    Drawn {
        segments: &'a [(&'a SegmentData, u32)],
        column: &'a str,
        identity_key: &'a IdentityKey,
    },
}

/// Which keys a pass offers: those after `after` whose value is at or before `window`, values
/// inverted for a descending read. With `most`, the pass is abandoned once it has offered more
/// keys than that.
#[derive(Clone, Copy)]
struct Bounds {
    descending: bool,
    after: Option<SortKey>,
    window: Option<u64>,
    most: Option<u64>,
}

/// Keys offered between two additions to a pass's shared count.
const SHARED_EVERY: u64 = 1 << 16;

/// One fold's part of a pass: the keys it keeps, how many it was offered, and their histogram.
struct Gather<'a> {
    best: Best,
    seen: u64,
    histogram: Option<Histogram>,
    /// Offered since the fold last added to the shared count.
    unshared: u64,
    shared: &'a Shared,
}

/// What every fold of a pass reads and writes: how many keys were offered, and whether that is
/// past the most a window may hold.
struct Shared {
    offered: AtomicU64,
    overfull: AtomicBool,
    most: Option<u64>,
}

impl Gather<'_> {
    /// Count the key of `value`, which belongs to the item `entity` names, and keep it where it
    /// comes early enough. `entity` is found only where the value alone does not settle that.
    fn offer(&mut self, bounds: &Bounds, shard: u32, value: u64, entity: impl FnOnce() -> u32) {
        let value = if bounds.descending { !value } else { value };
        if bounds.window.is_some_and(|window| value > window) {
            return;
        }
        let mut entity = Lazy::new(entity);
        if let Some(after) = bounds.after {
            let behind = match value.cmp(&after.value) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => (shard, entity.get()) <= (after.shard, after.entity),
                std::cmp::Ordering::Greater => false,
            };
            if behind {
                return;
            }
        }
        self.seen += 1;
        if let Some(histogram) = &mut self.histogram {
            histogram.add(value);
        }
        self.unshared += 1;
        if self.unshared == SHARED_EVERY {
            self.share();
        }
        if self.best.bound.is_some_and(|bound| value > bound.value) {
            return;
        }
        self.best.offer(SortKey {
            absent: false,
            value,
            shard,
            entity: entity.get(),
        });
    }

    /// Add the fold's offers to the shared count, and mark the pass overfull past its most.
    fn share(&mut self) {
        let total = self.shared.offered.fetch_add(self.unshared, Ordering::Relaxed) + self.unshared;
        self.unshared = 0;
        if self.shared.most.is_some_and(|most| total > most) {
            self.shared.overfull.store(true, Ordering::Relaxed);
        }
    }

    fn stopped(&self) -> bool {
        self.shared.overfull.load(Ordering::Relaxed)
    }

    fn merge(mut self, other: Gather<'_>) -> Self {
        self.best = self.best.merge(other.best);
        self.seen += other.seen;
        self.histogram = match (self.histogram, other.histogram) {
            (Some(a), Some(b)) => Some(a.merge(b)),
            (a, b) => a.or(b),
        };
        self
    }
}

/// A value found once, the first time it is asked for.
struct Lazy<T, F: FnOnce() -> T> {
    value: Option<T>,
    make: Option<F>,
}

impl<T: Copy, F: FnOnce() -> T> Lazy<T, F> {
    fn new(make: F) -> Self {
        Lazy {
            value: None,
            make: Some(make),
        }
    }

    fn get(&mut self) -> T {
        match self.value {
            Some(value) => value,
            None => {
                let value = (self.make.take().expect("made once"))();
                self.value = Some(value);
                value
            }
        }
    }
}

/// The first `cap` valued keys within `bounds` in every side, how many keys there were, and with
/// `histogram`, the counts of their values from that value. A request that has gone stops the
/// pass between pieces, and so does a window found to hold more than `bounds.most`.
fn pass_values(
    sides: &[Side],
    readers: &[Reader<'_>],
    bounds: Bounds,
    cap: usize,
    histogram: Option<u64>,
    cancel: &Option<CancelToken>,
) -> Passed {
    let shared = Shared {
        offered: AtomicU64::new(0),
        overfull: AtomicBool::new(false),
        most: bounds.most,
    };
    let fresh = || Gather {
        best: Best::new(cap),
        seen: 0,
        histogram: histogram.map(Histogram::new),
        unshared: 0,
        shared: &shared,
    };
    let mut gathered = fresh();
    for (side, reader) in sides.iter().zip(readers) {
        let shard = side.shard.raw();
        let ended = |gather: &Gather<'_>| crate::cancel::gone(cancel) || gather.stopped();
        let part = match reader {
            Reader::Held(layers) => {
                let end = side.set.maximum().map_or(0, |last| u64::from(last) + 1);
                (0..end.div_ceil(PIECE))
                    .into_par_iter()
                    .fold(fresh, |mut gather, p| {
                        if ended(&gather) {
                            return gather;
                        }
                        let range = p * PIECE..((p + 1) * PIECE).min(end);
                        let mut piece = Bitmap::from_range(range.start as u32..range.end as u32);
                        piece.and_inplace(&side.set);
                        for layer in layers {
                            let _ = layer.for_each_record_value_in(&piece, |entity, value| {
                                if let Some(value) = number_key(value) {
                                    gather.offer(&bounds, shard, value, || entity);
                                }
                                Ok::<(), ()>(())
                            });
                        }
                        gather.share();
                        gather
                    })
                    .reduce(fresh, Gather::merge)
            }
            Reader::Drawn {
                segments,
                column,
                identity_key,
            } => pieces(segments)
                .into_par_iter()
                .fold(fresh, |mut gather, (s, rows)| {
                    if ended(&gather) {
                        return gather;
                    }
                    let (segment, row_base) = segments[s];
                    let mut runs = |f: &mut dyn FnMut(Range<u32>)| {
                        for_each_run_in(&side.set, rows.clone(), &mut |run| f(run))
                    };
                    for_each_drawn_number(segment, row_base, column, &mut runs, |row, value| {
                        if let Some(value) = value.and_then(number_key) {
                            let entity =
                                || entity_in(segment, row - row_base, identity_key, side.shard);
                            gather.offer(&bounds, shard, value, entity);
                        }
                    });
                    gather.share();
                    gather
                })
                .reduce(fresh, Gather::merge),
        };
        gathered = gathered.merge(part);
    }
    let overfull = shared.overfull.load(Ordering::Relaxed);
    Passed {
        keys: gathered.best.sorted(),
        seen: gathered.seen,
        histogram: gathered.histogram,
        overfull,
    }
}
