//! The rows of one permutation page that a mask's bitset container selects.
//!
//! A page holds 2¹⁶ slots and a container 2¹⁶ bits, one for each slot, so the rows of the page's
//! entities in the mask are the slots whose bits are set, less those holding [`ROW_ABSENT`]. The
//! wide kernels test 16 or 8 slots a step against the mask's bits and the sentinel, and pack the
//! survivors into the output with one store, where reading the bits a value at a time takes a
//! branch per member and the cursor decode before it a store and a load per member.
//!
//! ⊘ **Measured** on a 4-core Xeon with AVX-512, 10⁹ entities, one thread, decode and slot read
//! against the AVX-512 kernel: 1.07 s against 0.77 s for a 10% grant, 1.64 s against 0.78 s for 25%
//! and 4.95 s against 0.89 s for 90%. The kernel's time barely moves with the grant: it is the time
//! one core takes to read the slot array.

use mosaica_roaring::WORDS;
use mosaica_types::ROW_ABSENT;

use crate::permutation::PAGE_ENTRIES;

const _: () = assert!(
    WORDS * 64 == PAGE_ENTRIES,
    "a bitset container's bits cover one page"
);

/// The most a wide kernel writes past the rows it keeps, which the output reserves beyond them.
pub(crate) const SLACK: usize = 16;

/// Replace `out` with the rows of `slots` whose bits are set in `words`, in slot order, leaving out
/// [`ROW_ABSENT`].
pub(crate) fn select(slots: &[u32; PAGE_ENTRIES], words: &[u64; WORDS], out: &mut Vec<u32>) {
    out.clear();
    out.reserve(PAGE_ENTRIES + SLACK);
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx512f") {
            // SAFETY: the feature was detected, and `out` has room for every slot and the slack.
            unsafe { x86::avx512(slots, words, out) };
            return;
        }
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: as above.
            unsafe { x86::avx2(slots, words, out) };
            return;
        }
    }
    scalar(slots, words, out);
}

/// [`select`] a bit at a time.
fn scalar(slots: &[u32; PAGE_ENTRIES], words: &[u64; WORDS], out: &mut Vec<u32>) {
    for (index, &word) in words.iter().enumerate() {
        let mut bits = word;
        while bits != 0 {
            let row = slots[index * 64 + bits.trailing_zeros() as usize];
            if row != ROW_ABSENT {
                out.push(row);
            }
            bits &= bits - 1;
        }
    }
}

#[cfg(target_arch = "x86_64")]
mod x86 {
    use std::arch::x86_64::*;

    use super::*;

    /// 16 slots a step: a compare against the sentinel, the mask's 16 bits, and a compress.
    ///
    /// # Safety
    ///
    /// The CPU has AVX-512F, and `out` has spare capacity for `PAGE_ENTRIES + SLACK` rows.
    #[target_feature(enable = "avx512f")]
    pub(super) unsafe fn avx512(
        slots: &[u32; PAGE_ENTRIES],
        words: &[u64; WORDS],
        out: &mut Vec<u32>,
    ) {
        let absent = _mm512_set1_epi32(ROW_ABSENT as i32);
        let base = out.as_mut_ptr();
        let mut kept = 0usize;
        for (index, &word) in words.iter().enumerate() {
            if word == 0 {
                continue;
            }
            for quarter in 0..4 {
                let bits = (word >> (16 * quarter)) as u16;
                if bits == 0 {
                    continue;
                }
                // SAFETY: `index * 64 + quarter * 16 + 16` is at most `PAGE_ENTRIES`, and `kept`
                // is at most the slots read so far, so the store lands inside the reserved room.
                unsafe {
                    let at = slots.as_ptr().add(index * 64 + quarter * 16);
                    let lanes = _mm512_loadu_si512(at.cast());
                    let keep = bits & _mm512_cmpneq_epi32_mask(lanes, absent);
                    _mm512_storeu_si512(
                        base.add(kept).cast(),
                        _mm512_maskz_compress_epi32(keep, lanes),
                    );
                    kept += keep.count_ones() as usize;
                }
            }
        }
        // SAFETY: the first `kept` rows were written above.
        unsafe { out.set_len(kept) };
    }

    /// 8 slots a step: a compare against the sentinel, the mask's 8 bits, and a permutation from
    /// [`PACK`] that moves the survivors to the front.
    ///
    /// # Safety
    ///
    /// The CPU has AVX2, and `out` has spare capacity for `PAGE_ENTRIES + SLACK` rows.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn avx2(
        slots: &[u32; PAGE_ENTRIES],
        words: &[u64; WORDS],
        out: &mut Vec<u32>,
    ) {
        let absent = _mm256_set1_epi32(ROW_ABSENT as i32);
        let base = out.as_mut_ptr();
        let mut kept = 0usize;
        for (index, &word) in words.iter().enumerate() {
            if word == 0 {
                continue;
            }
            for eighth in 0..8 {
                let bits = (word >> (8 * eighth)) as u8;
                if bits == 0 {
                    continue;
                }
                // SAFETY: as in `avx512`, eight slots at a time.
                unsafe {
                    let at = slots.as_ptr().add(index * 64 + eighth * 8);
                    let lanes = _mm256_loadu_si256(at.cast());
                    let hits = _mm256_cmpeq_epi32(lanes, absent);
                    let missing = _mm256_movemask_ps(_mm256_castsi256_ps(hits)) as u8;
                    let keep = bits & !missing;
                    let order = _mm256_loadu_si256(PACK[usize::from(keep)].as_ptr().cast());
                    _mm256_storeu_si256(
                        base.add(kept).cast(),
                        _mm256_permutevar8x32_epi32(lanes, order),
                    );
                    kept += keep.count_ones() as usize;
                }
            }
        }
        // SAFETY: the first `kept` rows were written above.
        unsafe { out.set_len(kept) };
    }

    /// For each 8-bit mask, the lanes it keeps in ascending order, then lane 0 to fill.
    static PACK: [[u32; 8]; 256] = {
        let mut table = [[0u32; 8]; 256];
        let mut mask = 0;
        while mask < 256 {
            let mut lane = 0;
            let mut at = 0;
            while lane < 8 {
                if mask & (1 << lane) != 0 {
                    table[mask][at] = lane as u32;
                    at += 1;
                }
                lane += 1;
            }
            mask += 1;
        }
        table
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    /// Slots of distinct rows with some absent, and words from empty to full at several
    /// densities, including whole words of each.
    fn inputs(seed: u64) -> (Box<[u32; PAGE_ENTRIES]>, Vec<[u64; WORDS]>) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut slots: Box<[u32; PAGE_ENTRIES]> = vec![0u32; PAGE_ENTRIES]
            .into_boxed_slice()
            .try_into()
            .expect("one page");
        for (i, slot) in slots.iter_mut().enumerate() {
            *slot = if rng.gen_bool(0.1) {
                ROW_ABSENT
            } else {
                (i as u32) * 7 + 3
            };
        }
        let mut masks = vec![[0u64; WORDS], [u64::MAX; WORDS]];
        for density in [0.001, 0.05, 0.5, 0.95] {
            let mut words = [0u64; WORDS];
            for (i, word) in words.iter_mut().enumerate() {
                *word = match i % 7 {
                    0 => 0,
                    1 => u64::MAX,
                    _ => (0..64)
                        .filter(|_| rng.gen_bool(density))
                        .fold(0, |w, b| w | 1 << b),
                };
            }
            masks.push(words);
        }
        (slots, masks)
    }

    #[test]
    fn every_kernel_this_cpu_runs_selects_what_the_bits_select() {
        for seed in 0..3 {
            let (slots, masks) = inputs(seed);
            for words in &masks {
                let mut want = Vec::new();
                scalar(&slots, words, &mut want);
                let expected: Vec<u32> = (0..PAGE_ENTRIES)
                    .filter(|&i| words[i / 64] >> (i % 64) & 1 != 0 && slots[i] != ROW_ABSENT)
                    .map(|i| slots[i])
                    .collect();
                assert_eq!(want, expected, "the scalar form selects the set bits' rows");
                let mut got = Vec::new();
                select(&slots, words, &mut got);
                assert_eq!(got, want, "the dispatched kernel");
                #[cfg(target_arch = "x86_64")]
                for (name, present) in [
                    ("avx512f", std::arch::is_x86_feature_detected!("avx512f")),
                    ("avx2", std::arch::is_x86_feature_detected!("avx2")),
                ] {
                    if !present {
                        continue;
                    }
                    let mut wide = Vec::with_capacity(PAGE_ENTRIES + SLACK);
                    // SAFETY: the feature is present and the capacity reserved.
                    unsafe {
                        match name {
                            "avx512f" => x86::avx512(&slots, words, &mut wide),
                            _ => x86::avx2(&slots, words, &mut wide),
                        }
                    }
                    assert_eq!(wide, want, "the {name} kernel");
                }
            }
        }
    }
}
