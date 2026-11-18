//! The approximate queries: adaptive k-NN and threshold queries over the clusters, with one raw knob psi.
//!
//! Each cluster holding a visible row is keyed by u_c = mu_c + psi sigma_c(q): mu_c its centre score (the int8
//! centres), sigma_c(q) = sqrt(q' S_c q) the spread of its rows' scores along the query, S_c the cluster's covariance as
//! its stored principal directions (`order::OrderState`) and an isotropic rest. u_c is an optimistic estimate of the
//! cluster's best score, on the score scale, and psi says how optimistic (theory: the best multiplier is 3 to 4 at
//! every unit size and dataset, `hierarchy/theory.md` 3.4).
//!
//! - k-NN: clusters are read in u order, each read whole by `KnnReader` (its visible rows scored from their stored
//!   vectors, or the index path at the running k-th best), until the next cluster's u falls below the running k-th
//!   best; k items are returned whenever k are visible. The sigmas are computed lazily: the clusters come in the
//!   `spread` order (`q::Approx`, free from the centre scores), and the `LOOKAHEAD` next ones have their sigma, so
//!   that a cluster whose spread puts it ahead can overtake within that window. The probe races a scan of the visible
//!   rows: before each step, when its modelled cost would pass that of the scan (C_s), the rows not yet read are
//!   scanned instead. The answer is then exact and the whole costs at most 2 C_s (the probe spends at most C_s, the
//!   rest of the scan at most C_s, and no row is read twice).
//! - threshold at t: the clusters whose u reaches t are read (the support table first drops those wholly below t
//!   and takes in those wholly above it without a read), each exactly as the exact query reads it. That set does not
//!   depend on what is read, so the race is settled before reading: when a scan of the visible rows, or the exact query
//!   (every cluster the support table leaves open), costs no more than the set, the query hands over to the cheaper,
//!   which answers exactly.
//! - a sigma is computed only where it costs less than reading the cluster it could save (16 int8 rows of n against
//!   the cluster's direct read); a cluster cheaper to read than to price takes its key's upper bound
//!   (sigma <= sqrt(max(lambda_1, rest)) |q|) and is read on it.
//! - each cluster is read directly or on the index path, whichever the measured cost model says is cheaper
//!   (`Measured`), with a group's setup and the anchor search charged to the first cluster that needs them. The model's
//!   constants are timed on this machine on first use and cached with the index (`measured_constants`).

use super::q::*;
use super::*;
use crate::kernel::Stats;
use roaring::RoaringBitmap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

/// psi by default. Measured paired on glove, yahoo and LAION (k = 10): psi 4 gives recall 0.78-0.89 unfiltered and
/// psi 5 gives 0.90-0.985; filtered sets reach more at the same psi. 4.5 sits between: about 0.85-0.96 unfiltered,
/// 0.90 and above under the random and region visible sets measured.
pub const PSI: f64 = 4.5;
/// The order's key when the knob asks for everything (psi infinite: the order then changes no answer).
const PSI_ALL: f64 = 3.5;
/// The clusters next in the `spread` order that have their sigma before the probe reaches them: the window in which a
/// cluster can overtake by its spread. Each query computes this many sigmas beyond those it reads (16 int8 rows of n
/// each), so the window costs a fixed few microseconds.
pub const LOOKAHEAD: usize = 8;

/// The cost model of the approximate queries, in microseconds on the calling thread, timed on this machine (`measure`):
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
        Some(m) => log(format!("[approx] measured constants {}: {why}, in {:.1}s; the exact planner's rule from them: {}", m.to_kv().trim().replace('\n', " "), t.s(),
                               m.rule().map_or("every cluster scored directly".into(), |(r, kp)| format!("r_rows={r} r_kappa={kp:.3}")))),
        None => log(format!("[approx] no measured constants: {why}")),
    }
    m
}


/// What one approximate query did. `path`: "probe" (k-NN stopped by its knob; a threshold query's clusters read),
/// "race" (k-NN: the rest scanned once the probe would cost more than a scan), "scan" (the visible rows scanned before
/// probing), "exact" (a threshold query handed to the exact one), "empty". Costs are modelled (`Measured`): `fixed`
/// the order's (centre scores; the anchor search of a threshold query), `cost` everything, `scan_cost` a scan of the
/// visible rows, `exact_cost` (threshold) every cluster the support table leaves open, at its cheaper path.
#[derive(Clone, Debug, Default)]
pub struct AdaptiveInfo {
    pub path: &'static str,
    pub visited: usize,
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
    /// the `spread` order (the stream the sigmas are computed in) and the int8 centres
    ap: Approx,
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
    /// The cheaper path for a cluster of `rows` rows, `nv` visible, given what is already set up (`anchors` the anchors
    /// a first search would score), and its cost; marks what the index path sets up.
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
    /// The state for `ix` with the constants `m`: the `spread` order's radii and int8 centres, and each cluster's
    /// spread's rest and bound from its stored directions.
    pub fn new(ix: &Index2, m: Measured) -> Result<Adaptive, String> {
        if !matches!(ix.full, Full2::Real { .. }) {
            return Err("the approximate queries read stored vectors, and this index has none; query it exactly (knn2, threshold2)".into());
        }
        let st = ix.order.clone().ok_or("this index holds no order state (the clusters' directions the approximate queries read); rebuild it")?;
        let r = st.r;
        let rest: Vec<f64> = (0..ix.k()).map(|c| {
            let lam = &st.lam[c * r..(c + 1) * r];
            let top: f64 = lam.iter().map(|&l| l as f64).sum();
            (st.rad[c] * st.rad[c] - top).max(0.0) / (st.n0.max(r + 1) - r) as f64
        }).collect();
        let top = (0..ix.k()).map(|c| st.lam.get(c * r).map_or(0.0, |&l| l as f64).max(rest[c])).collect();
        Ok(Adaptive { m, ap: Approx::spread(ix), st, rest, top })
    }

    /// sigma_c(q) of cluster ci from its stored directions (`p` and `pf` scratch of r values), the query's f32 values
    /// and |q|^2.
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

    /// Approximate k-NN with knob `psi` (`PSI` by default; infinity reads every cluster and answers exactly). Returns
    /// k items whenever k are visible. `plan` is the index path's (its dial is the certified one).
    pub fn knn(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (KnnOut2, AdaptiveInfo) {
        assert!(kk > 0, "approximate k-NN needs k of at least 1");
        let tm = Timer::new();
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let all = psi.is_nan() || psi == f64::INFINITY;
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
        let qm = self.ap.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - tc;
        let ts = tick();
        let b = self.ap.bonus(&qm);
        let mut stream: BinaryHeap<Key> = live.iter().map(|&ci| Key(qm[ci] + b * self.ap.rad[ci], Reverse(ci))).collect();
        st.cyc[P_ORDER] += tick() - ts;
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let psi = if all { PSI_ALL } else { psi };
        let sc = self.sigma_cost();
        let mut p = vec![0f32; self.st.r];
        let mut window: BinaryHeap<Key> = BinaryHeap::new();
        let mut ch = Charges { m, ix, pr: &pr, anchored: !ix.nukey.is_empty(), gset: vec![false; ix.groups.len()] };
        let mut spent = fixed;
        let mut visited = vec![false; k];
        let mut seen: Vec<usize> = Vec::new();
        let mut raced = false;
        let mut rd = KnnReader::new(ix, &pr, vis, kk, plan, st);
        loop {
            let ts = tick();
            let mut sig = 0;
            while window.len() < LOOKAHEAD && !stream.is_empty() {
                let ci = stream.peek().unwrap().1 .0;
                // a cluster cheaper to read than to price is read on its key's upper bound
                if m.direct(nv_of(ci)) <= sc {
                    stream.pop();
                    window.push(Key(qm[ci] + psi * (self.top[ci] * qq).sqrt(), Reverse(ci)));
                    continue;
                }
                if spent + sc > cs {
                    raced = true;
                    break;
                }
                stream.pop();
                let tsp = tick();
                let s = self.sigma(ix, ci, &pr.q32, qq, &mut p);
                sig += tick() - tsp;
                spent += sc;
                info.spreads += 1;
                window.push(Key(qm[ci] + psi * s, Reverse(ci)));
            }
            let next = if raced { None } else { window.pop() };
            rd.st.cyc[P_SPREAD] += sig;
            rd.st.cyc[P_ORDER] += tick() - ts - sig;
            let Some(Key(key, Reverse(ci))) = next else { break };
            // the batched direct rows are scored first (their time is the reads')
            if !all && rd.kth() > key {
                break;
            }
            let tch = tick();
            let nv = nv_of(ci);
            if spent + m.direct(nv) > cs {
                raced = true;
                rd.st.cyc[P_CHOICE] += tick() - tch;
                break;
            }
            seen.push(ci);
            let (path, cost, gnew) = ch.choose(ci, nv, Self::anchors(ix, seen.len()));
            if path == KnnPath::Index {
                if !ch.anchored {
                    pr.anc_among(ix, Some(&seen));
                    ch.anchored = true;
                }
                info.index += 1;
                info.groups += gnew as usize;
            } else {
                info.direct += 1;
            }
            spent += cost;
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
        rd.st.cyc[C_SPREADS] += (info.spreads * self.st.r) as u64;
        rd.st.cyc[C_DIRECT] += info.direct as u64;
        rd.st.cyc[C_INDEX] += info.index as u64;
        rd.st.cyc[C_GROUPS] += info.groups as u64;
        rd.st.cyc[C_VISITED] += info.visited as u64;
        let (ents, st) = rd.finish();
        (out(ents, st), info)
    }

    /// The approximate threshold query at t with knob `psi` (`PSI` by default): the visible rows scoring at least t of
    /// the clusters the support table takes in and of those whose key reaches t, ascending; a subset of the exact
    /// answer, and the exact answer when psi is infinite or the query hands over (`AdaptiveInfo::path` "scan" or
    /// "exact"). `plan` is the reads' (and the exact query's on a hand-over).
    pub fn threshold(&self, ix: &Index2, q: &[f32], t: f64, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (ThrOut2, AdaptiveInfo) {
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let all = psi.is_nan() || psi == f64::INFINITY;
        let nusort = !ix.nukey.is_empty();
        let fixed = m.i8_us * k as f64 + m.anc_us * Self::anchors(ix, live.len()) as f64;
        let mut info = AdaptiveInfo { fixed, scan_cost: m.scan(nvis), ..AdaptiveInfo::default() };
        let opts = Opts2 { vis, verify: false, want_iv: false, plan };
        let done = |mut out: ThrOut2, st: &Stats| {
            out.st.add(st);
            out.st.cyc[P_QUERY] += tick() - t0;
            out
        };
        if nvis == 0 {
            info.path = "empty";
            return (done(ThrOut2 { st: Stats::new(nl), npow: ix.meta.npow, ..Default::default() }, &st), info);
        }
        let cs = info.scan_cost;
        if fixed >= cs {
            info.path = "scan";
            info.cost = cs;
            return (done(scan_visible(ix, &pr, t, vis, false, plan.pscan), &st), info);
        }
        let tc = tick();
        let qm = self.ap.centre_scores(ix, &pr.q32);
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
        // the open clusters whose key reaches t; a sigma only for those whose bound reaches it, and where it costs
        // less than the read it could save
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let (sc, mut p, mut sig) = (self.sigma_cost(), vec![0f32; self.st.r], 0);
        let mut spent = fixed;
        let mut take = Vec::new();
        let mut priced: Vec<usize> = Vec::new();
        for &ci in &open {
            if !all && qm[ci] + psi * (self.top[ci] * qq).sqrt() < t {
                continue;
            }
            if all || m.direct(nv_of(ci)) <= sc {
                take.push(ci);
            } else {
                priced.push(ci);
            }
        }
        // the spreads are spent before anything is read: a scan that costs no more is taken first
        if !all && spent + priced.len() as f64 * sc >= cs {
            st.cyc[P_ORDER] += tick() - ts;
            info.path = "scan";
            info.cost = spent + cs;
            return (done(scan_visible(ix, &pr, t, vis, false, plan.pscan), &st), info);
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
        if all || cs < cp || (!open.is_empty() && info.exact_cost <= cp) {
            let exact = all || info.exact_cost <= cs;
            info.path = if exact { "exact" } else { "scan" };
            info.cost = spent + cs.min(info.exact_cost);
            let out = if exact { threshold2(ix, &pr, t, &opts) } else { scan_visible(ix, &pr, t, vis, false, plan.pscan) };
            return (done(out, &st), info);
        }
        info.path = "probe";
        info.cost = spent + cp;
        info.visited = take.len();
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
        st.cyc[C_VISITED] += info.visited as u64;
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
}
