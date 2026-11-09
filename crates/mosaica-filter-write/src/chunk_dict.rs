//! The dictionary of one chunk of keyword values: its distinct keys in sorted order, and each
//! row's rank among them.
//!
//! Two ways to reach it, chosen by how often a row brings a key the chunk has not seen:
//!
//! - **Hash.** Each row is looked up in a [`KeyTable`] and gets a provisional id in first-seen
//!   order; only the distinct keys are sorted, once, at the end ([`rank_table`]). The cost per row
//!   is one probe, so a column with few keys pays almost nothing for its rows.
//! - **Sort.** Every row is sorted on its key ([`rank_rows`]). Where nearly every row is a new key
//!   there is nothing for a table to save, and the sort's 8-byte prefix settles most comparisons
//!   without reading the keys.
//!
//! [`prefers_sort`] and [`WindowRule`] are the rule between them, shared by the build's keyword
//! pass and the flush.
//! Either way the output is the same: the ranks name the sorted distinct keys, so which way a
//! chunk took is not in any artefact.

use std::io;

use rayon::slice::ParallelSliceMut;

use crate::invalid;

/// Rows a hashed chunk reads between two looks at its rate of new keys ([`WindowRule`]).
pub const KEY_WINDOW_ROWS: u64 = 1 << 20;

/// Whether rows of which more than one in ten brought a key not seen before are better ranked by
/// sorting every row than by hashing.
pub fn prefers_sort(new_keys: u64, rows: u64) -> bool {
    new_keys.saturating_mul(10) > rows
}

/// When a hashed chunk stops hashing: at the end of a window, never its first, whose new keys
/// [`prefers_sort`] and are more than four fifths of the window's before.
#[derive(Default)]
pub struct WindowRule {
    rows: u64,
    new_keys: u64,
    previous: Option<u64>,
}

impl WindowRule {
    /// Count one row of a window of `window` rows, `new` where its key was not in the table.
    /// Returns whether the chunk should stop hashing.
    pub fn row(&mut self, new: bool, window: u64) -> bool {
        self.rows += 1;
        self.new_keys += u64::from(new);
        if self.rows < window {
            return false;
        }
        let new_keys = std::mem::take(&mut self.new_keys);
        self.rows = 0;
        match self.previous.replace(new_keys) {
            Some(previous) => {
                prefers_sort(new_keys, window)
                    && new_keys.saturating_mul(5) > previous.saturating_mul(4)
            }
            None => false,
        }
    }
}

/// Where one key's bytes are in its owner's storage. `at` is the owner's to interpret, an offset
/// into a buffer or an index into a slice; nothing here reads it except through the owner's
/// `bytes` function.
#[derive(Clone, Copy, Debug)]
pub struct KeyRef {
    pub at: u32,
    pub len: u32,
}

impl KeyRef {
    /// The reference to `key`, found by its owner at `at`. A key longer than a `u32` can measure
    /// is refused.
    pub fn new(key: &[u8], at: u32) -> io::Result<KeyRef> {
        let len = u32::try_from(key.len()).map_err(|_| {
            invalid(format!(
                "a keyword value of {} bytes is longer than the u32 its length is held in; \
                 shorten it below 4 GiB",
                key.len()
            ))
        })?;
        Ok(KeyRef { at, len })
    }
}

/// The key's first eight bytes, big-endian and zero-padded.
///
/// **They order two keys as their bytes do** wherever the two differ in them: a key shorter than
/// the other pads with the zeros that make "shorter is less" true. Only keys that agree in all
/// eight need their bytes compared ([`rank_rows`]).
pub fn key_prefix(key: &[u8]) -> u64 {
    let take = key.len().min(8);
    let mut head = [0u8; 8];
    head[..take].copy_from_slice(&key[..take]);
    u64::from_be_bytes(head)
}

/// One row's key, its [`key_prefix`] and its position among the rows being ranked.
#[derive(Clone, Copy, Debug)]
pub struct KeyRow {
    pub prefix: u64,
    pub key: KeyRef,
    pub row: u32,
}

impl KeyRow {
    pub fn new(bytes: &[u8], key: KeyRef, row: u32) -> KeyRow {
        KeyRow {
            prefix: key_prefix(bytes),
            key,
            row,
        }
    }
}

/// Rank rows by sorting them: `ranks[row]` becomes the rank of that row's key among the distinct
/// keys, and `emit` is called once per distinct key, ascending. Returns the distinct count.
///
/// `rows` is reordered, by a sort where `sort` says. `ranks` must have a slot for every `row` the
/// rows name.
///
/// **No tiebreak on the row.** A tiebreak makes every element distinct, which defeats the
/// equal-element partitioning the unstable sort does, so a column of two dozen keys would pay
/// `n log n` comparisons to separate rows that need no order among themselves.
pub fn rank_rows<'k, E>(
    rows: &mut [KeyRow],
    bytes: impl Fn(&KeyRef) -> &'k [u8] + Sync,
    ranks: &mut [u32],
    sort: SortOn,
    mut emit: impl FnMut(&KeyRef) -> Result<(), E>,
) -> Result<u32, E> {
    let order = |a: &KeyRow, b: &KeyRow| {
        a.prefix
            .cmp(&b.prefix)
            .then_with(|| bytes(&a.key).cmp(bytes(&b.key)))
    };
    match sort {
        SortOn::Pool => rows.par_sort_unstable_by(order),
        SortOn::ThisThread => rows.sort_unstable_by(order),
    }
    let mut distinct = 0u32;
    let mut start = 0usize;
    while start < rows.len() {
        if let Some(ahead) = rows.get(start + PREFETCH_ROWS) {
            prefetch(bytes(&ahead.key));
        }
        let head = rows[start];
        let key = bytes(&head.key);
        let mut end = start + 1;
        while end < rows.len() && rows[end].prefix == head.prefix && bytes(&rows[end].key) == key {
            end += 1;
        }
        emit(&head.key)?;
        for row in &rows[start..end] {
            ranks[row.row as usize] = distinct;
        }
        distinct += 1;
        start = end;
    }
    Ok(distinct)
}

/// Where a ranking sorts: on the rayon pool, or on the calling thread, for a caller that already
/// runs one ranking a pool thread and whose waits would otherwise let the pool start more work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOn {
    Pool,
    ThisThread,
}

/// How far ahead [`rank_rows`] asks for a key.
const PREFETCH_ROWS: usize = 16;

/// Ask for `bytes` to be brought into cache before they are read. A hint with no effect on what
/// is read; on a target without one it does nothing.
#[inline]
fn prefetch(bytes: &[u8]) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch reads nothing and faults on nothing, whatever the address.
    unsafe {
        std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(
            bytes.as_ptr().cast(),
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = bytes;
}

/// Rank a table's distinct keys: `perm[id]` becomes the rank of provisional id `id`, and `emit`
/// is called once per key, ascending. Returns the distinct count.
///
/// The table is left empty, its memory released before the sort.
pub fn rank_table<'k, E>(
    table: &mut KeyTable,
    bytes: impl Fn(&KeyRef) -> &'k [u8] + Sync,
    perm: &mut Vec<u32>,
    sort: SortOn,
    emit: impl FnMut(&KeyRef) -> Result<(), E>,
) -> Result<u32, E> {
    let entries = std::mem::take(table).entries;
    let mut rows: Vec<KeyRow> = entries
        .iter()
        .enumerate()
        .map(|(id, &key)| KeyRow::new(bytes(&key), key, id as u32))
        .collect();
    drop(entries);
    perm.clear();
    perm.resize(rows.len(), 0);
    rank_rows(&mut rows, bytes, perm, sort, emit)
}

/// An open-addressing table from a key to its provisional id, the ids issued in first-seen order.
///
/// **It holds no key bytes**: each entry is a [`KeyRef`] into storage its owner keeps, read back
/// through the `bytes` function the lookups are given. That is what lets the build's chunk keep
/// its keys in one buffer it reuses, and the flush keep them where the batch already has them.
///
/// A slot is a `u64`: the hash's high 32 bits above the entry's index, `u64::MAX` when empty. The
/// table doubles at half full, so a probe is short and the high bits rehash without the key.
#[derive(Default)]
pub struct KeyTable {
    slots: Vec<u64>,
    entries: Vec<KeyRef>,
    /// The id the last lookup returned, so a run of one key costs a comparison and no probe.
    last: Option<u32>,
}

/// The result of [`KeyTable::find`]: the id the key already has, or the place to give it one.
pub enum Lookup {
    Found(u32),
    Vacant(Vacant),
}

/// A key [`KeyTable::find`] did not hold, to be passed to [`KeyTable::insert`] before the next
/// lookup.
pub struct Vacant {
    tag: u32,
}

const EMPTY: u64 = u64::MAX;

impl KeyTable {
    /// Distinct keys held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Where the key with provisional id `id` is held.
    pub fn key(&self, id: u32) -> KeyRef {
        self.entries[id as usize]
    }

    /// The bytes the table would allocate, beyond what it holds, while taking `keys` new keys:
    /// each array it would replace, at its new size, since the old one is live while it moves.
    pub fn growth(&self, keys: usize) -> usize {
        let held = self.entries.len() + keys;
        let mut slots = 0;
        let mut size = self.slots.len();
        while held * 2 > size {
            size = (size * 2).max(16);
            slots += size * std::mem::size_of::<u64>();
        }
        let entries = if held > self.entries.capacity() {
            held.max(self.entries.capacity() * 2) * std::mem::size_of::<KeyRef>()
        } else {
            0
        };
        slots + entries
    }

    /// The bytes the table has allocated.
    pub fn allocated(&self) -> usize {
        self.slots.capacity() * std::mem::size_of::<u64>()
            + self.entries.capacity() * std::mem::size_of::<KeyRef>()
    }

    /// Empty the table, keeping the capacity it has.
    pub fn clear(&mut self) {
        self.slots.fill(EMPTY);
        self.entries.clear();
        self.last = None;
    }

    /// The provisional id of `key`, or the vacancy to insert it at.
    pub fn find<'k>(&mut self, key: &[u8], bytes: impl Fn(&KeyRef) -> &'k [u8]) -> Lookup {
        if let Some(last) = self.last {
            if bytes(&self.entries[last as usize]) == key {
                return Lookup::Found(last);
            }
        }
        let tag = hash_tag(key);
        if !self.slots.is_empty() {
            let mask = self.slots.len() - 1;
            let mut at = tag as usize & mask;
            loop {
                let slot = self.slots[at];
                if slot == EMPTY {
                    break;
                }
                if (slot >> 32) as u32 == tag {
                    let id = slot as u32;
                    if bytes(&self.entries[id as usize]) == key {
                        self.last = Some(id);
                        return Lookup::Found(id);
                    }
                }
                at = (at + 1) & mask;
            }
        }
        Lookup::Vacant(Vacant { tag })
    }

    /// Give the key [`Self::find`] reported vacant the next id, returning it. `key` is where its
    /// owner now keeps the bytes.
    pub fn insert(&mut self, vacant: Vacant, key: KeyRef) -> io::Result<u32> {
        let id = u32::try_from(self.entries.len())
            .ok()
            .filter(|&id| id != u32::MAX)
            .ok_or_else(|| {
                invalid(
                    "a keyword chunk holds more distinct keys than a u32 can number; give the \
                     build a smaller memory budget so its chunks are smaller",
                )
            })?;
        if (self.entries.len() + 1) * 2 > self.slots.len() {
            self.grow();
        }
        self.place(vacant.tag, id);
        self.entries.push(key);
        self.last = Some(id);
        Ok(id)
    }

    fn place(&mut self, tag: u32, id: u32) {
        let mask = self.slots.len() - 1;
        let mut at = tag as usize & mask;
        while self.slots[at] != EMPTY {
            at = (at + 1) & mask;
        }
        self.slots[at] = ((tag as u64) << 32) | id as u64;
    }

    fn grow(&mut self) {
        let capacity = (self.slots.len() * 2).max(16);
        let old = std::mem::replace(&mut self.slots, vec![EMPTY; capacity]);
        for slot in old.into_iter().filter(|&slot| slot != EMPTY) {
            self.place((slot >> 32) as u32, slot as u32);
        }
    }
}

/// The high 32 bits of a hash of `key`: a multiply-rotate over its words, finished with the
/// MurmurHash3 mixer so the high bits depend on every byte.
fn hash_tag(key: &[u8]) -> u32 {
    const K: u64 = 0x517c_c1b7_2722_0a95;
    let mut h = key.len() as u64;
    let (words, rest) = key.as_chunks::<8>();
    for word in words {
        h = (h.rotate_left(5) ^ u64::from_le_bytes(*word)).wrapping_mul(K);
    }
    if !rest.is_empty() {
        let mut w = [0u8; 8];
        w[..rest.len()].copy_from_slice(rest);
        h = (h.rotate_left(5) ^ u64::from_le_bytes(w)).wrapping_mul(K);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    (h >> 32) as u32
}

/// One batch's dictionary, held in memory: the sorted distinct keys, and each value's ordinal into
/// them. The flush's keyword extent is this.
///
/// The batch is hashed until [`WindowRule`] says to sort it, and then the whole batch is sorted.
pub fn dictionary_of<'a>(values: &[&'a str]) -> io::Result<(Vec<u32>, Vec<&'a str>)> {
    if u32::try_from(values.len()).is_err() {
        return Err(invalid(format!(
            "a keyword batch of {} values is more than a u32 can address; flush it in smaller \
             batches",
            values.len()
        )));
    }
    let bytes = |key: &KeyRef| values[key.at as usize].as_bytes();
    let mut keys: Vec<&'a str> = Vec::new();

    let mut table = KeyTable::default();
    let mut ids: Vec<u32> = Vec::with_capacity(values.len());
    let mut rule = WindowRule::default();
    let mut sorted = false;
    for (row, value) in values.iter().enumerate() {
        let (id, new) = match table.find(value.as_bytes(), bytes) {
            Lookup::Found(id) => (id, false),
            Lookup::Vacant(vacant) => (
                table.insert(vacant, KeyRef::new(value.as_bytes(), row as u32)?)?,
                true,
            ),
        };
        ids.push(id);
        if rule.row(new, KEY_WINDOW_ROWS) {
            sorted = true;
            break;
        }
    }

    if sorted {
        drop(table);
        drop(ids);
        let mut rows: Vec<KeyRow> = values
            .iter()
            .enumerate()
            .map(|(row, value)| {
                let key = KeyRef::new(value.as_bytes(), row as u32)?;
                Ok(KeyRow::new(value.as_bytes(), key, row as u32))
            })
            .collect::<io::Result<_>>()?;
        let mut ordinals = vec![0u32; values.len()];
        rank_rows(&mut rows, bytes, &mut ordinals, SortOn::Pool, |key| {
            keys.push(values[key.at as usize]);
            Ok::<(), io::Error>(())
        })?;
        return Ok((ordinals, keys));
    }

    let mut perm = Vec::new();
    rank_table(&mut table, bytes, &mut perm, SortOn::Pool, |key| {
        keys.push(values[key.at as usize]);
        Ok::<(), io::Error>(())
    })?;
    for id in ids.iter_mut() {
        *id = perm[*id as usize];
    }
    Ok((ids, keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dictionary the batch owes, derived the obvious way.
    fn expected<'a>(values: &[&'a str]) -> (Vec<u32>, Vec<&'a str>) {
        let mut keys = values.to_vec();
        keys.sort_unstable();
        keys.dedup();
        let ordinals = values
            .iter()
            .map(|v| keys.binary_search(v).expect("held") as u32)
            .collect();
        (ordinals, keys)
    }

    /// Keys that agree in their first eight bytes, a key that is a proper prefix of another, keys
    /// shorter than eight bytes, a non-ASCII key, and runs of one key.
    fn shapes(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| match i % 9 {
                0 => "a".to_string(),
                1 => "ab".to_string(),
                2 => "prefix-".to_string(),
                3 => format!("prefix-{:03}", i % 13),
                4 => "ünïcode-ключ".to_string(),
                5 => format!("uuid-000{i:06}"),
                _ => format!("run-{}", i / 9 % 4),
            })
            .collect()
    }

    #[test]
    fn the_dictionary_is_the_sorted_distinct_keys_and_each_value_s_rank() {
        let owned = shapes(5_000);
        let values: Vec<&str> = owned.iter().map(String::as_str).collect();
        assert_eq!(
            dictionary_of(&values).expect("dictionary"),
            expected(&values)
        );
        assert_eq!(dictionary_of(&[]).expect("empty"), (vec![], vec![]));
    }

    /// After its second window a batch of unique keys is sorted rather than hashed, and its
    /// dictionary is the same as a batch hashed throughout.
    #[test]
    fn a_batch_that_switches_to_sorting_writes_the_same_dictionary() {
        let n = 2 * KEY_WINDOW_ROWS as usize + 10;
        let owned: Vec<String> = (0..n)
            .map(|i| format!("{:08x}", (i * 2_654_435_761) % n))
            .collect();
        let values: Vec<&str> = owned.iter().map(String::as_str).collect();
        assert_eq!(
            dictionary_of(&values).expect("dictionary"),
            expected(&values)
        );
    }

    #[test]
    fn rank_rows_and_rank_table_agree() {
        let owned = shapes(1_000);
        let bytes = |key: &KeyRef| owned[key.at as usize].as_bytes();
        let mut table = KeyTable::default();
        let mut ids = Vec::new();
        for (row, value) in owned.iter().enumerate() {
            ids.push(match table.find(value.as_bytes(), bytes) {
                Lookup::Found(id) => id,
                Lookup::Vacant(v) => table
                    .insert(v, KeyRef::new(value.as_bytes(), row as u32).unwrap())
                    .unwrap(),
            });
        }
        let mut perm = Vec::new();
        let mut from_table = Vec::new();
        rank_table(&mut table, bytes, &mut perm, SortOn::Pool, |k| {
            from_table.push(bytes(k).to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();

        let mut rows: Vec<KeyRow> = owned
            .iter()
            .enumerate()
            .map(|(row, v)| {
                KeyRow::new(
                    v.as_bytes(),
                    KeyRef::new(v.as_bytes(), row as u32).unwrap(),
                    row as u32,
                )
            })
            .collect();
        let mut ranks = vec![0u32; owned.len()];
        let mut from_rows = Vec::new();
        rank_rows(&mut rows, bytes, &mut ranks, SortOn::Pool, |k| {
            from_rows.push(bytes(k).to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();

        assert_eq!(from_table, from_rows);
        let through_table: Vec<u32> = ids.iter().map(|&id| perm[id as usize]).collect();
        assert_eq!(through_table, ranks);
    }

    #[test]
    fn the_rule_sorts_above_one_new_key_in_ten() {
        assert!(!prefers_sort(0, 100));
        assert!(!prefers_sort(10, 100));
        assert!(prefers_sort(11, 100));
    }

    /// Feed windows of 100 rows, the `i`-th bringing `new[i]` new keys, and say which window first
    /// stops the hashing.
    fn first_stop(new: &[u64]) -> Option<usize> {
        let mut rule = WindowRule::default();
        for (i, &keys) in new.iter().enumerate() {
            let mut stop = false;
            for row in 0..100 {
                stop |= rule.row(row < keys, 100);
            }
            if stop {
                return Some(i);
            }
        }
        None
    }

    #[test]
    fn a_window_rule_judges_neither_the_first_window_nor_a_falling_rate() {
        // Every key new: the first window is not judged, the second is.
        assert_eq!(first_stop(&[100, 100, 100]), Some(1));
        // A table learning its keys: the rate falls by more than a fifth each window.
        assert_eq!(first_stop(&[90, 60, 40, 26, 17, 11, 7]), None);
        // Falling, then flat above one in ten.
        assert_eq!(first_stop(&[90, 60, 50, 45]), Some(2));
        // At or below one in ten, never.
        assert_eq!(first_stop(&[10, 10, 10]), None);
    }
}
