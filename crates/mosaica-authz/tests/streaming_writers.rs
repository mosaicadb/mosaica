//! Byte-identity tests: `DictStreamWriter` against `DictWriter`, and the two posting encoders.

use std::fs;
use tempfile::TempDir;
use mosaica_authz::{encode_posting, Dict, DictStreamWriter, DictWriter};

/// Distinct descriptors of varied lengths, including one empty descriptor. Distinctness is the
/// streaming writer's caller contract; without it `DictWriter` would deduplicate.
fn descriptors(n: usize) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new()];
    for i in 0..n.saturating_sub(1) {
        let mut d = format!("descriptor-{i:05}-").into_bytes();
        d.extend(std::iter::repeat_n(b'x', i % 13));
        out.push(d);
    }
    out
}

#[test]
fn dict_stream_writer_matches_dict_writer_bytes() {
    let descriptors = descriptors(1000);

    let buffered_dir = TempDir::new().unwrap();
    let mut buffered = DictWriter::new(buffered_dir.path());
    for (i, d) in descriptors.iter().enumerate() {
        assert_eq!(buffered.intern(d).raw(), i as u32);
    }
    let buffered_paths = buffered.finish().unwrap();

    let streamed_dir = TempDir::new().unwrap();
    let mut streamed = DictStreamWriter::new(streamed_dir.path());
    for (i, d) in descriptors.iter().enumerate() {
        assert_eq!(streamed.append(d).unwrap().raw(), i as u32);
    }
    assert_eq!(streamed.len(), descriptors.len() as u32);
    let streamed_paths = streamed.finish().unwrap();

    assert_eq!(buffered_paths.len(), 1);
    assert_eq!(streamed_paths.len(), 1);
    assert_eq!(
        streamed_paths[0].file_name(),
        buffered_paths[0].file_name(),
        "extent naming must match"
    );
    assert_eq!(
        fs::read(&streamed_paths[0]).unwrap(),
        fs::read(&buffered_paths[0]).unwrap(),
        "extent bytes must be identical"
    );

    // The streamed extent must load and resolve ids exactly as assigned.
    let dict = Dict::load(&streamed_paths).unwrap();
    assert_eq!(dict.len(), descriptors.len() as u32);
    assert_eq!(dict.lookup(&descriptors[7]).unwrap().raw(), 7);
}

#[test]
fn dict_stream_writer_matches_dict_writer_bytes_when_empty() {
    let buffered_dir = TempDir::new().unwrap();
    let buffered_paths = DictWriter::new(buffered_dir.path()).finish().unwrap();

    let streamed_dir = TempDir::new().unwrap();
    let streamed = DictStreamWriter::new(streamed_dir.path());
    assert!(streamed.is_empty());
    let streamed_paths = streamed.finish().unwrap();

    assert_eq!(
        fs::read(&streamed_paths[0]).unwrap(),
        fs::read(&buffered_paths[0]).unwrap(),
        "an empty dictionary must still write an identical (empty) extent"
    );
}

/// `encode_posting_bitmap` and `encode_posting` agree byte for byte, across the tag boundary.
/// This sweeps the boundary explicitly: cardinalities either side of `small_term_threshold` take
/// different arms, tag 0 raw `u32` LEs and tag 1 run-optimised portable Roaring, and both arms
/// must match.
#[test]
fn the_bitmap_and_slice_encoders_agree_byte_for_byte() {
    use croaring::Bitmap;
    use mosaica_authz::encode_posting_bitmap;

    const THRESHOLD: u32 = 8;

    let cases: Vec<Vec<u32>> = vec![
        vec![],
        vec![7],
        (0..THRESHOLD).collect(),     // exactly at the threshold: tag 0
        (0..THRESHOLD + 1).collect(), // one past it: tag 1
        (0..5_000u32).collect(),      // one long run
        (0..5_000u32).map(|i| i * 977).collect(), // scattered, no runs
        vec![0, u32::MAX / 2, u32::MAX - 1], // sparse across the whole space
    ];

    for entities in cases {
        let from_slice = encode_posting(0, &entities, THRESHOLD)
            .expect("the view encoder accepts sorted input");
        let from_bitmap = encode_posting_bitmap(&Bitmap::of(&entities), THRESHOLD)
            .expect("the bitmap encoder accepts any bitmap");
        assert_eq!(
            from_slice,
            from_bitmap,
            "the two encoders disagree at cardinality {} (tag {})",
            entities.len(),
            from_slice.first().copied().unwrap_or(255)
        );
    }
}
