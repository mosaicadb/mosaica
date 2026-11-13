//! Every query-profile-dependent build parameter, derived from the eigenvalues of each group's local basis and a profile
//! file (G x n query-offset variances in the local basis): the nested bit allocation of the deterministic levels, their
//! certificate weights, the randomised levels' whitening, block membership and rates. The same functions serve the old
//! tree code (distortion table g_tree, tan^2 from g_tree) and the fast-scan code (its own tables).
use crate::util::*;

pub const MENU: [f64; 12] = [0.0, 0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

pub struct Derived {
    pub frb: Vec<u8>,    // G x NDL x n cumulative bits
    pub dl: Vec<f64>,    // G x NDL x n weights (model distortion)
    pub dt: Vec<f64>,    // G x n
    pub ridx: Vec<i32>,  // G x n
    pub rates: Vec<f64>, // G x NR x J
}

pub fn load_profile(path: &str) -> Vec<f64> {
    read_npy(path).f64s()
}

/// Greedy nested allocation on sum sqrt(lam Q g(b)) (largest marginal gain per bit, first maximum on ties).
pub fn det_alloc(lam: &[f64], q: &[f64], g: &[f64], dlv: &[usize]) -> Vec<Vec<u8>> {
    let n = lam.len();
    let a: Vec<f64> = (0..n).map(|c| (q[c] * lam[c]).sqrt()).collect();
    let sg: Vec<f64> = g.iter().map(|x| x.sqrt()).collect();
    let mut b = vec![0usize; n];
    let mut gain: Vec<f64> = (0..n).map(|c| a[c] * (sg[0] - sg[1])).collect();
    let mut used = 0;
    let mut out = Vec::new();
    for &lb in dlv {
        while used < 8 * lb {
            let mut i = 0;
            for c in 1..n {
                if gain[c] > gain[i] {
                    i = c;
                }
            }
            if gain[i] <= 0.0 {
                break;
            }
            b[i] += 1;
            used += 1;
            gain[i] = if b[i] < 8 { a[i] * (sg[b[i]] - sg[b[i] + 1]) } else { -1.0 };
        }
        out.push(b.iter().map(|&x| x as u8).collect());
    }
    out
}

/// Positions coded at a rate: integer rates code every position; sub-bit rates code the first positions, rounded up to
/// `quantum` positions (the fast-scan code needs whole 4-unit groups: quantum 16).
pub fn bits_of(r: f64, nb: usize, quantum: usize) -> usize {
    if r < 1.0 {
        let k = (r * nb as f64) as usize;
        if quantum <= 1 { k } else { ((k + quantum - 1) / quantum * quantum).min(nb) }
    } else {
        (r * nb as f64) as usize
    }
}

/// Greedy design of the randomised levels' block rates (port of the numpy `design`).
pub fn design(s: &[f64], nb: &[usize], budgets: &[usize], c: f64, tof: &dyn Fn(f64) -> f64, quantum: usize) -> Vec<Vec<f64>> {
    let j = s.len();
    let h = |ri: &[usize]| -> f64 {
        let mut unc = 0.0;
        let mut v = 0.0;
        let mut any = false;
        for k in 0..j {
            if ri[k] == 0 {
                unc += s[k];
            } else {
                any = true;
                v += s[k] * s[k] * tof(MENU[ri[k]]) / (nb[k] - 1) as f64;
            }
        }
        if any { unc + c * v.sqrt() } else { s.iter().sum() }
    };
    let mut ri = vec![0usize; j];
    let mut used = 0usize;
    let mut out = Vec::new();
    for &bud in budgets {
        loop {
            let h0 = h(&ri);
            let mut best: Option<(f64, usize, usize, usize)> = None;
            for k in 0..j {
                for k2 in ri[k] + 1..MENU.len() {
                    let db = bits_of(MENU[k2], nb[k], quantum) as i64 - bits_of(MENU[ri[k]], nb[k], quantum) as i64;
                    if db <= 0 || used + db as usize > bud {
                        continue;
                    }
                    let mut r2 = ri.clone();
                    r2[k] = k2;
                    let gn = (h0 - h(&r2)) / db as f64;
                    if best.map_or(true, |b| gn > b.0) {
                        best = Some((gn, k, k2, db as usize));
                    }
                }
            }
            match best {
                Some((gn, k, k2, db)) if gn > 0.0 => {
                    ri[k] = k2;
                    used += db;
                }
                _ => break,
            }
        }
        out.push(ri.iter().map(|&k| MENU[k]).collect());
    }
    out
}

/// Derive the profile-dependent parameters for every group. `gdet` is the model distortion by depth used for the
/// allocation and weights (g(0) = 1), `tof` the randomised code's tan^2 by rate, `quantum` the sub-bit position quantum.
pub fn derive(lam_all: &[f64], prof_all: &[f64], n: usize, g: usize, dlv: &[usize], rlv: &[usize], bounds: &[usize], csg: f64, gdet: &[f64], tof: &dyn Fn(f64) -> f64, quantum: usize) -> Derived {
    let ndl = dlv.len();
    let nr = rlv.len();
    let jn = bounds.len() - 1;
    let mut d = Derived { frb: vec![0; g * ndl * n], dl: vec![0.0; g * ndl * n], dt: vec![0.0; g * n], ridx: vec![0; g * n], rates: vec![0.0; g * nr * jn] };
    for gi in 0..g {
        let lam = &lam_all[gi * n..(gi + 1) * n];
        let q = &prof_all[gi * n..(gi + 1) * n];
        let fr = det_alloc(lam, q, gdet, dlv);
        for l in 0..ndl {
            for c in 0..n {
                let b = fr[l][c] as usize;
                d.frb[(gi * ndl + l) * n + c] = b as u8;
                d.dl[(gi * ndl + l) * n + c] = ((lam[c] * gdet[b] + 1e-10) / (q[c] + 1e-10)).powf(0.25);
            }
        }
        let lres: Vec<f64> = (0..n).map(|c| lam[c] * gdet[fr[ndl - 1][c] as usize]).collect();
        for c in 0..n {
            d.dt[gi * n + c] = ((lres[c] + 1e-10) / (q[c] + 1e-10)).powf(0.25);
        }
        let ai: Vec<f64> = (0..n).map(|c| (lres[c] * q[c]).sqrt()).collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| ai[b].partial_cmp(&ai[a]).unwrap());
        for c in 0..n {
            d.ridx[gi * n + c] = order[c] as i32;
        }
        let sj: Vec<f64> = (0..jn).map(|j| order[bounds[j]..bounds[j + 1]].iter().map(|&c| ai[c]).sum()).collect();
        let nb: Vec<usize> = (0..jn).map(|j| bounds[j + 1] - bounds[j]).collect();
        let budgets: Vec<usize> = rlv.iter().map(|&b| 8 * (b - dlv[ndl - 1])).collect();
        let rates = design(&sj, &nb, &budgets, csg, tof, quantum);
        for lr in 0..nr {
            for j in 0..jn {
                d.rates[(gi * nr + lr) * jn + j] = rates[lr][j];
            }
        }
    }
    d
}

/// tan^2 of the old tree code at rate r (the numpy Tof).
pub fn tof_tree(gtree: &[f64]) -> impl Fn(f64) -> f64 + '_ {
    move |r: f64| {
        if r == 0.0 {
            f64::INFINITY
        } else if r < 1.0 {
            std::f64::consts::PI / (2.0 * r) - 1.0
        } else {
            let g = gtree[r as usize];
            g / (1.0 - g)
        }
    }
}
