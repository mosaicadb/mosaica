//! **What a keyword filter costs across data shapes: ordinals stored flat against per block, and a
//! scan of the viewer's whole visible set against one of a viewport's items.**
//!
//! A keyword column's ordinals are written a block of 2¹⁶ entities at a time in the narrowest
//! encoding the block allows (`mosaica_filter::BlockOrdinals`), and a viewport's filter may be
//! evaluated over the items its tiles hold where that reads less (`narrowing_pays` in the engine).
//! Both pay off by how a corpus's values are distributed, so this measures a grid of shapes rather
//! than one corpus:
//!
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
//! cargo build --release -p mosaica-bench --bin keyword_shapes
//! target/release/keyword_shapes --entities 33554432 --dir /path/on/the/disk/under/test
//! ```
//!
//! # Measured 2026-11-08, a 4-core cloud container with 15 GB of memory, release
//!
//! 2²⁵ entities, every one carrying a value, median of 7. A cold scan of 138 MB took about
//! 50–65 ms here, so this disc reads about 2.5 GB/s; the ratios, not the times, are what carry
//! to another machine.
//!
//! | distinct | order | flat MB | blocks MB | warm flat ms | warm blocks ms | cold flat ms | cold blocks ms |
//! |---:|---|---:|---:|---:|---:|---:|---:|
//! | 16 | uniform | 138 | 35 | 12.2 | 13.2 | 63.3 | 21.8 |
//! | 16 | skewed | 138 | 35 | 11.9 | 12.0 | 53.5 | 21.8 |
//! | 16 | sorted | 138 | 0 | 10.4 | 0.4 | 63.9 | 0.5 |
//! | 10 000 | uniform | 138 | 90 | 10.1 | 12.0 | 59.9 | 42.4 |
//! | 10 000 | skewed | 138 | 89 | 16.6 | 15.7 | 65.8 | 41.4 |
//! | 10 000 | sorted | 138 | 35 | 8.2 | 8.4 | 52.4 | 20.6 |
//! | 10⁷ | uniform | 138 | 138 | 9.7 | 12.0 | 49.7 | 60.0 |
//! | 10⁷ | skewed | 138 | 138 | 10.6 | 12.0 | 55.7 | 63.8 |
//! | 10⁷ | sorted | 138 | 110 | 10.3 | 10.9 | 51.7 | 50.7 |
//! | 2²⁵ | uniform | 138 | 138 | 9.1 | 12.4 | 58.4 | 68.4 |
//! | 2²⁵ | skewed | 138 | 138 | 10.6 | 11.6 | 58.4 | 59.2 |
//! | 2²⁵ | sorted | 138 | 138 | 10.5 | 14.0 | 55.3 | 61.1 |
//!
//! Blocks shrink the column wherever a block holds fewer distinct values than entities, and a cold
//! scan then reads less: 2.5–3× faster at 16 values and 1.4–1.6× at 10 000 drawn independently.
//! Where nearly every value is distinct, every block is `Plain` and the file is flat storage's
//! size; the scan costs 10–35% more warm, for the per-block walk, and cold figures within this
//! disc's run-to-run spread of about 10–20%.
//!
//! A viewport's candidate, at 10 000 values drawn uniformly, stored per block, against a scan of
//! the whole set (12.0 ms warm, 42.4 ms cold). `narrows` is the engine's rule:
//!
//! | share | layout | entities | blocks | warm ms | cold ms | narrows |
//! |---:|---|---:|---:|---:|---:|---|
//! | 10⁻⁴ | scattered | 3 355 | 512 | 3.9 | 76.6 | no |
//! | 10⁻⁴ | contiguous | 3 355 | 1 | 0.1 | 0.3 | yes |
//! | 10⁻³ | scattered | 33 535 | 512 | 3.5 | 52.3 | no |
//! | 10⁻³ | contiguous | 33 554 | 2 | 0.1 | 0.6 | yes |
//! | 10⁻² | scattered | 333 850 | 512 | 8.6 | 55.1 | no |
//! | 10⁻² | contiguous | 335 544 | 6 | 0.4 | 1.3 | yes |
//!
//! A scattered viewport touches every block, and reading a few values from every block costs more
//! cold than reading the whole column in order, because each fault reads ahead about a block. So
//! the rule counts blocks: it narrows a viewport whose items fall in few blocks, where the gain is
//! 30–140×, and leaves the scattered ones to the whole scan, which gives up a 1.4–3× warm gain to
//! avoid a 1.2–1.8× cold loss.

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
#[command(about = "Time keyword scans over a grid of value distributions and candidates")]
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
        "| distinct | order | flat MB | blocks MB | widths (c/u8/u16/plain) | warm flat ms | warm \
         blocks ms | cold flat ms | cold blocks ms | cold faults flat/blocks |"
    );
    println!("|---:|---|---:|---:|---|---:|---:|---:|---:|---:|");
    let mut narrow_rows = Vec::new();
    for distinct in [16u64, 10_000, 10_000_000, u64::from(n)] {
        for order in [Order::Uniform, Order::Skewed, Order::Sorted] {
            let values: Vec<u32> = (0..n).map(|e| order.value(e, distinct, n as u64)).collect();
            let needle = values[n as usize / 2];
            let flat = root.join("flat");
            let blocks = root.join("blocks");
            std::fs::create_dir_all(&flat)?;
            std::fs::create_dir_all(&blocks)?;
            let codes = Codes::U32(values.into());
            write_value_column(
                &flat.join(VALUES_FILE),
                &flat.join(PRESENCE_FILE),
                &codes,
                None,
            )?;
            let mut writer = ValueColumnWriter::create(
                &blocks.join(VALUES_FILE),
                &blocks.join(PRESENCE_FILE),
                ColumnKind::Ordinals,
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
            let (cold_flat, f_flat, _) = scan(&flat, &whole, needle, args.repeat, true, &pool)?;
            let (cold_blocks, f_blocks, _) =
                scan(&blocks, &whole, needle, args.repeat, true, &pool)?;
            println!(
                "| {distinct} | {} | {:.0} | {:.0} | {widths} | {} | {} | {} | {} | {f_flat}/{f_blocks} |",
                order.name(),
                size(&flat)?,
                size(&blocks)?,
                ms(warm_flat),
                ms(warm_blocks),
                ms(cold_flat),
                ms(cold_blocks),
            );
            if distinct == 10_000 && matches!(order, Order::Uniform) {
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
