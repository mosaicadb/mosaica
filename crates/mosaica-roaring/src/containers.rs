//! A bitmap's containers read in place, and a probe that finds a value's container with one load.
//!
//! croaring answers a membership test by a binary search over the container keys, behind a call
//! into C, and walks a bitmap a value at a time. [`Containers`] hands a caller the containers
//! themselves, as CRoaring holds them: 1,024 words, a sorted list, or a list of runs. [`Probe`]
//! indexes them by key, so a test reads one table entry and then the container.
//!
//! Both are taken from a [`Readable`], which is the bitmap itself where every container lies where
//! its type can be read, and an owned copy otherwise. Only a portable-format view holds a container
//! elsewhere: its payloads sit at whatever offset the serialised stream puts them, and CRoaring
//! reads them unaligned. A copy is allocated by CRoaring, aligned.
//!
//! The container structs are CRoaring's own, mirrored below for the version `croaring-sys` builds.
//! croaring wraps only the bitmap, whose `roaring_array_t` lists the containers by key with a
//! pointer and a type code each; the structs behind the pointers are internal to CRoaring and
//! appear in no binding. The version assertion below stops a build against any other CRoaring,
//! whose structs have not been compared with these.

use std::borrow::Cow;
use std::ffi::c_void;

use croaring::Bitmap;

use crate::WORDS;

const _: () = assert!(
    croaring_sys::ROARING_VERSION_MAJOR == 5
        && croaring_sys::ROARING_VERSION_MINOR == 2
        && croaring_sys::ROARING_VERSION_REVISION == 2,
    "the container structs below mirror CRoaring 5.2.2; compare them with this version's roaring.h"
);

const BITSET: u8 = 1;
const ARRAY: u8 = 2;
const RUN: u8 = 3;
const SHARED: u8 = 4;

/// `bitset_container_t`.
#[repr(C)]
struct BitsetStruct {
    cardinality: i32,
    words: *const u64,
}

/// `array_container_t`.
#[repr(C)]
struct ArrayStruct {
    cardinality: i32,
    capacity: i32,
    array: *const u16,
}

/// `run_container_t`.
#[repr(C)]
struct RunStruct {
    n_runs: i32,
    capacity: i32,
    runs: *const Run,
}

/// `shared_container_t`, as far as its reference count, which is not read.
#[repr(C)]
struct SharedStruct {
    container: *const c_void,
    typecode: u8,
}

/// One run of a run container: `start..=start + length`. CRoaring's `rle16_t`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub start: u16,
    pub length: u16,
}

/// One container's members, low 16 bits only, as CRoaring holds them.
#[derive(Clone, Copy, Debug)]
pub enum Container<'a> {
    /// More than [`crate::ARRAY_MAX`] members, a bit each.
    Bitset(&'a [u64; WORDS]),
    /// Members ascending.
    Array(&'a [u16]),
    /// Runs ascending, none touching the next.
    Run(&'a [Run]),
}

impl Container<'_> {
    /// How many members the container holds.
    pub fn cardinality(&self) -> u32 {
        match self {
            Container::Bitset(words) => words.iter().map(|w| w.count_ones()).sum(),
            Container::Array(members) => members.len() as u32,
            Container::Run(runs) => runs.iter().map(|r| u32::from(r.length) + 1).sum(),
        }
    }
}

/// The containers of one bitmap, ascending by key, borrowed from it.
#[derive(Clone, Copy)]
pub struct Containers<'a> {
    keys: &'a [u16],
    typecodes: &'a [u8],
    containers: &'a [*mut c_void],
}

// SAFETY: the pointers lead into a bitmap borrowed immutably for `'a`, which nothing can write while
// the borrow lasts, and every access through them is a read. `Bitmap` itself is `Send + Sync`.
unsafe impl Send for Containers<'_> {}
unsafe impl Sync for Containers<'_> {}

/// A bitmap whose every container can be read in place: the bitmap itself, or a copy of it.
pub struct Readable<'a>(Cow<'a, Bitmap>);

impl<'a> Readable<'a> {
    /// `bitmap` borrowed where its containers are aligned for their types, and copied otherwise.
    pub fn of(bitmap: &'a Bitmap) -> Self {
        if aligned(&Containers::of(bitmap)) {
            Readable(Cow::Borrowed(bitmap))
        } else {
            let copy = bitmap.clone();
            debug_assert!(
                aligned(&Containers::of(&copy)),
                "CRoaring allocates containers aligned"
            );
            Readable(Cow::Owned(copy))
        }
    }

    pub fn bitmap(&self) -> &Bitmap {
        &self.0
    }

    pub fn containers(&self) -> Containers<'_> {
        Containers::of(&self.0)
    }

    pub fn probe(&self) -> Probe<'_> {
        Probe::of(self.containers())
    }
}

/// Whether every container's payload is aligned for the type it is read as.
fn aligned(containers: &Containers<'_>) -> bool {
    (0..containers.len()).all(|i| {
        let (typecode, container) = containers.unwrapped(i);
        // SAFETY: as in `Containers::get`, which reads the same structs.
        unsafe {
            match typecode {
                BITSET => read::<BitsetStruct>(container).words.is_aligned(),
                ARRAY => read::<ArrayStruct>(container).array.is_aligned(),
                RUN => read::<RunStruct>(container).runs.is_aligned(),
                _ => false,
            }
        }
    })
}

/// The struct at `at`, which need not be aligned: a portable view packs its container structs
/// after one byte of type code per container.
///
/// # Safety
///
/// `at` holds a `T`.
unsafe fn read<T>(at: *const c_void) -> T {
    unsafe { at.cast::<T>().read_unaligned() }
}

impl<'a> Containers<'a> {
    /// The containers of `bitmap`, which are read only once [`aligned`] has passed them.
    fn of(bitmap: &'a Bitmap) -> Self {
        let raw = (bitmap as *const Bitmap).cast::<croaring_sys::roaring_bitmap_t>();
        // SAFETY: `Bitmap` is `repr(transparent)` over `roaring_bitmap_t` (counts.rs asserts its
        // size and alignment), so `raw` is the bitmap's own array of containers for as long as
        // `bitmap` is borrowed. Its three arrays each hold `size` entries.
        unsafe {
            let array = &(*raw).high_low_container;
            let size = usize::try_from(array.size).unwrap_or(0);
            if size == 0 {
                return Containers {
                    keys: &[],
                    typecodes: &[],
                    containers: &[],
                };
            }
            Containers {
                keys: std::slice::from_raw_parts(array.keys, size),
                typecodes: std::slice::from_raw_parts(array.typecodes, size),
                containers: std::slice::from_raw_parts(array.containers, size),
            }
        }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Every key, ascending.
    pub fn keys(&self) -> &'a [u16] {
        self.keys
    }

    /// The type code and the struct at `index`, a shared container's own.
    fn unwrapped(&self, index: usize) -> (u8, *const c_void) {
        let typecode = self.typecodes[index];
        let container = self.containers[index] as *const c_void;
        if typecode != SHARED {
            return (typecode, container);
        }
        // SAFETY: a shared container wraps a container of the type it records, and lives as long
        // as the bitmap.
        let shared = unsafe { read::<SharedStruct>(container) };
        (shared.typecode, shared.container)
    }

    /// The container at `index`, in key order.
    pub fn get(&self, index: usize) -> Container<'a> {
        let (typecode, container) = self.unwrapped(index);
        // SAFETY: CRoaring stores a container of the type its code names at each index, and the
        // containers live as long as the bitmap. A bitset holds 1,024 words; an array's cardinality
        // and a run container's run count say how many entries their pointers hold. `Readable`
        // made these only for a bitmap whose payloads are aligned for their types; the structs
        // themselves are read unaligned.
        unsafe {
            match typecode {
                BITSET => {
                    let bitset = read::<BitsetStruct>(container);
                    Container::Bitset(&*(bitset.words as *const [u64; WORDS]))
                }
                ARRAY => {
                    let array = read::<ArrayStruct>(container);
                    let len = usize::try_from(array.cardinality).unwrap_or(0);
                    Container::Array(slice_or_empty(array.array, len))
                }
                RUN => {
                    let run = read::<RunStruct>(container);
                    let len = usize::try_from(run.n_runs).unwrap_or(0);
                    Container::Run(slice_or_empty(run.runs, len))
                }
                other => unreachable!("CRoaring has no container type {other}"),
            }
        }
    }

    /// How many members the container at `index` holds, from the count CRoaring keeps with it.
    pub fn cardinality(&self, index: usize) -> u32 {
        let (typecode, container) = self.unwrapped(index);
        // SAFETY: as in `Self::get`. A bitset's count is negative while a lazy operation is
        // still repairing it, which no finished bitmap is left in; it is counted here if it is.
        unsafe {
            match typecode {
                BITSET => match u32::try_from(read::<BitsetStruct>(container).cardinality) {
                    Ok(cardinality) => cardinality,
                    Err(_) => self.get(index).cardinality(),
                },
                ARRAY => read::<ArrayStruct>(container).cardinality.max(0) as u32,
                _ => self.get(index).cardinality(),
            }
        }
    }

    /// `(key, container)` for every container, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = (u16, Container<'a>)> + '_ {
        (0..self.len()).map(|i| (self.keys[i], self.get(i)))
    }
}

/// `len` entries from `ptr`, which may be null where `len` is zero.
///
/// # Safety
///
/// Where `len` is positive, `ptr` holds `len` initialised entries for `'a`.
unsafe fn slice_or_empty<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// A bitmap laid out for membership tests in no particular order.
///
/// croaring's `contains` binary-searches the keys, which over a bitmap of 10⁹ values is fourteen
/// unpredictable branches before the container is touched, and each test is a call into C. Here a
/// key indexes a table, the table names a container, and the container is read in place. A bitset
/// container is one word read; an array or run container is a binary search over its own entries.
///
/// [`Probe::prefetch`] starts the load a later test of a value in a bitset container will make, so
/// a caller testing values in a known order can overlap the cache misses. A test of an array or run
/// container is not helped by it and is left alone.
pub struct Probe<'a> {
    /// Per key, the index of its container in `entries` plus one, or 0 where there is none.
    table: Box<[u32]>,
    entries: Vec<Entry<'a>>,
}

#[derive(Clone, Copy)]
enum Entry<'a> {
    Bitset(&'a [u64; WORDS]),
    Array(&'a [u16]),
    Run(&'a [Run]),
}

// SAFETY: the same as `Containers`: every reference is into a bitmap borrowed immutably for `'a`.
unsafe impl Send for Probe<'_> {}
unsafe impl Sync for Probe<'_> {}

impl<'a> Probe<'a> {
    fn of(containers: Containers<'a>) -> Self {
        let mut table = vec![0u32; 1 << 16].into_boxed_slice();
        let mut entries = Vec::with_capacity(containers.len());
        for (key, container) in containers.iter() {
            entries.push(match container {
                Container::Bitset(words) => Entry::Bitset(words),
                Container::Array(members) => Entry::Array(members),
                Container::Run(runs) => Entry::Run(runs),
            });
            table[usize::from(key)] = entries.len() as u32;
        }
        Probe { table, entries }
    }

    #[inline]
    fn entry(&self, value: u32) -> Option<&Entry<'a>> {
        // The table holds 2¹⁶ entries and the key is the value's high 16 bits.
        let index = self.table[(value >> 16) as usize];
        index.checked_sub(1).map(|i| &self.entries[i as usize])
    }

    #[inline]
    pub fn contains(&self, value: u32) -> bool {
        let low = value as u16;
        match self.entry(value) {
            None => false,
            Some(Entry::Bitset(words)) => words[usize::from(low >> 6)] >> (low & 63) & 1 != 0,
            Some(Entry::Array(members)) => {
                let at = members.partition_point(|&m| m < low);
                members.get(at) == Some(&low)
            }
            Some(Entry::Run(runs)) => {
                let after = runs.partition_point(|r| r.start <= low);
                after > 0 && {
                    let run = runs[after - 1];
                    u32::from(low) <= u32::from(run.start) + u32::from(run.length)
                }
            }
        }
    }

    /// Start loading the word [`Self::contains`] will read for `value`, where its container is a
    /// bitset. A hint: it changes no answer.
    #[inline]
    pub fn prefetch(&self, value: u32) {
        if let Some(Entry::Bitset(words)) = self.entry(value) {
            prefetch_read(&words[usize::from((value as u16) >> 6)]);
        }
    }
}

#[inline(always)]
fn prefetch_read<T>(at: &T) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch reads nothing and faults on nothing; SSE is part of every x86-64 target.
    unsafe {
        std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(
            (at as *const T).cast::<i8>(),
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = at;
}

#[cfg(test)]
mod tests {
    use super::*;
    use croaring::{BitmapView, Frozen, Portable};

    /// One container of each kind, plus a full block, the first and last keys, and runs touching a
    /// block's ends.
    fn every_kind() -> Bitmap {
        let mut bitmap = Bitmap::new();
        // An array at key 0, holding the block's first and last values.
        bitmap.add_many(&[0, 7, 4_095, 65_535]);
        // A bitset at key 1.
        bitmap.add_many(&(65_536..65_536 + 30_000).step_by(2).collect::<Vec<u32>>());
        // A full block at key 2, which run-optimises to one run.
        bitmap.add_range(2 << 16..3 << 16);
        // Runs at key 5 touching both ends of the block.
        bitmap.add_range(5 << 16..(5 << 16) + 300);
        bitmap.add_range((5 << 16) + 1_000..(5 << 16) + 1_200);
        bitmap.add_range((6 << 16) - 10..6 << 16);
        // The last key.
        bitmap.add_many(&[u32::MAX - 3, u32::MAX]);
        bitmap.run_optimize();
        bitmap
    }

    fn members(containers: &Containers<'_>) -> Vec<u32> {
        let mut out = Vec::new();
        for (key, container) in containers.iter() {
            let base = u32::from(key) << 16;
            match container {
                Container::Bitset(words) => {
                    for (i, &word) in words.iter().enumerate() {
                        let mut word = word;
                        while word != 0 {
                            out.push(base + (i as u32) * 64 + word.trailing_zeros());
                            word &= word - 1;
                        }
                    }
                }
                Container::Array(lows) => out.extend(lows.iter().map(|&l| base + u32::from(l))),
                Container::Run(runs) => {
                    for run in runs {
                        let start = base + u32::from(run.start);
                        out.extend(start..=start + u32::from(run.length));
                    }
                }
            }
        }
        out
    }

    /// What the reader reads, against croaring, for one bitmap however it is held.
    fn reads_as(bitmap: &Bitmap, want: &Bitmap, what: &str) {
        let readable = Readable::of(bitmap);
        let containers = readable.containers();
        assert_eq!(members(&containers), want.to_vec(), "{what}: members");
        let cardinality: u64 = containers
            .iter()
            .map(|(_, c)| u64::from(c.cardinality()))
            .sum();
        assert_eq!(cardinality, want.cardinality(), "{what}: cardinality");
        for index in 0..containers.len() {
            assert_eq!(
                containers.cardinality(index),
                containers.get(index).cardinality(),
                "{what}: the recorded count of container {index}"
            );
        }
        let kinds: Vec<&str> = containers
            .iter()
            .map(|(_, c)| match c {
                Container::Bitset(_) => "bitset",
                Container::Array(_) => "array",
                Container::Run(_) => "run",
            })
            .collect();
        for kind in ["bitset", "array", "run"] {
            assert!(
                kinds.contains(&kind),
                "{what}: no {kind} container in {kinds:?}"
            );
        }
        let probe = readable.probe();
        let mut values: Vec<u32> = want.iter().collect();
        // Values beside each member, the ends of every block touched, and blocks with nothing.
        for v in want.iter() {
            values.extend([v.wrapping_sub(1), v.wrapping_add(1)]);
        }
        for key in [0u32, 1, 2, 3, 4, 5, 6, 0xFFFF] {
            values.extend([key << 16, (key << 16) | 0xFFFF, (key << 16) | 0x8000]);
        }
        for v in values {
            probe.prefetch(v);
            assert_eq!(probe.contains(v), want.contains(v), "{what}: contains {v}");
        }
    }

    #[test]
    fn every_container_kind_reads_as_croaring_holds_it() {
        let bitmap = every_kind();
        reads_as(&bitmap, &bitmap, "owned");
    }

    #[test]
    fn a_frozen_and_a_portable_view_read_as_their_bitmap() {
        let bitmap = every_kind();
        let mut buffer = Vec::new();
        let frozen = bitmap.serialize_into_vec::<Frozen>(&mut buffer);
        let view = unsafe { BitmapView::deserialize::<Frozen>(frozen) };
        assert!(
            matches!(Readable::of(&view).0, Cow::Borrowed(_)),
            "a frozen view is read in place"
        );
        reads_as(&view, &bitmap, "frozen");
        // A portable stream at every offset of an 8-byte word, so some payloads land misaligned.
        let portable = bitmap.serialize::<Portable>();
        let mut buffer = vec![0u8; portable.len() + 8];
        let mut copied = 0;
        for offset in 0..8 {
            buffer[offset..offset + portable.len()].copy_from_slice(&portable);
            let bytes = &buffer[offset..offset + portable.len()];
            let view = unsafe { BitmapView::deserialize::<Portable>(bytes) };
            copied += usize::from(matches!(Readable::of(&view).0, Cow::Owned(_)));
            reads_as(&view, &bitmap, &format!("portable at offset {offset}"));
        }
        assert!(copied > 0, "some offset leaves a payload misaligned");
    }

    #[test]
    fn shared_containers_read_as_the_containers_they_wrap() {
        let mut bitmap = every_kind();
        // SAFETY: `Bitmap` is `repr(transparent)` over `roaring_bitmap_t`, and the flag is one
        // CRoaring reads on every copy.
        unsafe {
            let raw = (&mut bitmap as *mut Bitmap).cast::<croaring_sys::roaring_bitmap_t>();
            croaring_sys::roaring_bitmap_set_copy_on_write(raw, true);
        }
        let copy = bitmap.clone();
        let readable = Readable::of(&copy);
        let shared = readable
            .containers()
            .typecodes
            .iter()
            .filter(|&&code| code == SHARED)
            .count();
        assert!(shared > 0, "copy-on-write shares the copy's containers");
        reads_as(&copy, &every_kind(), "shared");
    }

    #[test]
    fn an_empty_bitmap_has_no_containers_and_contains_nothing() {
        let empty = Bitmap::new();
        let readable = Readable::of(&empty);
        assert!(readable.containers().is_empty());
        let probe = readable.probe();
        for v in [0, 1, 65_536, u32::MAX] {
            assert!(!probe.contains(v));
        }
    }
}
