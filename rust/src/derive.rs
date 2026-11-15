//! The query profile (G x n query-offset variances in each group's local basis) and the greedy design of the randomised
//! levels' block rates that the build derives from it.
use crate::util::*;

pub const MENU: [f64; 12] = [0.0, 0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

pub fn load_profile(path: &str) -> Vec<f64> {
    read_npy(path).f64s()
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
    design_by(s, nb, budgets, &|_| c, tof, quantum)
}

/// As `design`, with the bound's constant a function of the set of coded blocks (bit k: block k).
pub fn design_by(s: &[f64], nb: &[usize], budgets: &[usize], cf: &dyn Fn(u32) -> f64, tof: &dyn Fn(f64) -> f64, quantum: usize) -> Vec<Vec<f64>> {
    let j = s.len();
    let h = |ri: &[usize]| -> f64 {
        let mut unc = 0.0;
        let mut v = 0.0;
        let mut set = 0u32;
        for k in 0..j {
            if ri[k] == 0 {
                unc += s[k];
            } else {
                set |= 1 << k;
                v += s[k] * s[k] * tof(MENU[ri[k]]) / (nb[k] - 1) as f64;
            }
        }
        if set != 0 { unc + cf(set) * v.sqrt() } else { s.iter().sum() }
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
