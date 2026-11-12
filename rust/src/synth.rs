//! Synthetic items from the fitted cluster model, for scale beyond the 1M real vectors.
//! Item i of cluster c: z~ = ||r|| * sqrt(lam / sum lam) * xi, xi ~ N(0, I) from a per-item seed, ||r|| drawn from the real
//! cluster's residual norms; z = t z~ with t chosen so that x := m + V z has unit norm (up to the basis defect). The
//! item IS x = m + V z in exact arithmetic, so q.x = q.m + (V^T q).z exactly and nothing but z is ever needed. Full
//! vectors are regenerated from the seed when a query falls back to them.
use crate::index::*;
use crate::query::Prep;
use rand::{Rng, SeedableRng};
use rand_distr::StandardNormal;

pub struct Synth {
    pub scale: usize,
    pub sizes: Vec<usize>,       // items per cluster
    pub rnorms: Vec<Vec<f32>>,   // real residual norms per cluster
    pub mv64: Vec<Vec<f64>>,     // V^T m per cluster
    pub mn: Vec<f64>,
    pub lamsc: Vec<Vec<f64>>,    // per group sqrt(lam / sum lam)
    pub ext: Vec<u32>,           // row -> item seed (set after the build)
    pub row_cl: Vec<u32>,
    pub n: usize,
    pub coff: Vec<usize>,
}

impl Synth {
    pub fn new(p: &Params, real: &Index, scale: usize) -> Synth {
        let n = p.meta.n;
        let k = p.meta.k;
        let sizes: Vec<usize> = (0..k).map(|c| (p.cstart[c + 1] - p.cstart[c]) * scale).collect();
        let rnorms = real.cl.iter().map(|c| {
            (c.r0..c.r1).map(|r| {
                let a = real.ahat[r] as f64 / 32767.0;
                (1.0 + c.mn * c.mn - 2.0 * c.mn * a).max(1e-12).sqrt() as f32
            }).collect()
        }).collect();
        let mv64 = (0..k).map(|c| {
            let g = p.grp[c];
            let m = &p.centres[c * n..(c + 1) * n];
            let v = &real.groups[g].vf64;
            let mut out = vec![0f64; n];
            for i in 0..n {
                for kk in 0..n {
                    out[kk] += m[i] as f64 * v[i * n + kk];
                }
            }
            out
        }).collect();
        let mn = (0..k).map(|c| p.centres[c * n..(c + 1) * n].iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt()).collect();
        let lamsc = (0..p.meta.g).map(|g| {
            let l = &p.lam[g * n..(g + 1) * n];
            let s: f64 = l.iter().sum();
            l.iter().map(|&x| (x / s).sqrt()).collect()
        }).collect();
        let mut coff = vec![0usize; k + 1];
        for c in 0..k {
            coff[c + 1] = coff[c] + sizes[c];
        }
        Synth { scale, sizes, rnorms, mv64, mn, lamsc, ext: Vec::new(), row_cl: Vec::new(), n, coff }
    }
    pub fn seed_of(&self, c: usize, i: usize) -> u32 {
        (self.coff[c] + i) as u32
    }
    /// z of item `sid` in cluster c (and its stored key)
    pub fn gen(&self, c: usize, g: usize, sid: u32, z: &mut [f64]) -> (i16, f64) {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0x5eed_0000_0000 ^ sid as u64);
        let rn = &self.rnorms[c];
        let r = rn[rng.gen_range(0..rn.len())] as f64;
        let ls = &self.lamsc[g];
        let mut zz = 0.0;
        let mut mz = 0.0;
        let mv = &self.mv64[c];
        for k in 0..self.n {
            let xi: f64 = rng.sample(StandardNormal);
            z[k] = r * ls[k] * xi;
            zz += z[k] * z[k];
            mz += mv[k] * z[k];
        }
        let mn = self.mn[c];
        let (a2, b, cc) = (zz, 2.0 * mz, mn * mn - 1.0);
        let t = (-b + (b * b - 4.0 * a2 * cc).max(0.0).sqrt()) / (2.0 * a2);
        let mut mz2 = 0.0;
        for k in 0..self.n {
            z[k] *= t;
            mz2 += mv[k] * z[k];
        }
        let a = (mn * mn + mz2) / mn;
        ((a.clamp(-1.0, 1.0) * 32767.0).round() as i16, zz * t * t)
    }
    pub fn exact_score(&self, ix: &Index, pr: &Prep, row: usize) -> f64 {
        let c = self.row_cl[row] as usize;
        let g = ix.cl[c].g;
        let mut st = crate::kernel::Stats::new(0);
        let gq = crate::query::get_gq(ix, pr, g, &mut st);
        let mut z = vec![0f64; self.n];
        self.gen(c, g, self.ext[row], &mut z);
        let qm = crate::util::dot64(&pr.q64, &ix.centres_f64(c));
        qm + crate::util::dot64(&gq.qv, &z)
    }
    pub fn score(&self, ix: &Index, pr: &Prep, row: usize) -> f64 {
        self.exact_score(ix, pr, row)
    }
    pub fn x_f64(&self, ix: &Index, row: usize, out: &mut [f64]) {
        let c = self.row_cl[row] as usize;
        let g = ix.cl[c].g;
        let n = self.n;
        let mut z = vec![0f64; n];
        self.gen(c, g, self.ext[row], &mut z);
        let v = &ix.groups[g].vf64;
        let m = &ix.centres[c * n..(c + 1) * n];
        for i in 0..n {
            let mut s = m[i] as f64;
            for k in 0..n {
                s += v[i * n + k] * z[k];
            }
            out[i] = s;
        }
    }
}

pub struct SynthSource<'a> {
    pub sy: Synth,
    pub groups: &'a [GroupPlan],
}

impl<'a> Source for SynthSource<'a> {
    fn items(&self, c: usize, p: &Params, _gp: &GroupPlan, _mhat: &[f64]) -> Items {
        let n = self.sy.n;
        let g = p.grp[c];
        let nc = self.sy.sizes[c];
        let mut z = vec![0f64; nc * n];
        let mut key: Vec<(i16, u32, usize)> = Vec::with_capacity(nc);
        let mut x2max = 0f64;
        let mut tmp = vec![0f64; n];
        let e = self.groups[g].e_basis;
        for i in 0..nc {
            let sid = self.sy.seed_of(c, i);
            let (ah, zz) = self.sy.gen(c, g, sid, &mut tmp);
            x2max = x2max.max(e * zz + 1e-12);
            z[i * n..(i + 1) * n].copy_from_slice(&tmp);
            key.push((ah, sid, i));
        }
        key.sort();
        let mut zs = vec![0f64; nc * n];
        for (j, k) in key.iter().enumerate() {
            zs[j * n..(j + 1) * n].copy_from_slice(&z[k.2 * n..(k.2 + 1) * n]);
        }
        Items { ah: key.iter().map(|k| k.0).collect(), z: zs, ent: key.iter().map(|k| k.1).collect(), ext: key.iter().map(|k| k.1).collect(), x2max, rmax: 0.0 }
    }
    fn full(self: Box<Self>) -> Full {
        Full::Synth(self.sy)
    }
}
