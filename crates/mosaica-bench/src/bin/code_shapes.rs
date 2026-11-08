//! **What a keyword or category filter costs across data shapes: codes stored flat against per
//! block, and a scan of the viewer's whole visible set against one of a viewport's items.**
//!
//! A keyword column's ordinals and a `u16` or `u32` category's codes are written a block of 2¹⁶
//! entities at a time in the narrowest encoding the block allows
//! (`mosaica_filter::BlockOrdinals`), and a viewport's filter may be
//! evaluated over the items its tiles hold where that reads less (`narrowing_pays` in the engine).
//! Both pay off by how a corpus's values are distributed, so this measures a grid of shapes rather
//! than one corpus:
//!
//! - **codes**: four bytes wide, as a keyword's ordinals and a `u32` category's codes are, or two,
//!   as a `u16` category's are;
//! - **distinct**: how many values the column holds, from a handful to one per entity;
//! - **order**: how values fall in entity order. `uniform` draws each entity's value independently,
//!   `skewed` draws from a Zipf-like tail where a few values dominate, `sorted` gives runs of equal
//!   values, as a corpus ingested by source would;
//! - **candidate**: the visible set whole, or a viewport's share of it, either scattered across
//!   entity space (insertion order unrelated to position) or contiguous (insertion order following
//!   position).
//!
//! Each scan is an `eq` for a held value on the engine's pool. **Cold** scans evict the file from the
//! page cache first and report the major faults they took, so a cold figure that faulted nothing
//! was answered from memory and is marked.
//!
//! ```text
//! cargo build --release -p mosaica-bench --bin code_shapes
//! target/release/code_shapes --entities 33554432 --dir /path/on/the/disk/under/test
//! ```
//!
//! # Measured 2026-11-08, a 4-core cloud container with 15 GB of memory, release
//!
//! 2²⁵ entities, every one carrying a value, median of 9. A cold scan of 138 MB took about
//! 55–65 ms here, so this disc reads about 2.3 GB/s; the ratios, not the times, are what carry
//! to another machine. Cold figures moved by up to 20% between otherwise identical runs.
//!
//! | codes | distinct | order | flat MB | blocks MB | warm flat ms | warm blocks ms | cold flat ms | cold blocks ms |
//! |---|---:|---|---:|---:|---:|---:|---:|---:|
//! | u32 | 16 | uniform | 138 | 35 | 11.8 | 13.2 | 61.4 | 23.0 |
//! | u32 | 16 | skewed | 138 | 35 | 11.1 | 11.1 | 61.3 | 21.9 |
//! | u32 | 16 | sorted | 138 | 0 | 10.6 | 0.3 | 56.6 | 0.4 |
//! | u32 | 10 000 | uniform | 138 | 90 | 11.2 | 7.3 | 61.9 | 47.9 |
//! | u32 | 10 000 | skewed | 138 | 89 | 17.9 | 14.9 | 66.2 | 47.7 |
//! | u32 | 10 000 | sorted | 138 | 35 | 8.8 | 7.3 | 67.6 | 21.1 |
//! | u32 | 10⁷ | uniform | 138 | 138 | 10.3 | 12.5 | 62.7 | 85.5 |
//! | u32 | 10⁷ | skewed | 138 | 138 | 11.1 | 12.2 | 61.7 | 74.1 |
//! | u32 | 10⁷ | sorted | 138 | 110 | 11.1 | 9.2 | 63.9 | 60.0 |
//! | u32 | 2²⁵ | uniform | 138 | 138 | 11.1 | 12.4 | 62.9 | 73.9 |
//! | u32 | 2²⁵ | skewed | 138 | 138 | 12.5 | 11.3 | 60.3 | 70.0 |
//! | u32 | 2²⁵ | sorted | 138 | 138 | 9.9 | 11.6 | 60.5 | 71.0 |
//! | u16 | 16 | uniform | 71 | 35 | 12.5 | 13.4 | 33.9 | 24.9 |
//! | u16 | 16 | skewed | 71 | 35 | 11.1 | 11.5 | 31.7 | 21.4 |
//! | u16 | 16 | sorted | 71 | 0 | 6.5 | 0.4 | 30.3 | 0.5 |
//! | u16 | 1 000 | uniform | 71 | 69 | 7.7 | 8.1 | 32.5 | 34.1 |
//! | u16 | 1 000 | skewed | 71 | 69 | 10.2 | 12.3 | 35.3 | 40.1 |
//! | u16 | 1 000 | sorted | 71 | 35 | 6.6 | 9.0 | 33.8 | 20.0 |
//! | u16 | 60 000 | uniform | 71 | 69 | 6.7 | 7.9 | 31.8 | 36.7 |
//! | u16 | 60 000 | skewed | 71 | 69 | 7.5 | 6.7 | 32.8 | 35.2 |
//! | u16 | 60 000 | sorted | 71 | 35 | 5.1 | 9.5 | 32.2 | 18.7 |
//!
//! Blocks shrink a column wherever a block holds few distinct codes for its entities, and a cold
//! scan then reads less: 2.5–3× faster for four-byte codes at 16 values, 1.3–1.4× at 10 000 drawn
//! independently, 1.4–1.7× for two-byte codes at 16 values or in runs, and far more where whole
//! blocks hold one value. Where a block holds too many distinct codes to shrink, it is stored
//! plain, and the file is flat storage's size; a cold scan of such a column measured 0–35% slower
//! than the flat one across runs, with the same faults, and a warm one within about 15%. A
//! two-byte column whose values come in runs scans 1.4–1.9× slower warm, though it reads half the
//! bytes cold.
//!
//! A viewport's candidate, at 10 000 four-byte values drawn uniformly, stored per block, against a
//! scan of the whole set (7.3 ms warm, 47.9 ms cold). `narrows` is the engine's rule:
//!
//! | share | layout | entities | blocks | warm ms | cold ms | narrows |
//! |---:|---|---:|---:|---:|---:|---|
//! | 10⁻⁴ | scattered | 3 355 | 512 | 2.9 | 88.6 | no |
//! | 10⁻⁴ | contiguous | 3 355 | 1 | 0.1 | 0.4 | yes |
//! | 10⁻³ | scattered | 33 535 | 512 | 3.5 | 59.5 | no |
//! | 10⁻³ | contiguous | 33 554 | 2 | 0.1 | 0.6 | yes |
//! | 10⁻² | scattered | 333 850 | 512 | 9.4 | 69.1 | no |
//! | 10⁻² | contiguous | 335 544 | 6 | 0.3 | 1.4 | yes |
//!
//! A scattered viewport touches every block, and reading a few values from every block costs more
//! cold than reading the whole column in order, because each fault reads ahead about a block. So
//! the rule counts blocks: it narrows a viewport whose items fall in few blocks, where the gain is
//! 30–100×, and leaves the scattered ones to the whole scan.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Parser;
use croaring::Bitmap;
use mosaica_filter::{
    write_value_column, Access, Codes, ColumnKind, ValueColumn, ValueColumnWriter, PRESENCE_FILE,
    VALUES_FILE,
};
use mosaica_types::AttrLocalId;

type BoxError = Box<dyn std::error::Error>;

#[derive(Parser)]
#[command(about = "Time keyword and category scans over a grid of value distributions")]
struct Args {
    /// Entities in the column; every one carries a value.
    #[arg(long, default_value_t = 1 << 25)]
    entities: u32,
    /// Runs per figure; the median is reported.
    #[arg(long, default_value_t = 5)]
    repeat: usize,
    /// Where the columns are written. A temporary directory where unset.
    #[arg(long)]
    dir: Option<PathBuf>,
    /// Measure only columns of codes this many bytes wide, 2 or 4.
    #[arg(long)]
    only_bytes: Option<u8>,
    /// Skip the cold scans.
    #[arg(long)]
    warm_only: bool,
}

fn mix(e: u64) -> u64 {
    let mut x = e.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

#[derive(Clone, Copy)]
enum Order {
    Uniform,
    Skewed,
    Sorted,
}

impl Order {
    fn name(self) -> &'static str {
        match self {
            Order::Uniform => "uniform",
            Order::Skewed => "skewed",
            Order::Sorted => "sorted",
        }
    }

    /// Entity `e`'s ordinal among `distinct`, of `n` entities.
    fn value(self, e: u32, distinct: u64, n: u64) -> u32 {
        match self {
            Order::Uniform => (mix(u64::from(e)) % distinct) as u32,
            Order::Skewed => {
                let u = (mix(u64::from(e)) >> 11) as f64 / (1u64 << 53) as f64;
                ((distinct as f64 * u.powi(4)) as u64).min(distinct - 1) as u32
            }
            Order::Sorted => (u64::from(e) * distinct / n) as u32,
        }
    }
}

fn evict(path: &Path) -> std::io::Result<()> {
    let file = File::open(path)?;
    // Written pages are dropped only once they are on disc.
    file.sync_all()?;
    // SAFETY: a valid descriptor and a whole-file range.
    unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
    Ok(())
}

/// Major page faults this process has taken.
fn major_faults() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    stat.rsplit_once(") ")
        .and_then(|(_, rest)| rest.split_whitespace().nth(9)?.parse().ok())
        .unwrap_or(0)
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1_000.0)
}

/// The median of `repeat` scans of `candidate` for `needle`, and the major faults one took. Cold,
/// each scan opens the column after evicting its file.
fn scan(
    dir: &Path,
    candidate: &Bitmap,
    needle: u32,
    repeat: usize,
    cold: bool,
    pool: &rayon::ThreadPool,
) -> Result<(Duration, u64, u64), BoxError> {
    let mut samples = Vec::with_capacity(repeat);
    let mut faults = 0;
    let mut matched = 0;
    if !cold {
        let warm = ValueColumn::open_dir(dir, Access::Mapped)?;
        pool.install(|| warm.scan_eq(candidate, AttrLocalId::new(needle)));
    }
    for _ in 0..repeat {
        let column = if cold {
            evict(&dir.join(VALUES_FILE))?;
            ValueColumn::open_dir(dir, Access::Mapped)?
        } else {
            ValueColumn::open_dir(dir, Access::Mapped)?
        };
        let before = major_faults();
        let started = Instant::now();
        let found = pool.install(|| column.scan_eq(candidate, AttrLocalId::new(needle)));
        samples.push(started.elapsed());
        faults = major_faults() - before;
        matched = found.cardinality();
    }
    Ok((median(samples), faults, matched))
}

fn main() -> Result<(), BoxError> {
    let args = Args::parse();
    let scratch = tempfile::tempdir()?;
    let root = args.dir.clone().unwrap_or_else(|| scratch.path().to_path_buf());
    let n = args.entities;
    let pool = rayon::ThreadPoolBuilder::new().build()?;
    let whole = Bitmap::from_range(0..n);
    println!(
        "{n} entities, {} threads, median of {}",
        pool.current_num_threads(),
        args.repeat
    );
    println!();
    println!(
        "| codes | distinct | order | flat MB | blocks MB | widths (c/u8/u16/plain) | warm flat ms \
         | warm blocks ms | cold flat ms | cold blocks ms | cold faults flat/blocks |"
    );
    println!("|---|---:|---|---:|---:|---|---:|---:|---:|---:|---:|");
    let mut narrow_rows = Vec::new();
    let grid = [16u64, 10_000, 10_000_000, u64::from(n)]
        .map(|d| (4u8, d))
        .into_iter()
        .chain([(2u8, 16u64), (2, 1_000), (2, 60_000)]);
    for (bytes, distinct) in grid.filter(|(b, _)| args.only_bytes.is_none_or(|o| o == *b)) {
        for order in [Order::Uniform, Order::Skewed, Order::Sorted] {
            let values: Vec<u32> = (0..n).map(|e| order.value(e, distinct, n as u64)).collect();
            let needle = values[n as usize / 2];
            let flat = root.join("flat");
            let blocks = root.join("blocks");
            std::fs::create_dir_all(&flat)?;
            std::fs::create_dir_all(&blocks)?;
            let (codes, kind) = match bytes {
                2 => (
                    Codes::U16(values.iter().map(|&v| v as u16).collect::<Vec<_>>().into()),
                    ColumnKind::Ordinals16,
                ),
                _ => (Codes::U32(values.into()), ColumnKind::Ordinals),
            };
            write_value_column(
                &flat.join(VALUES_FILE),
                &flat.join(PRESENCE_FILE),
                &codes,
                None,
            )?;
            let mut writer = ValueColumnWriter::create(
                &blocks.join(VALUES_FILE),
                &blocks.join(PRESENCE_FILE),
                kind,
            )?;
            writer.push(&codes)?;
            writer.finish(None)?;
            drop(codes);
            let size = |dir: &Path| -> Result<f64, BoxError> {
                Ok(std::fs::metadata(dir.join(VALUES_FILE))?.len() as f64 / 1e6)
            };
            let widths = match ValueColumn::open_dir(&blocks, Access::Read)?.codes() {
                Codes::Ordinals(o) => {
                    let (w, _) = o.census();
                    format!("{}/{}/{}/{}", w[1], w[2], w[3], w[4])
                }
                _ => "flat".to_string(),
            };
            let (warm_flat, _, m1) = scan(&flat, &whole, needle, args.repeat, false, &pool)?;
            let (warm_blocks, _, m2) = scan(&blocks, &whole, needle, args.repeat, false, &pool)?;
            assert_eq!(m1, m2, "both columns match the same entities");
            let cold = |dir: &Path| match args.warm_only {
                true => Ok((Duration::ZERO, 0, 0)),
                false => scan(dir, &whole, needle, args.repeat, true, &pool),
            };
            let (cold_flat, f_flat, _) = cold(&flat)?;
            let (cold_blocks, f_blocks, _) = cold(&blocks)?;
            println!(
                "| u{} | {distinct} | {} | {:.0} | {:.0} | {widths} | {} | {} | {} | {} | {f_flat}/{f_blocks} |",
                u32::from(bytes) * 8,
                order.name(),
                size(&flat)?,
                size(&blocks)?,
                ms(warm_flat),
                ms(warm_blocks),
                ms(cold_flat),
                ms(cold_blocks),
            );
            if bytes == 4 && distinct == 10_000 && matches!(order, Order::Uniform) {
                for share in [1e-4, 1e-3, 1e-2] {
                    let k = ((f64::from(n) * share) as u32).max(1);
                    let mut scattered = Bitmap::new();
                    for i in 0..k {
                        scattered.add((mix(u64::from(i) + 7) % u64::from(n)) as u32);
                    }
                    let start = n / 3;
                    let contiguous = Bitmap::from_range(start..start + k);
                    for (layout, candidate) in [("scattered", scattered), ("contiguous", contiguous)]
                    {
                        let (warm, _, _) =
                            scan(&blocks, &candidate, needle, args.repeat, false, &pool)?;
                        let (cold, faults, _) =
                            scan(&blocks, &candidate, needle, args.repeat, true, &pool)?;
                        let blocks = u64::from(candidate.statistics().n_containers);
                        let narrows = u64::from(k) * 16 <= u64::from(n)
                            && blocks * 4 <= u64::from(whole.statistics().n_containers);
                        narrow_rows.push(format!(
                            "| {share} | {layout} | {} | {blocks} | {} | {} | {faults} | {} | {} | {narrows} |",
                            candidate.cardinality(),
                            ms(warm),
                            ms(cold),
                            ms(warm_blocks),
                            ms(cold_blocks),
                        ));
                    }
                }
            }
            std::fs::remove_dir_all(&flat)?;
            std::fs::remove_dir_all(&blocks)?;
        }
    }
    println!();
    println!("Viewport candidates, 10 000 distinct values drawn uniformly, stored per block:");
    println!();
    println!(
        "| share | layout | entities | blocks | warm ms | cold ms | cold faults | whole warm ms \
         | whole cold ms | narrows |"
    );
    println!("|---:|---|---:|---:|---:|---:|---:|---:|---:|---|");
    for row in narrow_rows {
        println!("{row}");
    }
    Ok(())
}
