//! The approximate process: one rule for approximate k-NN and approximate threshold queries, which chooses the
//! clusters to read, the existing path each is read by (its visible rows scored directly, or the index path, which
//! needs its group set up), the order, and when to stop. Inside a cluster nothing changes: the index path, its
//! bounds and the exact decisions are the library's. Every answer is decided exactly where it is read.
//!
//! Per query, over the clusters holding a visible row (units):
//! - each cluster's centre score mu_c = q.m_c (int8 centres), its spread s_c(q) from the chosen tier (`Spread`), and
//!   its value: the predicted share of the answer n_c^vis P_c(tau) / Lambda(tau), P from the stored exceedance table
//!   (`calib::Table`) at zeta = (tau - mu_c) / s_c, Lambda(tau) the sum of n^vis P over the units;
//! - k-NN: tau = tau*, the root of Lambda(tau*) = k, solved once and never updated; threshold: tau = t, the shares
//!   normalised by max(Lambda(t), 1), and the clusters the support table decides wholly below t dropped and wholly
//!   above emitted without a read;
//! - tau* is solved over the units that hold more than `EPS_MASS` of the answer together (`decide`: their masses at a
//!   lower bound of tau, tau* of the top units alone), the rest kept at that bound;
//! - each unit's cost by the cost model (`Costs1`): read directly, dir + full n^vis; on the index path, cl + item n^vis,
//!   with its group's setup grp(n) charged once per group;
//! - the plan at a price lambda (`plan_at`): the closed set of actions maximising (sum of shares) - lambda (cost), a
//!   group set up exactly when the index path saves its clusters more than its setup; a per-query recall target takes
//!   the cheapest plan of the prices' family whose shares reach it and a budget the largest within it (`sweep`), a
//!   mean recall the price the build fitted for it under the query's visible share (`calib::Band`); a k-NN plan holds
//!   at least k visible rows (`fill_to_k`);
//! - the plan run best value per cost first, each group's setup before its first cluster; k-NN reads its clusters
//!   at the running k-th best (`KnnReader`, on the path planned), a threshold query as the exact one does
//!   (`run_planned`) over the planned clusters only;
//! - hand-over: when the plan's modelled cost reaches that of scanning the visible rows, the query scans them; when
//!   it reaches that of reading every unit (an upper bound of the exact path's), or the dial asks for everything, the
//!   exact query runs. Both answer exactly.
//!
//! The same decision (`decide`) runs at query time from the int8 features and at build, on the calibration's
//! pseudo-queries, to fit the mean-recall dial (`calib`).
//!
//! The units form one level (clusters in groups); a finer level (sub-clusters as row labels) would add units under a
//! cluster with their own table row and an expansion cost, and the plan's closed sets one more layer.

use super::calib::{Band, Calib, Table, Tier};
use super::q::*;
use super::*;
use crate::kernel::Stats;

/// The undecided share of a cluster's visible rows that an approximate query's cut leaves on the index path: sets
/// `Costs1::cl`. Hand-set (the phase timings do not separate it); to be refitted from timings.
pub const UNDECIDED: f64 = 0.1;
/// Microseconds per cluster scored directly, beyond its rows. Hand-set; to be refitted from timings.
pub const DIR_US: f64 = 0.2;
/// The share of the answer that the units left without their exact spreads may hold together, at most (bounded at
/// a lower bound of tau with their spreads' upper bounds); their values stay at those bounds, so tau and the shares
/// are within this of the ones with every spread exact.
pub const EPS_MASS: f64 = 1e-3;
/// The units of the largest centre scores whose tau* bounds the query's from below (any number gives a valid bound;
/// more, a tighter one at more work).
pub const TOP_UNITS: usize = 64;

/// The process's cost model, in microseconds on one thread, for vectors of n dimensions (only ratios move the plan;
/// the scale sets the unit of the price). Fitted to timings on this machine at n = 128, 384 and 512 (the suite's
/// glove, yahoo and LAION indexes, AVX-512), except the two hand-set constants above:
/// - `grp`, a group's setup (basis product, post-processing, rotations), a + b n^2 fitted by least squares to the
///   medians of 4, 15 and 28 us (`bench/suite/notes/visit-order/groups-setup.md`): 1.85 + 9.7e-5 n^2;
/// - `item`, a visible row on the index path, a + b n fitted to 0.027, 0.040 and 0.057 us (the query's work past the
///   group setup at nprobe 16 over the rows it read, less `cl` a cluster: `results/phases-1-int8-order.txt`):
///   0.016 + 7.4e-5 n;
/// - `full`, a visible row scored from its stored vector, fitted to 0.030, 0.045 and 0.070 us (the stored-vector
///   phase per row under random-1): 0.020 + 8.4e-5 n;
/// - `cl`, a cluster's setup on the index path, `COSTS`' cl_us0 + cl_us1 `UNDECIDED`; `dir`, `DIR_US`;
/// - `row8`, one int8 row of n against the query (a centre score or a spread's direction), fitted to the order phase
///   per cluster, 0.045, 0.067 and 0.070 us: 0.0385 + 6.5e-5 n.
#[derive(Clone, Copy, Debug)]
pub struct Costs1 {
    pub grp: f64,
    pub cl: f64,
    pub item: f64,
    pub dir: f64,
    pub full: f64,
    pub row8: f64,
}

impl Costs1 {
    pub fn for_dim(n: usize) -> Costs1 {
        let x = n as f64;
        Costs1 { grp: 1.85 + 9.7e-5 * x * x, cl: COSTS.cl_us0 + COSTS.cl_us1 * UNDECIDED, item: 0.016 + 7.4e-5 * x, dir: DIR_US, full: 0.020 + 8.4e-5 * x, row8: 0.0385 + 6.5e-5 * x }
    }
    /// A cluster of `nv` visible rows scored directly.
    pub fn direct(&self, nv: f64) -> f64 {
        self.dir + self.full * nv
    }
    /// A cluster of `nv` visible rows on the index path, its group's setup apart.
    pub fn index(&self, nv: f64) -> f64 {
        self.cl + self.item * nv
    }
}

// ---------------- the plan ----------------

/// What the plan does with a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Skip,
    Direct,
    Index,
    /// A threshold query's unit that the support table puts wholly at or above t: its visible rows are the answer's,
    /// emitted without being read; always taken, outside the plan.
    Sure,
}

/// Units in groups, for the plan: each unit's value, its direct cost and its index-path cost, units of one group
/// contiguous (`gstart`), and each unit's index in the caller's list (`units`).
pub struct Blocks {
    pub units: Vec<usize>,
    pub gstart: Vec<usize>,
    pub gid: Vec<usize>,
    pub v: Vec<f64>,
    pub cd: Vec<f64>,
    pub cx: Vec<f64>,
}

impl Blocks {
    /// The units `idx` of a list whose groups, values and visible rows are `grp`, `v` and `nv`, costed by `cm`.
    pub fn new(idx: &[usize], grp: &[usize], v: &[f64], nv: &[f64], cm: &Costs1) -> Blocks {
        // by group, in the given order within a group (a counting sort)
        let ng = idx.iter().map(|&i| grp[i] + 1).max().unwrap_or(0);
        let mut start = vec![0usize; ng + 1];
        for &i in idx {
            start[grp[i] + 1] += 1;
        }
        for g in 0..ng {
            start[g + 1] += start[g];
        }
        let mut by = vec![0usize; idx.len()];
        for &i in idx {
            by[start[grp[i]]] = i;
            start[grp[i]] += 1;
        }
        Blocks::of(by.iter().map(|&i| (i, grp[i], v[i], cm.direct(nv[i]), cm.index(nv[i]))))
    }

    /// From (index, group, value, direct cost, index cost), grouped by group.
    pub fn of(it: impl Iterator<Item = (usize, usize, f64, f64, f64)>) -> Blocks {
        let mut b = Blocks { units: Vec::new(), gstart: Vec::new(), gid: Vec::new(), v: Vec::new(), cd: Vec::new(), cx: Vec::new() };
        for (i, g, v, cd, cx) in it {
            if b.gid.last() != Some(&g) {
                b.gstart.push(b.units.len());
                b.gid.push(g);
            }
            b.units.push(i);
            b.v.push(v);
            b.cd.push(cd);
            b.cx.push(cx);
        }
        b.gstart.push(b.units.len());
        b
    }

    /// The modelled cost of reading every unit, each group set up when its index path saves more than its setup.
    pub fn all_cost(&self, ag: f64) -> f64 {
        (0..self.gid.len()).map(|j| {
            let r = self.gstart[j]..self.gstart[j + 1];
            let closed: f64 = self.cd[r.clone()].iter().sum();
            let open: f64 = ag + r.map(|i| self.cd[i].min(self.cx[i])).sum::<f64>();
            closed.min(open)
        }).sum()
    }
}

/// The plan at price `lam` (value per microsecond) with group setup `ag`: per unit, D = (v - lam cd)+ read directly
/// and I = (v - lam cx)+ on the index path; group g is set up exactly when the sum over its units of (I - D)+ exceeds
/// lam ag, and then each of its units takes the larger of I and D, else D; a unit of net value 0 is skipped. It
/// maximises (value) - lam (cost) over the closed sets (an index-path read only with its group set up). Writes each
/// unit's action (in `b`'s order) and returns the plan's value, cost and groups set up.
pub fn plan_at(b: &Blocks, lam: f64, ag: f64, act: &mut Vec<Act>) -> (f64, f64, usize) {
    act.clear();
    act.resize(b.v.len(), Act::Skip);
    let (mut value, mut cost, mut groups) = (0.0, 0.0, 0);
    for j in 0..b.gid.len() {
        let r = b.gstart[j]..b.gstart[j + 1];
        // (I - D)+ per unit: lam (cd - cx) where both reads are worth something (written so, not as a difference of
        // two nearly equal net values), else I
        let mut gain = 0.0;
        for i in r.clone() {
            let (d, x) = (b.v[i] > lam * b.cd[i], b.v[i] > lam * b.cx[i]);
            gain += if d && x { (lam * (b.cd[i] - b.cx[i])).max(0.0) } else if x { b.v[i] - lam * b.cx[i] } else { 0.0 };
        }
        let open = gain > lam * ag;
        if open {
            groups += 1;
            cost += ag;
        }
        for i in r {
            let (d, x) = (b.v[i] > lam * b.cd[i], b.v[i] > lam * b.cx[i]);
            act[i] = if open && x && (!d || b.cx[i] < b.cd[i]) {
                cost += b.cx[i];
                Act::Index
            } else if d {
                cost += b.cd[i];
                Act::Direct
            } else {
                continue;
            };
            value += b.v[i];
        }
    }
    (value, cost, groups)
}

/// Every distinct plan of `plan_at` as the price falls from infinity to 0, as (a price inside the interval of prices
/// that give it, its value, its cost), from the empty plan on; O(K log K). As the price falls past a unit's v / cd or
/// v / cx, its direct or index-path read becomes worth something; between those events each group's setup pays from
/// the price A / (ag - B) on, with A + lam B the sum over its units of (I - D)+ (linear in the price there), so a
/// group's opening is an event too. The plans are not nested (a unit read directly at one price may be read on the
/// index path at a lower one), so a dial that searches them takes the best of the whole family.
pub fn sweep(b: &Blocks, ag: f64) -> Vec<(f64, f64, f64)> {
    #[derive(Default, Clone, Copy)]
    struct G {
        a: f64,
        bb: f64,
        ov: f64,
        oc: f64,
        cv: f64,
        cc: f64,
        open: bool,
        ver: u32,
    }
    let n = b.v.len();
    let mut gof = vec![0usize; n];
    for j in 0..b.gid.len() {
        gof[b.gstart[j]..b.gstart[j + 1]].fill(j);
    }
    // unit events, largest price first: (price, unit, index read)
    let mut ev: Vec<(f64, usize, bool)> = Vec::with_capacity(2 * n);
    for i in 0..n {
        if b.v[i] > 0.0 {
            ev.push((b.v[i] / b.cd[i], i, false));
            ev.push((b.v[i] / b.cx[i], i, true));
        }
    }
    ev.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
    let (mut d_on, mut x_on) = (vec![false; n], vec![false; n]);
    let mut gs = vec![G::default(); b.gid.len()];
    // a unit's terms in its group's sums: (A, B, open value, open cost, closed value, closed cost)
    let terms = |i: usize, d: bool, x: bool| -> [f64; 6] {
        let mut t = [0.0; 6];
        if d {
            t[4] = b.v[i];
            t[5] = b.cd[i];
        }
        if x && (!d || b.cx[i] < b.cd[i]) {
            t[2] = b.v[i];
            t[3] = b.cx[i];
        } else if d {
            t[2] = b.v[i];
            t[3] = b.cd[i];
        }
        if x && !d {
            t[0] = b.v[i];
            t[1] = -b.cx[i];
        } else if x && d && b.cd[i] > b.cx[i] {
            t[1] = b.cd[i] - b.cx[i];
        }
        t
    };
    let part = |g: &G| if g.open { (g.ov, g.oc + ag) } else { (g.cv, g.cc) };
    let (mut tv, mut tc) = (0f64, 0f64);
    let mut flips: std::collections::BinaryHeap<(OrdF, usize, u32)> = std::collections::BinaryHeap::new();
    let mut out = Vec::new();
    let mut prev = f64::INFINITY;
    let rep = |lo: f64, hi: f64| if hi == f64::INFINITY { 2.0 * lo } else if lo <= 0.0 { 0.5 * hi } else { (lo * hi).sqrt() };
    let mut e = 0;
    loop {
        while flips.peek().is_some_and(|f| gs[f.1].ver != f.2) {
            flips.pop();
        }
        let lu = ev.get(e).map_or(f64::NEG_INFINITY, |x| x.0);
        let lf = flips.peek().map_or(f64::NEG_INFINITY, |f| f.0 .0);
        let le = lu.max(lf);
        if le == f64::NEG_INFINITY || le <= 0.0 {
            break;
        }
        if le < prev {
            out.push((rep(le, prev), tv, tc));
            prev = le;
        }
        if lf >= lu {
            let f = flips.pop().unwrap();
            let before = part(&gs[f.1]);
            gs[f.1].open = true;
            gs[f.1].ver += 1;
            let after = part(&gs[f.1]);
            tv += after.0 - before.0;
            tc += after.1 - before.1;
            continue;
        }
        let (_, i, x) = ev[e];
        e += 1;
        let j = gof[i];
        let before = part(&gs[j]);
        if x {
            x_on[i] = true;
        } else {
            d_on[i] = true;
        }
        // the group's sums afresh (sums kept by adding and subtracting would leave a residue that opens it)
        let mut t = [0f64; 6];
        for w in b.gstart[j]..b.gstart[j + 1] {
            for (a, z) in t.iter_mut().zip(terms(w, d_on[w], x_on[w])) {
                *a += z;
            }
        }
        let g = &mut gs[j];
        (g.a, g.bb, g.ov, g.oc, g.cv, g.cc) = (t[0], t[1], t[2], t[3], t[4], t[5]);
        // the group's setup just below this price, and the price it starts to pay at if it does not yet
        let k = ag - g.bb;
        let (open, flip) = if k > 0.0 {
            let ls = g.a / k;
            (ls >= le, (ls < le && ls > 0.0).then_some(ls))
        } else {
            (g.a > 0.0 || k < 0.0, None)
        };
        g.open = open;
        g.ver += 1;
        if let Some(ls) = flip {
            flips.push((OrdF(ls), j, g.ver));
        }
        let after = part(&gs[j]);
        tv += after.0 - before.0;
        tc += after.1 - before.1;
    }
    out.push((rep(0.0, prev), tv, tc));
    out
}

/// An f64 ordered for the heap (total order).
#[derive(Clone, Copy, PartialEq)]
struct OrdF(f64);
impl Eq for OrdF {}
impl PartialOrd for OrdF {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for OrdF {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

/// The price of the cheapest plan of the family (`sweep`) whose value reaches `r` (the largest value's when none
/// does).
pub fn price_for_value(states: &[(f64, f64, f64)], r: f64) -> f64 {
    states.iter().filter(|s| s.1 >= r).min_by(|a, b| a.2.total_cmp(&b.2).then(b.0.total_cmp(&a.0)))
        .or_else(|| states.iter().max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2))))
        .map_or(0.0, |s| s.0)
}

/// The price of the most valuable plan of the family (`sweep`) costing at most `budget` (the empty plan's when none
/// does).
pub fn price_for_cost(states: &[(f64, f64, f64)], budget: f64) -> f64 {
    states.iter().filter(|s| s.2 <= budget).max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2)))
        .or(states.first())
        .map_or(f64::INFINITY, |s| s.0)
}

/// A k-NN plan made to hold at least `kk` visible rows (`nv`, by unit), so that it returns k items whenever k are
/// visible: the skipped units by value per cost, best first, each on the index path when its group is set up and
/// that is the cheaper, else read directly. Returns the value and cost added.
pub fn fill_to_k(b: &Blocks, act: &mut [Act], nv: &[f64], kk: usize) -> (f64, f64) {
    let mut rows: f64 = (0..b.v.len()).filter(|&i| act[i] != Act::Skip).map(|i| nv[b.units[i]]).sum();
    if rows >= kk as f64 {
        return (0.0, 0.0);
    }
    let mut open = vec![false; b.gid.len()];
    let mut skipped = Vec::new();
    for j in 0..b.gid.len() {
        for i in b.gstart[j]..b.gstart[j + 1] {
            open[j] |= act[i] == Act::Index;
            if act[i] == Act::Skip {
                skipped.push((j, i));
            }
        }
    }
    skipped.sort_by(|x, y| (b.v[y.1] / b.cd[y.1].min(b.cx[y.1])).total_cmp(&(b.v[x.1] / b.cd[x.1].min(b.cx[x.1]))).then(x.1.cmp(&y.1)));
    let (mut dv, mut dc) = (0.0, 0.0);
    for (j, i) in skipped {
        if rows >= kk as f64 {
            break;
        }
        act[i] = if open[j] && b.cx[i] < b.cd[i] { Act::Index } else { Act::Direct };
        dc += if act[i] == Act::Index { b.cx[i] } else { b.cd[i] };
        dv += b.v[i];
        rows += nv[b.units[i]];
    }
    (dv, dc)
}

// ---------------- values ----------------

/// e^x for x in [-708, 709] (clamped), within about 2e-16 relative: x = k ln 2 + r with |r| <= ln 2 / 2, e^r by its
/// Taylor polynomial to degree 13, 2^k from its bits. Plain arithmetic in a fixed order: the same bits on every
/// instruction set, and a loop over it vectorises.
#[inline(always)]
pub fn exp_fast(x: f64) -> f64 {
    const LN2_HI: f64 = 6.931_471_803_691_238_164_9e-1;
    const LN2_LO: f64 = 1.908_214_929_270_587_700_02e-10;
    let x = x.clamp(-708.0, 709.0);
    let k = (x * std::f64::consts::LOG2_E).round();
    let r = (x - k * LN2_HI) - k * LN2_LO;
    let mut p = 1.0 / 6_227_020_800.0;
    for c in [1.0 / 479_001_600.0, 1.0 / 39_916_800.0, 1.0 / 3_628_800.0, 1.0 / 362_880.0, 1.0 / 40_320.0, 1.0 / 5_040.0, 1.0 / 720.0, 1.0 / 120.0, 1.0 / 24.0, 1.0 / 6.0, 0.5, 1.0, 1.0] {
        p = p * r + c;
    }
    p * f64::from_bits(((k as i64 + 1023) as u64) << 52)
}

/// A query of the process: k-NN at k, or a threshold.
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Knn(usize),
    Thr(f64),
}

/// A query's units: per unit its cluster and group, visible rows, centre score, log relative radius, spread and its
/// tier, whether that spread is exact (else an upper bound of the tier's), and whether the support table puts it
/// wholly above a threshold.
#[derive(Clone, Default)]
pub struct Units {
    pub cl: Vec<usize>,
    pub grp: Vec<usize>,
    pub nv: Vec<f64>,
    pub mu: Vec<f64>,
    pub lr: Vec<f64>,
    pub s: Vec<f64>,
    pub tier: Vec<Tier>,
    pub exact: Vec<bool>,
    pub sure: Vec<bool>,
}

impl Units {
    pub fn push(&mut self, cl: usize, grp: usize, nv: f64, mu: f64, lr: f64, s: f64, tier: Tier, exact: bool, sure: bool) {
        self.cl.push(cl);
        self.grp.push(grp);
        self.nv.push(nv);
        self.mu.push(mu);
        self.lr.push(lr);
        self.s.push(s.max(1e-12));
        self.tier.push(tier);
        self.exact.push(exact);
        self.sure.push(sure);
    }
}

/// Exact spreads for units on request: writes the spread of the given tier into `u.s` for the units `idx` (marking
/// them exact) and returns the directions it projected (rows of n).
pub type Fill<'a> = dyn FnMut(&mut Units, &[usize], Tier) -> usize + 'a;

/// What a decision needs besides the query's units: the tables (main or control), the margin, the cost model, the
/// mean-recall dial's bands for this setting, and the query's visible share of the rows.
pub struct Model<'a> {
    pub tabs: [&'a Table; 2],
    pub delta: f64,
    pub cm: Costs1,
    pub bands: &'a [Band],
    pub vis_share: f64,
    /// The query's visible rows.
    pub nvis: f64,
}

impl Model<'_> {
    #[inline]
    fn ln_p(&self, u: &Units, i: usize, tau: f64) -> f64 {
        if u.sure[i] {
            return 0.0;
        }
        self.tabs[u.tier[i] as usize].ln_p((tau - u.mu[i]) / u.s[i], u.lr[i])
    }
}

/// Units as contiguous arrays: centre score, inverse spread, log relative radius and visible rows.
struct Soa {
    mu: Vec<f64>,
    inv: Vec<f64>,
    lr: Vec<f64>,
    nv: Vec<f64>,
}

impl Soa {
    fn of(u: &Units, idx: impl Iterator<Item = usize>) -> Soa {
        let mut a = Soa { mu: Vec::new(), inv: Vec::new(), lr: Vec::new(), nv: Vec::new() };
        for i in idx {
            a.mu.push(u.mu[i]);
            a.inv.push(1.0 / u.s[i]);
            a.lr.push(u.lr[i]);
            a.nv.push(u.nv[i]);
        }
        a
    }

    /// Each unit's mass n^vis P(t) into `w` and its derivative in t into `wd` (one loop of table look-ups and
    /// `exp_fast`, no branch: it vectorises).
    fn masses(&self, tab: &Table, t: f64, w: &mut [f64], wd: &mut [f64]) {
        for j in 0..self.mu.len() {
            let (l, d) = tab.ln_p_slope((t - self.mu[j]) * self.inv[j], self.lr[j]);
            let e = self.nv[j] * exp_fast(l);
            w[j] = e;
            wd[j] = e * d * self.inv[j];
        }
    }
}

/// The root tau* of Lambda(tau) = kk over the units `idx`, from `lo` (Lambda(lo) >= kk; -inf: unknown): Newton steps
/// on ln Lambda with its derivative from the tables' slopes, kept inside a bracket (halving it when a step leaves
/// it). Returns tau* and the evaluations of Lambda.
fn solve_star(m: &Model, u: &Units, idx: &[usize], kk: f64, lo: f64) -> (f64, usize) {
    let lk = kk.ln();
    let mut ev = 0;
    // the units by tier, as contiguous arrays (each tier's masses one loop without branches)
    let parts: Vec<(&Table, Soa)> = [Tier::Group, Tier::Own].iter().map(|&t| (m.tabs[t as usize], Soa::of(u, idx.iter().copied().filter(|&i| u.tier[i] == t && !u.sure[i])))).filter(|p| !p.1.mu.is_empty()).collect();
    let sure: f64 = idx.iter().filter(|&&i| u.sure[i]).map(|&i| u.nv[i]).sum();
    let (mut w, mut wd) = (Vec::new(), Vec::new());
    let mut eval = |t: f64| -> (f64, f64) {
        ev += 1;
        let (mut sw, mut sd) = (sure, 0f64);
        for (tab, a) in &parts {
            w.resize(a.mu.len(), 0.0);
            wd.resize(a.mu.len(), 0.0);
            a.masses(tab, t, &mut w, &mut wd);
            for (x, y) in w.iter().zip(&wd) {
                sw += x;
                sd += y;
            }
        }
        (sw.max(1e-300).ln() - lk, sd / sw.max(1e-300))
    };
    let step0 = idx.iter().map(|&i| u.s[i]).fold(0.0, f64::max).max(1e-9);
    let top = idx.iter().map(|&i| u.mu[i]).fold(f64::NEG_INFINITY, f64::max);
    let (mut a, mut b) = (if lo.is_finite() { lo } else { top }, f64::INFINITY);
    let (mut ha, mut da) = eval(a);
    let mut step = step0;
    while ha < 0.0 && step < 1e6 * step0 {
        b = a;
        a -= step;
        step *= 2.0;
        (ha, da) = eval(a);
    }
    if ha < 0.0 {
        return (a, ev);
    }
    let (mut x, mut hx, mut dx) = (a, ha, da);
    let mut step = step0;
    for _ in 0..100 {
        if hx.abs() < 1e-12 {
            return (x, ev);
        }
        let newton = if dx < 0.0 { x - hx / dx } else { f64::NAN };
        let y = if newton > a && newton < b {
            newton
        } else if b.is_finite() {
            0.5 * (a + b)
        } else {
            step *= 2.0;
            a + step
        };
        let (hy, dy) = eval(y);
        if hy >= 0.0 { a = y } else { b = y }
        (x, hx, dx) = (y, hy, dy);
        if b.is_finite() && b - a <= 1e-14 * (1.0 + x.abs()) {
            return (x, ev);
        }
    }
    (x, ev)
}

/// tau and every unit's share at it: tau* for k-NN over the units `cand` (the rest, at their bounds `ub`, hold under
/// `EPS_MASS` of it), t for a threshold; shares n^vis P(tau) over Lambda(tau) (k-NN) or max(Lambda(t), 1)
/// (threshold), Lambda over every unit. Records Lambda and the evaluations in `info`.
pub fn shares(m: &Model, u: &Units, cand: &[usize], ub: &[f64], target: Target, lo: f64, info: &mut ProcInfo) -> (f64, Vec<f64>) {
    let tau = match target {
        Target::Thr(t) => t,
        Target::Knn(kk) => {
            let (t, ev) = solve_star(m, u, cand, kk as f64, lo);
            info.evals += ev;
            t
        }
    };
    let mut w = ub.to_vec();
    for &i in cand {
        w[i] = u.nv[i] * exp_fast(m.ln_p(u, i, tau));
    }
    let sum: f64 = w.iter().sum();
    info.lambda = sum;
    let norm = match target {
        Target::Knn(_) => sum.max(1e-300),
        Target::Thr(_) => sum.max(1.0),
    };
    (tau, w.into_iter().map(|x| x / norm).collect())
}

/// What a decision chose.
pub enum Outcome {
    /// The plan over the units the support table leaves open (`Blocks::units` index the query's units).
    Plan { b: Blocks, act: Vec<Act> },
    Scan,
    Exact,
}

/// The process's decision for one query from its units (spreads at their tier's upper bound or exact): the units
/// that can hold the answer get their exact spreads (`fill`), tau and the shares, the plan at the dial, for `Own` the
/// units near its margin their own spreads and the plan again, for k-NN at least k visible rows, then the hand-over.
/// `info.scan_cost` must hold the cost of scanning the visible rows.
pub fn decide(m: &Model, u: &mut Units, fill: &mut Fill, target: Target, stop: Stop, spread: Spread, info: &mut ProcInfo) -> Outcome {
    let n = u.cl.len();
    let ag = m.cm.grp;
    let tier0 = if spread == Spread::OwnAll { Tier::Own } else { Tier::Group };
    // every unit's spread of the first tier (bounds from its largest variance prune none on real data: the spread
    // is a fraction of them, so the group tier's product over every group is cheaper than a choice of groups)
    let need: Vec<usize> = (0..n).filter(|&i| !u.exact[i] && !u.sure[i]).collect();
    info.spreads += fill(u, &need, tier0);
    // a lower bound of tau: t, or tau* of the units of the largest centre scores alone (Lambda over a part is at most
    // Lambda over all), started from the largest of the top units' own roots
    let lo = match target {
        Target::Thr(t) => t,
        Target::Knn(kk) => {
            let mut top: Vec<usize> = (0..n).collect();
            let mt = n.min(TOP_UNITS);
            if mt < n {
                top.select_nth_unstable_by(mt - 1, |&a, &b| u.mu[b].total_cmp(&u.mu[a]).then(a.cmp(&b)));
            }
            top.truncate(mt);
            let own = top.iter().filter_map(|&i| m.tabs[u.tier[i] as usize].zeta_at((kk as f64 / u.nv[i]).ln(), u.lr[i]).map(|z| u.mu[i] + u.s[i] * z)).fold(f64::NEG_INFINITY, f64::max);
            let (t, ev) = solve_star(m, u, &top, kk as f64, own);
            info.evals += ev;
            let mass: f64 = top.iter().map(|&i| u.nv[i] * exp_fast(m.ln_p(u, i, t))).sum();
            if mass >= kk as f64 * (1.0 - 1e-9) { t } else { own }
        }
    };
    // each unit's mass at the lower bound of tau, a bound of its mass at tau; the units of the smallest bounds, together
    // at most EPS_MASS of the answer, keep their bounds (taken by binary exponent, smallest first) and the rest are
    // solved over
    let want = match target {
        Target::Knn(kk) => kk as f64,
        Target::Thr(_) => 1.0,
    };
    let ub: Vec<f64> = (0..n).map(|i| if u.sure[i] || !lo.is_finite() || lo <= u.mu[i] { u.nv[i] } else { u.nv[i] * exp_fast(m.ln_p(u, i, lo)) }).collect();
    let cand: Vec<usize> = {
        let expo = |x: f64| (x.to_bits() >> 52) as usize & 2047;
        let mut by = vec![0f64; 2048];
        for &x in &ub {
            by[expo(x)] += x;
        }
        let (mut held, mut cut) = (0.0, 2048);
        for (e, &x) in by.iter().enumerate() {
            if held + x > EPS_MASS * want {
                cut = e;
                break;
            }
            held += x;
        }
        (0..n).filter(|&i| expo(ub[i]) >= cut).collect()
    };
    info.candidates = cand.len();
    let (mut tau, mut v) = shares(m, u, &cand, &ub, target, lo, info);
    // the units the support table puts wholly above t are emitted without a read: outside the plan, their shares
    // taken as given (a recall target asks the plan for the rest)
    let free: Vec<usize> = (0..n).filter(|&i| !u.sure[i]).collect();
    let sure: Vec<usize> = (0..n).filter(|&i| u.sure[i]).collect();
    let sure_share = |v: &[f64]| sure.iter().map(|&i| v[i]).sum::<f64>();
    let mut b = Blocks::new(&free, &u.grp, &v, &u.nv, &m.cm);
    let mut act = Vec::new();
    let mut lam = dial_price(m, &b, stop, target, info.lambda, sure_share(&v));
    let (mut val, mut cost, mut groups) = plan_at(&b, lam, ag, &mut act);
    if spread == Spread::Own {
        // the plan's marginal value per cost (the best unit's when it is empty), and the units within the margin of it
        let dens = |b: &Blocks, i: usize| b.v[i] / b.cd[i].min(b.cx[i]);
        let taken: Vec<usize> = (0..b.v.len()).filter(|&i| act[i] != Act::Skip).collect();
        let rho = if taken.is_empty() { (0..b.v.len()).map(|i| dens(&b, i)).fold(0.0, f64::max) } else { taken.iter().map(|&i| dens(&b, i)).fold(f64::INFINITY, f64::min) };
        let (floor, ceil) = (rho * (-m.delta).exp(), rho * m.delta.exp());
        let refine: Vec<usize> = (0..b.v.len()).filter(|&i| (floor..=ceil).contains(&dens(&b, i))).map(|i| b.units[i]).collect();
        info.refined = refine.len();
        info.spreads += fill(u, &refine, Tier::Own);
        (tau, v) = shares(m, u, &cand, &ub, target, lo, info);
        b = Blocks::new(&free, &u.grp, &v, &u.nv, &m.cm);
        lam = dial_price(m, &b, stop, target, info.lambda, sure_share(&v));
        (val, cost, groups) = plan_at(&b, lam, ag, &mut act);
    }
    if let Target::Knn(kk) = target {
        let (dv, dc) = fill_to_k(&b, &mut act, &u.nv, kk);
        val += dv;
        cost += dc;
    }
    info.tau = tau;
    info.lam = lam;
    info.pred = (val + sure_share(&v)).min(1.0);
    info.cost = cost;
    info.groups = groups;
    info.all_cost = b.all_cost(ag);
    info.direct = act.iter().filter(|&&a| a == Act::Direct).count();
    info.index = act.iter().filter(|&&a| a == Act::Index).count();
    if cost >= info.scan_cost {
        return Outcome::Scan;
    }
    if !b.v.is_empty() && cost >= info.all_cost {
        return Outcome::Exact;
    }
    Outcome::Plan { b, act }
}

/// Whether a query should scan its `nvis` visible rows before anything else: k-NN with at most k of them, or a scan
/// costing no more than the centre scores of the `k` clusters and the search for the nearest of `anchors` anchors
/// (threshold queries with a support table).
pub fn early_scan(cm: &Costs1, k: usize, anchors: usize, nvis: usize, target: Target) -> bool {
    matches!(target, Target::Knn(kk) if nvis <= kk) || nvis as f64 * cm.full <= anchors as f64 * cm.full + k as f64 * cm.row8
}

/// The price the dial asks for: given; the build's for a mean recall (its band nearest the query's k or predicted
/// answer share and visible share); the cheapest plan reaching a recall (less the shares taken without a read) or
/// the most valuable within a budget, over the family of plans (`sweep`).
fn dial_price(m: &Model, b: &Blocks, stop: Stop, target: Target, lambda: f64, sure: f64) -> f64 {
    match stop {
        Stop::Price(l) => l,
        Stop::MeanRecall(r) => match target {
            Target::Knn(kk) => calib::band_price(m.bands, true, kk as f64, m.vis_share, r),
            Target::Thr(_) => calib::band_price(m.bands, false, lambda.max(1.0) / m.nvis.max(1.0), m.vis_share, r),
        },
        Stop::Recall(r) => price_for_value(&sweep(b, m.cm.grp), (r - sure).max(0.0)),
        Stop::Budget(bu) => price_for_cost(&sweep(b, m.cm.grp), bu),
    }
}

// ---------------- the dials and options ----------------

/// Where each unit's spread comes from (the experiment's arms; the timed suite decides which stay).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    /// The group's covariance for every unit (`Tier::Group`).
    Group,
    /// The group's first, then the cluster's own covariance (`Tier::Own`) for the units whose group-tier value per
    /// cost lies within a factor e^delta (`Calib::delta`) of the plan's marginal one, on either side (the units whose
    /// decision the own spread can change), and the plan made again.
    Own,
    /// The cluster's own covariance for every unit.
    OwnAll,
}

impl Spread {
    pub const ALL: [Spread; 3] = [Spread::Group, Spread::Own, Spread::OwnAll];
    pub fn parse(s: &str) -> Result<Spread, String> {
        match s {
            "group" => Ok(Spread::Group),
            "own" => Ok(Spread::Own),
            "own-all" => Ok(Spread::OwnAll),
            _ => Err(format!("the spread is group, own or own-all, not {s:?}")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Spread::Group => "group",
            Spread::Own => "own",
            Spread::OwnAll => "own-all",
        }
    }
}

/// When the process stops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop {
    /// A price: stop where a further microsecond is expected to add less than this share of the answer.
    Price(f64),
    /// A mean recall over queries: the price the build fitted for it on held-out pseudo-queries (`calib::Band`).
    MeanRecall(f64),
    /// A recall per query: the cheapest plan whose predicted shares reach it.
    Recall(f64),
    /// A budget of modelled microseconds for the reads (centre scores and spreads apart): the largest plan within it.
    Budget(f64),
}

impl Stop {
    /// Whether the dial asks for every answer.
    pub fn everything(self) -> bool {
        match self {
            Stop::Price(l) => l <= 0.0,
            Stop::MeanRecall(r) | Stop::Recall(r) => r >= 1.0,
            Stop::Budget(b) => b == f64::INFINITY,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcOpts {
    pub spread: Spread,
    pub stop: Stop,
    /// The control tables (plain pseudo-queries only), with their own mean-recall bands.
    pub plain: bool,
    pub plan: Plan,
}

impl ProcOpts {
    pub fn new(stop: Stop) -> ProcOpts {
        ProcOpts { spread: Spread::Group, stop, plain: false, plan: Plan::default() }
    }
}

/// What one query's process did.
#[derive(Clone, Debug, Default)]
pub struct ProcInfo {
    /// "plan", "scan" (the visible rows scanned), "exact" (the exact query), or "empty" (no visible row).
    pub path: &'static str,
    pub tau: f64,
    pub lam: f64,
    /// The planned units' predicted shares, summed: the predicted recall.
    pub pred: f64,
    /// Modelled microseconds: the plan's reads; the centre scores and spreads (an estimate made before them); a scan
    /// of the visible rows; reading every unit.
    pub cost: f64,
    pub fixed: f64,
    pub scan_cost: f64,
    pub all_cost: f64,
    pub units: usize,
    /// Units tau* is solved over (the rest hold under `EPS_MASS` of the answer together).
    pub candidates: usize,
    pub direct: usize,
    pub index: usize,
    pub groups: usize,
    /// Directions projected (rows of n) for the spreads; units given their own spread; evaluations of Lambda.
    pub spreads: usize,
    pub refined: usize,
    pub evals: usize,
    /// Lambda(tau): the units' predicted answers at tau (k at tau* for k-NN).
    pub lambda: f64,
}

// ---------------- queries ----------------

/// The process's per-index state: the int8 centres, each cluster's log relative radius, each cluster's and group's
/// rest and largest variance (the spreads' upper bounds), and the cost model.
pub struct Process {
    cen8: Vec<i8>,
    csc: Vec<f32>,
    lr: Vec<f64>,
    orest: Vec<f64>,
    otop: Vec<f64>,
    grest: Vec<f64>,
    gtop: Vec<f64>,
    pub cm: Costs1,
}

/// A query's plan ready to run, or the path it is handed to.
enum Made {
    Run { u: Units, b: Blocks, act: Vec<Act> },
    Scan,
    Exact,
    Empty,
}

impl Process {
    /// The state for `ix`; an error when the index has no stored vectors or holds no calibration.
    pub fn new(ix: &Index2) -> Result<Process, String> {
        if !matches!(ix.full, Full2::Real { .. }) {
            return Err("the approximate process reads stored vectors, and this index has none; query it exactly (knn2, threshold2)".into());
        }
        let cal = ix.calib.as_ref().ok_or(format!("this index holds no calibration for the approximate process: the build fits one only for an index of at least {} rows in at least one cluster, and this one has {} rows in {} clusters; query it exactly (knn2, threshold2) or by a cluster order (knn2_fast)", calib::MIN_ROWS, ix.nrows, ix.k()))?;
        let st = ix.order.as_ref().ok_or("this index holds no order state; rebuild it")?;
        let (cen8, csc) = int8_centres(&ix.centres32, ix.n()).unwrap_or_default();
        let (r, gr) = (st.r, cal.gcov.r);
        let top = |lam: &[f32]| lam.first().map_or(0.0, |&l| l as f64);
        Ok(Process {
            cen8,
            csc,
            lr: calib::lr_of(&st.rad, cal.med_rad),
            orest: (0..ix.k()).map(|c| calib::rest(st.rad[c], &st.lam[c * r..(c + 1) * r], st.n0)).collect(),
            otop: (0..ix.k()).map(|c| top(&st.lam[c * r..(c + 1) * r])).collect(),
            grest: (0..ix.groups.len()).map(|g| calib::rest(cal.gcov.rad[g], &cal.gcov.lam[g * gr..(g + 1) * gr], cal.gcov.n0)).collect(),
            gtop: (0..ix.groups.len()).map(|g| top(&cal.gcov.lam[g * gr..(g + 1) * gr])).collect(),
            cm: Costs1::for_dim(ix.n()),
        })
    }

    /// q.m of every cluster in f32 (int8 centres where the fit stored them so).
    fn centre_scores(&self, ix: &Index2, q32: &[f32]) -> Vec<f64> {
        let n = ix.n();
        if self.cen8.is_empty() {
            return (0..ix.k()).map(|ci| unsafe { dot32(q32, &ix.centres32[ci * n..(ci + 1) * n]) }).collect();
        }
        let mut out = vec![0f32; ix.k()];
        unsafe { crate::kernel::gemv_i8(q32, &self.cen8, n, &mut out) };
        out.iter().zip(&self.csc).map(|(&x, &s)| x as f64 * s as f64).collect()
    }

    /// The query's units, their decision, or the path it is handed to.
    fn make(&self, ix: &Index2, cal: &Calib, pr: &Prep2, vis: Option<&Vis2>, target: Target, o: &ProcOpts, info: &mut ProcInfo, st: &mut Stats) -> Made {
        let t0 = tick();
        let k = ix.k();
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        info.units = live.len();
        info.scan_cost = nvis as f64 * self.cm.full;
        if nvis == 0 {
            return Made::Empty;
        }
        let anchors = if matches!(target, Target::Thr(_)) && ix.nukey.is_empty() { ix.nanch } else { 0 };
        info.fixed = (anchors as f64 * self.cm.full) + k as f64 * self.cm.row8;
        if matches!(o.plan.mode, PlanMode::Scan) || early_scan(&self.cm, k, anchors, nvis, target) {
            return Made::Scan;
        }
        if o.stop.everything() {
            return Made::Exact;
        }
        // a threshold query's support-table verdicts: units wholly below t are dropped, those wholly above are sure
        let verdict: Vec<Option<Pre>> = match target {
            Target::Thr(t) if anchors > 0 => {
                pr.anc_among(ix, Some(&live));
                live.iter().map(|&ci| support_pre(ix, pr, t, nv_of(ci), ci, false)).collect()
            }
            _ => vec![None; live.len()],
        };
        let qm = self.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - t0;
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let st0 = ix.order.as_ref().expect("Process::new checked the order state");
        let tier0 = if o.spread == Spread::OwnAll { Tier::Own } else { Tier::Group };
        let mut u = Units::default();
        for (j, &ci) in live.iter().enumerate() {
            if matches!(verdict[j], Some(Pre::Skip)) {
                continue;
            }
            let g = ix.cl[ci].g;
            // the tier's spread at its largest: every variance at most the largest of the top one and the rest
            let s_max = match tier0 {
                Tier::Group => (self.gtop[g].max(self.grest[g]) * qq).sqrt() * st0.rad[ci] / cal.gcov.rad[g].max(1e-300),
                Tier::Own => (self.otop[ci].max(self.orest[ci]) * qq).sqrt(),
            };
            u.push(ci, g, nv_of(ci) as f64, qm[ci], self.lr[ci], s_max, tier0, false, matches!(verdict[j], Some(Pre::In)));
        }
        if u.cl.is_empty() {
            return Made::Empty;
        }
        st.cyc[P_SPREAD] += tick() - ts;
        let tp = tick();
        // exact spreads on request: a group's sigma once for all its units, a cluster's own sigma per unit
        let mut gsig: Vec<f64> = vec![-1.0; ix.groups.len()];
        let mut p = vec![0f32; cal.gcov.r.max(st0.r)];
        let mut spread_cyc = 0u64;
        let mut fill = |u: &mut Units, idx: &[usize], tier: Tier| -> usize {
            let tf = tick();
            let mut rows = 0;
            let ng = ix.groups.len();
            if tier == Tier::Group {
                // most groups asked for: one product over every group's directions
                let mut want: Vec<usize> = idx.iter().map(|&i| u.grp[i]).filter(|&g| gsig[g] < 0.0).collect();
                want.sort_unstable();
                want.dedup();
                if 2 * want.len() >= ng {
                    let r = cal.gcov.r;
                    let mut pall = vec![0f32; ng * r];
                    unsafe { crate::kernel::gemv_i8(&pr.q32, &cal.gcov.dirs8, ix.n(), &mut pall) };
                    for g in 0..ng {
                        let pf: Vec<f64> = pall[g * r..(g + 1) * r].iter().map(|&x| x as f64).collect();
                        gsig[g] = calib::sigma(&pf, &cal.gcov.lam[g * r..(g + 1) * r], &cal.gcov.dsc[g * r..(g + 1) * r], self.grest[g], qq);
                    }
                    rows += ng * r;
                }
            }
            for &i in idx {
                let (ci, g) = (u.cl[i], u.grp[i]);
                u.s[i] = match tier {
                    Tier::Group => {
                        if gsig[g] < 0.0 {
                            let r = cal.gcov.r;
                            unsafe { crate::kernel::gemv_i8(&pr.q32, &cal.gcov.dirs8[g * r * ix.n()..(g + 1) * r * ix.n()], ix.n(), &mut p[..r]) };
                            let pf: Vec<f64> = p[..r].iter().map(|&x| x as f64).collect();
                            gsig[g] = calib::sigma(&pf, &cal.gcov.lam[g * r..(g + 1) * r], &cal.gcov.dsc[g * r..(g + 1) * r], self.grest[g], qq);
                            rows += r;
                        }
                        gsig[g] * st0.rad[ci] / cal.gcov.rad[g].max(1e-300)
                    }
                    Tier::Own => {
                        let r = st0.r;
                        unsafe { crate::kernel::gemv_i8(&pr.q32, st0.dirs(ci, r, ix.n()), ix.n(), &mut p[..r]) };
                        let pf: Vec<f64> = p[..r].iter().map(|&x| x as f64).collect();
                        rows += r;
                        calib::sigma(&pf, &st0.lam[ci * r..(ci + 1) * r], &st0.dsc[ci * r..(ci + 1) * r], self.orest[ci], qq)
                    }
                }
                .max(1e-12);
                u.tier[i] = tier;
                u.exact[i] = true;
            }
            spread_cyc += tick() - tf;
            rows
        };
        let model = Model { tabs: [cal.table(Tier::Group, o.plain), cal.table(Tier::Own, o.plain)], delta: cal.delta, cm: self.cm, bands: cal.bands(o.spread, o.plain), vis_share: nvis as f64 / ix.nrows.max(1) as f64, nvis: nvis as f64 };
        let out = decide(&model, &mut u, &mut fill, target, o.stop, o.spread, info);
        st.cyc[P_SPREAD] += spread_cyc;
        st.cyc[P_SOLVE] += (tick() - tp).saturating_sub(spread_cyc);
        match out {
            Outcome::Scan => Made::Scan,
            Outcome::Exact => Made::Exact,
            Outcome::Plan { b, act } => Made::Run { u, b, act },
        }
    }

    /// For inspection: the query's tau and each unit's cluster, predicted share of the answer and planned action, as
    /// the plan was made (empty when the query is handed to a scan or the exact path, or nothing is visible).
    pub fn shares_for(&self, ix: &Index2, q: &[f32], target: Target, vis: Option<&Vis2>, o: &ProcOpts) -> (f64, Vec<(usize, f64, Act)>, ProcInfo) {
        let cal = ix.calib.as_ref().expect("Process::new checked the calibration");
        let pr = prep2(ix, q);
        let mut info = ProcInfo::default();
        let made = self.make(ix, cal, &pr, vis, target, o, &mut info, &mut Stats::new(ix.meta.nl()));
        match made {
            Made::Run { u, b, act } => {
                let sure = (0..u.cl.len()).filter(|&i| u.sure[i]).map(|i| (u.cl[i], u.nv[i] / info.lambda.max(1.0), Act::Sure));
                (info.tau, (0..b.v.len()).map(|i| (u.cl[b.units[i]], b.v[i], act[i])).chain(sure).collect(), info)
            }
            _ => (info.tau, Vec::new(), info),
        }
    }

    /// The planned actions, best value per cost first: a unit read directly at v / cd; the index-path units of a group
    /// at their group's (their values over the setup and their costs), and among those by their own v / cx.
    fn order(b: &Blocks, act: &[Act], ag: f64) -> Vec<(usize, Act)> {
        let mut keyed: Vec<(f64, f64, usize, Act)> = Vec::new();
        for j in 0..b.gid.len() {
            let r = b.gstart[j]..b.gstart[j + 1];
            let (gv, gc) = r.clone().filter(|&i| act[i] == Act::Index).fold((0.0, ag), |a, i| (a.0 + b.v[i], a.1 + b.cx[i]));
            for i in r {
                match act[i] {
                    Act::Direct => keyed.push((b.v[i] / b.cd[i], 0.0, i, Act::Direct)),
                    Act::Index => keyed.push((gv / gc, b.v[i] / b.cx[i], i, Act::Index)),
                    Act::Skip | Act::Sure => {}
                }
            }
        }
        keyed.sort_by(|x, y| y.0.total_cmp(&x.0).then(y.1.total_cmp(&x.1)).then(x.2.cmp(&y.2)));
        keyed.into_iter().map(|x| (x.2, x.3)).collect()
    }

    /// Approximate k-NN: the k best visible items of the planned clusters, decided exactly where read, k of them
    /// whenever k are visible (the plan holds at least k visible rows: `fill_to_k`; all of them when fewer are
    /// visible); with `dial`, the index path's per-level factors (`Dial`).
    pub fn knn(&self, ix: &Index2, q: &[f32], kk: usize, vis: Option<&Vis2>, dial: Option<&[f32]>, o: &ProcOpts) -> (KnnOut2, ProcInfo) {
        assert!(kk > 0, "approximate k-NN needs k of at least 1");
        let tm = Timer::new();
        let cal = ix.calib.as_ref().expect("Process::new checked the calibration");
        let nl = ix.meta.nl();
        let t0 = tick();
        let mut pr = prep2(ix, q);
        if let Some(d) = dial {
            pr.dial = d.to_vec();
        }
        let mut st = Stats::new(nl);
        let mut info = ProcInfo::default();
        let made = self.make(ix, cal, &pr, vis, Target::Knn(kk), o, &mut info, &mut st);
        let out = |ents: Vec<(f64, u32)>, mut st: Stats| {
            st.cyc[P_QUERY] += tick() - t0;
            KnnOut2 { ents, st, t_seed: 0.0, t_sweep: tm.ms(), t_sel: 0.0, npow: ix.meta.npow }
        };
        match made {
            Made::Empty => {
                info.path = "empty";
                (out(Vec::new(), st), info)
            }
            Made::Scan => {
                info.path = "scan";
                let nv = vis.map_or(ix.nrows, |v| v.rows.len()) as u64;
                st.full_reads += nv;
                st.touched += nv;
                (out(scan_knn(ix, &pr.q16, &pr.q32, pr.qn, kk, vis), st), info)
            }
            Made::Exact => {
                info.path = "exact";
                let mut x = knn2(ix, q, kk, vis, o.plan);
                x.st.add(&st);
                (x, info)
            }
            Made::Run { u, b, act } => {
                info.path = "plan";
                let ord = Process::order(&b, &act, self.cm.grp);
                let tp = tick();
                let cls: Vec<usize> = ord.iter().map(|&(i, _)| u.cl[b.units[i]]).collect();
                // q.m in f64 for the clusters on the index path only (its bounds read it; a direct read does not)
                let idx: Vec<usize> = ord.iter().filter(|x| x.1 == Act::Index).map(|&(i, _)| u.cl[b.units[i]]).collect();
                if !idx.is_empty() {
                    pr.qms(ix, Some(&idx));
                }
                let mut gs: Vec<usize> = idx.iter().map(|&ci| ix.cl[ci].g).collect();
                gs.sort_unstable();
                gs.dedup();
                st.cyc[P_PLAN] += tick() - tp;
                setup_groups(ix, &pr, &gs, &mut st);
                st.cyc[C_DIRECT] += info.direct as u64;
                st.cyc[C_INDEX] += info.index as u64;
                st.cyc[C_GROUPS] += info.groups as u64;
                st.cyc[C_SPREADS] += info.spreads as u64;
                let mut rd = KnnReader::new(ix, &pr, vis, kk, o.plan, st);
                for (ci, (_, a)) in cls.iter().zip(&ord) {
                    rd.visit(*ci, if *a == Act::Direct { KnnPath::Direct } else { KnnPath::Index });
                }
                let (ents, st) = rd.finish();
                (out(ents, st), info)
            }
        }
    }

    /// Approximate threshold query: the visible rows of the planned clusters whose stored vector x has q.x >= t
    /// (a subset of the exact answer), decided exactly as `threshold2` decides them.
    pub fn threshold(&self, ix: &Index2, q: &[f32], t: f64, vis: Option<&Vis2>, o: &ProcOpts) -> (ThrOut2, ProcInfo) {
        let cal = ix.calib.as_ref().expect("Process::new checked the calibration");
        let t0 = tick();
        let pr = prep2(ix, q);
        let mut st = Stats::new(ix.meta.nl());
        let mut info = ProcInfo::default();
        let made = self.make(ix, cal, &pr, vis, Target::Thr(t), o, &mut info, &mut st);
        let opts = Opts2 { vis, verify: false, want_iv: false, plan: o.plan };
        let mut out = match made {
            Made::Empty => {
                info.path = "empty";
                ThrOut2 { st: Stats::new(ix.meta.nl()), npow: ix.meta.npow, ..Default::default() }
            }
            Made::Scan => {
                info.path = "scan";
                scan_visible(ix, &pr, t, vis, false, o.plan.pscan)
            }
            Made::Exact => {
                info.path = "exact";
                threshold2(ix, &pr, t, &opts)
            }
            Made::Run { u, b, act } => {
                info.path = "plan";
                let tc = tick();
                let k = ix.k();
                let mut pre = vec![Pre::Skip; k];
                let mut use_idx = vec![false; k];
                let idx: Vec<usize> = (0..b.v.len()).filter(|&i| act[i] == Act::Index).map(|i| u.cl[b.units[i]]).collect();
                if !ix.nukey.is_empty() {
                    let mut gs: Vec<usize> = idx.iter().map(|&ci| ix.cl[ci].g).collect();
                    gs.sort_unstable();
                    gs.dedup();
                    setup_groups(ix, &pr, &gs, &mut st);
                }
                let flat = matches!(o.plan.mode, PlanMode::Flat);
                for (j, &ci) in u.cl.iter().enumerate() {
                    if u.sure[j] {
                        pre[ci] = Pre::In;
                    }
                }
                for i in 0..b.v.len() {
                    let ci = u.cl[b.units[i]];
                    pre[ci] = match act[i] {
                        Act::Skip | Act::Sure => Pre::Skip,
                        Act::Direct => Pre::Direct,
                        Act::Index => {
                            let p = cut_pre(ix, &pr, t, vis, ci, !ix.nukey.is_empty());
                            use_idx[ci] = !flat && matches!(p, Pre::Open { ncand, .. } if ncand > 0);
                            p
                        }
                    };
                }
                st.cyc[C_DIRECT] += info.direct as u64;
                st.cyc[C_INDEX] += info.index as u64;
                st.cyc[C_GROUPS] += info.groups as u64;
                st.cyc[C_SPREADS] += info.spreads as u64;
                run_planned(ix, &pr, t, &opts, &pre, &use_idx, tick() - tc)
            }
        };
        out.st.add(&st);
        out.st.cyc[P_QUERY] += tick() - t0;
        (out, info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    /// The best (value - lam cost) over every closed set of actions, by enumeration: each unit skipped, read directly
    /// or (with its group set up) on the index path, each group's setup charged once if any of its units uses it.
    fn brute(b: &Blocks, lam: f64, ag: f64) -> f64 {
        let n = b.v.len();
        let mut best = f64::NEG_INFINITY;
        for code in 0..3usize.pow(n as u32) {
            let (mut x, mut obj) = (code, 0.0);
            let mut open = vec![false; b.gid.len()];
            for j in 0..b.gid.len() {
                for i in b.gstart[j]..b.gstart[j + 1] {
                    match x % 3 {
                        1 => obj += b.v[i] - lam * b.cd[i],
                        2 => {
                            obj += b.v[i] - lam * b.cx[i];
                            open[j] = true;
                        }
                        _ => {}
                    }
                    x /= 3;
                }
            }
            obj -= lam * ag * open.iter().filter(|&&o| o).count() as f64;
            best = best.max(obj);
        }
        best
    }

    fn random_blocks(rng: &mut rand_chacha::ChaCha8Rng, nu: usize) -> Blocks {
        let mut items: Vec<(usize, usize, f64, f64, f64)> = (0..nu).map(|i| (i, rng.gen_range(0..3), rng.gen::<f64>().powi(3), rng.gen_range(0.2..20.0), rng.gen_range(0.2..20.0))).collect();
        items.sort_by_key(|x| x.1);
        Blocks::of(items.into_iter())
    }

    /// The plan at a price is the maximiser of value - price x cost over the closed sets (brute force over up to 8
    /// units in up to 3 groups), and its value and cost are its actions'.
    #[test]
    fn the_plan_is_the_maximiser_at_its_price() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        for trial in 0..400 {
            let nu = rng.gen_range(1..=8);
            let b = random_blocks(&mut rng, nu);
            let ag = rng.gen_range(0.0..30.0);
            let lam = 10f64.powf(rng.gen_range(-4.0..0.0));
            let mut act = Vec::new();
            let (v, c, g) = plan_at(&b, lam, ag, &mut act);
            let want = brute(&b, lam, ag);
            assert!((v - lam * c - want).abs() <= 1e-9 * (1.0 + want.abs()), "trial {trial}: the plan's objective {} against the best {want}", v - lam * c);
            let mut open = vec![false; b.gid.len()];
            let (mut v2, mut c2) = (0.0, 0.0);
            for j in 0..b.gid.len() {
                for i in b.gstart[j]..b.gstart[j + 1] {
                    match act[i] {
                        Act::Direct => (v2, c2) = (v2 + b.v[i], c2 + b.cd[i]),
                        Act::Index => {
                            (v2, c2) = (v2 + b.v[i], c2 + b.cx[i]);
                            open[j] = true;
                        }
                        Act::Skip | Act::Sure => {}
                    }
                }
            }
            let ng = open.iter().filter(|&&o| o).count();
            assert_eq!(g, ng, "trial {trial}");
            assert!((v - v2).abs() < 1e-12 && (c - c2 - ag * ng as f64).abs() < 1e-9, "trial {trial}");
        }
    }

    /// The sweep lists every plan of the price family: each state is plan_at's at its price, and plan_at at any price
    /// gives one of the states; so the dials that search it find the best of the family (checked against a fine grid
    /// of prices).
    #[test]
    fn the_sweep_lists_every_plan_of_the_prices() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(23);
        let mut act = Vec::new();
        for trial in 0..300 {
            let nu = rng.gen_range(1..=30);
            let b = random_blocks(&mut rng, nu);
            let ag = rng.gen_range(0.0..30.0);
            let states = sweep(&b, ag);
            for &(lam, v, c) in &states {
                let (pv, pc, _) = plan_at(&b, lam, ag, &mut act);
                assert!((pv - v).abs() < 1e-9 && (pc - c).abs() < 1e-9 * (1.0 + c), "trial {trial}: state at {lam} ({v}, {c}) is plan_at's ({pv}, {pc})");
            }
            for s in 0..400 {
                let lam = 10f64.powf(2.0 - s as f64 / 40.0);
                let (pv, pc, _) = plan_at(&b, lam, ag, &mut act);
                assert!(states.iter().any(|x| (x.1 - pv).abs() < 1e-9 && (x.2 - pc).abs() < 1e-9 * (1.0 + pc)), "trial {trial}: plan_at at {lam} ({pv}, {pc}) not in the sweep");
                // the dials' choices are at least as good as any price's plan
                let r = pv;
                let lr = price_for_value(&states, r - 1e-12);
                let (_, cr, _) = plan_at(&b, lr, ag, &mut act);
                assert!(cr <= pc + 1e-9 * (1.0 + pc), "trial {trial}: the recall dial's plan costs {cr}, a price's {pc}");
                let lb = price_for_cost(&states, pc * (1.0 + 1e-9) + 1e-12);
                let (vb, _, _) = plan_at(&b, lb, ag, &mut act);
                assert!(vb >= pv - 1e-9, "trial {trial}: the budget dial's plan holds {vb}, a price's {pv}");
            }
        }
    }

    /// exp_fast is e^x to 4e-16 relative over the range the shares use.
    #[test]
    fn exp_fast_is_exp() {
        let mut worst = 0f64;
        for i in 0..200_000 {
            let x = -745.0 + i as f64 * 0.00376;
            if x < -708.0 {
                continue;
            }
            let (a, b) = (exp_fast(x), x.exp());
            worst = worst.max(((a - b) / b).abs());
        }
        assert!(worst < 4e-16, "{worst}");
    }

    /// erfc to about 1e-15 relative (continued fraction above 2, series below).
    fn erfc(x: f64) -> f64 {
        if x < 0.0 {
            return 2.0 - erfc(-x);
        }
        if x < 2.0 {
            let (mut sum, mut term, mut k) = (x, x, 0.0);
            while term.abs() > 1e-17 * sum.abs() {
                k += 1.0;
                term *= -x * x / k;
                sum += term / (2.0 * k + 1.0);
            }
            return 1.0 - 2.0 / std::f64::consts::PI.sqrt() * sum;
        }
        let mut f = 0.0;
        for k in (1..60).rev() {
            f = k as f64 / 2.0 / (x + f);
        }
        (-x * x).exp() / std::f64::consts::PI.sqrt() / (x + f)
    }

    /// A Gaussian tail tabulated as the build tabulates P: the same grid, a tail slope and beta.
    fn gauss_table() -> Table {
        let (lo, step) = (-3.0, 0.15);
        let g: Vec<f64> = (0..calib::NG).map(|i| (0.5 * erfc((lo + (i as f64 + 0.5) * step) / std::f64::consts::SQRT_2)).ln()).collect();
        let slope = (g[calib::NG - 1] - g[calib::NG - 9]) / (8.0 * step);
        Table { lo, step, g, beta: 0.3, slope }
    }

    /// tau* from the solver is the root of Lambda (each unit's P by std's exp) to within EPS_MASS, the share the units
    /// left out of the solve may hold, on random units of a Gaussian table; a plain bisection finds the same root.
    #[test]
    fn tau_star_solves_lambda() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        let t = gauss_table();
        let cm = Costs1::for_dim(128);
        let model = Model { tabs: [&t, &t], delta: 0.5, cm, bands: &[], vis_share: 1.0, nvis: 1.0 };
        for trial in 0..300 {
            let m: usize = rng.gen_range(1..300);
            let mut u = Units::default();
            for c in 0..m {
                u.push(c, c % 7, rng.gen_range(1..3000) as f64, rng.gen_range(-1.0..1.0), rng.gen_range(-0.5..0.5), rng.gen_range(0.01..0.3), Tier::Group, true, false);
            }
            let tot: f64 = u.nv.iter().sum();
            let kk = rng.gen_range(1..=(tot * 0.4) as usize).min(1000);
            let full = |tau: f64| (0..m).map(|i| u.nv[i] * t.ln_p((tau - u.mu[i]) / u.s[i], u.lr[i]).exp()).sum::<f64>();
            let mut fill = |_: &mut Units, _: &[usize], _: Tier| 0;
            let mut uu = u.clone();
            let mut inf = ProcInfo { scan_cost: f64::INFINITY, ..ProcInfo::default() };
            let _ = decide(&model, &mut uu, &mut fill, Target::Knn(kk), Stop::Price(1e-3), Spread::Group, &mut inf);
            let tau = inf.tau;
            // over every unit (the units left at their bounds hold at most EPS_MASS of it)
            let r = full(tau) / kk as f64 - 1.0;
            assert!(r > -1e-9 && r < EPS_MASS + 1e-9, "trial {trial}: Lambda {} at tau* for k {kk}", full(tau));
            // a plain bisection of the units' own Lambda gives the same root
            let (mut a, mut b) = (-20.0, 20.0);
            for _ in 0..200 {
                let c = 0.5 * (a + b);
                if full(c) >= kk as f64 * (1.0 + r) { a = c } else { b = c }
            }
            assert!((tau - a).abs() < 1e-9 * (1.0 + tau.abs()), "trial {trial}: tau* {tau} against the bisection's {a}");
            assert!(inf.evals <= 40, "trial {trial}: {} evaluations", inf.evals);
        }
    }
}
