//! The approximate process: one rule for approximate k-NN and approximate threshold queries, which chooses the
//! clusters to read, the existing path each is read by (its visible rows scored directly, or the index path, which
//! needs its group set up), the order, and when to stop. Inside a cluster nothing changes: the index path, its
//! bounds and the exact decisions are the library's. Every answer is decided exactly where it is read.
//!
//! Per query, over the clusters holding a visible row (units):
//! - each cluster's centre score mu_c = q.m_c (int8 centres), its spread s_c(q) from the chosen tier (`Spread`), and
//!   its value: the predicted share of the answer s_c = n_c^vis P_c(tau) / Lambda(tau), P from the stored exceedance
//!   table (`calib::Table`) at zeta = (tau - mu_c) / s_c, Lambda(tau) the sum of n^vis P over the units;
//! - k-NN: tau = tau*, the root of Lambda(tau*) = k, solved once and never updated; threshold: tau = t, the shares
//!   normalised by max(Lambda(t), 1), and clusters the support table decides (wholly below t, or wholly above) valued
//!   exactly;
//! - each unit's cost by the cost model (`Costs1`): read directly, dir + full n^vis; on the index path, cl + item n^vis,
//!   with its group's setup grp(n) charged once per group;
//! - the plan at a price lambda (`plan_at`): the closed set of actions maximising (sum of shares) - lambda (cost), a
//!   group set up exactly when the index path saves its clusters more than its setup; a per-query recall target takes
//!   the cheapest plan whose shares reach it, a budget the largest plan within it, a mean recall the price the build
//!   fitted for it (`calib::Band`);
//! - the plan run best value per cost first, each group's setup before its first cluster; k-NN reads its clusters
//!   at the running k-th best as `knn2_fast` does (`KnnReader`), a threshold query as the exact one does
//!   (`run_planned`) over the planned clusters only;
//! - hand-over: when the plan's modelled cost reaches that of scanning the visible rows, the query scans them; when
//!   it reaches that of reading every unit (an upper bound of the exact path's), or the dial asks for everything, the
//!   exact query runs. Both answer exactly.
//!
//! The units form one level (clusters in groups); a finer level (sub-clusters as row labels) would add units under a
//! cluster with their own table row and an expansion cost, and the plan's closed sets one more layer.

use super::calib::{Calib, Tier};
use super::q::*;
use super::*;
use crate::kernel::Stats;

/// The process's cost model, in microseconds on one thread, for vectors of n dimensions (only ratios move the plan;
/// the scale sets the unit of the price). Derived from timings on this machine at n = 128, 384 and 512 (the suite's
/// glove, yahoo and LAION indexes, AVX-512):
/// - `grp`, a group's setup (basis product, post-processing, rotations), a + b n^2 fitted by least squares to the
///   medians of 4, 15 and 28 us (`bench/suite/notes/visit-order/groups-setup.md`): 1.85 + 9.7e-5 n^2;
/// - `item`, a visible row on the index path, a + b n fitted to 0.027, 0.040 and 0.057 us (the query's work past the
///   group setup at nprobe 16 over the rows it read, less `cl` a cluster: `results/phases-1-int8-order.txt`):
///   0.016 + 7.4e-5 n;
/// - `full`, a visible row scored from its stored vector, fitted to 0.030, 0.045 and 0.070 us (the stored-vector
///   phase per row under random-1): 0.020 + 8.4e-5 n;
/// - `cl`, a cluster's setup on the index path, `COSTS`' cl_us0 + cl_us1 d at the undecided share d = 0.1 an
///   approximate query's cut leaves (4 us; not separable in the phase timings); `dir`, a cluster scored directly,
///   0.2 us;
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
        Costs1 { grp: 1.85 + 9.7e-5 * x * x, cl: COSTS.cl_us0 + COSTS.cl_us1 * 0.1, item: 0.016 + 7.4e-5 * x, dir: 0.2, full: 0.020 + 8.4e-5 * x, row8: 0.0385 + 6.5e-5 * x }
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
/// contiguous (`gstart`), and each unit's position in the caller's list (`units`).
pub struct Blocks {
    pub units: Vec<usize>,
    pub gstart: Vec<usize>,
    pub gid: Vec<usize>,
    pub v: Vec<f64>,
    pub cd: Vec<f64>,
    pub cx: Vec<f64>,
}

impl Blocks {
    /// The units `idx` of a list of clusters `cl` (of `ix`) with values `v` and visible rows `nv`, costed by `cm`.
    pub fn new(ix: &Index2, idx: &[usize], cl: &[usize], v: &[f64], nv: &[f64], cm: &Costs1) -> Blocks {
        let mut idx = idx.to_vec();
        idx.sort_by_key(|&i| (ix.cl[cl[i]].g, cl[i]));
        Blocks::of(idx.iter().map(|&i| (i, ix.cl[cl[i]].g, v[i], cm.direct(nv[i]), cm.index(nv[i]))))
    }

    /// From (position, group, value, direct cost, index cost), grouped by group.
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

    /// The units whose values sum to all but `drop` of the total, the smallest left out (for the dials that search
    /// over prices).
    fn without_least(&self, drop: f64) -> Blocks {
        let mut o: Vec<usize> = (0..self.v.len()).collect();
        o.sort_by(|&a, &b| self.v[a].total_cmp(&self.v[b]).then(a.cmp(&b)));
        let mut keep = vec![true; self.v.len()];
        let mut acc = 0.0;
        for &i in &o {
            acc += self.v[i];
            if acc > drop {
                break;
            }
            keep[i] = false;
        }
        let mut items = Vec::new();
        for j in 0..self.gid.len() {
            for i in self.gstart[j]..self.gstart[j + 1] {
                if keep[i] {
                    items.push((self.units[i], self.gid[j], self.v[i], self.cd[i], self.cx[i]));
                }
            }
        }
        Blocks::of(items.into_iter())
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
        let mut gain = 0.0;
        for i in r.clone() {
            let (dn, xn) = ((b.v[i] - lam * b.cd[i]).max(0.0), (b.v[i] - lam * b.cx[i]).max(0.0));
            gain += (xn - dn).max(0.0);
        }
        let open = gain > lam * ag;
        if open {
            groups += 1;
            cost += ag;
        }
        for i in r {
            let (dn, xn) = (b.v[i] - lam * b.cd[i], b.v[i] - lam * b.cx[i]);
            act[i] = if open && xn > dn.max(0.0) {
                cost += b.cx[i];
                Act::Index
            } else if dn > 0.0 {
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

/// The root of mass(tau) = want, mass positive and non-increasing (a sum of n P(tau)), by modified regula falsi on
/// ln mass from a bracket grown from `top` in steps of `step` doubling; the bracket's end when mass never reaches
/// `want` or never falls below it. Returns tau and the evaluations of mass.
pub fn solve_tau(mass: &dyn Fn(f64) -> f64, top: f64, step: f64, want: f64) -> (f64, usize) {
    let lw = want.ln();
    let mut ev = 0usize;
    let mut h = |t: f64| {
        ev += 1;
        mass(t).max(1e-300).ln() - lw
    };
    let step = if step > 0.0 && step.is_finite() { step } else { 1.0 };
    let (mut a, mut d) = (top, step);
    let mut ha = h(a);
    while ha < 0.0 && d < 1e6 * step {
        a -= d;
        d *= 2.0;
        ha = h(a);
    }
    if ha < 0.0 {
        return (a, ev);
    }
    let (mut b, mut d) = (a + step, step);
    let mut hb = h(b);
    while hb >= 0.0 && d < 1e6 * step {
        a = b;
        ha = hb;
        b += d;
        d *= 2.0;
        hb = h(b);
    }
    if hb >= 0.0 {
        return (b, ev);
    }
    let mut side = 0i8;
    // the bracket's width two steps back: a step after two that did not halve it bisects
    let mut width = [b - a; 2];
    for it in 0..200 {
        let w = b - a;
        let slow = it >= 2 && w > 0.5 * width[it % 2];
        width[it % 2] = w;
        let c = b - hb * (b - a) / (hb - ha);
        let c = if c > a && c < b && !slow { c } else { 0.5 * (a + b) };
        let hc = h(c);
        if hc.abs() < 1e-10 || (b - a) <= 1e-13 * (1.0 + c.abs()) {
            return (c, ev);
        }
        if hc < 0.0 {
            b = c;
            hb = hc;
            if side == -1 {
                ha *= 0.5;
            }
            side = -1;
        } else {
            a = c;
            ha = hc;
            if side == 1 {
                hb *= 0.5;
            }
            side = 1;
        }
    }
    (0.5 * (a + b), ev)
}

/// Where each unit's spread comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    /// The group's covariance for every unit (`Tier::Group`).
    Group,
    /// The group's first, then the cluster's own covariance (`Tier::Own`) for the units whose group-tier value per
    /// cost lies within the calibration's margin (`Calib::delta`) of the plan's marginal one or above it, and the plan
    /// made again.
    Own,
    /// The cluster's own covariance for every unit.
    OwnAll,
}

impl Spread {
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
    fn everything(self) -> bool {
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
    /// The control tables (plain pseudo-queries only).
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
    /// Modelled microseconds: the plan's reads; the centre scores and spreads; a scan of the visible rows; reading
    /// every unit.
    pub cost: f64,
    pub fixed: f64,
    pub scan_cost: f64,
    pub all_cost: f64,
    pub units: usize,
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

/// The process's per-index state: the int8 centres, each cluster's log relative radius and its own spread's rest,
/// each group's rest, and the cost model.
pub struct Process {
    cen8: Vec<i8>,
    csc: Vec<f32>,
    lr: Vec<f64>,
    orest: Vec<f64>,
    grest: Vec<f64>,
    pub cm: Costs1,
}

/// A query's units and their features.
struct Units {
    cl: Vec<usize>,
    nv: Vec<f64>,
    mu: Vec<f64>,
    s: Vec<f64>,
    tier: Vec<Tier>,
    /// threshold queries: the support table's verdict (Some(true): wholly above t; the units wholly below are gone)
    sure: Vec<bool>,
}

/// A query of the process: k-NN at k, or a threshold.
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Knn(usize),
    Thr(f64),
}

/// A plan ready to run, or the path the query was handed to.
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
        let r = st.r;
        let orest = (0..ix.k()).map(|c| {
            let top: f64 = st.lam[c * r..(c + 1) * r].iter().map(|&l| l as f64).sum();
            (st.rad[c] * st.rad[c] - top).max(0.0) / (st.n0.max(r + 1) - r) as f64
        }).collect();
        let grest = (0..ix.groups.len()).map(|g| cal.gcov.rest(g)).collect();
        Ok(Process { cen8, csc, lr: cal.lr(ix), orest, grest, cm: Costs1::for_dim(ix.n()) })
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

    /// sigma_g(q) of every group (one int8 product over every group's directions).
    fn group_sigmas(&self, ix: &Index2, cal: &Calib, q32: &[f32], qq: f64) -> Vec<f64> {
        let (gc, n) = (&cal.gcov, ix.n());
        let r = gc.r;
        let ng = ix.groups.len();
        let mut p = vec![0f32; ng * r];
        unsafe { crate::kernel::gemv_i8(q32, &gc.dirs8, n, &mut p) };
        (0..ng).map(|g| {
            let (mut v, mut pp) = (0f64, 0f64);
            for j in 0..r {
                let x = p[g * r + j] as f64 * gc.dsc[g * r + j] as f64;
                v += gc.lam[g * r + j] as f64 * x * x;
                pp += x * x;
            }
            (v + self.grest[g] * (qq - pp).max(0.0)).max(0.0).sqrt()
        }).collect()
    }

    /// sigma_c(q) of cluster c from the order state's directions.
    fn own_sigma(&self, ix: &Index2, c: usize, q32: &[f32], qq: f64, p: &mut [f32]) -> f64 {
        let st = ix.order.as_ref().unwrap();
        let (r, n) = (st.r, ix.n());
        unsafe { crate::kernel::gemv_i8(q32, st.dirs(c, r, n), n, &mut p[..r]) };
        let (mut v, mut pp) = (0f64, 0f64);
        for j in 0..r {
            let x = p[j] as f64 * st.dsc[c * r + j] as f64;
            v += st.lam[c * r + j] as f64 * x * x;
            pp += x * x;
        }
        (v + self.orest[c] * (qq - pp).max(0.0)).max(0.0).sqrt()
    }

    /// Unit i's ln P at tau.
    #[inline]
    fn ln_p(&self, cal: &Calib, plain: bool, u: &Units, i: usize, tau: f64) -> f64 {
        if u.sure[i] {
            return 0.0;
        }
        cal.table(u.tier[i], plain).ln_p((tau - u.mu[i]) / u.s[i], self.lr[u.cl[i]])
    }

    /// tau and the units' shares at it: tau* for k-NN (solved once), t for a threshold query.
    fn shares(&self, cal: &Calib, o: &ProcOpts, u: &Units, target: &Target, info: &mut ProcInfo) -> (f64, Vec<f64>) {
        let tau = match *target {
            Target::Thr(t) => t,
            Target::Knn(kk) => {
                let mass = |t: f64| (0..u.cl.len()).map(|i| u.nv[i] * self.ln_p(cal, o.plain, u, i, t).exp()).sum::<f64>();
                let top = u.mu.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let step = u.s.iter().cloned().fold(0.0, f64::max);
                let (t, ev) = solve_tau(&mass, top, step, kk as f64);
                info.evals += ev;
                t
            }
        };
        let w: Vec<f64> = (0..u.cl.len()).map(|i| u.nv[i] * self.ln_p(cal, o.plain, u, i, tau).exp()).collect();
        let sum: f64 = w.iter().sum();
        info.lambda = sum;
        let norm = match target {
            Target::Knn(_) => sum.max(1e-300),
            Target::Thr(_) => sum.max(1.0),
        };
        (tau, w.into_iter().map(|x| x / norm).collect())
    }

    /// The plan at the dial: its price, value, cost and groups.
    fn plan_dial(&self, cal: &Calib, tier: Tier, b: &Blocks, stop: Stop, target: &Target, nvis: f64, lam_thr: f64, act: &mut Vec<Act>) -> (f64, f64, f64, usize) {
        let ag = self.cm.grp;
        let search = |ok: &dyn Fn(f64, f64) -> bool, hi_ok: bool, act: &mut Vec<Act>| -> f64 {
            // prices over the units' densities: none is taken above the largest
            let top = (0..b.v.len()).map(|i| b.v[i] / b.cd[i].min(b.cx[i])).fold(0.0, f64::max) * (1.0 + 1e-9);
            let p = b.without_least(if hi_ok { 0.0 } else { 1e-9 });
            let (mut lo, mut hi) = (top * 2f64.powi(-60), top);
            let at = |lam: f64, act: &mut Vec<Act>| {
                let (v, c, _) = plan_at(&p, lam, ag, act);
                ok(v, c)
            };
            if hi_ok {
                // ok holds at high prices (a budget): the lowest price within it
                if at(lo, act) {
                    return lo;
                }
                for _ in 0..40 {
                    let mid = (lo * hi).sqrt();
                    if at(mid, act) { hi = mid } else { lo = mid }
                }
                hi
            } else {
                // ok holds at low prices (a recall): the highest price reaching it
                if !at(lo, act) {
                    return 0.0;
                }
                for _ in 0..40 {
                    let mid = (lo * hi).sqrt();
                    if at(mid, act) { lo = mid } else { hi = mid }
                }
                lo
            }
        };
        let lam = match stop {
            Stop::Price(l) => l,
            Stop::MeanRecall(r) => match target {
                Target::Knn(kk) => cal.price(tier, true, *kk as f64, r),
                Target::Thr(_) => cal.price(tier, false, lam_thr / nvis.max(1.0), r),
            },
            Stop::Recall(r) => search(&|v, _| v >= r, false, act),
            Stop::Budget(bu) => search(&|_, c| c <= bu, true, act),
        };
        let (v, c, g) = plan_at(b, lam, ag, act);
        (lam, v, c, g)
    }

    /// The query's units, shares and plan, or the path it is handed to.
    fn make(&self, ix: &Index2, cal: &Calib, pr: &Prep2, vis: Option<&Vis2>, target: Target, o: &ProcOpts, info: &mut ProcInfo, st: &mut Stats) -> Made {
        let t0 = tick();
        let k = ix.k();
        let nv_of = |ci: usize| vis.map_or(ix.cl[ci].r1 - ix.cl[ci].r0, |v| v.cl_cnt[ci] as usize);
        let live: Vec<usize> = (0..k).filter(|&ci| nv_of(ci) > 0).collect();
        let nvis: usize = live.iter().map(|&ci| nv_of(ci)).sum();
        info.units = live.len();
        info.scan_cost = nvis as f64 * self.cm.full;
        let tier0 = if o.spread == Spread::OwnAll { Tier::Own } else { Tier::Group };
        let r = cal.gcov.r;
        if nvis == 0 {
            return Made::Empty;
        }
        if matches!(o.plan.mode, PlanMode::Scan) || matches!(target, Target::Knn(kk) if nvis <= kk) {
            return Made::Scan;
        }
        if o.stop.everything() {
            return Made::Exact;
        }
        // a threshold query's support-table verdicts (its anchor's search costs a row product per anchor): units
        // wholly below t are dropped, those wholly above are sure
        let verdict: Vec<Option<Pre>> = match target {
            Target::Thr(t) if ix.nukey.is_empty() && ix.nanch > 0 => {
                info.fixed += ix.nanch as f64 * self.cm.full;
                if info.scan_cost <= info.fixed {
                    return Made::Scan;
                }
                pr.anc_among(ix, Some(&live));
                live.iter().map(|&ci| support_pre(ix, pr, t, nv_of(ci), ci, false)).collect()
            }
            _ => vec![None; live.len()],
        };
        let free: Vec<usize> = (0..live.len()).filter(|&j| verdict[j].is_none()).collect();
        // the centre scores and the spreads of the units the plan decides
        let spread_rows = if free.is_empty() { 0 } else if tier0 == Tier::Group { ix.groups.len() * r } else { free.len() * r };
        info.fixed += (k + spread_rows) as f64 * self.cm.row8;
        if info.scan_cost <= info.fixed {
            return Made::Scan;
        }
        let qm = self.centre_scores(ix, &pr.q32);
        st.cyc[P_PLAN] += tick() - t0;
        let ts = tick();
        let qq: f64 = pr.q32.iter().map(|&x| x as f64 * x as f64).sum();
        let mut u = Units { cl: Vec::new(), nv: Vec::new(), mu: Vec::new(), s: Vec::new(), tier: Vec::new(), sure: Vec::new() };
        let sg = if tier0 == Tier::Group && !free.is_empty() { self.group_sigmas(ix, cal, &pr.q32, qq) } else { Vec::new() };
        let mut p = vec![0f32; r.max(1)];
        for (j, &ci) in live.iter().enumerate() {
            let sure = matches!(verdict[j], Some(Pre::In));
            if matches!(verdict[j], Some(Pre::Skip)) {
                continue;
            }
            let s = match tier0 {
                _ if sure => 1.0,
                Tier::Group => sg[ix.cl[ci].g] * ix.order.as_ref().unwrap().rad[ci] / cal.gcov.rad[ix.cl[ci].g].max(1e-300),
                Tier::Own => self.own_sigma(ix, ci, &pr.q32, qq, &mut p),
            };
            u.cl.push(ci);
            u.nv.push(nv_of(ci) as f64);
            u.mu.push(qm[ci]);
            u.s.push(s.max(1e-12));
            u.tier.push(tier0);
            u.sure.push(sure);
        }
        info.spreads += spread_rows;
        st.cyc[P_SPREAD] += tick() - ts;
        if u.cl.is_empty() {
            return Made::Empty;
        }
        let tp = tick();
        let (mut tau, mut v) = self.shares(cal, o, &u, &target, info);
        let lam_size = |u: &Units, tau: f64| (0..u.cl.len()).map(|i| u.nv[i] * self.ln_p(cal, o.plain, u, i, tau).exp()).sum::<f64>();
        // the units the support table puts wholly above t are emitted without a read: outside the plan, their shares
        // taken as given (a recall target asks the plan for the rest)
        let free: Vec<usize> = (0..u.cl.len()).filter(|&i| !u.sure[i]).collect();
        let sure = |v: &[f64]| (0..u.cl.len()).filter(|&i| u.sure[i]).map(|i| v[i]).sum::<f64>();
        let stop = |v: &[f64]| match o.stop {
            Stop::Recall(r) => Stop::Recall((r - sure(v)).max(0.0)),
            s => s,
        };
        let mut b = Blocks::new(ix, &free, &u.cl, &v, &u.nv, &self.cm);
        let mut act = Vec::new();
        let size = if matches!(target, Target::Thr(_)) { lam_size(&u, tau) } else { 0.0 };
        let (mut lam, mut val, mut cost, mut groups) = self.plan_dial(cal, tier0, &b, stop(&v), &target, nvis as f64, size, &mut act);
        st.cyc[P_SOLVE] += tick() - tp;
        if o.spread == Spread::Own {
            let ts = tick();
            // the plan's marginal value per cost (the best unit's when it is empty), and the units within the margin of it
            let dens = |i: usize| b.v[i] / b.cd[i].min(b.cx[i]);
            let taken: Vec<usize> = (0..b.v.len()).filter(|&i| act[i] != Act::Skip).collect();
            let rho = if taken.is_empty() { (0..b.v.len()).map(dens).fold(0.0, f64::max) } else { taken.iter().map(|&i| dens(i)).fold(f64::INFINITY, f64::min) };
            let floor = rho * (-cal.delta).exp();
            for i in 0..b.v.len() {
                let j = b.units[i];
                if dens(i) >= floor && !u.sure[j] {
                    u.s[j] = self.own_sigma(ix, u.cl[j], &pr.q32, qq, &mut p).max(1e-12);
                    u.tier[j] = Tier::Own;
                    info.refined += 1;
                }
            }
            info.spreads += info.refined * r;
            st.cyc[P_SPREAD] += tick() - ts;
            let tp = tick();
            (tau, v) = self.shares(cal, o, &u, &target, info);
            b = Blocks::new(ix, &free, &u.cl, &v, &u.nv, &self.cm);
            let size = if matches!(target, Target::Thr(_)) { lam_size(&u, tau) } else { 0.0 };
            (lam, val, cost, groups) = self.plan_dial(cal, Tier::Own, &b, stop(&v), &target, nvis as f64, size, &mut act);
            st.cyc[P_SOLVE] += tick() - tp;
        }
        if let Target::Knn(kk) = target {
            let (dv, dc) = fill_to_k(&b, &mut act, &u.nv, kk);
            val += dv;
            cost += dc;
        }
        info.tau = tau;
        info.lam = lam;
        info.pred = (val + sure(&v)).min(1.0);
        info.cost = cost;
        info.groups = groups;
        info.all_cost = b.all_cost(self.cm.grp);
        info.direct = act.iter().filter(|&&a| a == Act::Direct).count();
        info.index = act.iter().filter(|&&a| a == Act::Index).count();
        if cost >= info.scan_cost {
            return Made::Scan;
        }
        if !b.v.is_empty() && cost >= info.all_cost {
            return Made::Exact;
        }
        Made::Run { u, b, act }
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
                st.full_reads += vis.map_or(ix.nrows, |v| v.rows.len()) as u64;
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
                pr.qms(ix, Some(&cls));
                let mut gs: Vec<usize> = ord.iter().filter(|x| x.1 == Act::Index).map(|&(i, _)| ix.cl[u.cl[b.units[i]]].g).collect();
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

    /// The plan at a price is the maximiser of value - price x cost over the closed sets (brute force over up to 8
    /// units in up to 3 groups), and its value and cost are its actions'.
    #[test]
    fn the_plan_is_the_maximiser_at_its_price() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        for trial in 0..400 {
            let nu = rng.gen_range(1..=8);
            let mut items: Vec<(usize, usize, f64, f64, f64)> = (0..nu).map(|i| (i, rng.gen_range(0..3), rng.gen::<f64>().powi(3), rng.gen_range(0.2..20.0), rng.gen_range(0.2..20.0))).collect();
            items.sort_by_key(|x| x.1);
            let b = Blocks::of(items.into_iter());
            let ag = rng.gen_range(0.0..30.0);
            let lam = 10f64.powf(rng.gen_range(-4.0..0.0));
            let mut act = Vec::new();
            let (v, c, g) = plan_at(&b, lam, ag, &mut act);
            let want = brute(&b, lam, ag);
            assert!((v - lam * c - want).abs() <= 1e-9 * (1.0 + want.abs()), "trial {trial}: the plan's objective {} against the best {want}", v - lam * c);
            // value, cost and groups are those of the actions taken
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

    /// tau* is the root of the mass: within 1e-9 of k relative, from brackets far on either side.
    #[test]
    fn tau_star_solves_the_mass() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        for _ in 0..200 {
            let m: usize = rng.gen_range(1..40);
            let mu: Vec<f64> = (0..m).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let s: Vec<f64> = (0..m).map(|_| rng.gen_range(0.01..0.3)).collect();
            let nv: Vec<f64> = (0..m).map(|_| rng.gen_range(1..3000) as f64).collect();
            let tot: f64 = nv.iter().sum();
            let want = rng.gen_range(1.0..tot * 0.5);
            // a Gaussian tail
            let mass = |t: f64| (0..m).map(|i| nv[i] * 0.5 * erfc((t - mu[i]) / s[i] / std::f64::consts::SQRT_2)).sum::<f64>();
            for top in [mu.iter().cloned().fold(f64::NEG_INFINITY, f64::max), -5.0, 5.0] {
                let (t, _) = solve_tau(&mass, top, 0.1, want);
                assert!((mass(t) / want - 1.0).abs() < 1e-8, "mass {} at {t}, want {want}", mass(t));
            }
        }
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
}
