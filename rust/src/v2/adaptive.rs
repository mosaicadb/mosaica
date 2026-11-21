//! The approximate queries: adaptive k-NN and threshold queries over the clusters, with one raw knob psi >= 0.
//!
//! Each cluster holding a visible row has the key u_c = mu_c + psi sigma_c(q): mu_c its centre score (the int8
//! centres), sigma_c(q) = sqrt(q' S_c q) the spread of its rows' scores along the query, S_c the cluster's covariance
//! as its stored principal directions (`order::OrderState`) and an isotropic rest. u_c is an optimistic estimate of the
//! cluster's best score, on the score scale, and psi says how optimistic (theory: the best multiplier is 3 to 4 at
//! every unit size and dataset, `hierarchy/theory.md` 3.4). Every sigma is at most sqrt(top_c) |q|, top_c the
//! cluster's largest variance along a direction, so b_c = mu_c + psi sqrt(top_c) |q| bounds u_c from above.
//!
//! - k-NN is the exact query (`knn2`) when psi is infinite. Otherwise it reads the clusters in an order that does not
//!   depend on psi, and psi moves only where it stops. The order is by o_c = mu_c + lambda sigma_c(q), lambda = delta +
//!   `ORDER_C` rho, where delta and rho are the mean and spread of the visible rows' scores in each cluster's own
//!   units, z = (s - mu_c) / sigma_c, over the rows of every cluster the probe has read directly so far: 0 and 1 when
//!   the visible rows sit in their clusters as all rows do, other values when the visible set favours some part of
//!   each cluster. Until two rows have been scored they are 0 and 1. Under a visible set made of regions the shift
//!   differs from cluster to cluster, so one cluster's says little of the next's, and the estimate pools them all.
//!   The clusters come from a stream by mu_c + lambda rad_c sqrt(q' C q / tr C) (the lower cluster first among equal
//!   values; re-keyed at the current lambda each time the rows behind the estimate have doubled), rad_c the cluster's
//!   RMS radius and C the
//!   pooled
//!   within-cluster covariance (`order::Pooled`, sum_c n_c S_c / N as its top directions and an isotropic rest): the
//!   spread the cluster would have if it were shaped as the clusters are on average. A part of q along which no
//!   cluster varies leaves it unchanged (it moves every mu_c alike when the centres agree along it), where
//!   rad_c |q| / sqrt(n0) would grow with it. The next `LOOKAHEAD` of the stream are priced (sigma computed) in a
//!   window; the probe reads the window's cluster of largest o_c, and stops when the window's largest
//!   mu_c + (delta + psi rho) sigma_c is below the running k-th best (minus infinity until k items are held: k items
//!   are returned whenever k are visible). So a
//!   smaller psi reads and prices the first of what a larger one does. The stop is a heuristic: a cluster further
//!   down the stream, not yet priced, may still have a u above the k-th best; ruling that out exactly prices every
//!   cluster on real data (the bound b_c is about 5 times the spread). A cluster gets its sigma only when that costs
//!   less than reading it directly; else rad_c s stands for it in both keys. A cluster of at most r visible rows (r
//!   the stored directions: its sigma would cost about as much as reading it) takes no sigma, window slot or pricing:
//!   it is read into the direct batch as it leaves the stream, unless its stop key with rad_c s is below the running
//!   k-th best. Each cluster is read directly or on the
//!   index path, whichever the measured constants price lower: the index path at the rows its sorted range at the
//!   running k-th best leaves; a new group's setup is bought once the savings its clusters read directly would have
//!   had reach its cost (set up when that cluster is read). There is no race: with direct reads batched, the worst a
//!   probe does on a sparse visible set is reading the visible rows cluster by cluster, about a scan.
//! - a threshold query at t reads the clusters whose u reaches t, after the support table has dropped those wholly
//!   below t and taken in those wholly above it without a read; each is read exactly as the exact query reads it. The
//!   decision comes first from the bounds sqrt(low_c) |q| <= sigma_c <= sqrt(top_c) |q| (low_c the cluster's least
//!   variance along a direction it holds): a cluster whose key's lower bound reaches t is read, one whose upper bound
//!   does not is not, and only the rest get their sigmas (the largest bound first). With psi infinite every open
//!   cluster is read, which is the exact answer. On a visible set the exact query scans before
//!   anything else (`Plan::threshold_scans_first`: every cluster holding a visible row could be scored directly, in
//!   sum) the query is the exact one from the start, before the centre scores (path "sparse"): there every cluster
//!   read costs more than its rows' share of that scan, and the bounds decide too little for the fixed work and the
//!   sigmas to pay off (the paired runs at 1% visible: 1.04 to 1.29 times the exact query's time to recall 0.9 and
//!   0.95, but for yahoo random-1). It is decided from the visible set alone, so every query on it hands over.
//! - both queries price a read by one small model (`Measured`): a direct read a fixed cost and a cost per visible row,
//!   the index path a cost for the sorted range, a fixed cost and a cost per row the range leaves, the row costs at
//!   the cluster's visible share; a group's setup; a sigma. A threshold query prices the index path as k-NN does: at
//!   the rows its sorted range at t leaves, and the group's setup when the group is new. Both paths give the same
//!   answers: the constants change k-NN's answer only through which clusters get a sigma, and a threshold query's not
//!   at all. They are timed on this index at open and cached with it (`measured_constants`).

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
/// The k-NN order's multiplier of the visible rows' spread: the order is mu + (delta + `ORDER_C` rho) sigma. Rows to
/// recall are flat in it from 2.5 to 3.5 over k 1 to 1000, three datasets and their visible sets
/// (`bench/suite/notes/visit-order/results/order-stress/`).
pub const ORDER_C: f64 = 3.0;
/// How far ahead of the probe k-NN looks: the clusters next in the stream that have their sigma, among which the probe
/// reads the largest order key and whose largest key the stop compares with the running k-th best. A structural choice, not
/// fitted: it costs this many sigmas beyond those of the clusters read, and a cluster can overtake by its spread only
/// within it.
pub const LOOKAHEAD: usize = 8;

/// The visible shares the cost model is timed at (one row in s): a cluster's read is priced at its own visible share
/// nv / S, between the two nearest of these (linear in log s).
pub const SHARES: [u64; 4] = [1, 4, 16, 64];

/// The cost model of the approximate queries, in microseconds on the calling thread, timed on this index at open
/// (`measure`), each constant a cost of one thing the reads do:
/// - `dir_us`: a cluster read directly, beyond its rows (its visible rows found and batched);
/// - `row_us[s]`: one visible row read directly (its stored vector scored), at visible share 1 / `SHARES[s]`;
/// - `cl_us`: a cluster opened on the index path (its setup, tables and levels' fixed work), beyond its rows;
/// - `cut_row_us[s]`: one visible row its sorted range leaves, through the levels and, where it survives them, its
///   stored vector, at visible share 1 / `SHARES[s]`;
/// - `cut_us`: a cluster's sorted range at a k-th best (its centre score in f64, the range, the visible rows in it);
/// - `grp_us`: a group's setup as k-NN's index path pays it (`KnnReader`: the query's projections on the group's
///   basis, `get_gq2`, then the group's rotations alone, `get_rot`). Threshold queries charge it too (`Charges`),
///   though `run_planned` sets their groups up together (`setup_groups`, one dgemm per block for all of them), whose
///   cost per group it does not time;
/// - `sig_us`: one cluster's sigma (its r directions' products with the query).
///
/// Every `row_us` is above 0, the others at least 0, all finite (`check`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub dir_us: f64,
    pub row_us: [f64; 4],
    pub cl_us: f64,
    pub cut_row_us: [f64; 4],
    pub cut_us: f64,
    pub grp_us: f64,
    pub sig_us: f64,
}

const NAMES: [&str; 13] = ["dir_us", "row_us_1", "row_us_4", "row_us_16", "row_us_64", "cl_us", "cut_row_us_1", "cut_row_us_4", "cut_row_us_16", "cut_row_us_64", "cut_us", "grp_us", "sig_us"];

impl Measured {
    fn values(&self) -> [f64; 13] {
        let (r, c) = (self.row_us, self.cut_row_us);
        [self.dir_us, r[0], r[1], r[2], r[3], self.cl_us, c[0], c[1], c[2], c[3], self.cut_us, self.grp_us, self.sig_us]
    }

    fn from_values(v: [f64; 13]) -> Measured {
        Measured { dir_us: v[0], row_us: [v[1], v[2], v[3], v[4]], cl_us: v[5], cut_row_us: [v[6], v[7], v[8], v[9]], cut_us: v[10], grp_us: v[11], sig_us: v[12] }
    }

    /// Whether the constants can price a query: every one finite and at least 0, and a row read directly above 0 (a
    /// model in which reading costs nothing has no choice to make).
    pub fn check(&self) -> Result<(), String> {
        for (k, v) in NAMES.iter().zip(self.values()) {
            let pos = k.starts_with("row_us");
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
        let mut v = [f64::NAN; 13];
        for (i, k) in NAMES.iter().enumerate() {
            let x = text.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('='))).ok_or_else(|| format!("no {k}"))?;
            v[i] = x.trim().parse::<f64>().map_err(|_| format!("{k}={x} is not a number"))?;
        }
        let m = Measured::from_values(v);
        m.check()?;
        Ok(m)
    }

    /// A per-row cost of `by_share` at a cluster's visible share nv / rows: between the two nearest timed shares,
    /// linear in log share; the end ones beyond them.
    fn at_share(by_share: &[f64; 4], nv: usize, rows: usize) -> f64 {
        let x = ((rows.max(1) as f64 / nv.max(1) as f64).ln() / 4f64.ln()).clamp(0.0, 3.0);
        let i = (x as usize).min(2);
        by_share[i] + (by_share[i + 1] - by_share[i]) * (x - i as f64)
    }

    /// A cluster of `rows` rows read directly, `nv` of them visible.
    pub fn direct(&self, nv: usize, rows: usize) -> f64 {
        self.dir_us + Self::at_share(&self.row_us, nv, rows) * nv as f64
    }

    /// A cluster of `rows` rows, `nv` of them visible, on the index path when its sorted range leaves `c` of them (its
    /// group set up): the range, and the cluster opened only when the range leaves a row.
    pub fn index(&self, c: usize, nv: usize, rows: usize) -> f64 {
        self.cut_us + if c == 0 { 0.0 } else { self.cl_us + Self::at_share(&self.cut_row_us, nv, rows) * c as f64 }
    }

    /// The exact planner's rule in rows (`Plan::r_rows` R0, `r_kappa` kappa 0) these constants give: every visible
    /// row of a cluster left by its range (which the rule cannot see), at visible share 1, a cluster is cheaper read
    /// directly when dir + row v <= cut + cl + cut_row v, that is when v <= (cut + cl - dir) / (row - cut_row). None
    /// when the index path's rows cost no less than direct ones (every cluster would be scored directly).
    pub fn rule(&self) -> Option<(usize, f64)> {
        let d = self.row_us[0] - self.cut_row_us[0];
        (d > 0.0).then(|| (((self.cut_us + self.cl_us - self.dir_us) / d).max(0.0).round() as usize, 0.0))
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

/// The timing procedure (`measure`), structural choices rather than tuned values: `T_QUERIES` pseudo-queries (corpus
/// rows spread over the index), and for each, under every visible share of `SHARES` (one row in s, by a hash of the
/// row), the `T_CLUSTERS` clusters with the largest centre scores and `T_CLUSTERS` more spread evenly over the rest,
/// each read on the index path and directly (once each, in that order and in sequence, as a query reads clusters for
/// the first time: no repetition, no eviction) at two running k-th bests, the 10th and the 100th best of the first
/// clusters' visible rows; each one's sorted range and sigma, and `T_GROUPS` group setups on fresh queries. The two
/// read costs are fitted to those reads (relative least squares, weights at least 0), the rest are medians.
const T_QUERIES: usize = 8;
const T_CLUSTERS: usize = 16;
const T_GROUPS: usize = 4;

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

/// One timing of the cost model's constants on `ix` (`T_QUERIES` and the rest above). None when no pseudo-query holds
/// enough visible rows to time.
fn measure_once(ix: &Index2) -> Option<Measured> {
    let (n, k, nl) = (ix.n(), ix.k(), ix.meta.nl());
    let queries: Vec<Vec<f32>> = spread(ix.nrows, T_QUERIES).into_iter().filter_map(|row| {
        let x: Vec<f64> = ix.full_row_f16(row).iter().map(|&h| f16_to_f32(h) as f64).collect();
        let xn = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        (xn >= 0.5).then(|| x.iter().map(|v| (v / xn) as f32).collect())
    }).collect();
    // the visible sets: every row, then one in s by a hash of the row
    let sets: Vec<Option<Vis2>> = SHARES.iter().map(|&s| {
        (s > 1).then(|| {
            let bm: RoaringBitmap = (0..ix.nrows).filter(|&r| ((r as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32).is_multiple_of(s)).map(|r| ix.row_ent[r]).collect();
            Vis2::new(ix, &bm)
        })
    }).collect();
    let plan = Plan::default();
    let med = |v: &mut Vec<f64>| -> f64 {
        v.sort_by(|a, b| a.total_cmp(b));
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let us = |t: Instant| t.elapsed().as_secs_f64() * 1e6;
    let (mut xd, mut td, mut xi, mut ti) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut grp, mut cut, mut sig) = (Vec::new(), Vec::new(), Vec::new());
    for q in &queries {
        for (si, vis) in sets.iter().enumerate() {
            let vis = vis.as_ref();
            let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
            let pr = prep2(ix, q);
            let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
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
            let mut sample = top;
            sample.extend(spread(rest.len(), T_CLUSTERS).into_iter().map(|i| rest[i].1));
            if let Some(st) = ix.order.as_ref() {
                let mut p = vec![0f32; st.r];
                let t = Instant::now();
                for &ci in &sample {
                    std::hint::black_box(sigma_of(st, st.rest(ci), n, ci, &pr.q32, qq, &mut p));
                }
                sig.push(us(t) / sample.len() as f64);
            }
            let cuts = ix.nukey.is_empty();
            if cuts {
                pr.anc_among(ix, Some(&sample[..T_CLUSTERS.min(sample.len())]));
                let t = Instant::now();
                for &ci in &sample {
                    std::hint::black_box(cut_rows(ix, &pr, vis, ci, ix.qm(&pr.q64, ci), ts[0]));
                }
                cut.push(us(t) / sample.len() as f64);
            }
            let mut gs: Vec<usize> = sample.iter().map(|&ci| ix.cl[ci].g).collect();
            gs.sort_unstable();
            gs.dedup();
            setup_groups(ix, &pr, &gs, &mut Stats::new(nl));
            for &t in &ts {
                for &ci in &sample {
                    let nv = nv_of(ci);
                    let c = if cuts { cut_rows(ix, &pr, vis, ci, pr.qm(ix, ci), t) } else { nv };
                    let reader = || {
                        let mut r = KnnReader::new(ix, &pr, vis, 10, plan, Stats::new(nl));
                        r.prime(t);
                        r
                    };
                    let onehot = |x: f64| -> Vec<f64> { (0..SHARES.len()).map(|s| if s == si { x } else { 0.0 }).collect() };
                    if c > 0 {
                        let mut r = reader();
                        let t0 = Instant::now();
                        r.visit(ci, KnnPath::Index);
                        r.kth();
                        ti.push(us(t0).max(1e-3));
                        xi.push([vec![1.0], onehot(c as f64)].concat());
                    }
                    let mut r = reader();
                    let t0 = Instant::now();
                    r.visit(ci, KnnPath::Direct);
                    r.kth();
                    td.push(us(t0).max(1e-3));
                    xd.push([vec![1.0], onehot(nv as f64)].concat());
                }
            }
            if vis.is_none() {
                for &g in gs.iter().take(T_GROUPS) {
                    // as the index path sets a group up: its projections, then its rotations once a randomised
                    // level reads them
                    let (p2, mut st) = (prep2(ix, q), Stats::new(nl));
                    let t = Instant::now();
                    let gq = get_gq2(ix, &p2, g, &mut st);
                    get_rot(ix, &p2, gq, &mut st);
                    grp.push(us(t));
                }
            }
        }
    }
    if td.is_empty() {
        return None;
    }
    let (d, i) = (fit_rel(&xd, &td), fit_rel(&xi, &ti));
    let i = if xi.is_empty() { vec![0.0; 1 + SHARES.len()] } else { i };
    Some(Measured {
        dir_us: d[0],
        row_us: [d[1], d[2], d[3], d[4]],
        cl_us: i[0],
        cut_row_us: [i[1], i[2], i[3], i[4]],
        cut_us: med(&mut cut),
        grp_us: med(&mut grp),
        sig_us: med(&mut sig),
    })
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
pub const COMPUTE_KEY_VERSION: u32 = 9;

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
                        m.rule().map_or("every cluster scored directly".into(), |(r, kp)| format!("r_rows={r} r_kappa={kp:.3}"))));
            Ok(m)
        }
        Err(e) => {
            log(format!("[approx] no measured constants: {e}"));
            Err(e)
        }
    }
}

/// What one approximate query did. `path`: "probe" (k-NN stopped by its knob; a threshold query's clusters read),
/// "scan" (k-NN with no more visible rows than k, scanned), "exact" (k-NN at psi infinite: the exact query), "sparse"
/// (a threshold query that is the exact one from
/// the start, on a visible set the exact query scans first; nothing else done), "empty". `read`: the clusters the
/// probe read, in order; `priced`: the clusters given a sigma, in order; k-NN's `lam_read` and `lam_priced`: the order's
/// lambda when each read was chosen and the stream's when each priced cluster left it, `rekeys`: the stream's re-keyings,
/// `delta` and `rho`: the estimates at the end, `batched`: the clusters of at most r visible rows read straight into the
/// direct batch (not in `read`). Costs are modelled (`Measured`): `cost`
/// what the query paid in all; k-NN's `direct_cost`, `index_cost` (its groups' setups included) and `sigma_cost`;
/// a threshold query's `scan_cost` (every visible cluster read directly).
#[derive(Clone, Debug, Default)]
pub struct AdaptiveInfo {
    pub path: &'static str,
    pub read: Vec<u32>,
    pub priced: Vec<u32>,
    pub direct: usize,
    pub index: usize,
    pub groups: usize,
    pub spreads: usize,
    pub cost: f64,
    pub direct_cost: f64,
    pub index_cost: f64,
    pub sigma_cost: f64,
    pub scan_cost: f64,
    pub lam_read: Vec<f64>,
    pub lam_priced: Vec<f64>,
    pub rekeys: usize,
    pub delta: f64,
    pub rho: f64,
    pub batched: usize,
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
    /// per level, the spread of a row's error as a share of its certified half-width, the median over the groups: a
    /// deterministic level's `GroupPlan2::rho`, a randomised level's 1 / c (its half-width is c times the spread of the
    /// row's error, c the certified constant `GroupPlan2::csgl`); k-NN at psi reads each level at psi rho of its
    /// half-width (at most the certified width), psi spreads of the row's error at every level and dimension
    rho: Vec<f64>,
}

/// Per cluster, the path a threshold query reads it on and its cost, as k-NN prices it: the index path at the rows
/// its sorted range at t leaves (found only when the index path could be the cheaper) and its group's setup when the
/// group is new to the query, against the direct read; marks the groups the index path sets up.
struct Charges<'a> {
    m: &'a Measured,
    ix: &'a Index2,
    pr: &'a Prep2,
    vis: Option<&'a Vis2>,
    gset: Vec<bool>,
}

impl<'a> Charges<'a> {
    fn new(m: &'a Measured, ix: &'a Index2, pr: &'a Prep2, vis: Option<&'a Vis2>) -> Charges<'a> {
        Charges { m, ix, pr, vis, gset: vec![false; ix.groups.len()] }
    }

    /// The path for cluster ci, `nv` of its rows visible, at threshold t, its cost, and whether it sets up a group.
    fn choose(&mut self, ci: usize, nv: usize, t: f64) -> (KnnPath, f64, bool) {
        let (m, ix, pr) = (self.m, self.ix, self.pr);
        let rows = ix.cl[ci].r1 - ix.cl[ci].r0;
        let g = ix.cl[ci].g;
        let gnew = !self.gset[g] && pr.gq[g].get().is_none();
        let grp = if gnew { m.grp_us } else { 0.0 };
        let dir = m.direct(nv, rows);
        if dir <= m.cut_us + m.cl_us + grp {
            return (KnnPath::Direct, dir, false);
        }
        // the rows the sorted range leaves (every one on a norm-sorted index, whose ranges need the group set up)
        let c = if ix.nukey.is_empty() { cut_rows(ix, pr, self.vis, ci, pr.qm(ix, ci), t) } else { nv };
        let idx = m.index(c, nv, rows) + if c > 0 { grp } else { 0.0 };
        if idx < dir {
            self.gset[g] |= c > 0;
            (KnnPath::Index, idx, gnew && c > 0)
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
        let rest: Vec<f64> = (0..ix.k()).map(|c| st.rest(c)).collect();
        let top = (0..ix.k()).map(|c| st.lam.get(c * r).map_or(0.0, |&l| l as f64).max(rest[c])).collect();
        let low = (0..ix.k()).map(|c| st.lam[c * r..(c + 1) * r].iter().fold(rest[c], |a, &l| a.min(l as f64)) * (1.0 - 1e-9)).collect();
        let sizes: Vec<usize> = ix.cl.iter().map(|c| c.r1 - c.r0).collect();
        let pooled = super::order::Pooled::new(&st, &sizes, ix.n());
        let (nl, ndl) = (ix.meta.nl(), ix.meta.ndl());
        let rho: Vec<f64> = (0..nl).map(|l| {
            let mut r: Vec<f64> = ix.groups.iter().filter(|gp| gp.cal.is_some())
                .map(|gp| if l < ndl { gp.rho[l] as f64 } else { 1.0 / gp.csgl[l - ndl] as f64 })
                .collect();
            r.sort_by(f64::total_cmp);
            r.get(r.len() / 2).copied().unwrap_or(1.0)
        }).collect();
        Ok(Adaptive { m, centres: Approx::centres(ix), st, rest, top, low, pooled, rho })
    }

    /// Each level's factor on its half-width for a query at psi (`Prep2::dial`): min(1, psi rho_l); 1 (certified) at
    /// psi infinite.
    pub fn dial(&self, psi: f64) -> Vec<f32> {
        self.rho.iter().map(|&r| if psi.is_finite() { (psi * r).min(1.0) as f32 } else { 1.0 }).collect()
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

    /// Every cluster's value in the k-NN stream at the order's `lam`, mu + lam rad sqrt(q' C q / tr C), for the query
    /// `q` (for tests).
    #[cfg(test)]
    pub(crate) fn stream_keys(&self, ix: &Index2, q: &[f32], lam: f64) -> Vec<f64> {
        let pr = prep2(ix, q);
        let qm = self.centres.centre_scores(ix, &pr.q32);
        let s = self.stream_scale(&pr.q32, &mut vec![0f32; self.st.r]);
        (0..ix.k()).map(|ci| qm[ci] + lam * self.st.rad[ci] * s).collect()
    }

    /// The studies' reference keys, mu + z(1, v) rad s (`sigma` false) or mu + z(1, v) sigma, z(1, v) =
    /// Phi^-1(v / (v + 1)) of each cluster's v visible rows (for tests).
    #[cfg(test)]
    pub(crate) fn z_keys(&self, ix: &Index2, q: &[f32], vis: Option<&Vis2>, sigma: bool) -> Vec<f64> {
        let pr = prep2(ix, q);
        let qm = self.centres.centre_scores(ix, &pr.q32);
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let mut p = vec![0f32; self.st.r];
        let s = self.stream_scale(&pr.q32, &mut p);
        (0..ix.k()).map(|ci| {
            let v = vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize) as f64;
            let z = if v == 0.0 { 0.0 } else { inv_norm(v / (v + 1.0)) };
            qm[ci] + z * if sigma { self.sigma(ix, ci, &pr.q32, qq, &mut p) } else { self.st.rad[ci] * s }
        }).collect()
    }

    /// The cost of one sigma (measured, `Measured::sig_us`).
    fn sigma_cost(&self) -> f64 {
        self.m.sig_us
    }

    /// Approximate k-NN with knob `psi` (at least 0; `PSI` by default), as the module's comment says: the exact query
    /// (`knn2`) when psi is infinite; else the clusters in the order of mu + lambda sigma (lambda = delta + `ORDER_C`
    /// rho, from the rows of every cluster read directly so far; the stream by mu + lambda rad s, `LOOKAHEAD`
    /// priced ahead), whatever psi, psi moving only the stop, when the window's largest mu + (delta + psi rho) sigma is
    /// below the running k-th best (a heuristic). A cluster gets its sigma when
    /// that costs less than reading it directly, else rad s stands for it. Each cluster is read directly or on the
    /// index path, whichever the measured constants price lower (`Measured::direct`, `Measured::index` at the rows its
    /// sorted range at the running k-th best leaves; a new group bought once its clusters' foregone savings reach its
    /// setup's cost). Returns k items whenever
    /// k are visible. `plan` is the index path's and the exact query's.
    pub fn knn(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (KnnOut2, AdaptiveInfo) {
        assert!(kk > 0, "approximate k-NN needs k of at least 1");
        assert!(psi >= 0.0, "psi is at least 0 (infinity is the exact query), not {psi}");
        let tm = Timer::new();
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let rows_of = |ci: usize| ix.cl[ci].r1 - ix.cl[ci].r0;
        let nv_of = |ci: usize| vis.map_or(rows_of(ci), |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let mut info = AdaptiveInfo::default();
        let out = |ents: Vec<(f64, u32)>, mut st: Stats| {
            st.cyc[P_QUERY] += tick() - t0;
            KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
        };
        if nvis == 0 {
            info.path = "empty";
            return (out(Vec::new(), Stats::new(nl)), info);
        }
        if psi == f64::INFINITY {
            info.path = "exact";
            let mut o = knn2(ix, q, kk, vis, plan);
            // a scan counts its rows as read, not touched
            o.st.touched = o.st.touched.max(o.st.full_reads);
            return (o, info);
        }
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        if nvis <= kk {
            info.path = "scan";
            st.full_reads += nvis as u64;
            st.touched += nvis as u64;
            return (out(scan_knn(ix, &pr.q16, &pr.q32, pr.qn, kk, vis), st), info);
        }
        // scores below are the scaled query's (Prep2::unit), scaled back on the way out
        let mut pr = pr.unit();
        pr.dial = self.dial(psi);
        let tc = tick();
        let qm = self.centres.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - tc;
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let mut p = vec![0f32; self.st.r];
        // the visible rows' scores in their clusters' units, z = (s - mu) / sigma, over every cluster of the window read
        // directly (count, sum, sum of squares), and from them delta, rho and the order's lambda (0, 1 and ORDER_C until
        // two rows are in); the stream is re-keyed at the current lambda each time the rows behind it have doubled
        let (mut zn, mut zs, mut zss) = (0f64, 0f64, 0f64);
        let (mut delta, mut rho) = (0.0, 1.0);
        let mut lam = ORDER_C;
        let (mut lam_stream, mut zn_stream) = (ORDER_C, 0f64);
        // the stream by mu + lambda rad s, s the pooled covariance's share along the query, and the window of priced
        // clusters with their sigmas; the order key mu + lambda sigma, the stop key mu + (delta + psi rho) sigma
        let s = self.stream_scale(&pr.q32, &mut p);
        let stream_key = |ci: usize, lam: f64| Key(qm[ci] + lam * self.st.rad[ci] * s, Reverse(ci));
        let mut stream: BinaryHeap<Key> = live.iter().map(|&ci| stream_key(ci, lam)).collect();
        let mut window: Vec<(usize, f64)> = Vec::with_capacity(LOOKAHEAD);
        let stop_key = |w: &[(usize, f64)], delta: f64, rho: f64| w.iter().map(|&(ci, sg)| qm[ci] + (delta + psi * rho) * sg).fold(f64::NEG_INFINITY, f64::max);
        st.cyc[P_ORDER] += tick() - ts;
        let mut sig = 0;
        let mut anchored = !ix.nukey.is_empty();
        // a group new to this query, and the least the index path can cost a cluster beyond its rows
        let new_group = |g: usize| pr.gq[g].get().is_none();
        // a new group's setup is bought as skis are: while it is not set up, the savings the index path would have
        // given its clusters read directly are summed (`lost`), and the cluster whose saving takes the sum to the
        // setup's cost sets it up (at most twice the best purchase)
        let mut lost = vec![0f64; ix.groups.len()];
        let mut rd = KnnReader::new(ix, &pr, vis, kk, plan, st);
        loop {
            let ts = tick();
            // the running k-th best (the batched direct rows scored first; their time is the reads'), the moments of
            // the clusters read directly among them, the stop, the window filled from the stream, the stop again
            let kth = rd.kth();
            let (n, a, b) = rd.take_z();
            if n > 0.0 {
                (zn, zs, zss) = (zn + n, zs + a, zss + b);
                if zn >= 2.0 {
                    delta = zs / zn;
                    rho = (zss / zn - delta * delta).max(0.0).sqrt();
                    lam = delta + ORDER_C * rho;
                    if zn >= 2.0 * zn_stream && lam != lam_stream {
                        // the stream re-keyed at the new lambda (a heapify of the clusters left in it)
                        (lam_stream, zn_stream) = (lam, zn);
                        stream = stream.into_vec().into_iter().map(|Key(_, Reverse(c))| stream_key(c, lam)).collect();
                        info.rekeys += 1;
                    }
                }
            }
            if !window.is_empty() && kth > stop_key(&window, delta, rho) {
                rd.st.cyc[P_ORDER] += tick() - ts;
                break;
            }
            while window.len() < LOOKAHEAD {
                let Some(Key(_, Reverse(ci))) = stream.pop() else { break };
                // a cluster of at most r visible rows (a sigma would cost about as much as reading it) takes no sigma, no
                // window slot and no pricing: it goes straight into the direct batch, in stream order, unless its stop
                // key with the stream's spread is below the k-th best
                if nv_of(ci) <= self.st.r {
                    if kth <= qm[ci] + (delta + psi * rho) * self.st.rad[ci] * s {
                        rd.visit(ci, KnnPath::Direct);
                        info.batched += 1;
                    }
                    continue;
                }
                // its sigma when that costs less than reading the cluster directly, else the stream's spread
                let sg = if m.sig_us < m.direct(nv_of(ci), rows_of(ci)) {
                    let tsp = tick();
                    let sg = self.sigma(ix, ci, &pr.q32, qq, &mut p);
                    sig += tick() - tsp;
                    info.spreads += 1;
                    info.sigma_cost += m.sig_us;
                    info.priced.push(ci as u32);
                    info.lam_priced.push(lam_stream);
                    sg
                } else {
                    self.st.rad[ci] * s
                };
                window.push((ci, sg));
            }
            if window.is_empty() || kth > stop_key(&window, delta, rho) {
                rd.st.cyc[P_ORDER] += tick() - ts;
                break;
            }
            // the window's largest order key (the lower cluster first among equal keys)
            let key = |&(ci, sg): &(usize, f64)| Key(qm[ci] + lam * sg, Reverse(ci));
            let i = (0..window.len()).max_by(|&a, &b| key(&window[a]).cmp(&key(&window[b]))).unwrap();
            let (ci, sg) = window.swap_remove(i);
            info.lam_read.push(lam);
            rd.st.cyc[P_ORDER] += tick() - ts;
            let tch = tick();
            let (nv, rows, g) = (nv_of(ci), rows_of(ci), ix.cl[ci].g);
            let direct = m.direct(nv, rows);
            // the index path, unless it cannot be cheaper: at the rows the sorted range at the running k-th best
            // leaves (every visible row before k items are held), and whether it needs its group set up
            let (index0, need) = if direct <= m.cut_us + m.cl_us {
                (f64::INFINITY, false)
            } else if kth > f64::NEG_INFINITY && ix.nukey.is_empty() {
                if !anchored {
                    // the anchor the sorted ranges need, the nearest of the clusters read
                    let among: Vec<usize> = info.read.iter().map(|&c| c as usize).collect();
                    pr.anc_among(ix, Some(&among));
                    anchored = true;
                }
                let c = cut_rows(ix, &pr, vis, ci, pr.qm(ix, ci), kth);
                (m.index(c, nv, rows), c > 0 && new_group(g))
            } else {
                (m.index(nv, nv, rows) - m.cut_us, new_group(g))
            };
            let save = direct - index0;
            let take = save > 0.0 && (!need || lost[g] + save >= m.grp_us);
            if need && save > 0.0 && !take {
                lost[g] += save;
            }
            let index = index0 + if need { m.grp_us } else { 0.0 };
            info.read.push(ci as u32);
            let path = if take {
                info.index += 1;
                info.index_cost += index;
                if !anchored {
                    let among: Vec<usize> = info.read.iter().map(|&c| c as usize).collect();
                    pr.anc_among(ix, Some(&among));
                    anchored = true;
                }
                // a new group is set up by the read itself, when its first cluster is read
                info.groups += new_group(g) as usize;
                KnnPath::Index
            } else {
                info.direct += 1;
                info.direct_cost += direct;
                KnnPath::Direct
            };
            rd.st.cyc[P_CHOICE] += tick() - tch;
            // a cluster of the window read directly gives its rows' moments once they are scored
            if path == KnnPath::Direct {
                rd.visit_z(ci, qm[ci], sg);
            } else {
                rd.visit(ci, path);
            }
        }
        (info.delta, info.rho) = (delta, rho);
        rd.st.cyc[P_SPREAD] += sig;
        rd.st.cyc[P_ORDER] -= sig.min(rd.st.cyc[P_ORDER]);
        rd.st.cyc[C_SPREADS] += (info.spreads * self.st.r) as u64;
        rd.st.cyc[C_DIRECT] += info.direct as u64;
        rd.st.cyc[C_INDEX] += info.index as u64;
        rd.st.cyc[C_GROUPS] += info.groups as u64;
        rd.st.cyc[C_VISITED] += info.read.len() as u64;
        info.path = "probe";
        info.cost = info.direct_cost + info.index_cost + info.sigma_cost;
        let (ents, st) = rd.finish();
        (out(ents.into_iter().map(|(v, e)| (pr.from_w(v), e)).collect(), st), info)
    }

    /// The approximate threshold query at t with knob `psi` (at least 0; `PSI` by default), as the module's comment
    /// says: the visible rows scoring at least t of the clusters the support table takes in and of those whose key
    /// reaches t, ascending; a subset of the exact answer, and the exact answer when psi is infinite or the query
    /// hands over (`AdaptiveInfo::path` "sparse": on a visible set `plan`'s exact query scans first). `plan` is the
    /// reads' (and the exact query's on a hand-over).
    pub fn threshold(&self, ix: &Index2, q: &[f32], t: f64, vis: Option<&Vis2>, psi: f64, plan: Plan) -> (ThrOut2, AdaptiveInfo) {
        assert!(psi >= 0.0, "psi is at least 0 (infinity reads every cluster), not {psi}");
        check_threshold(t);
        let t0 = tick();
        let (k, nl, m) = (ix.k(), ix.meta.nl(), &self.m);
        let pr = prep2(ix, q);
        let mut st = Stats::new(nl);
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        let all = psi == f64::INFINITY;
        let nusort = !ix.nukey.is_empty();
        let scan_cost = live.iter().map(|&ci| m.direct(nv_of(ci), ix.cl[ci].r1 - ix.cl[ci].r0)).sum();
        let mut info = AdaptiveInfo { scan_cost, ..AdaptiveInfo::default() };
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
        // a visible set the exact query scans first: the exact query, before anything else
        if plan.threshold_scans_first(ix, vis) {
            info.path = "sparse";
            info.cost = info.scan_cost;
            let out = threshold2(ix, &pr, t, &opts);
            return (done(out, &st), info);
        }
        // the scaled query (Prep2::unit) and t in its units; its answer's scores are scaled back on the way out
        let pr = pr.unit();
        let t = pr.thr_w(ix, t);
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
        // each read's path and cost (as k-NN prices it, at the rows the sorted range at t leaves)
        let tch = tick();
        let mut ch = Charges::new(m, ix, &pr, vis);
        let mut paths: Vec<(usize, KnnPath, bool)> = Vec::new();
        let mut cp = 0.0;
        for &ci in &sure {
            let (path, cost, gnew) = ch.choose(ci, nv_of(ci), t);
            cp += cost;
            paths.push((ci, path, gnew));
        }
        st.cyc[P_CHOICE] += tick() - tch;
        // the undecided clusters' sigmas, the largest bound first
        let mut spent = 0.0;
        let mut take = sure;
        let ts = tick();
        for &(_, ci) in &undecided {
            let tsp = tick();
            let s = self.sigma(ix, ci, &pr.q32, qq, &mut p);
            sig += tick() - tsp;
            spent += sc;
            info.spreads += 1;
            if qm[ci] + psi * s >= t {
                take.push(ci);
                let (path, cost, gnew) = ch.choose(ci, nv_of(ci), t);
                cp += cost;
                paths.push((ci, path, gnew));
            }
        }
        st.cyc[P_SPREAD] += sig;
        st.cyc[P_ORDER] += tick() - ts - sig;
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
        let mut out = run_planned(ix, &pr, t, &opts, &pre, &use_idx, tick() - tc);
        pr.back(&mut out);
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

    /// Constants that would price nothing (a row read for free, a value not a number or below 0) are refused, from a
    /// cache's text too; a read is priced at its cluster's visible share, between the timed ones.
    #[test]
    fn degenerate_constants_are_refused() {
        let m = Measured { dir_us: 1.2, row_us: [0.05, 0.06, 0.08, 0.1], cl_us: 15.0, cut_row_us: [0.02, 0.03, 0.05, 0.08], cut_us: 0.5, grp_us: 56.0, sig_us: 0.4 };
        assert!(m.check().is_ok() && Measured::from_kv(&m.to_kv()).unwrap() == m);
        for bad in [Measured { row_us: [0.05, 0.0, 0.08, 0.1], ..m }, Measured { cl_us: -1.0, ..m }, Measured { grp_us: f64::NAN, ..m }, Measured { sig_us: f64::INFINITY, ..m }] {
            assert!(bad.check().is_err(), "{bad:?}");
            assert!(Measured::from_kv(&bad.to_kv()).is_err(), "{bad:?}");
        }
        // every row visible, one in 4, one in 8 (halfway between 4 and 16 in log share), one in 1,000 (beyond 64)
        assert!((m.direct(2000, 2000) - (1.2 + 0.05 * 2000.0)).abs() < 1e-9);
        assert!((m.direct(500, 2000) - (1.2 + 0.06 * 500.0)).abs() < 1e-9);
        assert!((m.direct(250, 2000) - (1.2 + 0.07 * 250.0)).abs() < 1e-9);
        assert!((m.direct(2, 2000) - (1.2 + 0.1 * 2.0)).abs() < 1e-9);
        assert!((m.index(0, 500, 2000) - 0.5).abs() < 1e-9 && (m.index(100, 500, 2000) - (0.5 + 15.0 + 0.03 * 100.0)).abs() < 1e-9);
    }
}
