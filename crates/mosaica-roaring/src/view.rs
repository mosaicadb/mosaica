//! A Roaring view over bytes another value keeps alive, such as a read-only file mapping.

use std::any::Any;
use std::ops::Deref;
use std::sync::Arc;

use croaring::{Bitmap, BitmapView, Frozen, Portable};

/// `bitmap`'s portable serialisation as a function of its set alone, run-optimised.
///
/// `run_optimize` keeps a container's kind where a run and an array serialise to the same size,
/// so a bitmap holding a run container (one a set operation made) and one built from the sorted
/// values would otherwise serialise one set two ways. Converting every run container back first
/// puts each container in the kind its cardinality gives, which is how a bitmap built from the
/// sorted values holds it, and `run_optimize` then decides the same way for both.
pub fn canonical_portable(mut bitmap: Bitmap) -> Vec<u8> {
    bitmap.remove_run_compression();
    bitmap.run_optimize();
    bitmap.serialize::<Portable>()
}

/// The portable format's cookie where some container is a run, and where none is.
const COOKIE_RUNS: u32 = 12347;
const COOKIE_NO_RUNS: u32 = 12346;

/// Below this many containers, a serialisation with a run container carries no offsets.
const OFFSETS_FROM: usize = 4;

/// Several [`canonical_portable`] serialisations of sets over ascending, disjoint spans of whole
/// 2¹⁶-value blocks, joined into the serialisation of their union without decoding a container.
///
/// A container's bytes depend on its own set alone, so the union's serialisation is one header
/// over every part's containers, then each part's containers as they stand. That is
/// [`canonical_portable`] of the union byte for byte: the header's cookie, run flags and offsets
/// follow from each container's key, cardinality and size, which each part's header gives.
pub struct Joined<'a> {
    header: Vec<u8>,
    containers: Vec<&'a [u8]>,
    cardinality: u64,
}

impl<'a> Joined<'a> {
    /// The parts joined, in order, or `None` where one is not a portable serialisation or their
    /// blocks do not ascend.
    pub fn of(parts: impl IntoIterator<Item = &'a [u8]>) -> Option<Joined<'a>> {
        let mut blocks: Vec<(u16, u16, bool, u32)> = Vec::new();
        let mut containers = Vec::new();
        for part in parts {
            containers.push(descriptors(part, &mut blocks)?);
        }
        let n = blocks.len();
        let runs = blocks.iter().any(|b| b.2);
        let mut header = Vec::with_capacity(8 + 8 * n + n.div_ceil(8));
        if runs {
            let last = u32::try_from(n.checked_sub(1)?)
                .ok()
                .filter(|l| *l <= 0xFFFF)?;
            header.extend_from_slice(&(COOKIE_RUNS | last << 16).to_le_bytes());
            let mut flags = vec![0u8; n.div_ceil(8)];
            for (i, block) in blocks.iter().enumerate() {
                flags[i / 8] |= u8::from(block.2) << (i % 8);
            }
            header.extend_from_slice(&flags);
        } else {
            header.extend_from_slice(&COOKIE_NO_RUNS.to_le_bytes());
            header.extend_from_slice(&u32::try_from(n).ok()?.to_le_bytes());
        }
        for &(key, card, _, _) in &blocks {
            header.extend_from_slice(&key.to_le_bytes());
            header.extend_from_slice(&card.to_le_bytes());
        }
        if !runs || n >= OFFSETS_FROM {
            let mut offset = u32::try_from(header.len() + 4 * n).ok()?;
            for &(_, _, _, size) in &blocks {
                header.extend_from_slice(&offset.to_le_bytes());
                offset = offset.checked_add(size)?;
            }
        }
        let cardinality = blocks.iter().map(|b| u64::from(b.1) + 1).sum();
        Some(Joined {
            header,
            containers,
            cardinality,
        })
    }

    /// The union's serialisation, in pieces: the header, then each part's containers.
    pub fn pieces(&self) -> impl Iterator<Item = &[u8]> {
        std::iter::once(self.header.as_slice()).chain(self.containers.iter().copied())
    }

    /// The members of the union.
    pub fn cardinality(&self) -> u64 {
        self.cardinality
    }
}

/// Push each container `bytes` holds, as `(key, cardinality − 1, is a run, size)`, onto `blocks`,
/// returning the containers' bytes, or `None` where `bytes` are not a portable serialisation or
/// its keys do not ascend past those already in `blocks`.
fn descriptors<'a>(bytes: &'a [u8], blocks: &mut Vec<(u16, u16, bool, u32)>) -> Option<&'a [u8]> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let cookie = u32_at(0)?;
    let (n, flags, mut at, offsets) = if cookie & 0xFFFF == COOKIE_RUNS {
        let n = (cookie >> 16) as usize + 1;
        (
            n,
            Some(bytes.get(4..4 + n.div_ceil(8))?),
            4 + n.div_ceil(8),
            n >= OFFSETS_FROM,
        )
    } else if cookie == COOKIE_NO_RUNS {
        (usize::try_from(u32_at(4)?).ok()?, None, 8, true)
    } else {
        return None;
    };
    let mut keys = at;
    at = at.checked_add(n.checked_mul(4 + 4 * usize::from(offsets))?)?;
    let payload = bytes.get(at..)?;
    let mut size_of_all = 0usize;
    for i in 0..n {
        let (key, card) = (u16_at(keys)?, u16_at(keys + 2)?);
        keys += 4;
        let run = flags.is_some_and(|flags| flags[i / 8] >> (i % 8) & 1 == 1);
        let size = match run {
            true => {
                2 + 4 * u32::from(u16::from_le_bytes(
                    payload.get(size_of_all..size_of_all + 2)?.try_into().ok()?,
                ))
            }
            false if card >= 4096 => 8192,
            false => 2 * (u32::from(card) + 1),
        };
        if blocks.last().is_some_and(|last| last.0 >= key) {
            return None;
        }
        blocks.push((key, card, run, size));
        size_of_all = size_of_all.checked_add(size as usize)?;
    }
    (size_of_all == payload.len()).then_some(payload)
}

/// The bitmap `bytes` hold in the portable format, or `None` unless serialising it gives back
/// exactly `bytes`.
///
/// The decoder reads the containers in sequence and ignores the offset header, which a view
/// follows to find each container. Requiring the bytes to be the bitmap's own serialisation makes
/// every offset the one the decoder used, and refuses trailing bytes. Deserialising keeps each
/// container's kind, so the output of any `serialize::<Portable>` passes, run-optimised or not.
/// `scratch` holds the serialisation, so a caller checking many bitmaps can reuse one buffer.
pub fn exact_portable(bytes: &[u8], scratch: &mut Vec<u8>) -> Option<Bitmap> {
    let bitmap = Bitmap::try_deserialize::<Portable>(bytes)?;
    scratch.clear();
    (*bitmap.serialize_into_vec::<Portable>(scratch) == *bytes).then_some(bitmap)
}

/// A bitmap read in place from bytes inside an allocation `owner` keeps alive.
///
/// The view borrows the bytes for as long as this value lives, which the type system cannot
/// state, so the one constructor of each format is `unsafe` and the fields are private.
pub struct OwnedView {
    /// Declared first, so it is dropped before the owner of the bytes it reads.
    view: BitmapView<'static>,
    bytes: &'static [u8],
    format: Format,
    owner: Arc<dyn Any + Send + Sync>,
}

#[derive(Clone, Copy)]
enum Format {
    Portable,
    Frozen,
}

/// `Bitmap` carries croaring's own `Send` and `Sync`, and a view over bytes nothing may write is
/// no weaker: every read is a read of a `roaring_bitmap_t` and of the immutable bytes behind it,
/// and nothing writes through a view. The owner is `Send + Sync` by its bound.
unsafe impl Send for OwnedView {}
unsafe impl Sync for OwnedView {}

impl OwnedView {
    /// The portable-format bitmap in `bytes`, or `None` where they are not one.
    ///
    /// # Safety
    ///
    /// `bytes` must lie inside an allocation `owner` keeps alive, unwritten and unmoved, for as
    /// long as `owner` lives. A read-only file mapping, and an `Arc` over one, keeps that.
    pub unsafe fn portable(bytes: &[u8], owner: Arc<dyn Any + Send + Sync>) -> Option<OwnedView> {
        // The view's deserialiser is unchecked, and would read bytes that are not a bitmap as
        // containers at whatever the header claimed.
        exact_portable(bytes, &mut Vec::new())?;
        // SAFETY: the bytes are exactly a bitmap's serialisation, and the caller's contract is
        // this one's.
        Some(unsafe { OwnedView::new(Format::Portable, bytes, owner) })
    }

    /// The frozen-format bitmap in `bytes`, or `None` where they are not one: not 32-byte aligned,
    /// or a length the header and its containers do not add up to.
    ///
    /// # Safety
    ///
    /// [`OwnedView::portable`]'s.
    pub unsafe fn frozen(bytes: &[u8], owner: Arc<dyn Any + Send + Sync>) -> Option<OwnedView> {
        // croaring's view panics where CRoaring refuses the layout, so CRoaring is asked first.
        // SAFETY: `roaring_bitmap_frozen_view` reads only inside `bytes`, and answers either null
        // or a header it allocated, which nothing else holds and which is freed here.
        unsafe {
            let probe =
                croaring_sys::roaring_bitmap_frozen_view(bytes.as_ptr().cast(), bytes.len());
            if probe.is_null() {
                return None;
            }
            croaring_sys::roaring_bitmap_free(probe);
        }
        // SAFETY: CRoaring accepts the layout, and the caller's contract is this one's.
        Some(unsafe { OwnedView::new(Format::Frozen, bytes, owner) })
    }

    /// # Safety
    ///
    /// `bytes` hold a bitmap in `format` that croaring's view accepts, and the caller's contract
    /// is [`OwnedView::portable`]'s.
    unsafe fn new(format: Format, bytes: &[u8], owner: Arc<dyn Any + Send + Sync>) -> OwnedView {
        // SAFETY: `owner` keeps the bytes for as long as it lives, and it is moved into the value
        // that holds the view, which drops the view first.
        let bytes: &'static [u8] = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(bytes) };
        OwnedView {
            view: unsafe { view_of(format, bytes) },
            bytes,
            format,
            owner,
        }
    }
}

/// # Safety
///
/// `bytes` hold a bitmap in `format` that croaring's view accepts.
unsafe fn view_of(format: Format, bytes: &[u8]) -> BitmapView<'_> {
    unsafe {
        match format {
            Format::Portable => BitmapView::deserialize::<Portable>(bytes),
            Format::Frozen => BitmapView::deserialize::<Frozen>(bytes),
        }
    }
}

impl Deref for OwnedView {
    type Target = Bitmap;

    fn deref(&self) -> &Bitmap {
        &self.view
    }
}

impl Clone for OwnedView {
    /// A view of the same bytes, whose owner is shared.
    fn clone(&self) -> Self {
        OwnedView {
            // SAFETY: the bytes this value's own view reads, kept by the owner cloned beside it.
            view: unsafe { view_of(self.format, self.bytes) },
            bytes: self.bytes,
            format: self.format,
            owner: Arc::clone(&self.owner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `bitmap` frozen into a buffer of its own, and where in the buffer the 32-byte aligned bytes
    /// lie.
    fn frozen(bitmap: &Bitmap) -> (Arc<Vec<u8>>, std::ops::Range<usize>) {
        let mut buffer = Vec::new();
        let bytes = bitmap.serialize_into_vec::<Frozen>(&mut buffer);
        let (at, len) = (bytes.as_ptr() as usize, bytes.len());
        let start = at - buffer.as_ptr() as usize;
        let range = start..start + len;
        (Arc::new(buffer), range)
    }

    #[test]
    fn a_whole_frozen_buffer_reads_as_its_bitmap_and_a_clone_as_the_same() {
        let bitmap = Bitmap::of(&[1, 5, 70_000, 1 << 31]);
        let (buffer, range) = frozen(&bitmap);
        let view = unsafe { OwnedView::frozen(&buffer[range], buffer.clone()) }.expect("frozen");
        assert_eq!(*view, bitmap);
        assert_eq!(*view.clone(), bitmap);
    }

    #[test]
    fn a_cut_or_misaligned_frozen_buffer_is_refused() {
        let bitmap = Bitmap::of(&[1, 5, 70_000, 1 << 31]);
        let (buffer, range) = frozen(&bitmap);
        for end in [range.start, range.start + 3, range.end - 1] {
            let cut = unsafe { OwnedView::frozen(&buffer[range.start..end], buffer.clone()) };
            assert!(cut.is_none(), "cut to {} bytes", end - range.start);
        }
        // The same bytes one byte past a 32-byte boundary.
        let mut shifted = vec![0u8; range.len() + 64];
        let at = (33 - shifted.as_ptr() as usize % 32) % 32;
        shifted[at..at + range.len()].copy_from_slice(&buffer[range.clone()]);
        let shifted = Arc::new(shifted);
        let bytes = &shifted[at..at + range.len()];
        assert_eq!(bytes.as_ptr() as usize % 32, 1);
        assert!(unsafe { OwnedView::frozen(bytes, shifted.clone()) }.is_none());
    }

    /// Bitmaps of every container kind, among them one `run_optimize` would change and its
    /// optimised form.
    fn shapes() -> Vec<Bitmap> {
        // A bitset of consecutive values and an array of consecutive values, both runs once
        // optimised.
        let unoptimised: Bitmap = (0..20_000).chain(70_000..70_100).collect();
        let mut optimised = unoptimised.clone();
        optimised.run_optimize();
        assert_ne!(
            unoptimised.serialize::<Portable>(),
            optimised.serialize::<Portable>()
        );
        let mut sparse_bitset: Bitmap = (0..20_000).map(|i| i * 3).collect();
        sparse_bitset.remove(3);
        let many_arrays: Bitmap = (0..6u32)
            .flat_map(|c| (0..50).map(move |i| (c << 16) + 2 * i))
            .collect();
        vec![
            Bitmap::new(),
            Bitmap::of(&[7]),
            unoptimised,
            optimised,
            sparse_bitset,
            many_arrays,
        ]
    }

    #[test]
    fn every_serialisation_passes_as_itself() {
        let mut scratch = Vec::new();
        for bitmap in shapes() {
            let bytes = bitmap.serialize::<Portable>();
            assert_eq!(exact_portable(&bytes, &mut scratch), Some(bitmap));
        }
    }

    /// A container offset the decoder never reads, moved by one byte, would have a view read the
    /// container from the wrong bytes.
    #[test]
    fn a_wrong_offset_header_or_a_trailing_byte_is_refused() {
        // Three array containers and no runs, so the bytes carry an offset header: a 4-byte
        // cookie, a 4-byte container count, 4 bytes of key and cardinality a container, then a
        // 4-byte offset a container.
        let bitmap: Bitmap = (0..3u32)
            .flat_map(|c| (0..100).map(move |i| (c << 16) + 2 * i))
            .collect();
        let bytes = bitmap.serialize::<Portable>();
        assert_eq!(
            &bytes[0..4],
            &12346u32.to_le_bytes(),
            "a no-run portable cookie"
        );
        assert_eq!(&bytes[4..8], &3u32.to_le_bytes(), "three containers");
        let mut scratch = Vec::new();

        let mut moved = bytes.clone();
        moved[4 + 4 + 3 * 4 + 4] += 1;
        assert!(
            Bitmap::try_deserialize::<Portable>(&moved).is_some(),
            "the decoder does not read the offsets"
        );
        assert_eq!(exact_portable(&moved, &mut scratch), None);
        let owner = Arc::new(moved);
        assert!(unsafe { OwnedView::portable(&owner, owner.clone()) }.is_none());

        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(exact_portable(&longer, &mut scratch), None);
    }

    #[test]
    fn bytes_that_are_not_a_portable_bitmap_are_refused() {
        let bitmap = Bitmap::of(&[3, 9, 1 << 20]);
        let owner = Arc::new(bitmap.serialize::<Portable>());
        let view = unsafe { OwnedView::portable(&owner, owner.clone()) }.expect("portable");
        assert_eq!(*view, bitmap);
        let cut = unsafe { OwnedView::portable(&owner[..owner.len() - 1], owner.clone()) };
        assert!(cut.is_none());
    }

    /// The union's [`canonical_portable`] serialisation, joined from parts cut at random block
    /// edges.
    fn joined(values: &[u32], cuts: &[u32], how: u32) -> Vec<u8> {
        let parts: Vec<Vec<u8>> = cuts
            .windows(2)
            .map(|cut| {
                let (lo, hi) = (u64::from(cut[0]) << 16, u64::from(cut[1]) << 16);
                let sub: Vec<u32> = values
                    .iter()
                    .copied()
                    .filter(|&v| (lo..hi).contains(&u64::from(v)))
                    .collect();
                canonical_portable(built(&sub, how))
            })
            .collect();
        let joined = Joined::of(parts.iter().map(Vec::as_slice)).expect("the parts join");
        assert_eq!(joined.cardinality(), values.len() as u64);
        joined.pieces().flatten().copied().collect()
    }

    /// `values`, sorted, built into a bitmap one of several ways, so that a part holds the run
    /// containers a set operation makes as well as the kinds a sorted build gives.
    fn built(values: &[u32], how: u32) -> Bitmap {
        match how % 3 {
            0 => Bitmap::of(values),
            1 => {
                let mut bitmap = Bitmap::new();
                for &v in values {
                    bitmap.add_range(v..=v);
                }
                bitmap.run_optimize();
                bitmap
            }
            _ => {
                let mut wider = Bitmap::of(values);
                if let (Some(&lo), Some(&hi)) = (values.first(), values.last()) {
                    wider.add_range(lo..=hi);
                    wider.run_optimize();
                    wider.and_inplace(&Bitmap::of(values));
                }
                wider
            }
        }
    }

    /// **Parts joined are the union's serialisation, byte for byte**: over random sets whose blocks
    /// are empty, sparse, dense, full, a few runs, at the array/bitset edge, or the run/array tie
    /// (1 + 2r members in r runs, which costs the same either way) at a block's ends, cut into up
    /// to eight parts with empty ones among them, so that some parts hold runs and others not and a
    /// part of fewer than four containers (no offsets) joins a whole of more.
    #[test]
    fn joined_parts_are_the_canonical_serialisation_of_their_union() {
        use rand::{rngs::StdRng, Rng, SeedableRng};
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..300 {
            let blocks = rng.gen_range(1..12u32);
            let mut values = Vec::new();
            for block in 0..blocks {
                let mut local: Vec<u32> = match rng.gen_range(0..9) {
                    0 => Vec::new(),
                    1 => (0..rng.gen_range(1..200))
                        .map(|_| rng.gen_range(0..65_536))
                        .collect(),
                    2 => (0..65_536).filter(|_| rng.gen_bool(0.3)).collect(),
                    3 => (0..rng.gen_range(1..6))
                        .flat_map(|_| {
                            let s = rng.gen_range(0..65_000);
                            s..s + rng.gen_range(1..500)
                        })
                        .collect(),
                    4 => [65_533, 65_534, 65_535, 0, 1, 2]
                        .into_iter()
                        .skip(3 * rng.gen_range(0..2))
                        .take(3)
                        .collect(),
                    5 => (0..65_536)
                        .step_by(15)
                        .take(4_096 + rng.gen_range(0..2))
                        .collect(),
                    6 => (0..65_536).collect(),
                    7 => {
                        let r = rng.gen_range(1..50u32);
                        let mut x = rng.gen_range(0..100);
                        let mut tie = Vec::new();
                        for i in 0..r {
                            let take = if i == 0 { r + 2 } else { 1 };
                            tie.extend(x..x + take);
                            x += take + 1 + rng.gen_range(0..20);
                        }
                        tie
                    }
                    _ => (0..rng.gen_range(1..20))
                        .map(|_| rng.gen_range(0..65_536))
                        .collect(),
                };
                local.sort_unstable();
                local.dedup();
                values.extend(local.into_iter().map(|x| block << 16 | x));
            }
            if values.is_empty() {
                continue;
            }
            let mut cuts: Vec<u32> = (0..rng.gen_range(0..8))
                .map(|_| rng.gen_range(0..=blocks))
                .chain([0, blocks])
                .collect();
            cuts.sort_unstable();
            let bytes = joined(&values, &cuts, rng.gen());
            assert_eq!(
                bytes,
                canonical_portable(Bitmap::of(&values)),
                "cuts {cuts:?}"
            );
            assert!(exact_portable(&bytes, &mut Vec::new()).is_some());
        }
    }

    /// The run/array tie on both sides of a cut, and the offset threshold crossed: one block with a
    /// run in each of two parts, neither with offsets, and five blocks together, with them.
    #[test]
    fn ties_at_a_cut_and_the_offset_threshold_join() {
        let values: Vec<u32> = (65_533..65_536)
            .chain((1 << 16)..(1 << 16 | 3))
            .chain((2 << 16)..(3 << 16))
            .chain([3 << 16 | 5])
            .chain((4 << 16 | 10)..(4 << 16 | 400))
            .collect();
        for cut in 0..=5u32 {
            for how in 0..3 {
                let bytes = joined(&values, &[0, cut, 5], how);
                assert_eq!(
                    bytes,
                    canonical_portable(Bitmap::of(&values)),
                    "cut {cut} {how}"
                );
            }
        }
    }

    /// Parts whose blocks do not ascend are refused.
    #[test]
    fn parts_out_of_order_are_refused() {
        let low = canonical_portable(Bitmap::of(&[1, 2]));
        let high = canonical_portable(Bitmap::of(&[1 << 16]));
        assert!(Joined::of([high.as_slice(), low.as_slice()]).is_none());
        assert!(Joined::of([low.as_slice(), low.as_slice()]).is_none());
        assert!(Joined::of([&low[..low.len() - 1]]).is_none());
    }
}
