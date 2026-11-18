//! The approximate queries: adaptive k-NN and threshold queries over the clusters, with one raw knob psi >= 0.
//!
//! Each cluster holding a visible row has the key u_c = mu_c + psi sigma_c(q): mu_c its centre score (the int8
//! centres), sigma_c(q) = sqrt(q' S_c q) the spread of its rows' scores along the query, S_c the cluster's covariance
//! as its stored principal directions (`order::OrderState`) and an isotropic rest. u_c is an optimistic estimate of the
//! cluster's best score, on the score scale, and psi says how optimistic (theory: the best multiplier is 3 to 4 at
//! every unit size and dataset, `hierarchy/theory.md` 3.4). Every sigma is at most sqrt(top_c) |q|, top_c the
//! cluster's largest variance along a direction, so b_c = mu_c + psi sqrt(top_c) |q| bounds u_c from above.
//!
//! - k-NN reads the clusters in decreasing u (the lower cluster first among equal keys), each whole by `KnnReader`
//!   (its visible rows scored from their stored vectors, or the index path at the running k-th best), and stops before
//!   the first cluster whose u is below the running k-th best (k items are held by then: until k are, the k-th best is
//!   minus infinity); it returns k items whenever k are visible. The sigmas are computed lazily under the bound: a
//!   cluster's sigma is computed only when its bound beats the largest key computed so far, so the clusters come in
//!   exactly the order, and the probe stops at exactly the cluster, that computing every sigma first would give. With
//!   psi infinite every cluster is read (in decreasing centre score; no sigma is computed) and the answer is exact.
//!   The probe races a scan of the visible rows: when it would stop it stops; otherwise, when its modelled cost with the
//!   next sigma or the next cluster's read would pass that of the scan (C_s), the rows not yet read are scanned instead.
//!   The answer is then exact and the whole costs at most 2 C_s (the probe spends at most C_s, the rest of the scan at
//!   most C_s, and no row is read twice).
//! - a threshold query at t reads the clusters whose u reaches t (a sigma computed only for a cluster whose bound
//!   reaches t), after the support table has dropped those wholly below t and taken in those wholly above it without a
//!   read; each is read exactly as the exact query reads it. That set does not depend on what is read, so the race is
//!   settled before reading: when a scan of the visible rows, or the exact query (every cluster the support table
//!   leaves open), costs no more than the sigmas and the reads, the query hands over to the cheaper, which answers
//!   exactly (with psi infinite it always does, unless the support table decided every cluster).
//! - each cluster is read directly or on the index path, whichever the measured cost model says is cheaper
//!   (`Measured`), with a group's setup and the anchor search charged to the first cluster that needs them. Both paths
//!   give the same answers: the constants change the answer only through the race and the hand-overs, which answer
//!   exactly. They are timed on this machine on first use and cached with the index (`measured_constants`).

use super::q::*;
use super::*;
use crate::kernel::Stats;
use roaring::RoaringBitmap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

/// psi by default, from the grid the paired runs swept (2 to 7): the recall it gives on each dataset and visible set
/// is to be measured on the re-timed runs.
pub const PSI: f64 = 4.5;

/// The cost model of the approximate queries, in microseconds on the calling thread, timed on this machine (`measure`):
/// - `i8_us`, a product of the query with an int8 row of n (centre scores, spreads' directions);
/// - `row_us`, `dir_us`: a cluster read directly costs dir + row v for its v visible rows; a scan of v rows, row v;
/// - `cl_us`, `size_us`, `vis_us`: a cluster of S rows on the index path costs cl + size S + vis v, at the running
///   k-th best of a query (its sorted range, levels and the rows left to their stored vectors included);
/// - `grp_us`, a group's setup (a + b n^2 at this index's n), charged to the first cluster of a group on the index path;
/// - `anc_us`, the search for the nearest anchor, per anchor searched, charged to the first cluster on the index path.
///
/// `i8_us`, `row_us` and `dir_us` are above 0, the others at least 0, all finite (`check`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub i8_us: f64,
    pub row_us: f64,
    pub dir_us: f64,
    pub cl_us: f64,
    pub size_us: f64,
    pub vis_us: f64,
    pub grp_us: f64,
    pub anc_us: f64,
}

const NAMES: [&str; 8] = ["i8_us", "row_us", "dir_us", "cl_us", "size_us", "vis_us", "grp_us", "anc_us"];

impl Measured {
    fn values(&self) -> [f64; 8] {
        [self.i8_us, self.row_us, self.dir_us, self.cl_us, self.size_us, self.vis_us, self.grp_us, self.anc_us]
    }

    fn from_values(v: [f64; 8]) -> Measured {
        Measured { i8_us: v[0], row_us: v[1], dir_us: v[2], cl_us: v[3], size_us: v[4], vis_us: v[5], grp_us: v[6], anc_us: v[7] }
    }

    /// Whether the constants can price a query: every one finite and at least 0, and a product, a row and a direct
    /// read above 0 (a model in which reading costs nothing would read every cluster and never race).
    pub fn check(&self) -> Result<(), String> {
        for (k, v) in NAMES.iter().zip(self.values()) {
            let pos = matches!(*k, "i8_us" | "row_us" | "dir_us");
            if !v.is_finite() || v < 0.0 || (pos && v <= 0.0) {
                return Err(format!("{k}={v} is not a number {}", if pos { "above 0" } else { "of at least 0" }));
            }
        }
        Ok(())
    }

    /// The constants as `name=value` lines.
    pub fn to_kv(&self) -> String {
        NAMES.iter().zip(self.values()).map(|(k, v)| format!("{k}={v}\n")).collect()
    }

    /// The constants of `name=value` lines (as `to_kv` writes them), checked (`check`).
    pub fn from_kv(text: &str) -> Result<Measured, String> {
        let mut v = [f64::NAN; 8];
        for (i, k) in NAMES.iter().enumerate() {
            let x = text.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('='))).ok_or_else(|| format!("no {k}"))?;
            v[i] = x.trim().parse::<f64>().map_err(|_| format!("{k}={x} is not a number"))?;
        }
        let m = Measured::from_values(v);
        m.check()?;
        Ok(m)
    }

    /// A cluster of `nv` visible rows read directly.
    pub fn direct(&self, nv: usize) -> f64 {
        self.dir_us + self.row_us * nv as f64
    }

    /// A cluster of `rows` rows, `nv` of them visible, on the index path (its group set up, the anchor found).
    pub fn index(&self, rows: usize, nv: usize) -> f64 {
        self.cl_us + self.size_us * rows as f64 + self.vis_us * nv as f64
    }

    /// A scan of `nv` visible rows.
    pub fn scan(&self, nv: usize) -> f64 {
        self.row_us * nv as f64
    }

    /// The exact planner's rule in rows (`Plan::r_rows` R0, `r_kappa` kappa) that these constants give for an index
    /// whose clusters hold about `typical` rows: a cluster of S rows holding v visible ones is cheaper read directly
    /// when dir + row v <= cl + size S + vis v, that is when v <= v*(S) = (cl - dir + size S) / (row - vis). The
    /// timing tells cl from size S only through clusters of different sizes, so where the sizes are nearly equal
    /// their split is poorly determined while v*(typical) is not: the rule keeps v*(typical) exactly and takes kappa
    /// = size / (row - vis) only up to v*(typical) / typical (R0 = v*(typical) - kappa typical, at least 0). None
    /// when the index path's rows cost no less than direct ones (every cluster would be scored directly).
    pub fn rule(&self, typical: usize) -> Option<(usize, f64)> {
        let d = self.row_us - self.vis_us;
        if d <= 0.0 {
            return None;
        }
        let s = typical.max(1) as f64;
        let v0 = ((self.cl_us - self.dir_us + self.size_us * s) / d).max(0.0);
        let kappa = (self.size_us / d).min(v0 / s).max(0.0);
        Some(((v0 - kappa * s).max(0.0).round() as usize, kappa))
    }
}

/// The median rows of the clusters holding any (the size `Measured::rule` predicts at).
pub fn typical_rows(ix: &Index2) -> usize {
    let mut s: Vec<usize> = ix.cl.iter().map(|c| c.r1 - c.r0).filter(|&s| s > 0).collect();
    s.sort_unstable();
    s.get(s.len() / 2).copied().unwrap_or(1)
}

/// The timing procedure (`measure`), structural choices rather than tuned values:
/// - `T_QUERIES` pseudo-queries (corpus rows spread over the index), and for each, under every visible share of
///   `T_SHARES` (one row in s, by a hash of the row), the `T_CLUSTERS` clusters with the largest centre scores, each
///   read directly and on the index path at two running k-th bests (the 10th and the 100th best of those clusters'
///   visible rows: a probe's at its end and before it);
/// - `T_GROUPS` group setups and one anchor search per pseudo-query, each on a fresh query;
/// - every timing the fastest of `T_REPS`, each after reading twice the core's L2 (`evict_bytes`), and at least
///   `T_FLOOR_US`.
///
/// What sets the fitted constants' accuracy: the samples (`T_QUERIES` x 4 shares x 2 thresholds x `T_CLUSTERS` per
/// path) and their spread in cluster size and visible rows (cl and size S are told apart only by clusters of
/// different sizes; `Measured::rule` relies on their sum at the typical size, which is well determined); the two
/// thresholds, which bracket how much of a cluster the index path's range and levels drop; and the eviction (with the
/// caches warm a direct read looks cheaper than a query's first read of the cluster). The repetitions and the floor
/// only keep outliers and the timer's resolution out.
const T_QUERIES: usize = 8;
const T_CLUSTERS: usize = 16;
const T_SHARES: [u64; 4] = [1, 4, 16, 64];
const T_GROUPS: usize = 4;
const T_REPS: usize = 3;
const T_FLOOR_US: f64 = 0.01;

/// Weights a and b >= 0 with sum_j a_j x_ij close to t_i in relative terms: least squares on (a.x_i - t_i) / t_i,
/// dropping the most negative weight (set to 0) until none is, or a weight the samples do not determine.
fn fit_rel(x: &[Vec<f64>], t: &[f64]) -> Vec<f64> {
    let m = x.first().map_or(0, |r| r.len());
    let mut on: Vec<bool> = vec![true; m];
    loop {
        let idx: Vec<usize> = (0..m).filter(|&j| on[j]).collect();
        let p = idx.len();
        if p == 0 {
            return vec![0.0; m];
        }
        // normal equations of the weighted problem, solved by elimination with partial pivoting
        let mut a = vec![vec![0f64; p + 1]; p];
        for (xi, &ti) in x.iter().zip(t) {
            let w = 1.0 / (ti * ti).max(1e-30);
            for (r, &jr) in idx.iter().enumerate() {
                for (c, &jc) in idx.iter().enumerate() {
                    a[r][c] += w * xi[jr] * xi[jc];
                }
                a[r][p] += w * xi[jr] * ti;
            }
        }
        let scale: Vec<f64> = (0..p).map(|r| a[r][r].abs().max(1e-300)).collect();
        let mut singular = None;
        for c in 0..p {
            let piv = (c..p).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
            a.swap(c, piv);
            if a[c][c].abs() <= 1e-10 * scale[c] {
                singular = Some(idx[c]);
                break;
            }
            let pivot = a[c].clone();
            for (r, row) in a.iter_mut().enumerate() {
                if r != c {
                    let f = row[c] / pivot[c];
                    for (x, y) in row[c..].iter_mut().zip(&pivot[c..]) {
                        *x -= f * y;
                    }
                }
            }
        }
        if let Some(j) = singular {
            on[j] = false;
            continue;
        }
        let w: Vec<f64> = (0..p).map(|r| a[r][p] / a[r][r]).collect();
        match (0..p).filter(|&r| w[r] < 0.0).min_by(|&i, &j| w[i].total_cmp(&w[j])) {
            Some(r) => on[idx[r]] = false,
            None => {
                let mut out = vec![0f64; m];
                for (r, &j) in idx.iter().enumerate() {
                    out[j] = w[r];
                }
                return out;
            }
        }
    }
}

/// The fastest of `T_REPS` timings of `f` after `setup` (untimed), in microseconds, at least `T_FLOOR_US`.
fn fastest<S>(mut setup: impl FnMut() -> S, mut f: impl FnMut(&mut S)) -> f64 {
    (0..T_REPS).map(|_| {
        let mut s = setup();
        let t = Instant::now();
        f(&mut s);
        t.elapsed().as_secs_f64() * 1e6
    }).fold(f64::INFINITY, f64::min).max(T_FLOOR_US)
}

/// Twice the largest per-core cache below the shared one (L2, from /sys; 2 MiB where it does not say): the bytes read
/// before each timing of a cluster's read, so that its rows and codes are no longer in the core's own caches, as for
/// a query that reaches the cluster for the first time.
fn evict_bytes() -> usize {
    let size = |i: usize| -> Option<(u32, usize)> {
        let p = format!("/sys/devices/system/cpu/cpu0/cache/index{i}");
        let level = std::fs::read_to_string(format!("{p}/level")).ok()?.trim().parse().ok()?;
        let s = std::fs::read_to_string(format!("{p}/size")).ok()?;
        let s = s.trim();
        let (num, mul) = match s.strip_suffix('K') {
            Some(x) => (x, 1 << 10),
            None => match s.strip_suffix('M') {
                Some(x) => (x, 1 << 20),
                None => (s, 1),
            },
        };
        Some((level, num.parse::<usize>().ok()? * mul))
    };
    2 * (0..8).filter_map(size).filter(|&(l, _)| l == 2).map(|x| x.1).max().unwrap_or(2 << 20)
}

/// One timing of the cost model's constants on `ix` (`T_QUERIES` and the rest above), fitted as `Measured` says
/// (relative least squares, weights at least 0); a direct read's per-cluster cost that the fit sets to 0 is floored at
/// the least any sample leaves for it beyond its rows (at least `T_FLOOR_US`). None when no pseudo-query holds enough
/// visible rows to time.
fn measure_once(ix: &Index2) -> Option<Measured> {
    let (n, k, nl) = (ix.n(), ix.k(), ix.meta.nl());
    let queries: Vec<Vec<f32>> = spread(ix.nrows, T_QUERIES).into_iter().filter_map(|row| {
        let x: Vec<f64> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h) as f64).collect();
        let xn = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        (xn >= 0.5).then(|| x.iter().map(|v| (v / xn) as f32).collect())
    }).collect();
    let q0 = queries.first()?;
    let evict: Vec<u64> = (0..evict_bytes() as u64 / 8).collect();
    // products with int8 rows
    let rows = k.max(256);
    let a8: Vec<i8> = (0..rows * n).map(|i| ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 57) as i8 - 64).collect();
    let mut out = vec![0f32; rows];
    let i8_us = fastest(|| (), |_| unsafe { crate::kernel::gemv_i8(q0, &a8, n, &mut out) }) / rows as f64;
    std::hint::black_box(&out);
    // the visible sets: every row, then one in s by a hash of the row
    let sets: Vec<Option<Vis2>> = T_SHARES.iter().map(|&s| {
        (s > 1).then(|| {
            let bm: RoaringBitmap = (0..ix.nrows).filter(|&r| ((r as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32).is_multiple_of(s)).map(|r| ix.row_ent[r]).collect();
            Vis2::new(ix, &bm)
        })
    }).collect();
    let plan = Plan::default();
    let (mut xd, mut td, mut xi, mut ti) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut grp, mut anc) = (Vec::new(), Vec::new());
    for q in &queries {
        for vis in &sets {
            let vis = vis.as_ref();
            let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
            let pr = prep2(ix, q);
            let mut live: Vec<(f64, usize)> = (0..k).filter(|&ci| nv_of(ci) > 0).map(|ci| (ix.qm(&pr.q64, ci), ci)).collect();
            live.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            let top: Vec<usize> = live.iter().take(T_CLUSTERS).map(|x| x.1).collect();
            let mut rd = KnnReader::new(ix, &pr, vis, 100, plan, Stats::new(nl));
            for &ci in &top {
                rd.visit(ci, KnnPath::Direct);
            }
            let best = rd.finish().0;
            let ts: Vec<f64> = [9, 99].iter().filter_map(|&i| best.get(i).map(|x| x.0)).collect();
            if ts.is_empty() {
                continue;
            }
            pr.qms(ix, Some(&top));
            if ix.nukey.is_empty() {
                pr.anc_among(ix, Some(&top));
            }
            let mut gs: Vec<usize> = top.iter().map(|&ci| ix.cl[ci].g).collect();
            gs.sort_unstable();
            gs.dedup();
            setup_groups(ix, &pr, &gs, &mut Stats::new(nl));
            for &t in &ts {
                for &ci in &top {
                    let (s, nv) = (ix.cl[ci].r1 - ix.cl[ci].r0, nv_of(ci));
                    let reader = || {
                        std::hint::black_box(evict.iter().fold(0u64, |a, &x| a.wrapping_add(x)));
                        let mut r = KnnReader::new(ix, &pr, vis, 10, plan, Stats::new(nl));
                        r.prime(t);
                        r
                    };
                    td.push(fastest(reader, |r| {
                        r.visit(ci, KnnPath::Direct);
                        r.kth();
                    }));
                    xd.push(vec![1.0, nv as f64]);
                    ti.push(fastest(reader, |r| {
                        r.visit(ci, KnnPath::Index);
                        r.kth();
                    }));
                    xi.push(vec![1.0, s as f64, nv as f64]);
                }
            }
            if vis.is_none() {
                for &g in gs.iter().take(T_GROUPS) {
                    grp.push(fastest(|| prep2(ix, q), |p| setup_groups(ix, p, &[g], &mut Stats::new(nl))));
                }
                if ix.nukey.is_empty() && ix.nanch > 0 {
                    anc.push(fastest(|| prep2(ix, q), |p| {
                        p.anc(ix);
                    }) / ix.nanch as f64);
                }
            }
        }
    }
    if td.is_empty() {
        return None;
    }
    let (d, i) = (fit_rel(&xd, &td), fit_rel(&xi, &ti));
    let dir_us = if d[0] > 0.0 { d[0] } else { xd.iter().zip(&td).map(|(x, &t)| t - d[1] * x[1]).fold(f64::INFINITY, f64::min).max(T_FLOOR_US) };
    let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    Some(Measured { i8_us, row_us: d[1], dir_us, cl_us: i[0], size_us: i[1], vis_us: i[2], grp_us: mean(&grp), anc_us: mean(&anc) })
}

/// The cost model's constants timed on `ix`, on the calling thread's pool (one thread in the bindings): `measure_once`,
/// timed again once when its constants fail `Measured::check`. A few seconds on a million rows. An error says why
/// they could not be timed (no stored vectors, too few rows) or what the timing gave.
pub fn measure(ix: &Index2) -> Result<Measured, String> {
    if !matches!(ix.full, Full2::Real { .. }) || ix.k() == 0 || ix.nrows < 2 {
        return Err("the cost model is timed on stored vectors, and this index has none (or fewer than 2 rows)".into());
    }
    let mut why = String::new();
    for _ in 0..2 {
        match measure_once(ix) {
            None => return Err("no pseudo-query found 10 visible rows to time the reads at; the index is too small".into()),
            Some(m) => match m.check() {
                Ok(()) => return Ok(m),
                Err(e) => why = format!("the timing gave {e} ({})", m.to_kv().trim().replace('\n', " ")),
            },
        }
    }
    Err(format!("{why}, twice; the machine may be too busy to time on: time it again when it is not"))
}

/// The cache of the measured constants in an index directory.
pub const COMPUTE_CACHE: &str = "compute-constants.txt";
/// The version of its key: changed when the timing or the key changes, so older caches are timed again.
pub const COMPUTE_KEY_VERSION: u32 = 2;

/// The key the measured constants are cached under: the key's version; the machine and CPU (`machine_and_cpu`) and the
/// instruction set the library was built for; the library (`store::LIB_FINGERPRINT`); the index (its shape and its
/// plan, `Index2::plan_hash`, which follows the build options that set the encoding, and whether it is norm-sorted);
/// and where its vectors are read from (in memory, or a file and its read policy). One line.
pub fn compute_key(ix: &Index2) -> String {
    let (machine, cpu) = machine_and_cpu();
    let isa = if cfg!(target_feature = "avx512f") { "avx512" } else if cfg!(target_feature = "avx2") { "avx2" } else { "base" };
    let rows = match ix.own_rows() {
        Some(r) => match r.file_path() {
            Some(p) => format!("file {} uring {} priced {}", p.display(), r.uring(), r.priced()),
            None => "memory".into(),
        },
        None => "parent's".into(),
    };
    format!("v{COMPUTE_KEY_VERSION}; machine {machine}; cpu {cpu}; isa {isa}; lib {}; n {}; rows {}; clusters {}; groups {}; plan {:016x}; nusort {}; vectors {rows}",
            super::store::LIB_FINGERPRINT, ix.n(), ix.nrows, ix.k(), ix.groups.len(), ix.plan_hash(), !ix.nukey.is_empty())
}

fn read_cache(path: &std::path::Path, key: &str) -> Result<Measured, String> {
    let s = std::fs::read_to_string(path).map_err(|_| format!("no cache at {}", path.display()))?;
    let k = s.lines().find_map(|l| l.strip_prefix("key=")).ok_or_else(|| format!("{} is not a cache of measured constants", path.display()))?;
    if k != key {
        return Err(format!("{} was for another machine, build or index ({k})", path.display()));
    }
    Measured::from_kv(&s).map_err(|e| format!("{}: {e}", path.display()))
}

/// The constants cached at `path` under `key`, else `time()`'s, saved there under `key` (written whole, under an
/// exclusive lock on `path` with the extension `lock`, so that two opens do not time at once: one that waited reads
/// what the other saved); and how they were found. Constants that fail `Measured::check` are never saved: an error
/// says why.
pub fn cached_measured(path: &std::path::Path, key: &str, time: impl FnOnce() -> Result<Measured, String>) -> Result<(Measured, String), String> {
    let why = match read_cache(path, key) {
        Ok(m) => return Ok((m, format!("read from {}", path.display()))),
        Err(e) => e,
    };
    let lock = lock_file(&path.with_extension("lock"));
    if lock.is_some() {
        if let Ok(m) = read_cache(path, key) {
            return Ok((m, format!("read from {}, saved by another open while this one waited", path.display())));
        }
    }
    let why = if why.starts_with("no cache") { format!("timed: {why}") } else { format!("timed again: {why}") };
    let m = time().and_then(|m| m.check().map(|_| m)).map_err(|e| format!("{why}; but they could not be timed: {e}"))?;
    let why = match write_atomic(path, &format!("key={key}\n{}", m.to_kv())) {
        Ok(()) => format!("{why}; saved in {}", path.display()),
        Err(e) => format!("{why}; not saved in {} ({e})", path.display()),
    };
    drop(lock);
    Ok((m, why))
}

/// The measured constants of this machine for `ix`: from `COMPUTE_CACHE` in the index directory `dir`, else from the
/// per-user cache (`user_cache_dir`, a file named by the key) when `dir` cannot be written, else timed (`measure`)
/// and saved there. Logged. An error says why they could be neither read nor timed.
pub fn measured_constants(ix: &Index2, dir: &str, log: &mut dyn FnMut(String)) -> Result<Measured, String> {
    let key = compute_key(ix);
    let main = std::path::Path::new(dir).join(COMPUTE_CACHE);
    let path = match user_cache_dir() {
        Some(u) if read_cache(&main, &key).is_err() && !dir_writable(std::path::Path::new(dir)) => {
            let _ = std::fs::create_dir_all(&u);
            u.join(format!("compute-constants-{:016x}.txt", fnv1a64(key.as_bytes())))
        }
        _ => main,
    };
    let t = Timer::new();
    match cached_measured(&path, &key, || measure(ix)) {
        Ok((m, why)) => {
            log(format!("[approx] measured constants {}: {why}, in {:.1}s; the exact planner's rule from them: {}", m.to_kv().trim().replace('\n', " "), t.s(),
                        m.rule(typical_rows(ix)).map_or("every cluster scored directly".into(), |(r, kp)| format!("r_rows={r} r_kappa={kp:.3}"))));
            Ok(m)
        }
        Err(e) => {
            log(format!("[approx] no measured constants: {e}"));
            Err(e)
        }
    }
}

/// What one approximate query did. `path`: "probe" (k-NN stopped by its knob; a threshold query's clusters read),
/// "race" (k-NN: the rest scanned once the probe would cost more than a scan), "scan" (the visible rows scanned
/// instead), "exact" (a threshold query handed to the exact one), "empty". `read`: the clusters the probe read, in
/// order (not those a race scanned). Costs are modelled (`Measured`): `fixed` the order's (centre scores; the anchor
/// search of a threshold query), `cost` what the query paid in all, `scan_cost` a scan of the visible rows,
/// `exact_cost` (threshold) every cluster the support table leaves open, at its cheaper path.
#[derive(Clone, Debug, Default)]
pub struct AdaptiveInfo {
    pub path: &'static str,
    pub read: Vec<u32>,
    pub direct: usize,
    pub index: usize,
    pub groups: usize,
    pub spreads: usize,
    pub fixed: f64,
    pub cost: f64,
    pub scan_cost: f64,
    pub exact_cost: f64,
}

/// A cluster's key in a max-heap, the lower cluster first among equal keys.
#[derive(PartialEq)]
struct Key(f64, Reverse<usize>);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0).then(self.1.cmp(&o.1))
    }
}

/// The approximate queries' state for one index.
pub struct Adaptive {
    pub m: Measured,
    /// the int8 centres
    centres: Approx,
    st: std::sync::Arc<super::order::OrderState>,
    /// per cluster, the variance its directions leave per other spanned dimension, and the largest variance along any
    /// direction (sigma <= sqrt(top |q|^2))
    rest: Vec<f64>,
    top: Vec<f64>,
}

/// Per cluster, the costs a query charges as it reads: a group's setup once, the anchor search once.
struct Charges<'a> {
    m: &'a Measured,
    ix: &'a Index2,
    pr: &'a Prep2,
    anchored: bool,
    gset: Vec<bool>,
}

impl Charges<'_> {
    /// The cheaper path for cluster ci, `nv` of its rows visible, given what is already set up (`anchors` the anchors a
    /// first search would score), its cost, and whether it sets up a group; marks what the index path sets up.
    fn choose(&mut self, ci: usize, nv: usize, anchors: usize) -> (KnnPath, f64, bool) {
        let (m, ix) = (self.m, self.ix);
        let rows = ix.cl[ci].r1 - ix.cl[ci].r0;
        let g = ix.cl[ci].g;
        let gnew = !self.gset[g] && self.pr.gq[g].get().is_none();
        let idx = m.index(rows, nv) + if gnew { m.grp_us } else { 0.0 } + if self.anchored { 0.0 } else { m.anc_us * anchors as f64 };
        let dir = m.direct(nv);
        if idx < dir {
            self.gset[g] = true;
            (KnnPath::Index, idx, gnew)
        } else {
            (KnnPath::Direct, dir, false)
        }
    }
}

impl Adaptive {
    /// The state for `ix` with the constants `m` (checked, `Measured::check`): the int8 centres, and each cluster's
    /// spread's rest and bound from its stored directions.
    pub fn new(ix: &Index2, m: Measured) -> Result<Adaptive, String> {
        if !matches!(ix.full, Full2::Real { .. }) {
            return Err("the approximate queries read stored vectors, and this index has none; query it exactly (knn2, threshold2)".into());
        }
        m.check().map_err(|e| format!("the cost model cannot price a query: {e}"))?;
        let st = ix.order.clone().ok_or("this index holds no order state (the clusters' directions the approximate queries read); rebuild it")?;
        let r = st.r;
        let rest: Vec<f64> = (0..ix.k()).map(|c| {
            let lam = &st.lam[c * r..(c + 1) * r];
            let top: f64 = lam.iter().map(|&l| l as f64).sum();
            (st.rad[c] * st.rad[c] - top).max(0.0) / (st.n0.max(r + 1) - r) as f64
        }).collect();
        let top = (0..ix.k()).map(|c| st.lam.get(c * r).map_or(0.0, |&l| l as f64).max(rest[c])).collect();
        Ok(Adaptive { m, centres: Approx::centres(ix), st, rest, top })
    }

    /// sigma_c(q) of cluster ci from its stored directions (`p` scratch of r values), the query's f32 values and |q|^2.
    fn sigma(&self, ix: &Index2, ci: usize, q32: &[f32], qq: f64, p: &mut [f32]) -> f64 {
        let (st, r, n) = (&self.st, self.st.r, ix.n());
        unsafe { crate::kernel::gemv_i8(q32, st.dirs(ci, r, n), n, &mut p[..r]) };
        let (mut v, mut pp) = (0f64, 0f64);
        for ((&pj, &sc), &l) in p[..r].iter().zip(&st.dsc[ci * r..(ci + 1) * r]).zip(&st.lam[ci * r..(ci + 1) * r]) {
            let x = pj as f64 * sc as f64;
            v += l as f64 * x * x;
            pp += x * x;
        }
        (v + self.rest[ci] * (qq - pp).max(0.0)).max(0.0).sqrt()
    }

    /// The upper bound of cluster ci's key (sigma <= sqrt(top |q|^2)).
    fn bound(&self, ci: usize, qm: &[f64], psi: f64, qq: f64) -> f64 {
        qm[ci] + psi * (self.top[ci] * qq).sqrt()
    }

    /// The cost of one sigma (its directions' products).
    fn sigma_cost(&self) -> f64 {
        self.m.i8_us * self.st.r as f64
    }

    /// The anchors a first anchor search scores when it may take the nearest of `among` clusters' anchors.
    fn anchors(ix: &Index2, among: usize) -> usize {
        if !ix.nukey.is_empty() {
            0
        } else if ix.anc_centres && ix.nanch == ix.k() {
            among
        } else {
            ix.nanch
        }
    }

    /// Approximate k-NN with knob `psi` (at least 0; `PSI` by default; infinity reads every cluster and answers
    /// exactly), as the module's comment says. Returns k items whenever k are visible. `plan` is the index path's
    /// (its dial is the certified one).
    pub fn knn(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (KnnOut2, AdaptiveInfo) {
        self.knn_priced(ix, q, kk, vis, psi, plan, false)
    }

    /// `knn` with every sigma computed before the probe (`eager`) instead of lazily under the bound: the clusters it
    /// reads and where it stops are the same, which the tests check; it spends more on sigmas.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn knn_priced(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan, eager: bool) -> (KnnOut2, AdaptiveInfo) {
        assert!(kk > 0, "approximate k-NN needs k of at least 1");
        assert!(psi >= 0.0, "psi is at least 0 (infinity reads every cluster), not {psi}");
        let tm = Timer::new();
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let all = psi == f64::INFINITY;
        let fixed = m.i8_us * k as f64;
        let mut info = AdaptiveInfo { fixed, scan_cost: m.scan(nvis), ..AdaptiveInfo::default() };
        let out = |ents: Vec<(f64, u32)>, mut st: Stats| {
            st.cyc[P_QUERY] += tick() - t0;
            KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
        };
        if nvis == 0 {
            info.path = "empty";
            return (out(Vec::new(), st), info);
        }
        let cs = info.scan_cost;
        if nvis <= kk || fixed >= cs {
            info.path = "scan";
            info.cost = cs;
            st.full_reads += nvis as u64;
            st.touched += nvis as u64;
            return (out(scan_knn(ix, &pr.q16, &pr.q32, pr.qn, kk, vis), st), info);
        }
        let tc = tick();
        let qm = self.centres.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - tc;
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        // clusters not yet priced, by their key's bound (psi infinite: by centre score, never priced), and priced ones
        // by their key
        let mut bound: BinaryHeap<Key> = live.iter().map(|&ci| Key(if all { qm[ci] } else { self.bound(ci, &qm, psi, qq) }, Reverse(ci))).collect();
        let mut priced: BinaryHeap<Key> = BinaryHeap::new();
        st.cyc[P_ORDER] += tick() - ts;
        let sc = self.sigma_cost();
        let mut p = vec![0f32; self.st.r];
        let mut spent = fixed;
        let mut sig = 0;
        let mut price = |ci: usize, spent: &mut f64, priced: &mut BinaryHeap<Key>, info: &mut AdaptiveInfo| {
            let tsp = tick();
            let s = self.sigma(ix, ci, &pr.q32, qq, &mut p);
            sig += tick() - tsp;
            *spent += sc;
            info.spreads += 1;
            priced.push(Key(qm[ci] + psi * s, Reverse(ci)));
        };
        if eager && !all {
            while let Some(Key(_, Reverse(ci))) = bound.pop() {
                price(ci, &mut spent, &mut priced, &mut info);
            }
        }
        let mut ch = Charges { m, ix, pr: &pr, anchored: !ix.nukey.is_empty(), gset: vec![false; ix.groups.len()] };
        let mut visited = vec![false; k];
        let mut raced = false;
        let mut rd = KnnReader::new(ix, &pr, vis, kk, plan, st);
        loop {
            let ts = tick();
            let next = if all {
                bound.pop()
            } else {
                // the stop first: the running k-th best (the batched direct rows scored first; their time is the
                // reads') above every key priced and every bound left
                let kth = rd.kth();
                let best = |h: &BinaryHeap<Key>| h.peek().map_or(f64::NEG_INFINITY, |x| x.0);
                if kth > best(&priced).max(best(&bound)) {
                    rd.st.cyc[P_ORDER] += tick() - ts;
                    break;
                }
                // price the clusters whose bound beats the best key priced
                while bound.peek().is_some_and(|b| priced.peek().is_none_or(|x| b > x)) {
                    if spent + sc > cs {
                        raced = true;
                        break;
                    }
                    let Key(_, Reverse(ci)) = bound.pop().unwrap();
                    price(ci, &mut spent, &mut priced, &mut info);
                }
                if raced {
                    rd.st.cyc[P_ORDER] += tick() - ts;
                    break;
                }
                match priced.pop() {
                    Some(x) if kth > x.0 => {
                        rd.st.cyc[P_ORDER] += tick() - ts;
                        break;
                    }
                    x => x,
                }
            };
            rd.st.cyc[P_ORDER] += tick() - ts;
            let Some(Key(_, Reverse(ci))) = next else { break };
            let tch = tick();
            let nv = nv_of(ci);
            if spent + m.direct(nv) > cs {
                raced = true;
                rd.st.cyc[P_CHOICE] += tick() - tch;
                break;
            }
            info.read.push(ci as u32);
            let (path, cost, gnew) = ch.choose(ci, nv, Self::anchors(ix, info.read.len()));
            if path == KnnPath::Index {
                if !ch.anchored {
                    let among: Vec<usize> = info.read.iter().map(|&c| c as usize).collect();
                    pr.anc_among(ix, Some(&among));
                    ch.anchored = true;
                }
                info.index += 1;
                info.groups += gnew as usize;
            } else {
                info.direct += 1;
            }
            spent += cost;
            rd.st.cyc[P_CHOICE] += tick() - tch;
            visited[ci] = true;
            rd.visit(ci, path);
        }
        rd.st.cyc[P_SPREAD] += sig;
        rd.st.cyc[P_ORDER] -= sig.min(rd.st.cyc[P_ORDER]);
        info.path = "probe";
        if raced {
            info.path = "race";
            for &ci in &live {
                if !visited[ci] {
                    let nv = nv_of(ci);
                    rd.visit(ci, KnnPath::Direct);
                    spent += m.scan(nv);
                }
            }
        }
        info.cost = spent;
        rd.st.cyc[C_SPREADS] += (info.spreads * self.st.r) as u64;
        rd.st.cyc[C_DIRECT] += info.direct as u64;
        rd.st.cyc[C_INDEX] += info.index as u64;
        rd.st.cyc[C_GROUPS] += info.groups as u64;
        rd.st.cyc[C_VISITED] += info.read.len() as u64;
        let (ents, st) = rd.finish();
        (out(ents, st), info)
    }

    /// The approximate threshold query at t with knob `psi` (at least 0; `PSI` by default), as the module's comment
    /// says: the visible rows scoring at least t of the clusters the support table takes in and of those whose key
    /// reaches t, ascending; a subset of the exact answer, and the exact answer when the query hands over
    /// (`AdaptiveInfo::path` "scan" or "exact", always when psi is infinite and the support table leaves a cluster
    /// open). `plan` is the reads' (and the exact
    /// query's on a hand-over).
    pub fn threshold(&self, ix: &Index2, q: &[f32], t: f64, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (ThrOut2, AdaptiveInfo) {
        assert!(psi >= 0.0, "psi is at least 0 (infinity reads every cluster), not {psi}");
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let all = psi == f64::INFINITY;
        let nusort = !ix.nukey.is_empty();
        let fixed = m.i8_us * k as f64 + m.anc_us * Self::anchors(ix, live.len()) as f64;
        let mut info = AdaptiveInfo { fixed, scan_cost: m.scan(nvis), ..AdaptiveInfo::default() };
        let opts = Opts2 { vis, verify: false, want_iv: false, plan };
        let done = |mut out: ThrOut2, st: &Stats| {
            out.st.add(st);
            out.st.cyc[P_QUERY] += tick() - t0;
            out
        };
        // a scan of the visible rows, its rows counted as touched
        let scan = |st: &mut Stats| {
            st.touched += nvis as u64;
            scan_visible(ix, &pr, t, vis, false, plan.pscan)
        };
        if nvis == 0 {
            info.path = "empty";
            return (done(ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() }, &st), info);
        }
        let cs = info.scan_cost;
        if fixed >= cs {
            info.path = "scan";
            info.cost = cs;
            let out = scan(&mut st);
            return (done(out, &st), info);
        }
        let tc = tick();
        let qm = self.centres.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - tc;
        // the support table's verdicts: wholly below t (dropped), wholly above (taken in), or open
        let tch = tick();
        if !nusort {
            pr.anc_among(ix, Some(&live));
        }
        let mut pre = vec![Pre::Skip; k];
        let mut open = Vec::new();
        for &ci in &live {
            match support_pre(ix, &pr, t, nv_of(ci), ci, nusort) {
                Some(p) => pre[ci] = p,
                None => open.push(ci),
            }
        }
        st.cyc[P_CHOICE] += tick() - tch;
        // the open clusters whose key reaches t: a sigma only for those whose bound reaches it
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let (sc, mut p, mut sig) = (self.sigma_cost(), vec![0f32; self.st.r], 0);
        let mut spent = fixed;
        let mut take = Vec::new();
        let priced: Vec<usize> = if all { Vec::new() } else { open.iter().copied().filter(|&ci| self.bound(ci, &qm, psi, qq) >= t).collect() };
        // the sigmas are spent before anything is read: a scan that costs no more is taken first
        if spent + priced.len() as f64 * sc >= cs {
            st.cyc[P_ORDER] += tick() - ts;
            info.path = "scan";
            info.cost = spent + cs;
            let out = scan(&mut st);
            return (done(out, &st), info);
        }
        for &ci in &priced {
            let tsp = tick();
            let s = self.sigma(ix, ci, &pr.q32, qq, &mut p);
            sig += tick() - tsp;
            spent += sc;
            info.spreads += 1;
            if qm[ci] + psi * s >= t {
                take.push(ci);
            }
        }
        if all {
            take = open.clone();
        }
        st.cyc[P_SPREAD] += sig;
        st.cyc[P_ORDER] += tick() - ts - sig;
        // the cost of reading them, against a scan and against the exact query (every open cluster)
        let tch = tick();
        let price = |set: &[usize]| -> (f64, Vec<(usize, KnnPath, bool)>) {
            let mut ch = Charges { m, ix, pr: &pr, anchored: true, gset: vec![false; ix.groups.len()] };
            let mut tot = 0.0;
            let paths = set.iter().map(|&ci| {
                let (path, cost, gnew) = ch.choose(ci, nv_of(ci), 0);
                tot += cost;
                (ci, path, gnew)
            }).collect();
            (tot, paths)
        };
        let (cp, paths) = price(&take);
        info.exact_cost = price(&open).0;
        st.cyc[P_CHOICE] += tick() - tch;
        // a scan cheaper than the reads, or reads as dear as the exact query's (every open cluster can hold answers)
        if cs < cp || (!open.is_empty() && info.exact_cost <= cp) {
            let exact = info.exact_cost <= cs;
            info.path = if exact { "exact" } else { "scan" };
            info.cost = spent + cs.min(info.exact_cost);
            let out = if exact { threshold2(ix, &pr, t, &opts) } else { scan(&mut st) };
            return (done(out, &st), info);
        }
        info.path = "probe";
        info.cost = spent + cp;
        info.read = take.iter().map(|&c| c as u32).collect();
        let tc = tick();
        if nusort {
            let mut gs: Vec<usize> = paths.iter().filter(|x| x.1 == KnnPath::Index).map(|x| ix.cl[x.0].g).collect();
            gs.sort_unstable();
            gs.dedup();
            setup_groups(ix, &pr, &gs, &mut st);
        }
        let mut use_idx = vec![false; k];
        let flat = matches!(plan.mode, PlanMode::Flat);
        for &(ci, path, gnew) in &paths {
            if path == KnnPath::Index {
                info.index += 1;
                info.groups += gnew as usize;
                let pc = cut_pre(ix, &pr, t, vis, ci, nusort);
                use_idx[ci] = !flat && matches!(pc, Pre::Open { ncand, .. } if ncand > 0);
                pre[ci] = pc;
            } else {
                info.direct += 1;
                pre[ci] = Pre::Direct;
            }
        }
        st.cyc[C_SPREADS] += (info.spreads * self.st.r) as u64;
        st.cyc[C_DIRECT] += info.direct as u64;
        st.cyc[C_INDEX] += info.index as u64;
        st.cyc[C_GROUPS] += info.groups as u64;
        st.cyc[C_VISITED] += info.read.len() as u64;
        let out = run_planned(ix, &pr, t, &opts, &pre, &use_idx, tick() - tc);
        (done(out, &st), info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fit recovers weights from exact samples, sets a weight that would be negative to 0, and one the samples do
    /// not determine (a column always 0) too.
    #[test]
    fn fit_rel_recovers_the_weights() {
        let x: Vec<Vec<f64>> = (0..40).map(|i| vec![1.0, (i % 7) as f64 * 100.0, (i * 37 % 11) as f64 * 50.0, 0.0]).collect();
        let t: Vec<f64> = x.iter().map(|r| 3.0 + 0.02 * r[1] + 0.5 * r[2]).collect();
        let w = fit_rel(&x, &t);
        for (a, b) in w.iter().zip([3.0, 0.02, 0.5, 0.0]) {
            assert!((a - b).abs() < 1e-9 * (1.0 + b), "{w:?}");
        }
        let t: Vec<f64> = x.iter().map(|r| 3.0 + 0.01 * r[1]).collect();
        let w = fit_rel(&x, &t);
        assert!(w.iter().all(|&a| a >= 0.0) && (w[1] - 0.01).abs() < 1e-9, "{w:?}");
        let t: Vec<f64> = x.iter().map(|r| (1.0 + r[1] - 0.001 * r[2]).max(1.0)).collect();
        assert!(fit_rel(&x, &t).iter().all(|&a| a >= 0.0));
    }

    /// Constants that would price nothing (a read or a product free, a value not a number) are refused, from a
    /// cache's text too; the rule keeps the crossover at the typical size whatever the split of cl and size.
    #[test]
    fn degenerate_constants_are_refused() {
        let m = Measured { i8_us: 0.02, row_us: 0.05, dir_us: 1.2, cl_us: 15.0, size_us: 0.004, vis_us: 0.015, grp_us: 56.0, anc_us: 0.08 };
        assert!(m.check().is_ok());
        for bad in [Measured { row_us: 0.0, ..m }, Measured { i8_us: 0.0, ..m }, Measured { dir_us: 0.0, ..m }, Measured { cl_us: -1.0, ..m }, Measured { grp_us: f64::NAN, ..m }, Measured { vis_us: f64::INFINITY, ..m }] {
            assert!(bad.check().is_err(), "{bad:?}");
            assert!(Measured::from_kv(&bad.to_kv()).is_err(), "{bad:?}");
        }
        // cl and size S traded against each other at the typical size give the same crossover there
        let (a, b) = (Measured { cl_us: 15.0, size_us: 0.004, ..m }, Measured { cl_us: 15.0 + 0.004 * 2000.0, size_us: 0.0, ..m });
        let at = |m: &Measured, s: f64| m.rule(2000).map(|(r0, kp)| r0 as f64 + kp * s).unwrap();
        assert!((at(&a, 2000.0) - at(&b, 2000.0)).abs() <= 1.0, "{} {}", at(&a, 2000.0), at(&b, 2000.0));
    }
}
