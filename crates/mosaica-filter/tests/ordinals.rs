//! A keyword layer's ordinals written a block at a time, read back, and asked every question the
//! filter asks, against the same ordinals held flat in memory.

use std::path::Path;

use croaring::Bitmap;
use mosaica_filter::{
    take_scan_work, write_value_column, Access, CodeSet, Codes, ColumnKind, Endpoint, Scalar,
    ValueColumn, ValueColumnWriter, PRESENCE_FILE, VALUES_FILE,
};
use mosaica_types::AttrLocalId;

/// A deterministic stream of numbers, so a failure names the same column every run.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n)) as u32
    }
}

const BLOCK: u32 = 1 << 16;

/// Values for `n` slots, the `b`-th block of them drawn from a domain chosen so each width is
/// written: one value, a few hundred, a few thousand, and all but unique.
fn shaped(n: usize, draw: &mut Draw) -> Vec<u32> {
    (0..n)
        .map(|slot| match (slot as u32 / BLOCK) % 4 {
            0 => 7,
            1 => 1_000 + draw.below(200) * 3,
            2 => 50_000 + draw.below(5_000),
            _ => draw.below(1 << 30),
        })
        .collect()
}

/// Write `values` as a keyword column through the streaming writer in uneven chunks, and open it.
fn written(dir: &Path, values: &[u32], presence: Option<&Bitmap>) -> ValueColumn {
    let values_path = dir.join(VALUES_FILE);
    let presence_path = dir.join(PRESENCE_FILE);
    let mut writer = ValueColumnWriter::create(&values_path, &presence_path, ColumnKind::Ordinals)
        .expect("create");
    for chunk in values.chunks(9_973) {
        writer.push(&Codes::U32(chunk.to_vec().into())).expect("push");
    }
    writer.finish(presence).expect("finish");
    for entry in std::fs::read_dir(dir).expect("dir") {
        let name = entry.expect("entry").file_name();
        assert!(!name.to_string_lossy().contains("spool"), "left {name:?} behind");
    }
    ValueColumn::open_dir(dir, Access::Mapped).expect("open")
}

fn flat(values: &[u32], presence: Option<&Bitmap>) -> ValueColumn {
    let codes = Codes::U32(values.to_vec().into());
    match presence {
        None => ValueColumn::universal(codes),
        Some(p) => ValueColumn::partial(codes, p.clone()).expect("flat"),
    }
}

fn int(v: u32, inclusive: bool) -> Option<Endpoint> {
    Some(Endpoint {
        value: Scalar::Int(i128::from(v)),
        inclusive,
    })
}

/// Every question the filter asks of a keyword layer, answered alike by both columns.
fn agree(blocks: &ValueColumn, flat: &ValueColumn, candidate: &Bitmap, values: &[u32]) {
    let held = |i: usize| values[i * 7_919 % values.len()];
    let needles = [held(0), held(1), held(2), held(3), 3, 999_999_999, u32::MAX];
    for &needle in &needles {
        let id = AttrLocalId::new(needle);
        assert_eq!(
            blocks.scan_eq(candidate, id),
            flat.scan_eq(candidate, id),
            "eq {needle}"
        );
    }
    let ids: Vec<AttrLocalId> = needles.iter().map(|&v| AttrLocalId::new(v)).collect();
    assert_eq!(blocks.scan_in(candidate, &ids), flat.scan_in(candidate, &ids), "in");
    let ranges = [
        (int(1_000, true), int(1_300, true)),
        (int(1_000, false), int(1_003, false)),
        (int(50_000, true), None),
        (None, int(7, true)),
        (int(10, true), int(9, true)),
        (int(u32::MAX, false), None),
        (None, int(0, false)),
        (None, None),
    ];
    for (lo, hi) in ranges {
        assert_eq!(
            blocks.scan_range(candidate, lo, hi),
            flat.scan_range(candidate, lo, hi),
            "range {lo:?}..{hi:?}"
        );
    }
    let mut set = CodeSet::over_domain(60_000);
    for v in [7, 1_003, 1_300, 52_345, 54_999, 59_999] {
        set.insert(v);
    }
    assert_eq!(
        blocks.scan_ordinal_set(candidate, &set),
        flat.scan_ordinal_set(candidate, &set),
        "set"
    );
    let mut a = Vec::new();
    let mut b = Vec::new();
    blocks.for_each_code_in(candidate, |e, v| a.push((e, v)));
    flat.for_each_code_in(candidate, |e, v| b.push((e, v)));
    assert_eq!(a, b, "codes over the candidate");
}

/// The candidates a filter is asked under: everything, nothing, a scattered sample, a contiguous
/// stretch crossing a block boundary, and one entity.
fn candidates(top: u32, draw: &mut Draw) -> Vec<Bitmap> {
    let mut scattered = Bitmap::new();
    for _ in 0..(top / 3) {
        scattered.add(draw.below(top));
    }
    vec![
        Bitmap::from_range(0..top),
        Bitmap::new(),
        scattered,
        Bitmap::from_range(BLOCK - 100..BLOCK * 2 + 100),
        Bitmap::of(&[BLOCK * 3 + 5]),
    ]
}

#[test]
fn a_column_every_entity_carries_answers_as_its_flat_ordinals_do() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut draw = Draw(0x9E37_79B9_7F4A_7C15);
    let n = (BLOCK * 4 + 1_234) as usize;
    let values = shaped(n, &mut draw);
    let blocks = written(dir.path(), &values, None);
    let flat = flat(&values, None);

    let Codes::Ordinals(stored) = blocks.codes() else {
        panic!("a keyword column reads back as ordinal blocks");
    };
    let (widths, bytes) = stored.census();
    assert_eq!(widths, [0, 2, 1, 1, 1], "const twice (the short last block too), u8, u16, plain");
    assert!(bytes < 4 * n as u64, "{bytes} bytes for {n} values");

    for candidate in candidates(n as u32 + 10, &mut draw) {
        agree(&blocks, &flat, &candidate, &values);
    }
    for entity in [0, 1, BLOCK - 1, BLOCK, BLOCK * 3 + 17, n as u32 - 1, n as u32] {
        assert_eq!(blocks.value_of(entity), flat.value_of(entity), "entity {entity}");
    }
    let mut decoded = Vec::new();
    assert!(blocks.codes().extend_codes(5, n - 10, &mut decoded));
    assert_eq!(decoded, values[5..n - 5]);
}

#[test]
fn a_column_with_holes_answers_as_its_flat_ordinals_do() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut draw = Draw(0xD1B5_4A32_D192_ED03);
    // Present in four blocks with an empty one between, the last high in entity space.
    let mut presence = Bitmap::new();
    for k in [0u32, 1, 3, 4, 60_000] {
        for _ in 0..40_000 {
            presence.add(k * BLOCK + draw.below(BLOCK));
        }
    }
    let n = presence.cardinality() as usize;
    let values = shaped(n, &mut draw);
    let blocks = written(dir.path(), &values, Some(&presence));
    let flat = flat(&values, Some(&presence));

    let mut cases = candidates(BLOCK * 5, &mut draw);
    cases.push(Bitmap::from_range(BLOCK * 60_000..BLOCK * 60_001));
    for candidate in cases {
        agree(&blocks, &flat, &candidate, &values);
    }
    for entity in presence.iter().step_by(997) {
        assert_eq!(blocks.value_of(entity), flat.value_of(entity), "entity {entity}");
    }
    assert_eq!(blocks.value_of(BLOCK * 2 + 1), None, "an entity in the empty block");
}

#[test]
fn a_held_value_and_an_absent_one_cost_the_same_scan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut draw = Draw(42);
    let values = shaped((BLOCK * 4) as usize, &mut draw);
    let blocks = written(dir.path(), &values, None);
    let candidate = candidates(BLOCK * 4, &mut draw).swap_remove(2);

    take_scan_work();
    let found = blocks.scan_eq(&candidate, AttrLocalId::new(values[BLOCK as usize + 1]));
    let held = take_scan_work();
    let missing = blocks.scan_eq(&candidate, AttrLocalId::new(3));
    let absent = take_scan_work();
    assert!(!found.is_empty() && missing.is_empty());
    assert!(held.slots > 0);
    assert_eq!(held, absent, "a held needle and an absent one walk the same slots");
    assert_eq!(held.slots, candidate.cardinality(), "every candidate value is compared once");
}

#[test]
fn a_parallel_scan_answers_as_a_serial_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut draw = Draw(7);
    let n = (BLOCK * 20) as usize;
    let values = shaped(n, &mut draw);
    let blocks = written(dir.path(), &values, None);
    let flat = flat(&values, None);
    let candidate = Bitmap::from_range(0..n as u32);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("pool");
    let needle = AttrLocalId::new(values[BLOCK as usize * 5 + 3]);
    let (parallel, work) = pool.install(|| {
        take_scan_work();
        let out = blocks.scan_eq(&candidate, needle);
        (out, take_scan_work())
    });
    assert_eq!(parallel, flat.scan_eq(&candidate, needle));
    assert_eq!(work.slots, n as u64, "each piece's work returns to the asking thread");
}

#[test]
fn a_rewritten_column_is_the_same_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut draw = Draw(11);
    let values = shaped((BLOCK * 3 + 9) as usize, &mut draw);
    let blocks = written(dir.path(), &values, None);
    let again = dir.path().join("again.arrow");
    write_value_column(&again, &dir.path().join("again.roaring"), blocks.codes(), None)
        .expect("rewrite");
    assert_eq!(
        std::fs::read(dir.path().join(VALUES_FILE)).expect("first"),
        std::fs::read(&again).expect("second")
    );
}

#[test]
fn blocks_that_do_not_fit_the_presence_are_refused_at_open() {
    let dir = tempfile::tempdir().expect("tempdir");
    let presence = Bitmap::from_range(0..1_000);
    written(dir.path(), &vec![5; 1_000], Some(&presence));
    // As many entities, in another block.
    mosaica_filter::write_presence(
        &dir.path().join(PRESENCE_FILE),
        &Bitmap::from_range(BLOCK..BLOCK + 1_000),
    )
    .expect("presence");
    assert!(ValueColumn::open_dir(dir.path(), Access::Read).is_err());
}

/// A `u16` category's codes, one of three shapes per block: one code, a few dozen, and nearly as
/// many as the block holds.
fn shaped16(n: usize, draw: &mut Draw) -> Vec<u16> {
    (0..n)
        .map(|slot| match (slot as u32 / BLOCK) % 3 {
            0 => 9,
            1 => 100 + draw.below(40) as u16,
            _ => 1 + draw.below(65_535) as u16,
        })
        .collect()
}

#[test]
fn a_u16_category_answers_and_reads_back_as_its_flat_codes() {
    use mosaica_filter::{CodeBytes, RecordValue};
    let dir = tempfile::tempdir().expect("tempdir");
    let mut draw = Draw(0x51_7CC1_B727_220A);
    let mut presence = Bitmap::new();
    for k in [0u32, 1, 2, 5] {
        for _ in 0..50_000 {
            presence.add(k * BLOCK + draw.below(BLOCK));
        }
    }
    let n = presence.cardinality() as usize;
    let codes = shaped16(n, &mut draw);

    let values_path = dir.path().join(VALUES_FILE);
    let presence_path = dir.path().join(PRESENCE_FILE);
    let mut writer =
        ValueColumnWriter::create(&values_path, &presence_path, ColumnKind::Ordinals16)
            .expect("create");
    for chunk in codes.chunks(7_001) {
        writer.push(&Codes::U16(chunk.to_vec().into())).expect("push");
    }
    writer.finish(Some(&presence)).expect("finish");
    let blocks = ValueColumn::open_dir(dir.path(), Access::Mapped).expect("open");
    let flat = ValueColumn::partial(Codes::U16(codes.clone().into()), presence.clone()).unwrap();

    let Codes::Ordinals(stored) = blocks.codes() else {
        panic!("a u16 category reads back as blocks");
    };
    assert_eq!(stored.code_bytes(), CodeBytes::Two);
    assert_eq!(ColumnKind::of(blocks.codes()), ColumnKind::Ordinals16);
    let (widths, bytes) = stored.census();
    assert_eq!(widths[0], 2, "the two blocks no entity is present in");
    assert!(widths[1] > 0 && widths[2] > 0 && widths[4] > 0, "{widths:?}");
    assert_eq!(widths[3], 0, "two-byte positions are never smaller than two-byte codes");
    assert!(bytes < 2 * n as u64, "{bytes} bytes for {n} two-byte codes");

    let wide: Vec<u32> = codes.iter().map(|&c| u32::from(c)).collect();
    let mut cases = candidates(BLOCK * 6, &mut draw);
    cases.push(presence.clone());
    for candidate in &cases {
        agree(&blocks, &flat, candidate, &wide);
        let mut seen = 0;
        blocks.visit_codes(candidate, |_| seen += 1);
        assert_eq!(seen, candidate.and_cardinality(&presence), "one code per candidate entity");
    }
    let records = |column: &ValueColumn| {
        let mut out = Vec::new();
        column
            .for_each_record_value_in(&presence, |e, v| {
                out.push((e, v));
                Ok::<_, ()>(())
            })
            .unwrap();
        out
    };
    let (a, b) = (records(&blocks), records(&flat));
    assert_eq!(a, b, "records read back at the declared width");
    assert!(matches!(a[0].1, RecordValue::U16(_)));
}

#[test]
fn a_u16_column_refuses_wider_codes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut writer = ValueColumnWriter::create(
        &dir.path().join(VALUES_FILE),
        &dir.path().join(PRESENCE_FILE),
        ColumnKind::Ordinals16,
    )
    .expect("create");
    assert!(writer.push(&Codes::U32(vec![70_000].into())).is_err());
}

#[test]
fn a_run_of_plain_blocks_answers_as_its_flat_codes() {
    let mut draw = Draw(0xA076_1D64_78BD_642F);
    let n = (BLOCK * 5 + 77) as usize;
    let values: Vec<u32> = (0..n).map(|_| draw.below(1 << 30)).collect();
    let dense = tempfile::tempdir().expect("tempdir");
    let blocks = written(dense.path(), &values, None);
    let Codes::Ordinals(stored) = blocks.codes() else {
        panic!("blocks");
    };
    assert_eq!(stored.census().0, [0, 0, 0, 0, 6], "every block plain");
    for candidate in candidates(n as u32 + 3, &mut draw) {
        agree(&blocks, &flat(&values, None), &candidate, &values);
    }

    let mut presence = Bitmap::new();
    for k in [0u32, 1, 2, 4] {
        for _ in 0..30_001 {
            presence.add(k * BLOCK + draw.below(BLOCK));
        }
    }
    let n = presence.cardinality() as usize;
    let partial = tempfile::tempdir().expect("tempdir");
    let blocks = written(partial.path(), &values[..n], Some(&presence));
    for candidate in candidates(BLOCK * 5, &mut draw) {
        agree(&blocks, &flat(&values[..n], Some(&presence)), &candidate, &values[..n]);
    }
}
