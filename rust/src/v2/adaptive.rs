//! The approximate queries: adaptive k-NN and threshold queries over the clusters, with one raw knob psi >= 0.
//!
//! Each cluster holding a visible row has the key u_c = mu_c + psi sigma_c(q): mu_c its centre score (the int8
//! centres), sigma_c(q) = sqrt(q' S_c q) the spread of its rows' scores along the query, S_c the cluster's covariance
//! as its stored principal directions (`order::OrderState`) and an isotropic rest. u_c is an optimistic estimate of the
//! cluster's best score, on the score scale, and psi says how optimistic (theory: the best multiplier is 3 to 4 at
//! every unit size and dataset, `hierarchy/theory.md` 3.4). Every sigma is at most sqrt(top_c) |q|, top_c the
//! cluster's largest variance along a direction, so b_c = mu_c + psi sqrt(top_c) |q| bounds u_c from above.
//!
//! - k-NN reads the clusters in an order that does not depend on psi, and psi moves only where it stops. The order is
//!   by o_c = mu_c + z_c sigma_c(q), z_c = z(1, v_c) = Phi^-1(v_c / (v_c + 1)) the expected best of the cluster's
//!   v_c visible rows in units of their spread (3.3 at 2,000 rows, 1.7 at 20; derived, no constant): the best of what
//!   the query can see. The clusters come from a stream by mu_c + z_c rad_c sqrt(q' C q / tr C) (the lower cluster
//!   first among equal values), rad_c the cluster's RMS radius and C the pooled within-cluster covariance
//!   (`order::Pooled`, sum_c n_c S_c / N as its top directions and an isotropic rest): the spread the cluster would
//!   have if it were shaped as the clusters are on average. A part of q along which no cluster varies leaves it
//!   unchanged (it moves every mu_c alike when the centres agree along it), where rad_c |q| / sqrt(n0) would grow
//!   with it. The next `LOOKAHEAD` of the stream are priced (sigma computed) in a window; the probe reads the
//!   window's cluster of largest o_c, each whole by `KnnReader`, and stops when the window's largest u_c is below the
//!   running k-th best (minus infinity until k items are held: k items are returned whenever k are visible). So a
//!   smaller psi reads and prices the first of what a larger one does. The stop is a heuristic: a cluster further
//!   down the stream, not yet priced, may still have a u above the k-th best; ruling that out exactly prices every
//!   cluster on real data (the bound b_c is about 5 times the spread). With psi infinite the query is the exact one
//!   (`knn2`).
//! - the probe races a scan of the visible rows (C_s): before each sigma, sorted range and read, when its modelled
//!   cost would pass C_s, the rows not yet read are scanned instead, and the answer is exact; the whole is at most
//!   2 C_s beyond the anchor search (the probe at most C_s, the rest of the scan at most C_s, no row read twice).
//!   And it hands over to that scan at once, before spending its budget, when what it projects to read costs more
//!   than scanning the rows not yet read: the projection takes the clusters not yet read whose stop key with the
//!   stream's spread, mu_c + psi rad_c s, reaches the running k-th best, each at its sigma and its cheaper read of
//!   every visible row (sorted once per query, the sums kept as the k-th best rises). Where the exact query scans
//!   first (sparse visible sets) the scan is the exact query, less the rows already read; elsewhere the exact
//!   query's cost is not predicted (pricing it cost as much as a dense probe, which stops far below it).
//! - a threshold query at t reads the clusters whose u reaches t, after the support table has dropped those wholly
//!   below t and taken in those wholly above it without a read; each is read exactly as the exact query reads it. The
//!   decision comes first from the bounds sqrt(low_c) |q| <= sigma_c <= sqrt(top_c) |q| (low_c the cluster's least
//!   variance along a direction it holds): a cluster whose key's lower bound reaches t is read, one whose upper bound
//!   does not is not, and only the rest are priced. When the reads already sure, with the sigmas still to price, cost
//!   what a scan of the visible rows or the exact query (every open cluster) costs, the query hands over to the cheaper
//!   of the two, which answers exactly, before computing any sigma; otherwise the sigmas are computed (the largest
//!   bound first) racing that hand-over, and the reads taken are compared with it once more. With psi infinite it
//!   always hands over, unless the support table decided every cluster. On a visible set the exact query scans before
//!   anything else (`Plan::threshold_scans_first`: every cluster holding a visible row could be scored directly, in
//!   sum) the query is the exact one from the start, before the centre scores (path "sparse"): there every cluster
//!   read costs more than its rows' share of that scan, and the bounds decide too little for the fixed work and the
//!   sigmas to pay off (the paired runs at 1% visible: 1.04 to 1.29 times the exact query's time to recall 0.9 and
//!   0.95, but for yahoo random-1). It is decided from the visible set alone, so every query on it hands over.
//! - each cluster is read directly or on the index path, whichever the measured cost model says is cheaper
//!   (`Measured`), with a group's setup and the anchor search charged to the first cluster that needs them. k-NN
//!   prices the index path at the running k-th best t from the rows its sorted range at t leaves (`cut_rows`) and the
//!   share of them expected above t (`above`, from the gap between t and mu_c in units of sigma_c): late in a query a
//!   far cluster's rows are nearly all dropped by its levels, which the index path pays little for. Both paths
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
/// How far ahead of the probe k-NN looks: the clusters next in the stream that have their sigma, among which the probe
/// reads the largest order key and whose largest key the stop compares with the running k-th best. A structural choice, not
/// fitted: it costs this many sigmas beyond those of the clusters read, and a cluster can overtake by its spread only
/// within it.
pub const LOOKAHEAD: usize = 8;

/// The cost model of the approximate queries, in microseconds on the calling thread, timed on this machine (`measure`):
/// - `i8_us`, a product of the query with an int8 row of n (centre scores, spreads' directions);
/// - `row_us`, `dir_us`: a cluster read directly costs dir + row v for its v visible rows; a scan of v rows, row v;
/// - `cl_us`, `size_us`, `vis_us`, `und_us`: a cluster of S rows on the index path costs cl + size S + (vis + und u) c
///   at the running k-th best t of a query, c its visible rows that the sorted range at t leaves (the rest dropped
///   unread; every visible row before k items are held) and u = Gbar((t - mu) / sigma) the share of them expected to
///   score above t (mu its centre score, sigma its spread along the query, Gbar the normal tail): the rows its levels
///   cannot drop, which go on to finer levels and their stored vectors; nothing when the range leaves no row;
/// - `grp_us`, a group's setup (a + b n^2 at this index's n), charged to the first cluster of a group on the index path;
/// - `anc_us`, the search for the nearest anchor, per anchor searched, charged to the first cluster that needs it;
/// - `cut_us`, pricing a cluster's index path: its centre score in f64 and its sorted range at a k-th best, with the
///   visible rows in it counted;
/// - `sig_us`, one cluster's sigma (its r directions' products with the query and their sum).
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
    pub cut_us: f64,
    pub und_us: f64,
    pub sig_us: f64,
}

const NAMES: [&str; 11] = ["i8_us", "row_us", "dir_us", "cl_us", "size_us", "vis_us", "grp_us", "anc_us", "cut_us", "und_us", "sig_us"];

impl Measured {
    fn values(&self) -> [f64; 11] {
        [self.i8_us, self.row_us, self.dir_us, self.cl_us, self.size_us, self.vis_us, self.grp_us, self.anc_us, self.cut_us, self.und_us, self.sig_us]
    }

    fn from_values(v: [f64; 11]) -> Measured {
        Measured { i8_us: v[0], row_us: v[1], dir_us: v[2], cl_us: v[3], size_us: v[4], vis_us: v[5], grp_us: v[6], anc_us: v[7], cut_us: v[8], und_us: v[9], sig_us: v[10] }
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
        let mut v = [f64::NAN; 11];
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

    /// A cluster of `rows` rows on the index path, `c` of its visible rows left by its sorted range and a share `u`
    /// of those expected above the running k-th best (its group set up, the anchor found): nothing when it leaves none.
    pub fn index(&self, rows: usize, c: usize, u: f64) -> f64 {
        if c == 0 { 0.0 } else { self.cl_us + self.size_us * rows as f64 + (self.vis_us + self.und_us * u) * c as f64 }
    }

    /// A scan of `nv` visible rows.
    pub fn scan(&self, nv: usize) -> f64 {
        self.row_us * nv as f64
    }

    /// The exact planner's rule in rows (`Plan::r_rows` R0, `r_kappa` kappa) that these constants give for an index
    /// whose clusters hold about `typical` rows: a cluster of S rows holding v visible ones is cheaper read directly
    /// when dir + row v <= cl + size S + vis v (every visible row left by the sorted range, which the rule cannot
    /// see, and decided by the levels: an exact query's answers are few), that is when v <= v*(S) = (cl - dir +
    /// size S) / (row - vis). The
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

/// The visible rows of cluster ci that its sorted range at a running k-th best `t` leaves (`sorted_range2`; `qm` its
/// centre score in f64), the anchor found: the rows its index path reads, the rest dropped unread.
pub fn cut_rows(ix: &Index2, pr: &Prep2, vis: Option<&Vis2>, ci: usize, qm: f64, t: f64) -> usize {
    let (u0, u1, _, _) = sorted_range2(ix, ci, &cut_params2(ix, ci, pr, qm), t);
    match vis {
        None => u1 - u0,
        Some(v) => v.rows.partition_point(|&r| (r as usize) < u1) - v.rows.partition_point(|&r| (r as usize) < u0),
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
///   `T_SHARES` (one row in s, by a hash of the row), the `T_CLUSTERS` clusters with the largest centre scores and
///   `T_CLUSTERS` more spread evenly over the rest by centre score, each read directly and on the index path at two
///   running k-th bests (the 10th and the 100th best of the first clusters' visible rows: a probe's at its end and
///   before it), so that the rows the index path's sorted range leaves run from all of a near cluster's to none of a
///   far one's; and each one's index path priced (`cut_us`) and sigma computed (`sig_us`);
/// - `T_GROUPS` group setups and one anchor search per pseudo-query, each on a fresh query;
/// - every timing the fastest of `T_REPS`, each after reading twice the core's L2 (`evict_bytes`), and at least
///   `T_FLOOR_US`.
///
/// What sets the fitted constants' accuracy: the samples (`T_QUERIES` x 4 shares x 2 thresholds x 2 `T_CLUSTERS` per
/// path; the index path's only where its sorted range leaves a row) and their spread in cluster size, visible rows
/// and rows left (cl and size S are told apart only by clusters of different sizes; `Measured::rule` relies on their
/// sum at the typical size, which is well determined); and the eviction (with the caches warm a direct read looks
/// cheaper than a query's first read of the cluster). The repetitions and the floor
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

/// Drops the stored vectors of rows [r0, r1) from every cache level (on x86-64; elsewhere nothing), so that a timed
/// read of a cluster finds them in memory, as a query reaching the cluster for the first time does.
fn flush_rows(ix: &Index2, r0: usize, r1: usize) {
    #[cfg(target_arch = "x86_64")]
    for r in r0..r1 {
        let row = ix.full_row_f16(r);
        let (a, b) = (row.as_ptr() as usize, row.as_ptr() as usize + 2 * row.len());
        let mut line = a & !63;
        while line < b {
            unsafe { core::arch::x86_64::_mm_clflush(line as *const u8) };
            line += 64;
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = (ix, r0, r1);
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
    let (mut grp, mut anc, mut cut, mut sig) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for q in &queries {
        for vis in &sets {
            let vis = vis.as_ref();
            let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
            let pr = prep2(ix, q);
            let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
            let mut sp = vec![0f32; ix.order.as_ref().map_or(0, |st| st.r)];
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
            let rest = &live[top.len()..];
            let mut top = top;
            top.extend(spread(rest.len(), T_CLUSTERS).into_iter().map(|i| rest[i].1));
            pr.qms(ix, Some(&top));
            if let Some(st) = ix.order.as_ref() {
                sig.push(fastest(|| (), |_| {
                    for &ci in &top {
                        std::hint::black_box(sigma_of(st, st.rest(ci), n, ci, &pr.q32, qq, &mut sp));
                    }
                }) / top.len() as f64);
            }
            if ix.nukey.is_empty() {
                pr.anc_among(ix, Some(&top));
                cut.push(fastest(|| (), |_| {
                    for &ci in &top {
                        std::hint::black_box(cut_rows(ix, &pr, vis, ci, ix.qm(&pr.q64, ci), ts[0]));
                    }
                }) / top.len() as f64);
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
                        flush_rows(ix, ix.cl[ci].r0, ix.cl[ci].r1);
                        let mut r = KnnReader::new(ix, &pr, vis, 10, plan, Stats::new(nl));
                        r.prime(t);
                        r
                    };
                    td.push(fastest(reader, |r| {
                        r.visit(ci, KnnPath::Direct);
                        r.kth();
                    }));
                    xd.push(vec![1.0, nv as f64]);
                    let c = if ix.nukey.is_empty() { cut_rows(ix, &pr, vis, ci, pr.qm(ix, ci), t) } else { nv };
                    if c > 0 {
                        let u = ix.order.as_ref().map_or(1.0, |st| above(t, pr.qm(ix, ci), sigma_of(st, st.rest(ci), n, ci, &pr.q32, qq, &mut sp)));
                        ti.push(fastest(reader, |r| {
                            r.visit(ci, KnnPath::Index);
                            r.kth();
                        }));
                        xi.push(vec![1.0, s as f64, c as f64, c as f64 * u]);
                    }
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
    Some(Measured { i8_us, row_us: d[1], dir_us, cl_us: i[0], size_us: i[1], vis_us: i[2], grp_us: mean(&grp), anc_us: mean(&anc), cut_us: mean(&cut), und_us: i[3], sig_us: mean(&sig) })
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
pub const COMPUTE_KEY_VERSION: u32 = 4;

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
/// instead), "exact" (a threshold query handed to the exact one), "sparse" (a threshold query that is the exact one
/// from the start, on a visible set the exact query scans first; nothing else done), "empty". `read`: the clusters the probe read, in
/// order (not those a race scanned); `priced`: the clusters given a sigma, in order. Costs are modelled
/// (`Measured`): `fixed` the order's (centre scores; k-NN's product with the pooled directions; a threshold query's
/// anchor search), `cost` what the query paid in all, `scan_cost` a scan of the visible rows, `exact_cost`
/// (threshold) every cluster the support table leaves open, at its cheaper path. `projected`: a k-NN race taken
/// because the reads projected cost more than scanning the rows not yet read.
#[derive(Clone, Debug, Default)]
pub struct AdaptiveInfo {
    pub path: &'static str,
    pub read: Vec<u32>,
    pub priced: Vec<u32>,
    pub direct: usize,
    pub index: usize,
    pub groups: usize,
    pub spreads: usize,
    pub fixed: f64,
    pub cost: f64,
    pub scan_cost: f64,
    pub exact_cost: f64,
    pub projected: bool,
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
    /// per cluster, the least variance along a direction it holds (sigma >= sqrt(low |q|^2))
    low: Vec<f64>,
    /// the pooled within-cluster covariance, whose share of its trace along the query scales the stream's radii
    pooled: super::order::Pooled,
    /// the k-NN order's multiplier z(1, v) = Phi^-1(v / (v + 1)) for v = 0 to the largest cluster's rows: the expected
    /// best of a cluster's v visible rows' scores in units of their spread (`z_best`)
    ztab: Vec<f64>,
}

/// Gbar(x) = P(Z > x) for a standard normal Z (erfc by its Chebyshev fit, relative error under 1.2e-7).
pub fn norm_sf(x: f64) -> f64 {
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.5 * z);
    let poly = -z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))));
    let erfc = t * poly.exp();
    if x >= 0.0 { 0.5 * erfc } else { 1.0 - 0.5 * erfc }
}

/// The share of a cluster's rows expected to score above t: Gbar((t - mu) / sigma) (0 or 1 when sigma is 0).
pub fn above(t: f64, mu: f64, sigma: f64) -> f64 {
    if sigma > 0.0 { norm_sf((t - mu) / sigma) } else { (mu >= t) as u8 as f64 }
}

/// sigma_c(q) of cluster ci from the stored directions `st` and its rest `rest` (`OrderState::rest`), the query's f32
/// values and |q|^2 (`p` scratch of r values).
pub fn sigma_of(st: &super::order::OrderState, rest: f64, n: usize, ci: usize, q32: &[f32], qq: f64, p: &mut [f32]) -> f64 {
    let r = st.r;
    unsafe { crate::kernel::gemv_i8(q32, st.dirs(ci, r, n), n, &mut p[..r]) };
    let (mut v, mut pp) = (0f64, 0f64);
    for ((&pj, &sc), &l) in p[..r].iter().zip(&st.dsc[ci * r..(ci + 1) * r]).zip(&st.lam[ci * r..(ci + 1) * r]) {
        let x = pj as f64 * sc as f64;
        v += l as f64 * x * x;
        pp += x * x;
    }
    (v + rest * (qq - pp).max(0.0)).max(0.0).sqrt()
}

/// z(1, n): the expected best of n normal draws in units of their spread, as the normal quantile of the best draw's
/// expected rank, n / (n + 1) (0 for one draw; 3.29 for 2,000).
pub fn z_best(n: usize) -> f64 {
    if n == 0 { 0.0 } else { inv_norm(n as f64 / (n as f64 + 1.0)) }
}

/// Per cluster, the costs a query charges as it reads: a group's setup once, the anchor search once. A group's setup
/// serves every later cluster of the group, which a choice cluster by cluster does not see, so it is bought as skis
/// are: while the group is not set up, its clusters are read directly and the savings the index path would have given
/// them (`lost`) are summed, and the cluster whose saving takes the sum to the setup's cost sets the group up. A group
/// set up then costs at most twice what the best choice for its clusters read would have.
struct Charges<'a> {
    m: &'a Measured,
    ix: &'a Index2,
    pr: &'a Prep2,
    anchored: bool,
    gset: Vec<bool>,
    lost: Vec<f64>,
}

impl<'a> Charges<'a> {
    fn new(m: &'a Measured, ix: &'a Index2, pr: &'a Prep2, anchored: bool) -> Charges<'a> {
        Charges { m, ix, pr, anchored, gset: vec![false; ix.groups.len()], lost: vec![0.0; ix.groups.len()] }
    }

    /// The path for cluster ci, `nv` of its rows visible, `c` of them left by its sorted range (`cut_rows`; `nv`
    /// when no k-th best cuts it) and a share `u` of those expected above the running k-th best (`above`), given what is already set up (`anchors` the anchors a first search would score), its
    /// cost, and whether it sets up a group; marks what the index path sets up. A range that leaves no row needs no
    /// group.
    fn choose(&mut self, ci: usize, nv: usize, c: usize, u: f64, anchors: usize) -> (KnnPath, f64, bool) {
        let (m, ix) = (self.m, self.ix);
        let rows = ix.cl[ci].r1 - ix.cl[ci].r0;
        let g = ix.cl[ci].g;
        let gnew = c > 0 && !self.gset[g] && self.pr.gq[g].get().is_none();
        let idx = m.index(rows, c, u) + if self.anchored { 0.0 } else { m.anc_us * anchors as f64 };
        let dir = m.direct(nv);
        let save = dir - idx;
        if save > 0.0 && (!gnew || self.lost[g] + save >= m.grp_us) {
            self.gset[g] = true;
            (KnnPath::Index, idx + if gnew { m.grp_us } else { 0.0 }, gnew)
        } else {
            self.lost[g] += save.max(0.0);
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
        let rest: Vec<f64> = (0..ix.k()).map(|c| st.rest(c)).collect();
        let top = (0..ix.k()).map(|c| st.lam.get(c * r).map_or(0.0, |&l| l as f64).max(rest[c])).collect();
        let low = (0..ix.k()).map(|c| st.lam[c * r..(c + 1) * r].iter().fold(rest[c], |a, &l| a.min(l as f64)) * (1.0 - 1e-9)).collect();
        let sizes: Vec<usize> = ix.cl.iter().map(|c| c.r1 - c.r0).collect();
        let pooled = super::order::Pooled::new(&st, &sizes, ix.n());
        let ztab = (0..=sizes.iter().copied().max().unwrap_or(0)).map(z_best).collect();
        Ok(Adaptive { m, centres: Approx::centres(ix), st, rest, top, low, pooled, ztab })
    }

    /// sigma_c(q) of cluster ci from its stored directions (`p` scratch of r values), the query's f32 values and |q|^2.
    fn sigma(&self, ix: &Index2, ci: usize, q32: &[f32], qq: f64, p: &mut [f32]) -> f64 {
        sigma_of(&self.st, self.rest[ci], ix.n(), ci, q32, qq, p)
    }

    /// The upper bound of cluster ci's key (sigma <= sqrt(top |q|^2)).
    fn bound(&self, ci: usize, qm: &[f64], psi: f64, qq: f64) -> f64 {
        qm[ci] + psi * (self.top[ci] * qq).sqrt()
    }

    /// Every cluster's key mu + psi sigma for the query `q` (for tests).
    #[cfg(test)]
    pub(crate) fn keys(&self, ix: &Index2, q: &[f32], psi: f64) -> Vec<f64> {
        let pr = prep2(ix, q);
        let qm = self.centres.centre_scores(ix, &pr.q32);
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let mut p = vec![0f32; self.st.r];
        (0..ix.k()).map(|ci| qm[ci] + psi * self.sigma(ix, ci, &pr.q32, qq, &mut p)).collect()
    }

    /// The stream's scale of the radii for the query's f32 values `q32` (`p` scratch of r values):
    /// sqrt(q' C q / tr C), C the pooled within-cluster covariance.
    pub(crate) fn stream_scale(&self, q32: &[f32], p: &mut [f32]) -> f64 {
        let qq: f64 = q32.iter().map(|&x| x as f64 * x as f64).sum();
        if self.pooled.tr > 0.0 { (self.pooled.quad(q32, qq, p) / self.pooled.tr).sqrt() } else { 0.0 }
    }

    /// z(1, v) for cluster ci's v visible rows.
    fn z(&self, ix: &Index2, vis: Option<&Vis2>, ci: usize) -> f64 {
        self.ztab[vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize)]
    }

    /// Every cluster's value in the k-NN stream, mu + z rad sqrt(q' C q / tr C), for the query `q` (for tests).
    #[cfg(test)]
    pub(crate) fn stream_keys(&self, ix: &Index2, q: &[f32], vis: Option<&Vis2>) -> Vec<f64> {
        let pr = prep2(ix, q);
        let qm = self.centres.centre_scores(ix, &pr.q32);
        let s = self.stream_scale(&pr.q32, &mut vec![0f32; self.st.r]);
        (0..ix.k()).map(|ci| qm[ci] + self.z(ix, vis, ci) * self.st.rad[ci] * s).collect()
    }

    /// Every cluster's k-NN order key mu + z sigma for the query `q` (for tests).
    #[cfg(test)]
    pub(crate) fn order_keys(&self, ix: &Index2, q: &[f32], vis: Option<&Vis2>) -> Vec<f64> {
        let pr = prep2(ix, q);
        let qm = self.centres.centre_scores(ix, &pr.q32);
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let mut p = vec![0f32; self.st.r];
        (0..ix.k()).map(|ci| qm[ci] + self.z(ix, vis, ci) * self.sigma(ix, ci, &pr.q32, qq, &mut p)).collect()
    }

    /// The cost of one sigma (measured, `Measured::sig_us`).
    fn sigma_cost(&self) -> f64 {
        self.m.sig_us
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

    /// Approximate k-NN with knob `psi` (at least 0; `PSI` by default), as the module's comment says: the clusters in
    /// the order of mu + z sigma (z = z(1, v) of each cluster's v visible rows; the stream by mu + z rad s, `LOOKAHEAD`
    /// priced ahead), whatever psi; psi moves only the stop, when the window's largest mu + psi sigma is below the
    /// running k-th best (a heuristic). Each cluster is read directly or on the index path at the rows its sorted range
    /// at the running k-th best leaves, whichever the measured constants price lower. The probe races a scan of the
    /// visible rows, and hands over to it at once when the reads it projects cost more than scanning the rows not yet
    /// read. psi infinite is the exact query (`knn2`). Returns k items whenever k are visible. `plan` is the index
    /// path's and the exact query's.
    pub fn knn(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (KnnOut2, AdaptiveInfo) {
        assert!(kk > 0, "approximate k-NN needs k of at least 1");
        assert!(psi >= 0.0, "psi is at least 0 (infinity is the exact query), not {psi}");
        let tm = Timer::new();
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        // the centre scores, and the stream's product with the pooled directions
        let fixed = m.i8_us * (k + self.st.r) as f64;
        let mut info = AdaptiveInfo { fixed, scan_cost: m.scan(nvis), ..AdaptiveInfo::default() };
        let out = |ents: Vec<(f64, u32)>, mut st: Stats| {
            st.cyc[P_QUERY] += tick() - t0;
            KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
        };
        if nvis == 0 {
            info.path = "empty";
            return (out(Vec::new(), Stats::new(nl)), info);
        }
        if psi == f64::INFINITY {
            (info.path, info.fixed) = ("exact", 0.0);
            let mut o = knn2(ix, q, kk, vis, plan);
            // a scan counts its rows as read, not touched
            o.st.touched = o.st.touched.max(o.st.full_reads);
            return (o, info);
        }
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
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
        let mut p = vec![0f32; self.st.r];
        // the stream by mu + z rad s, s the pooled covariance's share along the query, and the window of priced
        // clusters, each with its order key mu + z sigma, its stop key mu + psi sigma and its sigma
        let s = self.stream_scale(&pr.q32, &mut p);
        let z = |ci: usize| self.z(ix, vis, ci);
        let mut stream: BinaryHeap<Key> = live.iter().map(|&ci| Key(qm[ci] + z(ci) * self.st.rad[ci] * s, Reverse(ci))).collect();
        let mut window: Vec<(f64, f64, usize, f64)> = Vec::with_capacity(LOOKAHEAD);
        let stop_key = |w: &[(f64, f64, usize, f64)]| w.iter().map(|x| x.1).fold(f64::NEG_INFINITY, f64::max);
        let sc = self.sigma_cost();
        // the projection of what the probe has left to read: the live clusters by mu + psi rad s (the stop key with
        // the stream's spread), largest first, and the running sums of their sigmas and reads at every visible row
        let mut proj: Vec<(f64, usize)> = live.iter().map(|&ci| (qm[ci] + psi * self.st.rad[ci] * s, ci)).collect();
        proj.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut rank = vec![usize::MAX; k];
        let mut sum = Vec::with_capacity(proj.len() + 1);
        sum.push(0.0);
        for (i, &(_, ci)) in proj.iter().enumerate() {
            rank[ci] = i;
            let (rows, nv) = (ix.cl[ci].r1 - ix.cl[ci].r0, nv_of(ci));
            sum.push(sum[i] + sc + m.direct(nv).min(m.index(rows, nv, 1.0)));
        }
        let (mut upto, mut done) = (proj.len(), 0.0);
        st.cyc[P_ORDER] += tick() - ts;
        let mut spent = fixed;
        let mut sig = 0;
        let mut ch = Charges::new(m, ix, &pr, !ix.nukey.is_empty());
        let mut visited = vec![false; k];
        let mut unread = nvis;
        let mut over = false;
        let mut rd = KnnReader::new(ix, &pr, vis, kk, plan, st);
        loop {
            let ts = tick();
            // the running k-th best (the batched direct rows scored first; their time is the reads')
            let kth = rd.kth();
            let cut = kth > f64::NEG_INFINITY && ix.nukey.is_empty();
            if cut && !ch.anchored {
                // the anchor the sorted ranges need, the nearest of the clusters read
                let among: Vec<usize> = info.read.iter().map(|&c| c as usize).collect();
                pr.anc_among(ix, Some(&among));
                spent += m.anc_us * Self::anchors(ix, among.len()) as f64;
                ch.anchored = true;
            }
            // the stop, then the projection against a scan of the rows not yet read, then the window filled from the
            // stream (racing), then the stop again
            if !window.is_empty() && kth > stop_key(&window) {
                rd.st.cyc[P_ORDER] += tick() - ts;
                break;
            }
            if kth > f64::NEG_INFINITY {
                while upto > 0 && proj[upto - 1].0 < kth {
                    upto -= 1;
                    if visited[proj[upto].1] {
                        done -= sum[upto + 1] - sum[upto];
                    }
                }
                if sum[upto] - done > m.scan(unread) {
                    info.projected = true;
                    over = true;
                    rd.st.cyc[P_ORDER] += tick() - ts;
                    break;
                }
            }
            while window.len() < LOOKAHEAD && !stream.is_empty() {
                if spent + sc > cs {
                    over = true;
                    break;
                }
                let Key(_, Reverse(ci)) = stream.pop().unwrap();
                let tsp = tick();
                let sg = self.sigma(ix, ci, &pr.q32, qq, &mut p);
                sig += tick() - tsp;
                spent += sc;
                info.spreads += 1;
                info.priced.push(ci as u32);
                window.push((qm[ci] + z(ci) * sg, qm[ci] + psi * sg, ci, sg));
            }
            if over || window.is_empty() || kth > stop_key(&window) {
                rd.st.cyc[P_ORDER] += tick() - ts;
                break;
            }
            // the window's largest order key (the lower cluster first among equal keys)
            let i = (0..window.len()).max_by(|&a, &b| Key(window[a].0, Reverse(window[a].2)).cmp(&Key(window[b].0, Reverse(window[b].2)))).unwrap();
            let (_, _, ci, sg) = window.swap_remove(i);
            rd.st.cyc[P_ORDER] += tick() - ts;
            let tch = tick();
            let nv = nv_of(ci);
            // the rows its sorted range at the running k-th best leaves, and their share expected above it
            let (c, u) = if cut {
                if spent + m.cut_us > cs {
                    over = true;
                    rd.st.cyc[P_CHOICE] += tick() - tch;
                    break;
                }
                spent += m.cut_us;
                let qmf = pr.qm(ix, ci);
                (cut_rows(ix, &pr, vis, ci, qmf, kth), above(kth, qmf, sg))
            } else {
                (nv, 1.0)
            };
            let (path, cost, gnew) = ch.choose(ci, nv, c, u, Self::anchors(ix, info.read.len() + 1));
            if spent + cost > cs {
                over = true;
                rd.st.cyc[P_CHOICE] += tick() - tch;
                break;
            }
            info.read.push(ci as u32);
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
            unread -= nv;
            if rank[ci] < upto {
                done += sum[rank[ci] + 1] - sum[rank[ci]];
            }
            rd.visit(ci, path);
        }
        rd.st.cyc[P_SPREAD] += sig;
        rd.st.cyc[P_ORDER] -= sig.min(rd.st.cyc[P_ORDER]);
        rd.st.cyc[C_SPREADS] += (info.spreads * self.st.r) as u64;
        rd.st.cyc[C_DIRECT] += info.direct as u64;
        rd.st.cyc[C_INDEX] += info.index as u64;
        rd.st.cyc[C_GROUPS] += info.groups as u64;
        rd.st.cyc[C_VISITED] += info.read.len() as u64;
        info.path = "probe";
        if over {
            // the rows not yet read, scanned
            info.path = "race";
            for &ci in &live {
                if !visited[ci] {
                    rd.visit(ci, KnnPath::Direct);
                    spent += m.scan(nv_of(ci));
                }
            }
        }
        info.cost = spent;
        let (ents, st) = rd.finish();
        (out(ents, st), info)
    }

    /// The approximate threshold query at t with knob `psi` (at least 0; `PSI` by default), as the module's comment
    /// says: the visible rows scoring at least t of the clusters the support table takes in and of those whose key
    /// reaches t, ascending; a subset of the exact answer, and the exact answer when the query hands over
    /// (`AdaptiveInfo::path` "scan", "exact" or "sparse"; always when psi is infinite and the support table leaves a
    /// cluster open, and on a visible set `plan`'s exact query scans first). `plan` is the reads' (and the exact
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
        // a visible set the exact query scans first: the exact query, before anything else
        if plan.threshold_scans_first(ix, vis) {
            info.path = "sparse";
            (info.fixed, info.cost, info.exact_cost) = (0.0, cs, cs);
            let out = threshold2(ix, &pr, t, &opts);
            return (done(out, &st), info);
        }
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
        // the decision first from the bounds: sqrt(low |q|^2) <= sigma <= sqrt(top |q|^2) puts each open cluster
        // surely in (its key's lower bound reaches t), surely out (its upper bound does not), or undecided
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let (sc, mut p, mut sig) = (self.sigma_cost(), vec![0f32; self.st.r], 0);
        let (mut sure, mut undecided) = (Vec::new(), Vec::new());
        for &ci in &open {
            if all || qm[ci] + psi * (self.low[ci] * qq).sqrt() >= t {
                sure.push(ci);
            } else if self.bound(ci, &qm, psi, qq) >= t {
                undecided.push((self.bound(ci, &qm, psi, qq), ci));
            }
        }
        undecided.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        st.cyc[P_ORDER] += tick() - ts;
        // the reads' costs (each cluster's cheaper path, a group's setup once), against a scan and against the exact
        // query (every open cluster)
        let tch = tick();
        let mut ch = Charges::new(m, ix, &pr, true);
        let mut paths: Vec<(usize, KnnPath, bool)> = Vec::new();
        let mut cp = 0.0;
        for &ci in &sure {
            let (path, cost, gnew) = ch.choose(ci, nv_of(ci), nv_of(ci), 1.0, 0);
            cp += cost;
            paths.push((ci, path, gnew));
        }
        info.exact_cost = {
            let mut cx = Charges::new(m, ix, &pr, true);
            open.iter().map(|&ci| cx.choose(ci, nv_of(ci), nv_of(ci), 1.0, 0).1).sum()
        };
        let other = cs.min(info.exact_cost);
        st.cyc[P_CHOICE] += tick() - tch;
        let hand = |spent: f64, st: &mut Stats, info: &mut AdaptiveInfo| {
            let exact = info.exact_cost <= cs;
            info.path = if exact { "exact" } else { "scan" };
            info.cost = spent + other;
            if exact { threshold2(ix, &pr, t, &opts) } else { scan(st) }
        };
        // reads as dear as the exact query's (every open cluster surely holds answers), or the sure reads and the
        // undecided clusters' sigmas costing what a scan or the exact query would: hand over, before any sigma
        if (!open.is_empty() && sure.len() == open.len()) || cp + undecided.len() as f64 * sc >= other {
            let out = hand(fixed, &mut st, &mut info);
            return (done(out, &st), info);
        }
        // the undecided clusters' sigmas, the largest bound first, racing the hand-over: before each, when the sigmas
        // spent and the reads taken would pass what a scan or the exact query costs, hand over
        let mut spent = fixed;
        let mut take = sure;
        let ts = tick();
        for &(_, ci) in &undecided {
            if spent - fixed + cp + sc > other {
                st.cyc[P_SPREAD] += sig;
                st.cyc[P_ORDER] += tick() - ts - sig;
                let out = hand(spent, &mut st, &mut info);
                return (done(out, &st), info);
            }
            let tsp = tick();
            let s = self.sigma(ix, ci, &pr.q32, qq, &mut p);
            sig += tick() - tsp;
            spent += sc;
            info.spreads += 1;
            if qm[ci] + psi * s >= t {
                take.push(ci);
                let (path, cost, gnew) = ch.choose(ci, nv_of(ci), nv_of(ci), 1.0, 0);
                cp += cost;
                paths.push((ci, path, gnew));
            }
        }
        st.cyc[P_SPREAD] += sig;
        st.cyc[P_ORDER] += tick() - ts - sig;
        // the reads left against a scan and the exact query (the sigmas are spent either way)
        if other < cp || (!open.is_empty() && info.exact_cost <= cp) {
            let out = hand(spent, &mut st, &mut info);
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
        let m = Measured { i8_us: 0.02, row_us: 0.05, dir_us: 1.2, cl_us: 15.0, size_us: 0.004, vis_us: 0.015, grp_us: 56.0, anc_us: 0.08, cut_us: 0.0, und_us: 0.0, sig_us: 0.0 };
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
