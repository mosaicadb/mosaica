//! The simple arms of approximate k-NN: how far the simplest pieces go against the approximate process (`process`).
//! Cumulative steps (`Variant`) of one loop over the clusters holding a visible row, each cluster read whole by
//! `KnnReader` (its visible rows scored from their stored vectors, or the index path at the running k-th best):
//!
//! - S1: the `spread` order, key q.m + f sd(q) rad / sd(rad) at f = `SPREAD_F`, its first nprobe clusters (the knob)
//!   and as many more as it takes to hold k items; each cluster read directly or on the index path, whichever the
//!   measured cost model (`Measured`) says is cheaper;
//! - S2: the same key at f the knob, the probe stopped when the next cluster's key falls below the running k-th best.
//!   The key is on the score scale (q.m plus f times the centre scores' standard deviation, in units of the radii's),
//!   an optimistic estimate of the cluster's best score that grows with f;
//! - S3: S2 racing a scan of the visible rows: before each step, when the probe's modelled cost would pass that of the
//!   scan (C_s), the rows not yet read are scanned instead. The answer is then exact, and the whole costs at most
//!   2 C_s: the probe spends at most C_s, and the rest of the scan at most C_s, since no row is read twice;
//! - S4: S3 by the key mu_c + psi sigma_c(q), psi the knob (`PSI` when the knob asks for everything), sigma_c the
//!   spread of the cluster's scores from its group's covariance (the calibration's `GroupCov`, one product over all
//!   the groups' directions, scaled by the cluster's radius) or from the cluster's own directions (`OrderState`),
//!   computed in the `spread` order `LOOKAHEAD` clusters ahead of the probe.
//!
//! Every variant returns k items whenever k are visible. Its cost model's constants are timed on this machine and
//! cached with the index (`measured_constants`); nothing in it is set by hand.

use super::q::*;
use super::*;
use crate::kernel::Stats;
use roaring::RoaringBitmap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

/// S4's psi when the knob asks for everything (the order then does not change the answer): the middle of the best
/// multipliers measured at every unit size and dataset (`hierarchy/theory.md` 3.4: 3 to 4).
pub const PSI: f64 = 3.5;
/// S4 with the clusters' own spreads: how many clusters ahead of the probe, in the `spread` order, have theirs.
pub const LOOKAHEAD: usize = 8;

/// The cost model of the simple arms, in microseconds on the calling thread, timed on this machine (`measure`):
/// - `i8_us`, a product of the query with an int8 row of n (centre scores, spreads' directions);
/// - `row_us`, `dir_us`: a cluster read directly costs dir + row v for its v visible rows; a scan of v rows, row v;
/// - `cl_us`, `size_us`, `vis_us`: a cluster of S rows on the index path costs cl + size S + vis v, at the running
///   k-th best of a query (its sorted range, levels and the rows left to their stored vectors included);
/// - `grp_us`, a group's setup (a + b n^2 at this index's n), charged to the first cluster of a group on the index path;
/// - `anc_us`, the search for the nearest anchor, per anchor searched, charged to the first cluster on the index path.
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

    /// The constants as `name=value` lines.
    pub fn to_kv(&self) -> String {
        NAMES.iter().zip(self.values()).map(|(k, v)| format!("{k}={v}\n")).collect()
    }

    /// The constants of `name=value` lines (as `to_kv` writes them), each a number of at least 0.
    pub fn from_kv(text: &str) -> Result<Measured, String> {
        let mut v = [f64::NAN; 8];
        for (i, k) in NAMES.iter().enumerate() {
            let x = text.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('='))).ok_or_else(|| format!("no {k}"))?;
            v[i] = x.trim().parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0).ok_or_else(|| format!("{k}={x} is not a number of at least 0"))?;
        }
        Ok(Measured::from_values(v))
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

    /// The exact planner's rule in rows (`Plan::r_rows` R0, `r_kappa` kappa) that these constants give: a cluster of S
    /// rows holding v visible ones is scored directly when dir + row v <= cl + size S + vis v, that is when
    /// v <= (cl - dir) / (row - vis) + size / (row - vis) S. None when the index path's rows cost no less than
    /// direct ones (every cluster would be scored directly).
    pub fn rule(&self) -> Option<(usize, f64)> {
        let d = self.row_us - self.vis_us;
        (d > 0.0).then(|| (((self.cl_us - self.dir_us).max(0.0) / d).round() as usize, self.size_us / d))
    }
}

/// Pseudo-queries the timing runs (corpus rows spread over the index), clusters each reads (its best by centre score),
/// visible shares (one row in each of these) and repetitions (the fastest is kept).
const T_QUERIES: usize = 8;
const T_CLUSTERS: usize = 16;
const T_SHARES: [u64; 4] = [1, 4, 16, 64];
const T_REPS: usize = 3;

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
            for r in 0..p {
                if r != c {
                    let f = a[r][c] / a[c][c];
                    for cc in c..=p {
                        a[r][cc] -= f * a[c][cc];
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

/// The fastest of `T_REPS` timings of `f` after `setup` (untimed), in microseconds.
fn fastest<S>(mut setup: impl FnMut() -> S, mut f: impl FnMut(&mut S)) -> f64 {
    (0..T_REPS).map(|_| {
        let mut s = setup();
        let t = Instant::now();
        f(&mut s);
        t.elapsed().as_secs_f64() * 1e6
    }).fold(f64::INFINITY, f64::min)
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

/// The cost model's constants timed on `ix`, on the calling thread's pool (one thread in the bindings): products with
/// int8 rows over a block of rows; then for `T_QUERIES` pseudo-queries under each visible share of `T_SHARES`, each
/// of the `T_CLUSTERS` clusters with the largest centre scores read directly and on the index path at the 10th and
/// at the 100th best of those clusters' visible rows, each timing after the core's own caches were cleared
/// (`evict_bytes`), fitted as `Measured` says (relative least squares, weights at least 0); each group's setup and
/// the anchor search on a fresh query. A few seconds on a million rows. None without stored vectors.
pub fn measure(ix: &Index2) -> Option<Measured> {
    if !matches!(ix.full, Full2::Real { .. }) || ix.k() == 0 || ix.nrows < 2 {
        return None;
    }
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
            let bm: RoaringBitmap = (0..ix.nrows).filter(|&r| ((r as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) % s == 0).map(|r| ix.row_ent[r]).collect();
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
            // the 10th and the 100th best of those clusters' visible rows: a probe's running k-th best at its end, and
            // before it
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
                // a group's setup and the anchor search, each on a fresh query
                for &g in gs.iter().take(4) {
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
    let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    Some(Measured { i8_us, row_us: d[1], dir_us: d[0], cl_us: i[0], size_us: i[1], vis_us: i[2], grp_us: mean(&grp), anc_us: mean(&anc) })
}

/// The cache of the measured constants in an index directory.
pub const COMPUTE_CACHE: &str = "compute-constants.txt";
/// The version of its key: changed when the timing or the key changes, so older caches are timed again.
pub const COMPUTE_KEY_VERSION: u32 = 1;

/// The key the measured constants are cached under: the key's version, the machine and CPU (`machine_and_cpu`), the
/// instruction set the library was built for, and the index's shape (dimensions, rows, clusters, groups). One line.
pub fn compute_key(ix: &Index2) -> String {
    let (machine, cpu) = machine_and_cpu();
    let isa = if cfg!(target_feature = "avx512f") { "avx512" } else if cfg!(target_feature = "avx2") { "avx2" } else { "base" };
    format!("v{COMPUTE_KEY_VERSION}; machine {machine}; cpu {cpu}; isa {isa}; n {}; rows {}; clusters {}; groups {}", ix.n(), ix.nrows, ix.k(), ix.groups.len())
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
/// what the other saved); and how they were found. None when they could be neither read nor timed.
pub fn cached_measured(path: &std::path::Path, key: &str, time: impl FnOnce() -> Option<Measured>) -> (Option<Measured>, String) {
    let why = match read_cache(path, key) {
        Ok(m) => return (Some(m), format!("read from {}", path.display())),
        Err(e) => e,
    };
    let lock = lock_file(&path.with_extension("lock"));
    if lock.is_some() {
        if let Ok(m) = read_cache(path, key) {
            return (Some(m), format!("read from {}, saved by another open while this one waited", path.display()));
        }
    }
    let why = if why.starts_with("no cache") { format!("timed: {why}") } else { format!("timed again: {why}") };
    let Some(m) = time() else { return (None, format!("{why}; but they cannot be timed here (no stored vectors)")) };
    let why = match write_atomic(path, &format!("key={key}\n{}", m.to_kv())) {
        Ok(()) => format!("{why}; saved in {}", path.display()),
        Err(e) => format!("{why}; not saved in {} ({e})", path.display()),
    };
    drop(lock);
    (Some(m), why)
}

/// The measured constants of this machine for `ix`: from `COMPUTE_CACHE` in the index directory `dir`, else from the
/// per-user cache (`user_cache_dir`, a file named by the key) when `dir` cannot be written, else timed (`measure`)
/// and saved there. Logged. None without stored vectors.
pub fn measured_constants(ix: &Index2, dir: &str, log: &mut dyn FnMut(String)) -> Option<Measured> {
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
    let (m, why) = cached_measured(&path, &key, || measure(ix));
    match &m {
        Some(m) => log(format!("[simple] measured constants {}: {why}, in {:.1}s; the exact planner's rule from them: {}", m.to_kv().trim().replace('\n', " "), t.s(),
                               m.rule().map_or("every cluster scored directly".into(), |(r, kp)| format!("r_rows={r} r_kappa={kp:.3}")))),
        None => log(format!("[simple] no measured constants: {why}")),
    }
    m
}

/// A simple arm (see the module's comment).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    S1,
    S2,
    S3,
    S4Group,
    S4Own,
}

impl Variant {
    pub const ALL: [Variant; 5] = [Variant::S1, Variant::S2, Variant::S3, Variant::S4Group, Variant::S4Own];
    pub fn parse(s: &str) -> Result<Variant, String> {
        match s {
            "s1" => Ok(Variant::S1),
            "s2" => Ok(Variant::S2),
            "s3" => Ok(Variant::S3),
            "s4" => Ok(Variant::S4Group),
            "s4-own" => Ok(Variant::S4Own),
            _ => Err(format!("the simple arm is s1, s2, s3, s4 (group spreads) or s4-own (the clusters' own), not {s:?}")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Variant::S1 => "s1",
            Variant::S2 => "s2",
            Variant::S3 => "s3",
            Variant::S4Group => "s4",
            Variant::S4Own => "s4-own",
        }
    }
    /// Whether the probe races a scan.
    pub fn race(self) -> bool {
        matches!(self, Variant::S3 | Variant::S4Group | Variant::S4Own)
    }
}

/// What one query of a simple arm did. `path`: "probe" (stopped by its knob), "race" (the rest scanned once the probe
/// would cost more than a scan), "scan" (scanned before probing: k or fewer visible rows, or the order alone would
/// cost the scan), "empty". Costs are modelled (`Measured`): `fixed` the order's (centre scores, group spreads),
/// `cost` everything, `scan_cost` a scan of the visible rows.
#[derive(Clone, Debug, Default)]
pub struct SimpleInfo {
    pub path: &'static str,
    pub visited: usize,
    pub direct: usize,
    pub index: usize,
    pub groups: usize,
    pub spreads: usize,
    pub fixed: f64,
    pub cost: f64,
    pub scan_cost: f64,
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

/// The simple arms' state for one index.
pub struct Simple {
    pub m: Measured,
    ap: Approx,
    /// the clusters' own directions (S4 own), with each cluster's isotropic rest
    own: Option<(std::sync::Arc<super::order::OrderState>, Vec<f64>)>,
    /// each group's isotropic rest (S4 group; with the calibration's group covariances)
    grest: Vec<f64>,
}

impl Simple {
    /// The state for `ix` with the constants `m`: the `spread` order's radii and int8 centres, and the spreads' rests.
    pub fn new(ix: &Index2, m: Measured) -> Result<Simple, String> {
        if !matches!(ix.full, Full2::Real { .. }) {
            return Err("the simple arms read stored vectors, and this index has none; query it exactly (knn2)".into());
        }
        let ap = Approx::new(ix, OrderKind::Spread, SPREAD_F, Approx::radii(ix));
        let own = ix.order.clone().map(|st| {
            let r = st.r;
            let rest = (0..ix.k()).map(|c| super::calib::rest(st.rad[c], &st.lam[c * r..(c + 1) * r], st.n0)).collect();
            (st, rest)
        });
        let grest = ix.calib.as_ref().map_or(Vec::new(), |cal| {
            let g = &cal.gcov;
            (0..ix.groups.len()).map(|i| super::calib::rest(g.rad[i], &g.lam[i * g.r..(i + 1) * g.r], g.n0)).collect()
        });
        Ok(Simple { m, ap, own, grest })
    }

    /// Whether variant `v` can run on `ix`: S4 needs the calibration's group covariances, or the order state's
    /// directions.
    pub fn check(&self, ix: &Index2, v: Variant) -> Result<(), String> {
        match v {
            Variant::S4Group if ix.calib.is_none() => Err("s4 reads the group covariances of the approximate process's calibration, and this index has none (too few rows); use s4-own or s3".into()),
            Variant::S4Own if self.own.is_none() => Err("s4-own reads the clusters' directions of the order state, and this index has none; rebuild it".into()),
            _ => Ok(()),
        }
    }

    /// Approximate k-NN by variant `v` with knob `knob`: nprobe for S1, f for S2 and S3, psi for S4; infinity asks
    /// for every cluster. Returns k items whenever k are visible. `plan` is the index path's (its dial is the
    /// certified one).
    #[allow(clippy::too_many_arguments)]
    pub fn knn(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, v: Variant, knob: f64, plan: Plan) -> (KnnOut2, SimpleInfo) {
        assert!(kk > 0, "approximate k-NN needs k of at least 1");
        self.check(ix, v).unwrap();
        let tm = Timer::new();
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let all = knob.is_nan() || knob == f64::INFINITY;
        let gr = ix.calib.as_ref().map_or(0, |c| c.gcov.r);
        let fixed = m.i8_us * (k + if v == Variant::S4Group { ix.groups.len() * gr } else { 0 }) as f64;
        let mut info = SimpleInfo { fixed, scan_cost: m.scan(nvis), ..SimpleInfo::default() };
        let out = |ents: Vec<(f64, u32)>, mut st: Stats| {
            st.cyc[P_QUERY] += tick() - t0;
            KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
        };
        if nvis == 0 {
            info.path = "empty";
            return (out(Vec::new(), st), info);
        }
        let cs = info.scan_cost;
        if v.race() && (nvis <= kk || fixed >= cs) {
            info.path = "scan";
            info.cost = cs;
            st.full_reads += nvis as u64;
            st.touched += nvis as u64;
            return (out(scan_knn(ix, &pr.q16, &pr.q32, pr.qn, kk, vis), st), info);
        }
        // the order
        let tc = tick();
        let qm = self.ap.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - tc;
        let sdr = self.ap.sd_ratio(&qm);
        let spread_key = |ci: usize, f: f64| qm[ci] + f * sdr * self.ap.rad[ci];
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let psi = if all { PSI } else { knob };
        let mut spent = fixed;
        let mut heap: BinaryHeap<Key> = match v {
            Variant::S1 | Variant::S2 | Variant::S3 => {
                let f = if v == Variant::S1 || all { SPREAD_F } else { knob };
                let ts = tick();
                let h = live.iter().map(|&ci| Key(spread_key(ci, f), Reverse(ci))).collect();
                st.cyc[P_SOLVE] += tick() - ts;
                h
            }
            Variant::S4Group => {
                let cal = ix.calib.as_ref().unwrap();
                let g = &cal.gcov;
                let tsp = tick();
                let mut p = vec![0f32; ix.groups.len() * g.r];
                unsafe { crate::kernel::gemv_i8(&pr.q32, &g.dirs8, ix.n(), &mut p) };
                let gsig: Vec<f64> = (0..ix.groups.len()).map(|i| {
                    let pf: Vec<f64> = p[i * g.r..(i + 1) * g.r].iter().map(|&x| x as f64).collect();
                    super::calib::sigma(&pf, &g.lam[i * g.r..(i + 1) * g.r], &g.dsc[i * g.r..(i + 1) * g.r], self.grest[i], qq)
                }).collect();
                st.cyc[P_SPREAD] += tick() - tsp;
                st.cyc[C_SPREADS] += p.len() as u64;
                info.spreads = live.len();
                let ts = tick();
                let h = live.iter().map(|&ci| {
                    let gi = ix.cl[ci].g;
                    Key(qm[ci] + psi * gsig[gi] * self.ap.rad[ci] / g.rad[gi].max(1e-300), Reverse(ci))
                }).collect();
                st.cyc[P_SOLVE] += tick() - ts;
                h
            }
            // the stream in the `spread` order; the clusters given their spread move to `window`
            Variant::S4Own => {
                let ts = tick();
                let h = live.iter().map(|&ci| Key(spread_key(ci, SPREAD_F), Reverse(ci))).collect();
                st.cyc[P_SOLVE] += tick() - ts;
                h
            }
        };
        let mut window: BinaryHeap<Key> = BinaryHeap::new();
        let own_sigma = |ci: usize, p: &mut [f32]| -> f64 {
            let (st0, rest) = self.own.as_ref().unwrap();
            let r = st0.r;
            unsafe { crate::kernel::gemv_i8(&pr.q32, st0.dirs(ci, r, ix.n()), ix.n(), &mut p[..r]) };
            let pf: Vec<f64> = p[..r].iter().map(|&x| x as f64).collect();
            super::calib::sigma(&pf, &st0.lam[ci * r..(ci + 1) * r], &st0.dsc[ci * r..(ci + 1) * r], rest[ci], qq)
        };
        let own_r = self.own.as_ref().map_or(0, |o| o.0.r);
        let mut pbuf = vec![0f32; own_r];
        let nprobe = if v == Variant::S1 && !all { knob.max(0.0) as usize } else { usize::MAX };
        let anchors = |seen: usize| if ix.anc_centres && ix.nanch == k { seen } else { ix.nanch };
        let mut anchored = !ix.nukey.is_empty();
        let mut visited = vec![false; k];
        let mut seen: Vec<usize> = Vec::new();
        let mut raced = false;
        let mut rd = KnnReader::new(ix, &pr, vis, kk, plan, st);
        loop {
            let ts = tick();
            let mut sig = 0;
            if v == Variant::S4Own {
                while window.len() < LOOKAHEAD && !heap.is_empty() {
                    let sc = m.i8_us * own_r as f64;
                    if v.race() && spent + sc > cs {
                        raced = true;
                        break;
                    }
                    let ci = heap.pop().unwrap().1 .0;
                    let tsp = tick();
                    let s = own_sigma(ci, &mut pbuf);
                    sig += tick() - tsp;
                    rd.st.cyc[C_SPREADS] += own_r as u64;
                    spent += sc;
                    info.spreads += 1;
                    window.push(Key(qm[ci] + psi * s, Reverse(ci)));
                }
            }
            let next = if raced { None } else if v == Variant::S4Own { window.pop() } else { heap.pop() };
            rd.st.cyc[P_SPREAD] += sig;
            rd.st.cyc[P_SOLVE] += tick() - ts - sig;
            let Some(Key(key, Reverse(ci))) = next else { break };
            // the batched direct rows are scored first (their time is the reads')
            let kth = rd.kth();
            let stop = if v == Variant::S1 { info.visited >= nprobe && kth > f64::NEG_INFINITY } else { !all && kth > key };
            if stop {
                break;
            }
            let tch = tick();
            let (rows, nv) = (ix.cl[ci].r1 - ix.cl[ci].r0, nv_of(ci));
            let dir = m.direct(nv);
            if v.race() && spent + dir > cs {
                raced = true;
                rd.st.cyc[P_CHOICE] += tick() - tch;
                break;
            }
            seen.push(ci);
            let g = ix.cl[ci].g;
            let gnew = pr.gq[g].get().is_none();
            let anc = if anchored { 0.0 } else { m.anc_us * anchors(seen.len()) as f64 };
            let idx = m.index(rows, nv) + if gnew { m.grp_us } else { 0.0 } + anc;
            let path = if idx < dir { KnnPath::Index } else { KnnPath::Direct };
            if path == KnnPath::Index {
                if !anchored {
                    pr.anc_among(ix, Some(&seen));
                    anchored = true;
                }
                info.index += 1;
                info.groups += gnew as usize;
                spent += idx;
            } else {
                info.direct += 1;
                spent += dir;
            }
            rd.st.cyc[P_CHOICE] += tick() - tch;
            info.visited += 1;
            visited[ci] = true;
            rd.visit(ci, path);
        }
        info.path = "probe";
        if raced {
            info.path = "race";
            for &ci in &live {
                if !visited[ci] {
                    rd.visit(ci, KnnPath::Direct);
                    spent += m.scan(nv_of(ci));
                }
            }
        }
        info.cost = spent;
        rd.st.cyc[C_DIRECT] += info.direct as u64;
        rd.st.cyc[C_INDEX] += info.index as u64;
        rd.st.cyc[C_GROUPS] += info.groups as u64;
        rd.st.cyc[C_VISITED] += info.visited as u64;
        let (ents, st) = rd.finish();
        (out(ents, st), info)
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
}
