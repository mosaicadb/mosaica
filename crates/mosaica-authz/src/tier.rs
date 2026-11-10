//! The sparse delta postings tier a flush publishes: the terms its flushed items carried, and
//! nothing else.
//!
//! A postings file's ordinal-indexed layout, record *i* is term *i*, is right for a build, where
//! the ordinals are dense. A flush's terms are an arbitrary handful of ordinals, so the same
//! layout would write a file dense to the highest ordinal used. A tier is instead a keyed record
//! container ([`crate::records`]), which stores each term id beside its posting and is looked up by
//! binary search, with the same posting encoding: [`crate::encode_posting`] and the same
//! [`PostingRef`].
//!
//! [`DeltaTier::posting`] answers `None` for a term this tier does not carry, exactly as
//! [`crate::PostingsReader::posting`] does for a term beyond its range. A fragment build unions
//! over the session's satisfied terms and skips the misses, so a tier contributes only for the
//! terms it actually holds, never its whole term set, which would hand a viewer entities it was
//! never granted.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use arrow::array::LargeBinaryArray;
use arrow::buffer::ScalarBuffer;

use mosaica_types::TermId;

use crate::postings::{encode_posting, read_posting, PostingRef, POSTINGS_COLUMN};
use crate::records::{open_keyed_record_column, RecordWriter};

/// How many of a code's high bits the bucket table [`DeltaTier::open_indexed`] builds is indexed
/// by. The table is `2^BUCKET_BITS + 1` `u32` offsets, small enough to keep the search inside a
/// bucket within a cache line or two for a large key array.
const BUCKET_BITS: u32 = 20;

/// `1 << BUCKET_BITS`, named because the table carries a sentinel and is one longer.
const BUCKET_COUNT: usize = 1 << BUCKET_BITS;

/// The record count below which [`DeltaTier::open_indexed`] builds no table: below this the key
/// array is small enough to stay in cache, so a plain binary search is already cheap.
const BUCKET_TABLE_MIN_RECORDS: usize = 1 << 16;

/// `t[b]` is the first index whose code has bucket prefix `b`, with `t[BUCKET_COUNT]` the
/// sentinel. Free to build: the key array is already sorted, so one pass fills the table.
fn build_bucket_table(codes: &[u32]) -> Box<[u32]> {
    let mut table = vec![0u32; BUCKET_COUNT + 1];
    let mut at = 0usize;
    for (bucket, slot) in table.iter_mut().enumerate().take(BUCKET_COUNT) {
        while at < codes.len() && (codes[at] >> (u32::BITS - BUCKET_BITS)) < bucket as u32 {
            at += 1;
        }
        *slot = at as u32;
    }
    table[BUCKET_COUNT] = codes.len() as u32;
    table.into_boxed_slice()
}

/// Write a sparse tier: `entries` is `(term, sorted entity list)` in strictly ascending term
/// order, which the writer checks: the lookup is a binary search over the keys, so an unordered or
/// duplicated file would make a posting unreachable, which on this path is items a viewer is
/// entitled to not being there.
///
/// Each entity list must itself be sorted strictly ascending; [`encode_posting`] enforces that.
pub fn write_delta_tier(
    path: &Path,
    entries: &[(TermId, Vec<u32>)],
    small_term_threshold: u32,
) -> io::Result<()> {
    write_tier_entries(
        path,
        entries.iter().map(|(t, e)| (t.raw(), e.as_slice())),
        small_term_threshold,
    )
}

/// The same writer, addressed by bare record ordinals.
pub fn write_delta_tier_at(
    path: &Path,
    entries: &[(u32, Vec<u32>)],
    small_term_threshold: u32,
) -> io::Result<()> {
    write_tier_entries(
        path,
        entries.iter().map(|(t, e)| (*t, e.as_slice())),
        small_term_threshold,
    )
}

fn write_tier_entries<'a>(
    path: &Path,
    entries: impl IntoIterator<Item = (u32, &'a [u32])>,
    small_term_threshold: u32,
) -> io::Result<()> {
    let mut writer = RecordWriter::create_keyed(path, POSTINGS_COLUMN)?;
    for (term, entities) in entries {
        let record = encode_posting(term as usize, entities, small_term_threshold)?;
        writer.append_keyed(term, &record)?;
    }
    writer.finish()
}

/// One live delta postings tier, memory-mapped. Postings borrow from the mapping without
/// copying, exactly as [`crate::PostingsReader`]'s do.
#[derive(Debug)]
pub struct DeltaTier {
    terms: ScalarBuffer<u32>,
    postings: LargeBinaryArray,
    /// The bucket table over the key array's top [`BUCKET_BITS`] bits, or `None` where this tier
    /// was opened without one. See [`Self::open_indexed`]. Built in memory, per open.
    buckets: Option<Box<[u32]>>,
}

impl DeltaTier {
    pub fn open(path: &Path) -> io::Result<Self> {
        let (terms, postings) = open_keyed_record_column(path, POSTINGS_COLUMN)?;
        crate::postings::validate_records(&postings)?;
        Ok(DeltaTier {
            terms,
            postings,
            buckets: None,
        })
    }

    /// [`Self::open`], plus the bucket table its lookups then search inside. The answer is
    /// identical either way. A tier below [`BUCKET_TABLE_MIN_RECORDS`] records gets no table.
    pub fn open_indexed(path: &Path) -> io::Result<Self> {
        let mut tier = Self::open(path)?;
        if tier.terms.len() >= BUCKET_TABLE_MIN_RECORDS {
            tier.buckets = Some(build_bucket_table(&tier.terms));
        }
        Ok(tier)
    }

    /// Whether this tier holds a bucket table: an operator/bench observable, never a behaviour.
    pub fn is_bucketed(&self) -> bool {
        self.buckets.is_some()
    }

    /// The record index for `ordinal`, or `None` where this tier carries no such key.
    fn index_of(&self, ordinal: u32) -> Option<usize> {
        let codes = &self.terms[..];
        let Some(buckets) = self.buckets.as_deref() else {
            return codes.binary_search(&ordinal).ok();
        };
        let bucket = (ordinal >> (u32::BITS - BUCKET_BITS)) as usize;
        let lo = buckets[bucket] as usize;
        let hi = buckets[bucket + 1] as usize;
        codes[lo..hi].binary_search(&ordinal).ok().map(|at| lo + at)
    }

    /// How many terms this tier carries: its record count, never a dictionary bound.
    pub fn term_count(&self) -> u32 {
        self.terms.len() as u32
    }

    /// The bytes the records of the terms before `t` hold, for a writer that divides the file by
    /// bytes.
    pub fn bytes_before(&self, t: u32) -> u64 {
        let records = self.terms.partition_point(|&term| term < t);
        crate::records::record_bytes_before(&self.postings, records as u32)
    }

    /// The term ids this tier carries, ascending. Needed because [`Self::term_count`] is a
    /// record count, not an id domain: walking `0..term_count()` would find nothing.
    pub fn terms(&self) -> impl Iterator<Item = TermId> + '_ {
        self.terms.iter().map(|t| TermId::new(*t))
    }

    /// This tier's posting for `t`, or `None` if it carries none. `None` is an ordinary answer:
    /// a tier holds only the terms its flushed items carried.
    pub fn posting(&self, t: TermId) -> io::Result<Option<PostingRef<'_>>> {
        self.posting_at(t.raw())
    }

    /// The same lookup, addressed by a bare record ordinal.
    pub fn posting_at(&self, ordinal: u32) -> io::Result<Option<PostingRef<'_>>> {
        let Some(idx) = self.index_of(ordinal) else {
            return Ok(None);
        };
        read_posting(&self.postings, idx).map(Some)
    }

    /// The record ordinals this tier carries, ascending: [`Self::terms`] untyped.
    pub fn ordinals(&self) -> impl Iterator<Item = u32> + '_ {
        self.terms.iter().copied()
    }

    /// [`Self::posting_at`]'s first half alone: the binary search over the key array.
    #[cfg(feature = "bench-timing")]
    pub fn bench_record_index(&self, ordinal: u32) -> Option<usize> {
        self.index_of(ordinal)
    }

    /// [`Self::posting_at`]'s second half alone: the view over the mapped record's bytes.
    #[cfg(feature = "bench-timing")]
    pub fn bench_posting_at_index(&self, idx: usize) -> io::Result<PostingRef<'_>> {
        read_posting(&self.postings, idx)
    }
}

/// Coalesce several delta tiers into one, at `out`. Content-preserving: the same `(term, entity)`
/// pairs the inputs carried, unioned per term, deduplicated and re-sorted, nothing dropped.
///
/// A merge retires nothing: no tombstone is applied, since that is the compaction fold's job.
/// The dedup is required, not defensive: two tiers may legitimately carry the same term, and
/// concatenating their entity lists would yield a non-ascending sequence that [`encode_posting`]
/// hard-fails on.
///
/// Reads every input fully into memory: a tier holds one tick's arrivals.
pub fn coalesce_delta_tiers(
    inputs: &[PathBuf],
    out: &Path,
    small_term_threshold: u32,
) -> io::Result<()> {
    let mut by_term: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for path in inputs {
        let tier = DeltaTier::open(path)?;
        for term in tier.terms() {
            let Some(posting) = tier.posting(term)? else {
                continue;
            };
            posting.extend_into(by_term.entry(term.raw()).or_default());
        }
    }

    let entries: Vec<(TermId, Vec<u32>)> = by_term
        .into_iter()
        .map(|(term, mut entities)| {
            entities.sort_unstable();
            entities.dedup();
            (TermId::new(term), entities)
        })
        .collect();
    write_delta_tier(out, &entries, small_term_threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    fn entities_of(tier: &DeltaTier, code: u32) -> Option<Vec<u32>> {
        tier.posting_at(code).unwrap().map(|posting| posting.iter().collect())
    }

    /// The bucket table changes the search's bounds and nothing else. An off-by-one here would
    /// hand back a neighbour's posting, which on this path is a viewer shown another value's
    /// members.
    #[test]
    fn the_bucket_table_resolves_every_code_as_the_plain_search_does() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x0b7c);
        let mut codes: Vec<u32> = (0..(1u32 << 17)).map(|_| rng.gen::<u32>()).collect();
        codes.extend_from_slice(&[0, u32::MAX, 1 << 12, (1 << 12) - 1, 1u32 << 31]);
        codes.sort_unstable();
        codes.dedup();

        let entries: Vec<(u32, Vec<u32>)> = codes
            .iter()
            .enumerate()
            .map(|(at, code)| (*code, vec![at as u32]))
            .collect();
        let path = dir.path().join("bucketed");
        write_delta_tier_at(&path, &entries, 32).unwrap();

        let plain = DeltaTier::open(&path).unwrap();
        let bucketed = DeltaTier::open_indexed(&path).unwrap();
        assert!(!plain.is_bucketed());
        assert!(bucketed.is_bucketed(), "2¹⁷ records is over the floor");

        for code in &codes {
            assert_eq!(entities_of(&bucketed, *code), entities_of(&plain, *code));
            assert!(entities_of(&bucketed, *code).is_some(), "{code} is held");
        }

        let held: std::collections::HashSet<u32> = codes.iter().copied().collect();
        let mut absent: Vec<u32> = (0..20_000).map(|_| rng.gen::<u32>()).collect();
        for code in codes.iter().take(4096) {
            absent.push(code.wrapping_add(1));
            absent.push(code.wrapping_sub(1));
        }
        for code in absent.into_iter().filter(|c| !held.contains(c)) {
            assert_eq!(entities_of(&bucketed, code), entities_of(&plain, code));
            assert!(entities_of(&bucketed, code).is_none(), "{code} is absent");
        }
    }

    /// A tier under the floor keeps the plain search and answers identically.
    #[test]
    fn a_small_tier_is_not_bucketed_and_answers_the_same() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("small");
        let entries: Vec<(u32, Vec<u32>)> = (0..1_000u32).map(|c| (c * 7919, vec![c])).collect();
        write_delta_tier_at(&path, &entries, 32).unwrap();

        let tier = DeltaTier::open_indexed(&path).unwrap();
        assert!(!tier.is_bucketed());
        for (code, _) in &entries {
            assert!(tier.posting_at(*code).unwrap().is_some());
        }
        assert!(tier.posting_at(1).unwrap().is_none());
    }
}
