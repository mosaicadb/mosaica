//! The fit: from a partition of the vectors (`tools/partition.py`), the parameters the build reads, written as the
//! directory `tools/fit.py` writes, without its copy of the vectors: the build reads them from the partition in the
//! fit's row order (`ord{c}.u32`, `clcell.npy`, `clpos.npy`). Every pass goes cell by cell or cluster by cluster; nothing holds
//! the vectors in memory.
//!
//! usage: vexact fit OUTDIR --partition DIR [--basis pca|dual] [--query-sample QUERIES.npy] [--seed S] [--cert-time SECONDS]
//!        [--anchors A] [--text-anchors PROMPTS.npy] [--text-count 256]
//!        vexact anchors FITDIR [--anchors A] [--text-anchors PROMPTS.npy] [--text-count 256] [--seed S]

use crate::util::*;
use faer::linalg::solvers::DenseSolveCore;
use faer::{Mat, Side};
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::io::Write;

// Design constants (not fitted), as tools/fit.py: the guarantee's N and epsilon for the rate design, the
// deterministic level ends, the randomised level ends up to 256 B, randomised block boundaries as fractions of n, the
// H25 hedge weight, the fit format, the ridge of Sigma_Q.
const NTARGET: f64 = 1e9;
const EPS: f64 = 1e-9;
const DLV: [usize; 6] = [4, 8, 12, 16, 24, 32];
const RLV_BASE: [usize; 4] = [56, 96, 160, 256];
const BLOCK_FRACS: [f64; 3] = [0.125, 0.29167, 0.625];
const HEDGE: f64 = 0.25;
const FORMAT: u64 = 2;
const RIDGE: f64 = 0.1;

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).map(|i| args.get(i + 1).unwrap_or_else(|| panic!("{key} needs a value")).clone())
}

fn save<T: Copy>(dir: &str, name: &str, descr: &str, shape: &[usize], v: &[T]) {
    let b = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) };
    write_npy(&format!("{dir}/{name}.npy"), descr, shape, b);
}

/// Randomised level ends and block bounds, as tools/fit.py `schedule`.
fn schedule(n0: usize, n: usize) -> (Vec<usize>, Vec<usize>) {
    let cap = (0.4 * 2.0 * n0 as f64).min(256.0 * (n0 as f64 / 768.0).max(1.0)).min((DLV[5] + n) as f64);
    let mut rlv: Vec<usize> = RLV_BASE.iter().copied().filter(|&b| b as f64 <= cap).collect();
    if rlv.len() < RLV_BASE.len() {
        let top = (cap as usize / 8 * 8).max(DLV[5] + 8.min(n));
        let prev = *rlv.last().unwrap_or(&DLV[5]);
        if top > prev {
            let mut mids: Vec<usize> = Vec::new();
            loop {
                let last = *mids.last().unwrap_or(&prev);
                if top as f64 / last as f64 <= 1.8 {
                    break;
                }
                let m = ((last as f64 * top as f64).sqrt() / 8.0).round_ties_even() as usize * 8;
                if m <= last || m >= top {
                    break;
                }
                mids.push(m);
            }
            rlv.extend(mids);
            rlv.push(top);
        }
    } else {
        let mut b = RLV_BASE[3] as f64;
        loop {
            b = (b * 1.6 / 32.0).round_ties_even() * 32.0;
            if b > cap {
                break;
            }
            rlv.push(b as usize);
        }
    }
    assert!(!rlv.is_empty(), "empty randomised schedule");
    let mut bounds = vec![0usize];
    bounds.extend(BLOCK_FRACS.iter().map(|f| (f * n as f64 / 16.0).round_ties_even() as usize * 16));
    bounds.push(n);
    for j in 1..bounds.len() {
        bounds[j] = bounds[j].max(bounds[j - 1] + 16);
    }
    assert!(bounds[bounds.len() - 2] + 16 <= n, "n = {n} is too small for four randomised blocks");
    (rlv, bounds)
}

/// Spherical k-means of the rows of `s` (m x n, unit) to k centres, as combined/core.py `kmeans`.
fn kmeans(s: &[f32], n: usize, k: usize, rng: &mut ChaCha8Rng, iters: usize) -> Vec<f32> {
    let m = s.len() / n;
    let pick = rand::seq::index::sample(rng, m, k).into_vec();
    let mut c: Vec<f32> = pick.iter().flat_map(|&i| s[i * n..(i + 1) * n].iter().copied()).collect();
    for _ in 0..iters {
        let a = argmax_rows(s, &c, n);
        let mut sum = vec![0f64; k * n];
        let mut cnt = vec![0usize; k];
        for (i, &j) in a.iter().enumerate() {
            cnt[j] += 1;
            for t in 0..n {
                sum[j * n + t] += s[i * n + t] as f64;
            }
        }
        for j in 0..k {
            if cnt[j] == 0 {
                let r = rng.gen_range(0..m);
                for t in 0..n {
                    sum[j * n + t] = s[r * n + t] as f64;
                }
            }
            let nr = sum[j * n..(j + 1) * n].iter().map(|x| x * x).sum::<f64>().sqrt();
            for t in 0..n {
                c[j * n + t] = (sum[j * n + t] / nr) as f32;
            }
        }
    }
    c
}

/// For each row of `s`, the index of the row of `c` with the largest inner product.
fn argmax_rows(s: &[f32], c: &[f32], n: usize) -> Vec<usize> {
    let k = c.len() / n;
    s.par_chunks(n * 1024).flat_map_iter(|blk| {
        let m = blk.len() / n;
        let mut d = vec![0f32; m * k];
        unsafe { matrixmultiply::sgemm(m, n, k, 1.0, blk.as_ptr(), n as isize, 1, c.as_ptr(), 1, n as isize, 0.0, d.as_mut_ptr(), k as isize, 1) };
        (0..m).map(move |i| (0..k).fold(0, |b, j| if d[i * k + j] > d[i * k + b] { j } else { b }))
    }).collect()
}

fn mat(a: &[f64], n: usize) -> Mat<f64> {
    Mat::from_fn(n, n, |i, j| a[i * n + j])
}

/// Eigenvalues and eigenvectors (columns, row-major n x n) of a symmetric matrix, largest first.
fn eigh_desc(a: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let e = mat(a, n).self_adjoint_eigen(Side::Lower).expect("symmetric eigendecomposition failed");
    let (s, u) = (e.S().column_vector(), e.U());
    let mut o: Vec<usize> = (0..n).collect();
    o.sort_by(|&x, &y| s[y].partial_cmp(&s[x]).unwrap());
    let vals = o.iter().map(|&i| s[i]).collect();
    let mut vecs = vec![0f64; n * n];
    for (c, &i) in o.iter().enumerate() {
        for r in 0..n {
            vecs[r * n + c] = u[(r, i)];
        }
    }
    (vals, vecs)
}

fn matmul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let mut c = vec![0f64; n * n];
    unsafe { matrixmultiply::dgemm(n, n, n, 1.0, a.as_ptr(), n as isize, 1, b.as_ptr(), n as isize, 1, 0.0, c.as_mut_ptr(), n as isize, 1) };
    c
}

/// The f64 basis the build makes from V (f32, row-major, columns are basis vectors): its int8 copy with one scale per
/// column.
fn int8_basis64(v: &[f32], n: usize) -> Vec<f64> {
    let (v8t, vsc) = crate::v2::int8_basis(v, n);
    let mut q = vec![0f64; n * n];
    for i in 0..n {
        for c in 0..n {
            q[i * n + c] = v8t[c * n + i] as f64 * vsc[c];
        }
    }
    q
}

/// Covariance of the offsets q - (q.m^) m^ of unit queries `qs` from 40 centres each, drawn with weights `w`, as
/// tools/fit.py `offset_cov`.
fn offset_cov(qs: &[f32], mh: &[f32], n: usize, w: &WeightedIndex<usize>, rng: &mut ChaCha8Rng) -> Vec<f64> {
    let nq = qs.len() / n;
    let draws: Vec<usize> = (0..nq * 40).map(|_| w.sample(rng)).collect();
    let parts: Vec<Vec<f64>> = (0..nq).collect::<Vec<_>>().par_chunks(200).map(|qi| {
        let mut d = vec![0f64; qi.len() * 40 * n];
        for (a, &i) in qi.iter().enumerate() {
            let q = &qs[i * n..(i + 1) * n];
            for t in 0..40 {
                let m = &mh[draws[i * 40 + t] * n..(draws[i * 40 + t] + 1) * n];
                let dot: f32 = q.iter().zip(m).map(|(a, b)| a * b).sum();
                let row = &mut d[(a * 40 + t) * n..(a * 40 + t + 1) * n];
                for k in 0..n {
                    row[k] = (q[k] - dot * m[k]) as f64;
                }
            }
        }
        let rows = qi.len() * 40;
        let mut s = vec![0f64; n * n];
        unsafe { matrixmultiply::dgemm(n, rows, n, 1.0, d.as_ptr(), 1, n as isize, d.as_ptr(), n as isize, 1, 0.0, s.as_mut_ptr(), n as isize, 1) };
        s
    }).collect();
    let mut s = vec![0f64; n * n];
    for p in parts {
        s.iter_mut().zip(p).for_each(|(a, b)| *a += b);
    }
    let c = (nq * 40) as f64;
    s.iter_mut().for_each(|x| *x /= c);
    s
}

fn trace(a: &[f64], n: usize) -> f64 {
    (0..n).map(|i| a[i * n + i]).sum()
}

fn log(t: &Timer, s: String) {
    println!("[{:6.0}s] {s}", t.s());
}

pub fn main(args: &[String]) {
    let t = Timer::new();
    let pos: Vec<&String> = args.iter().skip(2).take_while(|a| !a.starts_with("--")).collect();
    let pdir = arg(args, "--partition");
    let (Some(out), Some(pdir), 1) = (pos.first(), pdir, pos.len()) else {
        eprintln!("usage: vexact fit OUTDIR --partition DIR [--basis pca|dual] [--query-sample QUERIES.npy] [--seed S] [--cert-time SECONDS] [--anchors A] [--text-anchors PROMPTS.npy] [--text-count 256]; DIR is the output of: python3 -I tools/partition.py VECTORS.npy DIR");
        std::process::exit(2);
    };
    let out = out.as_str();
    let basis = arg(args, "--basis").unwrap_or("pca".into());
    assert!(basis == "pca" || basis == "dual", "--basis is pca or dual, not {basis}");
    let sample = arg(args, "--query-sample");
    if sample.is_some() && basis != "dual" {
        eprintln!("--query-sample shapes Sigma_Q, which only the dual basis uses; add --basis dual");
        std::process::exit(2);
    }
    let seed: u64 = arg(args, "--seed").map_or(20261112, |s| s.parse().expect("--seed N"));
    let cert_time: f64 = arg(args, "--cert-time").map_or(600.0, |s| s.parse().expect("--cert-time SECONDS"));
    std::fs::create_dir_all(out).unwrap_or_else(|e| panic!("cannot create {out}: {e}"));
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let part = crate::bucket::Partition::open(&pdir);
    let (nrow, n0, n) = (part.rows, part.n_orig, part.n);
    if n0 < 50 {
        eprintln!("the vectors have {n0} dimensions; the randomised levels need at least 50 (the first ends at 40 bytes, which must be within 40% of the f16 vector). Pad the vectors with zero columns to 50 or more.");
        std::process::exit(2);
    }
    let ncell = part.cells.len();
    log(&t, format!("N {nrow} n {n0} (padded to {n}); {} fine centres in {ncell} cell(s) from {pdir}", part.k));

    // pass 1, cell by cell: each cell's rows listed by fine cluster (`ord{c}.u32`, a cluster's rows in cell order), and
    // every cluster's exact mean, stored int8 with one scale
    let mut cnt0 = vec![0usize; part.k];
    let mut pos0 = vec![0usize; part.k];
    let mut mean0: Vec<(Vec<i8>, f64)> = vec![(Vec::new(), 1.0); part.k];
    let mut placed = 0usize;
    for (c, cell) in part.cells.iter().enumerate() {
        let mut loc: Vec<(u32, u32)> = (0..cell.rows()).filter_map(|i| {
            let k = cell.cluster(i);
            (part.cluster_cell[k as usize] as usize == c).then_some((k, i as u32))
        }).collect();
        loc.sort_unstable();
        let ord: Vec<u32> = loc.iter().map(|x| x.1).collect();
        std::fs::write(format!("{out}/ord{c}.u32"), unsafe { std::slice::from_raw_parts(ord.as_ptr() as *const u8, ord.len() * 4) }).unwrap();
        placed += loc.len();
        let mut runs = Vec::new();
        let mut s = 0;
        while s < loc.len() {
            let e = s + loc[s..].partition_point(|x| x.0 == loc[s].0);
            runs.push((loc[s].0 as usize, s, e));
            s = e;
        }
        let means: Vec<(usize, usize, (Vec<i8>, f64))> = runs.par_iter().map(|&(k, s, e)| {
            let mut sum = vec![0f64; n];
            for &(_, i) in &loc[s..e] {
                sum.iter_mut().zip(cell.vector(n, i as usize)).for_each(|(a, &h)| *a += f16_to_f32(h) as f64);
            }
            let mean: Vec<f64> = sum.iter().map(|v| v / (e - s) as f64).collect();
            let mut sc = mean.iter().fold(0f64, |a, v| a.max(v.abs())) / 127.0;
            if sc == 0.0 {
                sc = 1.0;
            }
            (k, e - s, (mean.iter().map(|v| (v / sc).round_ties_even() as i8).collect(), sc))
        }).collect();
        for (i, (k, m, q)) in means.into_iter().enumerate() {
            (cnt0[k], pos0[k], mean0[k]) = (m, runs[i].1, q);
        }
    }
    assert_eq!(placed, nrow, "the partition in {pdir} places {placed} rows in its cells' clusters, not its {nrow} rows; rerun tools/partition.py");
    let keep: Vec<usize> = (0..part.k).filter(|&c| cnt0[c] > 0).collect();
    let k = keep.len();
    let mq8: Vec<i8> = keep.iter().flat_map(|&c| mean0[c].0.iter().copied()).collect();
    let msc: Vec<f64> = keep.iter().map(|&c| mean0[c].1).collect();
    drop(mean0);
    let mq: Vec<f32> = (0..k * n).map(|i| (mq8[i] as f64 * msc[i / n]) as f32).collect();
    let mh: Vec<f32> = mq.chunks(n).flat_map(|m| {
        let nr = m.iter().map(|&v| v * v).sum::<f32>().sqrt();
        m.iter().map(move |&v| if nr > 0.0 { v / nr } else { v })
    }).collect();

    // groups: enough items per group for a basis (20 n), at most 64 groups and 8 clusters per group on average
    let g0 = 1.max(64.min(k / 8).min(nrow / (20 * n0)));
    let gc = kmeans(&mh, n, g0, &mut rng, 20);
    let graw = argmax_rows(&mh, &gc, n);
    let mut gk: Vec<usize> = graw.clone();
    gk.sort();
    gk.dedup();
    let mut gmap = vec![usize::MAX; g0];
    gk.iter().enumerate().for_each(|(i, &g)| gmap[g] = i);
    let ng = gk.len();
    let grp: Vec<usize> = graw.iter().map(|&g| gmap[g]).collect();

    // clusters numbered group by group (a query visits clusters in this order and builds each one's tables from its
    // group's), rows of a cluster in cell order; the fit's rows are found through each cluster's cell and position
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by_key(|&i| (grp[i], i));
    let grp: Vec<usize> = order.iter().map(|&i| grp[i]).collect();
    let mq8: Vec<i8> = order.iter().flat_map(|&i| mq8[i * n..(i + 1) * n].iter().copied()).collect();
    let msc: Vec<f64> = order.iter().map(|&i| msc[i]).collect();
    let mq: Vec<f32> = order.iter().flat_map(|&i| mq[i * n..(i + 1) * n].iter().copied()).collect();
    let mh: Vec<f32> = order.iter().flat_map(|&i| mh[i * n..(i + 1) * n].iter().copied()).collect();
    let fine: Vec<usize> = order.iter().map(|&i| keep[i]).collect();
    let cnt: Vec<usize> = fine.iter().map(|&f| cnt0[f]).collect();
    let mut cstart = vec![0i64; k + 1];
    for c in 0..k {
        cstart[c + 1] = cstart[c] + cnt[c] as i64;
    }
    save(out, "cstart", "<i8", &[k + 1], &cstart);
    save(out, "clcell", "<i8", &[k], &fine.iter().map(|&f| part.cluster_cell[f] as i64).collect::<Vec<_>>());
    save(out, "clpos", "<i8", &[k], &fine.iter().map(|&f| pos0[f] as i64).collect::<Vec<_>>());
    log(&t, format!("K {k} clusters (sizes {}..{}), G {ng} groups", cnt.iter().min().unwrap(), cnt.iter().max().unwrap()));

    // pass 2: per group, residual covariance and mean |r|^2, residuals r = x - (stored centre), rows read through the
    // fit's row order
    let rows = crate::bucket::Buckets::open(&pdir, out);
    use crate::bucket::Rows;
    {
        let path = format!("{out}/perm.npy");
        let mut w = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(&path).unwrap_or_else(|e| panic!("cannot create {path}: {e}")));
        w.write_all(&npy_header_bytes("<i8", &[nrow])).unwrap();
        for r0 in (0..nrow).step_by(1 << 20) {
            let b: Vec<u8> = (r0..nrow.min(r0 + (1 << 20))).flat_map(|r| (rows.id(r) as i64).to_le_bytes()).collect();
            w.write_all(&b).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
        }
        w.flush().unwrap();
    }
    let acc: Vec<std::sync::Mutex<(Vec<f64>, f64)>> = (0..ng).map(|_| std::sync::Mutex::new((vec![0f64; n * n], 0f64))).collect();
    (0..k).into_par_iter().for_each(|c| {
        let (r0, r1) = (cstart[c] as usize, cstart[c + 1] as usize);
        let m = &mq[c * n..(c + 1) * n];
        let mut rm = vec![0f64; (r1 - r0) * n];
        let mut r2 = 0f64;
        for (i, r) in (r0..r1).enumerate() {
            for (t, &h) in rows.row(r).iter().enumerate() {
                let v = f16_to_f32(h) as f64 - m[t] as f64;
                rm[i * n + t] = v;
                r2 += v * v;
            }
        }
        let mut cv = vec![0f64; n * n];
        unsafe { matrixmultiply::dgemm(n, r1 - r0, n, 1.0, rm.as_ptr(), 1, n as isize, rm.as_ptr(), n as isize, 1, 0.0, cv.as_mut_ptr(), n as isize, 1) };
        let mut a = acc[grp[c]].lock().unwrap();
        a.0.iter_mut().zip(&cv).for_each(|(x, y)| *x += y);
        a.1 += r2;
    });
    let gsize: Vec<f64> = (0..ng).map(|g| (0..k).filter(|&c| grp[c] == g).map(|c| cnt[c]).sum::<usize>().max(1) as f64).collect();
    let covs: Vec<(Vec<f64>, f64)> = acc.into_iter().enumerate().map(|(g, m)| {
        let (mut cv, r2) = m.into_inner().unwrap();
        cv.iter_mut().for_each(|v| *v /= gsize[g]);
        (cv, r2 / gsize[g])
    }).collect();
    let r2: Vec<f64> = covs.iter().map(|c| c.1).collect();

    // rows of the fit, sampled, as unit f32
    let sample_rows = |rng: &mut ChaCha8Rng, m: usize| -> Vec<f32> {
        let mut idx = rand::seq::index::sample(rng, nrow, m.min(nrow)).into_vec();
        idx.sort();
        idx.iter().flat_map(|&r| rows.row(r).iter().map(|&h| f16_to_f32(h))).collect()
    };

    let mut lam = vec![0f64; ng * n];
    let mut vb = vec![0f32; ng * n * n];
    let mut cond_max = 0f64;
    let sigma_q_desc;
    if basis == "pca" {
        sigma_q_desc = String::new();
        let res: Vec<(Vec<f64>, Vec<f64>)> = covs.par_iter().map(|(cv, _)| eigh_desc(cv, n)).collect();
        for (g, (l, v)) in res.into_iter().enumerate() {
            for c in 0..n {
                lam[g * n + c] = l[c].max(1e-12);
            }
            vb[g * n * n..(g + 1) * n * n].iter_mut().zip(&v).for_each(|(a, &b)| *a = b as f32);
        }
    } else {
        // Sigma_Q: offsets of 3,000 corpus items (and the queries') from 40 centres each, drawn by cluster size
        let mut rq = ChaCha8Rng::seed_from_u64(3);
        let w = WeightedIndex::new(&cnt).unwrap();
        let qs = sample_rows(&mut rq, 3000);
        let mut sq = offset_cov(&qs, &mh, n, &w, &mut rq);
        sigma_q_desc = match &sample {
            None => "corpus items".to_string(),
            Some(path) => {
                let q = read_npy(path);
                let qn0 = q.shape[1];
                assert!(qn0 <= n, "{path} has {qn0} dimensions, more than the vectors' {n0}");
                let qv: Vec<f32> = match q.descr.as_str() {
                    "<f4" => q.f32s(),
                    "<f8" => q.f64s().iter().map(|&v| v as f32).collect(),
                    "<f2" => q.u16s().iter().map(|&h| f16_to_f32(h)).collect(),
                    d => panic!("{path} holds {d}; write the queries as float16, float32 or float64"),
                };
                let mut qs2 = vec![0f32; q.shape[0] * n];
                for i in 0..q.shape[0] {
                    let row = &mut qs2[i * n..(i + 1) * n];
                    row[..qn0].copy_from_slice(&qv[i * qn0..(i + 1) * qn0]);
                    let nr = row.iter().map(|&v| v as f64 * v as f64).sum::<f64>().sqrt() as f32;
                    row.iter_mut().for_each(|v| *v /= nr);
                }
                let st = offset_cov(&qs2, &mh, n, &w, &mut rq);
                let (ts, tt) = (trace(&sq, n), trace(&st, n));
                sq.iter_mut().zip(&st).for_each(|(a, &b)| *a = 0.5 * (*a / ts + b / tt));
                log(&t, format!("Sigma_Q: 3000 corpus items and {} queries from {path}", q.shape[0]));
                format!("corpus items + queries {}", std::fs::canonicalize(path).map_or(path.clone(), |p| p.display().to_string()))
            }
        };
        let ridge = RIDGE * trace(&sq, n) / n as f64;
        (0..n).for_each(|i| sq[i * n + i] += ridge);
        let (lq, uq) = eigh_desc(&sq, n);
        let scaled = |f: &dyn Fn(f64) -> f64| -> Vec<f64> {
            let mut us = uq.clone();
            for r in 0..n {
                for c in 0..n {
                    us[r * n + c] *= f(lq[c]);
                }
            }
            let mut ut = vec![0f64; n * n];
            for r in 0..n {
                for c in 0..n {
                    ut[c * n + r] = uq[r * n + c];
                }
            }
            matmul(&us, &ut, n)
        };
        let (a, ai) = (scaled(&|l| l.sqrt()), scaled(&|l| 1.0 / l.sqrt()));
        let res: Vec<(Vec<f32>, Vec<f64>, f64)> = covs.par_iter().map(|(cv, _)| {
            let (gam, wv) = eigh_desc(&matmul(&matmul(&a, cv, n), &a, n), n);
            let g0 = gam[0];
            let mut v = matmul(&ai, &wv, n);
            for r in 0..n {
                for c in 0..n {
                    v[r * n + c] *= gam[c].max(1e-12 * g0).powf(0.25);
                }
            }
            let v32: Vec<f32> = v.iter().map(|&x| x as f32).collect();
            // the coded coordinates z = V_Q8^-1 r under the build's int8 basis: their mean squares from the covariance
            let vq = int8_basis64(&v32, n);
            let vm = mat(&vq, n);
            let sv = vm.singular_values().expect("singular values of the int8 basis");
            let cond = sv.iter().cloned().fold(0.0, f64::max) / sv.iter().cloned().fold(f64::INFINITY, f64::min);
            let inv = vm.partial_piv_lu().inverse();
            let invr: Vec<f64> = (0..n * n).map(|i| inv[(i / n, i % n)]).collect();
            let ic = matmul(&invr, cv, n);
            let l: Vec<f64> = (0..n).map(|i| (0..n).map(|j| ic[i * n + j] * invr[i * n + j]).sum::<f64>().max(1e-30)).collect();
            (v32, l, cond)
        }).collect();
        for (g, (v, l, cond)) in res.into_iter().enumerate() {
            vb[g * n * n..(g + 1) * n * n].copy_from_slice(&v);
            lam[g * n..(g + 1) * n].copy_from_slice(&l);
            cond_max = cond_max.max(cond);
        }
    }
    drop(covs);
    log(&t, format!("bases done ({basis}); group sizes {}..{}", gsize.iter().cloned().fold(f64::INFINITY, f64::min), gsize.iter().cloned().fold(0.0, f64::max)));

    // profile: corpus items as pseudo-queries against their 4 nearest centres (by q.m with the stored centres), the mean
    // square of (q - m) in each group's coordinates; groups with 50 pairs or fewer take lam
    let profile = |pq: &[f32], proj: &(dyn Fn(usize) -> Vec<f64> + Sync)| -> Vec<f64> {
        let np = pq.len() / n;
        let mut d = vec![0f32; np * k];
        unsafe { matrixmultiply::sgemm(np, n, k, 1.0, pq.as_ptr(), n as isize, 1, mq.as_ptr(), 1, n as isize, 0.0, d.as_mut_ptr(), k as isize, 1) };
        let mut pairs: Vec<Vec<(usize, usize)>> = vec![Vec::new(); ng];
        for i in 0..np {
            let mut o: Vec<usize> = (0..k).collect();
            let row = &d[i * k..(i + 1) * k];
            let top = 4.min(k);
            o.select_nth_unstable_by(top - 1, |&a, &b| row[b].partial_cmp(&row[a]).unwrap());
            for &c in &o[..top] {
                pairs[grp[c]].push((i, c));
            }
        }
        let rows: Vec<Vec<f64>> = (0..ng).into_par_iter().map(|g| {
            if pairs[g].len() <= 50 {
                return lam[g * n..(g + 1) * n].to_vec();
            }
            let v = proj(g);
            let m = pairs[g].len();
            let mut dm = vec![0f64; m * n];
            for (j, &(i, c)) in pairs[g].iter().enumerate() {
                for t in 0..n {
                    dm[j * n + t] = (pq[i * n + t] - mq[c * n + t]) as f64;
                }
            }
            let mut p = vec![0f64; m * n];
            unsafe { matrixmultiply::dgemm(m, n, n, 1.0, dm.as_ptr(), n as isize, 1, v.as_ptr(), n as isize, 1, 0.0, p.as_mut_ptr(), n as isize, 1) };
            (0..n).map(|c| (0..m).map(|j| p[j * n + c] * p[j * n + c]).sum::<f64>() / m as f64).collect()
        }).collect();
        rows.concat()
    };
    let pq = sample_rows(&mut rng, 5000);
    let fitj;
    let prof: Vec<f64> = if basis == "pca" {
        let qi = profile(&pq, &|g| vb[g * n * n..(g + 1) * n * n].iter().map(|&x| x as f64).collect());
        fitj = serde_json::json!({"basis": "pca", "profile": "h25"});
        (0..ng * n).map(|i| {
            let (g, c) = (i / n, i % n);
            let iso = if c < n0 { qi[g * n..(g + 1) * n].iter().sum::<f64>() / n0 as f64 } else { 0.0 };
            ((1.0 - HEDGE) * qi[i].sqrt() + HEDGE * iso.sqrt()).powi(2)
        }).collect()
    } else {
        let mut r2g = ChaCha8Rng::seed_from_u64(seed + 2);
        let pq2 = sample_rows(&mut r2g, 5000);
        fitj = serde_json::json!({"basis": "dual", "profile": "qi", "ridge": RIDGE, "sigma_q": sigma_q_desc, "cond_max": cond_max});
        profile(&pq2, &|g| int8_basis64(&vb[g * n * n..(g + 1) * n * n], n))
    };
    let _ = pq;

    let (rlv, bounds) = schedule(n0, n);
    // the rate design's constant (two-sided, N = 1e9); the query-time constants follow the index size (src/v2/tail.rs)
    let l1 = (2.0 * NTARGET * rlv.len() as f64 / EPS).ln();
    let tt: serde_json::Value = serde_json::from_str(include_str!("tree_tables.json")).unwrap();
    let tab = |key: &str| -> Vec<f64> { tt[key].as_array().unwrap().iter().flat_map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap())).collect() };
    let gtree: Vec<f64> = tt["gtree"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    save(out, "grp", "<i8", &[k], &grp.iter().map(|&g| g as i64).collect::<Vec<_>>());
    save(out, "mq8", "|i1", &[k, n], &mq8);
    save(out, "msc", "<f8", &[k], &msc);
    save(out, "lam", "<f8", &[ng, n], &lam);
    save(out, "V", "<f4", &[ng, n, n], &vb);
    save(out, "r2", "<f8", &[ng], &r2);
    save(out, "profile", "<f8", &[ng, n], &prof);
    save(out, "tree_edges", "<f8", &[9, 256], &tab("tree_edges"));
    save(out, "tree_cent", "<f8", &[9, 256], &tab("tree_cent"));
    save(out, "gtree", "<f8", &[gtree.len()], &gtree);
    let mut qi = rand::seq::index::sample(&mut rng, nrow, 200.min(nrow)).into_vec().into_iter().map(|r| r as i64).collect::<Vec<_>>();
    qi.sort();
    save(out, "qi", "<i8", &[qi.len()], &qi);
    let meta = serde_json::json!({
        "N": nrow, "n": n, "n_orig": n0, "K": k, "G": ng, "metric": "angular", "DLV": DLV, "RLV": rlv, "BOUNDS": bounds,
        "CSG": (2.0 * l1).sqrt(), "L1": l1, "EPS": EPS,
        "partition": std::fs::canonicalize(&pdir).map_or(pdir.clone(), |p| p.display().to_string()),
        "format": FORMAT, "fit": fitj,
    });
    std::fs::write(format!("{out}/meta.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    let image: usize = arg(args, "--anchors").map_or(1024, |s| s.parse().expect("--anchors A"));
    let ntext: usize = arg(args, "--text-count").map_or(256, |s| s.parse().expect("--text-count N"));
    let na = write_anchors(out, image, arg(args, "--text-anchors").as_deref(), ntext, seed);
    log(&t, format!("saved {out}; {na} support-table anchors"));

    // the randomised levels' certified constant, by tools/certify.py within --cert-time (0 skips it: the build then
    // uses the closed-form bounds, which are larger)
    if cert_time > 0.0 {
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/certify.py");
        let st = std::process::Command::new("python3").args(["-I", script, out, "--time-cap", &cert_time.to_string()]).status();
        match st {
            Ok(s) if s.success() => log(&t, "certified".into()),
            _ => {
                eprintln!("{script} failed on {out}; the build would fall back to larger closed-form constants. Install numpy and scipy for python3 and rerun, or pass --cert-time 0 to accept the larger constants.");
                std::process::exit(1);
            }
        }
    }
}

/// The support table's anchors for the fit in `dir`, written to `dir/anchors.npy` (A x n, unit, f64): the directions
/// of all K centres when `image` >= K (one anchor per cluster), else `image` anchors by spherical k-means on the centre
/// directions; then, from `text` (training prompts, rows of n_orig or n), `ntext` more by k-means on the prompts. A
/// query's bound uses its nearest anchor, so what sets its cost is how densely the anchors cover the directions
/// queries come from, not K: a fixed A keeps the table K x A at any N.
pub fn write_anchors(dir: &str, image: usize, text: Option<&str>, ntext: usize, seed: u64) -> usize {
    let p = crate::index::load_params(dir);
    let (n, k) = (p.meta.n, p.meta.k);
    let unit64 = |v: &[f32]| -> Vec<f64> {
        let nr = v.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt().max(1e-300);
        v.iter().map(|&x| x as f64 / nr).collect()
    };
    let dirs: Vec<f32> = (0..k).flat_map(|c| unit64(&p.centres[c * n..(c + 1) * n]).into_iter().map(|x| x as f32)).collect();
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xa4c4);
    let mut out: Vec<f64> = if image >= k {
        (0..k).flat_map(|c| unit64(&p.centres[c * n..(c + 1) * n])).collect()
    } else {
        kmeans(&dirs, n, image, &mut rng, 25).chunks(n).flat_map(unit64).collect()
    };
    if let Some(path) = text {
        let t = read_npy(path);
        let w = t.shape[1];
        assert!(w <= n, "{path} has rows of {w} dimensions, more than the fit's {n}");
        let v: Vec<f32> = if t.descr.contains("f8") { t.f64s().into_iter().map(|x| x as f32).collect() } else { t.f32s() };
        let rows: Vec<f32> = v.chunks(w).flat_map(|r| {
            let mut x = r.to_vec();
            x.resize(n, 0.0);
            unit64(&x).into_iter().map(|y| y as f32)
        }).collect();
        let m = rows.len() / n;
        out.extend(kmeans(&rows, n, ntext.min(m), &mut rng, 25).chunks(n).flat_map(unit64));
    }
    let a = out.len() / n;
    save(dir, "anchors", "<f8", &[a, n], &out);
    a
}

/// `vexact anchors FITDIR [--anchors A] [--text-anchors PROMPTS.npy] [--text-count 256] [--seed S]`
pub fn anchors_cmd(args: &[String]) {
    let Some(dir) = args.get(2).filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: vexact anchors FITDIR [--anchors A (default 1024)] [--text-anchors PROMPTS.npy] [--text-count 256] [--seed S]");
        std::process::exit(2);
    };
    let image: usize = arg(args, "--anchors").map_or(1024, |s| s.parse().expect("--anchors A"));
    let ntext: usize = arg(args, "--text-count").map_or(256, |s| s.parse().expect("--text-count N"));
    let seed: u64 = arg(args, "--seed").map_or(20261112, |s| s.parse().expect("--seed S"));
    let a = write_anchors(dir, image, arg(args, "--text-anchors").as_deref(), ntext, seed);
    println!("{a} anchors written to {dir}/anchors.npy");
}
