mod derive;
mod index;
mod kernel;
mod query;
mod run2;
mod scale;
mod v2;
mod synth;
mod util;

use index::*;
use kernel::*;
use query::*;
use rayon::prelude::*;
use roaring::RoaringBitmap;
use std::io::Write;
use util::*;

const DATA: &str = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/rustdata/m768";
const RES: &str = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/research-branch/rust/results";

struct Ctx {
    p: Params,
    ix: Index,
    xs: RealSource,
    ent_xs: Vec<u32>,
    items: Vec<Vec<f32>>,
    texts: Vec<Vec<f32>>,
}

static LOG: std::sync::OnceLock<std::sync::Mutex<std::fs::File>> = std::sync::OnceLock::new();
macro_rules! out {
    ($($a:tt)*) => {{ let s = format!($($a)*); println!("{}", s); if let Some(l) = LOG.get() { writeln!(l.lock().unwrap(), "{}", s).unwrap(); } }};
}

fn pool(t: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap()
}

fn subset(mut p: Params, kk: usize) -> Params {
    if kk >= p.meta.k {
        return p;
    }
    let n = p.meta.n;
    p.meta.k = kk;
    p.cstart.truncate(kk + 1);
    p.grp.truncate(kk);
    p.centres.truncate(kk * n);
    p
}

fn arg_str(k: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == k).map(|i| args[i + 1].clone())
}

/// Parameters with the profile-dependent ones derived from `--profile <file.npy>` when given.
pub fn params_with_profile(kk: usize) -> Params {
    let mut p = load_params(DATA);
    if let Some(pf) = arg_str("--profile") {
        let (a, b) = apply_profile(&mut p, &pf);
        println!("profile {pf}: derived parameters differ from the exported ones in {a} frb and {b} rate entries");
    }
    subset(p, kk)
}

fn setup(kk: usize, nanch: usize, tag: &str) -> Ctx {
    let log = std::fs::OpenOptions::new().create(true).append(true).open(format!("{RES}/{tag}.log")).unwrap();
    let _ = LOG.set(std::sync::Mutex::new(log));
    out!("==== run at {:.0}: {:?}", now_s(), std::env::args().collect::<Vec<_>>());
    let t = Timer::new();
    let p = params_with_profile(kk);
    let xs = RealSource::open(&p.xs_path);
    let src = Box::new(RealSource::open(&p.xs_path));
    let mut lg = |s: String| {
        out!("[build] {s}");
    };
    let ix = build(&p, src, nanch.min(p.meta.k), &mut lg);
    let n = p.meta.n;
    let mut ent_xs = vec![u32::MAX; p.perm.len()];
    for (r, &e) in p.perm.iter().enumerate() {
        ent_xs[e as usize] = r as u32;
    }
    let items: Vec<Vec<f32>> = p.qi.iter().filter(|&&r| (r as usize) < p.cstart[p.meta.k]).map(|&r| xs.row(r as usize).iter().map(|&h| f16_to_f32(h)).collect()).collect();
    let texts: Vec<Vec<f32>> = p.text_q.chunks(n).map(|v| {
        let nn = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / nn).collect()
    }).collect();
    let c = Ctx { p, ix, xs, ent_xs, items, texts };
    out!("built {} rows, {} clusters, {} anchors in {:.1}s; stored bytes/item cumulative by level: {:?}", c.ix.nrows, c.ix.k(), c.ix.nanch, t.s(),
        c.ix.stored_bytes_per_item().iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>());
    let ne: usize = c.ix.nesc.iter().map(|m| m.len()).sum();
    out!("escapes: det norms {} ({:.3}% of item-levels), yh fallbacks {} ({:.3}%), tan fallbacks {}", ne, 100.0 * ne as f64 / (c.ix.nrows * NDL) as f64,
        c.ix.yfb.len(), 100.0 * c.ix.yfb.len() as f64 / (c.ix.nrows * J) as f64, c.ix.tfb.iter().map(|x| x.len()).sum::<usize>());
    c
}

impl Ctx {
    fn flat(&self) -> Flat<'_> {
        Flat { map: &self.xs.map, off: self.xs.off, n: self.p.meta.n, nrows: self.p.cstart[self.p.meta.k], ent: &self.p.perm }
    }
}

fn sym_diff(a: &RoaringBitmap, b: &[u32]) -> (u64, u64) {
    let bb = RoaringBitmap::from_sorted_iter(b.iter().copied()).unwrap();
    ((a - &bb).len(), (&bb - a).len())
}

const SIZES: [f64; 4] = [1e-4, 1e-3, 1e-2, 5e-2];

/// correctness at 1M: every threshold query against brute force, intervals checked at every level
fn verify(c: &mut Ctx, nitem: usize, ntext: usize, ncheck: usize) {
    let ix = &c.ix;
    let n = ix.n();
    let fl = c.flat();
    let nrows = fl.nrows;
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((nrows as f64 * f).round() as usize).max(1)).collect();
    let mut qs: Vec<(&str, &Vec<f32>)> = c.items.iter().take(nitem).map(|q| ("item", q)).collect();
    qs.extend(c.texts.iter().take(ntext).map(|q| ("text", q)));
    let mut tot = Stats::new(ix.meta.nl());
    let (mut wrong_in, mut wrong_out, mut knn_bad) = (0u64, 0u64, 0u64);
    let mut rows = Vec::new();
    let t = Timer::new();
    for (qi, (cls, q)) in qs.iter().enumerate() {
        let pr = prep(ix, q);
        let err = full_err(ix, &pr);
        let sc = fl.scores(&pr.q32);
        let ths = thresholds_for(&fl, &pr.q16, &sc, err, &sizes);
        for (si, &th) in ths.iter().enumerate() {
            let truth = fl.threshold(&pr.q16, &pr.q32, th, err, None, &c.ent_xs);
            let pr2 = prep(ix, q);
            let chk = (*cls == "item" && qi < ncheck) || (*cls == "text" && qi >= nitem.min(c.items.len()) && qi < nitem.min(c.items.len()) + ncheck);
            let o = Opts { vis: None, verify: chk, want_iv: false, rec: false };
            let r = threshold_prepped(ix, &pr2, th, &o);
            let bm = to_bitmap(ix, &r.rows);
            let (a, b) = sym_diff(&bm, &truth);
            wrong_in += a;
            wrong_out += b;
            tot.add(&r.st);
            rows.push(format!("{},{},{},{},{},{},{},{},{}", qi, cls, SIZES[si], th, truth.len(), a, b, r.st.touched, r.st.full_reads));
        }
        let kn = knn(ix, q, 10, None);
        let fk = fl.knn(&pr.q16, &pr.q32, 10, err, None, &c.ent_xs);
        let mut a: Vec<u32> = kn.ents.iter().map(|x| x.1).collect();
        let mut b: Vec<u32> = fk.iter().map(|x| x.1).collect();
        a.sort();
        b.sort();
        if a != b {
            knn_bad += 1;
            out!("knn mismatch q{} {}: {:?} vs {:?}", qi, cls, &kn.ents[..3.min(kn.ents.len())], &fk[..3.min(fk.len())]);
        }
        if qi % 50 == 0 {
            println!("verify {qi}/{} {:.0}s viol in/out {wrong_in}/{wrong_out} det {} rand {} checked {} maxsd {:.2}", qs.len(), t.s(), tot.viol_det, tot.viol_rand, tot.checked, tot.max_sd);
        }
    }
    let _ = n;
    out!("VERIFY {} queries x 4 answer sizes: result-set violations wrong-in {} wrong-out {}; interval checks {} (det violations {}, rand violations {}); largest randomised error {:.2} sd (c = {:.2}); k-NN k=10 mismatches {} of {}",
        qs.len(), wrong_in, wrong_out, tot.checked, tot.viol_det, tot.viol_rand, tot.max_sd, ix.meta.csg, knn_bad, qs.len());
    let mut f = std::fs::File::create(format!("{RES}/verify_1m.csv")).unwrap();
    writeln!(f, "q,class,answer_frac,t,truth_size,wrong_in,wrong_out,touched,full_reads").unwrap();
    for r in rows {
        writeln!(f, "{r}").unwrap();
    }
}

/// ns per item per level, all lanes active, no early exit (Interval mode to each level)
fn levels(c: &mut Ctx) {
    let ix = &c.ix;
    let nl = ix.meta.nl();
    let bytes = ix.stored_bytes_per_item();
    let mut lines = vec!["query,level,level_bytes_cum,stored_bytes_cum,ns_per_item_cum,ns_per_item_level,ns_per_stored_byte_level".to_string()];
    for (name, q) in [("item", c.items[0].clone()), ("text", c.texts[0].clone())] {
        let pr = prep(ix, &q);
        let mut st = Stats::new(nl);
        // set up every group and cluster once (not timed)
        let cqs: Vec<CQ> = (0..ix.k()).map(|ci| {
            let gq = get_gq(ix, &pr, ix.cl[ci].g, &mut st);
            cluster_q(ix, ci, gq, &pr, dot64(&pr.q64, ix.centres_f64(ci)))
        }).collect();
        let infl: Vec<f64> = (0..NDL).map(|l| cqs.iter().map(|c| c.infl[l] as f64).sum::<f64>() / cqs.len() as f64).collect();
        out!("levels {name}: mean over clusters of dwa(one alpha)/dwa(per-level alpha), det levels: {:?}", infl.iter().map(|x| (x * 1e4).round() / 1e4).collect::<Vec<_>>());
        let mut prev = 0.0;
        for l in 0..nl {
            let mut best = f64::MAX;
            for _rep in 0..5 {
                let t = Timer::new();
                let mut st = Stats::new(nl);
                for ci in 0..ix.k() {
                    let cl = &ix.cl[ci];
                    for bk in 0..cl.nblk {
                        let m = if bk + 1 == cl.nblk && (cl.r1 - cl.r0) % 16 != 0 { (1u16 << ((cl.r1 - cl.r0) % 16)) - 1 } else { 0xffff };
                        let mut rin = Vec::new();
                        let mut pend = Vec::new();
                        let mut out = BlockOut { rows_in: &mut rin, pend: &mut pend, iv: None, truth: None, rec: None };
                        unsafe { run_block::<1>(ix, ci, bk, &[&cqs[ci]], &[f32::NAN], &[m], Mode::Interval(l), &mut st, &mut out) };
                    }
                }
                best = best.min(t.s());
            }
            let ns = best * 1e9 / ix.nrows as f64;
            let lb = ix.meta.level_bytes(l) as f64;
            let sb = bytes[l] - if l == 0 { 2.0 } else { bytes[l - 1] };
            let line = format!("{},{},{},{:.2},{:.2},{:.2},{:.3}", name, l, lb, bytes[l], ns, ns - prev, (ns - prev) / sb);
            out!("levels {line}");
            lines.push(line);
            prev = ns;
        }
    }
    std::fs::write(format!("{RES}/levels.csv"), lines.join("\n") + "\n").unwrap();
}

/// memory read bandwidth (sum of u64 over a large buffer)
fn bandwidth(c: &mut Ctx) {
    let fl = c.flat();
    let bytes = &fl.map[fl.off..];
    for th in [1, 2, 3, 4] {
        let pl = pool(th);
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Timer::new();
            let s: u64 = pl.install(|| bytes.par_chunks(1 << 20).map(|ch| {
                let w = unsafe { std::slice::from_raw_parts(ch.as_ptr() as *const u64, ch.len() / 8) };
                w.iter().fold(0u64, |a, &x| a.wrapping_add(x))
            }).reduce(|| 0, |a, b| a.wrapping_add(b)));
            std::hint::black_box(s);
            best = best.min(t.s());
        }
        out!("bandwidth threads={} read {:.2} GB/s ({:.0} MB in {:.1} ms)", th, bytes.len() as f64 / best / 1e9, bytes.len() as f64 / 1e6, best * 1e3);
    }
}

/// bytes read per item (of all N): code + scalars at each level entered, plus full vectors
fn bytes_read(ix: &Index, st: &Stats) -> f64 {
    let sb = ix.stored_bytes_per_item();
    let nl = ix.meta.nl();
    let mut b = 0.0;
    for l in 0..nl {
        let inc = sb[l] - if l == 0 { 0.0 } else { sb[l - 1] };
        b += st.enter[l] as f64 * inc;
    }
    b += st.full_reads as f64 * 2.0 * ix.n() as f64;
    b / ix.nrows as f64
}

/// Bytes read per item (of N) and items touched, by class and answer size; no timing.
fn bytesq(c: &mut Ctx, nq: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((fl.nrows as f64 * f).round() as usize).max(1)).collect();
    for (cls, qs) in [("item", c.items.clone()), ("text", c.texts.clone())] {
        let mut br = vec![0.0; 4];
        let mut tch = vec![0.0; 4];
        let k = qs.len().min(nq);
        for q in qs.iter().take(nq) {
            let pr = prep(ix, q);
            let sc = fl.scores(&pr.q32);
            let ths = thresholds_for(&fl, &pr.q16, &sc, full_err(ix, &pr), &sizes);
            for si in 0..4 {
                let r = threshold_prepped(ix, &prep(ix, q), ths[si], &Opts { vis: None, verify: false, want_iv: false, rec: false });
                br[si] += bytes_read(ix, &r.st) / k as f64;
                tch[si] += r.st.touched as f64 / ix.nrows as f64 / k as f64;
            }
        }
        out!("bytesq old {cls} ({k} queries): bytes read per item {:?}; touched {:?}", br.iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<_>>(), tch.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>());
    }
}

fn e2e(c: &mut Ctx, nq: usize, nflat: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let n = ix.n();
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((nrows as f64 * f).round() as usize).max(1)).collect();
    // int8 copy of the data for the int8 baseline (global scale 127)
    let x8: Vec<i8> = (0..nrows * n).into_par_iter().map(|i| {
        let r = i / n;
        let k = i % n;
        (f16_to_f32(fl.row(r)[k]) * 127.0).round().clamp(-127.0, 127.0) as i8
    }).collect();
    let mut lines = vec!["class,threads,what,answer_frac,median_ms,mean_ms,p90_ms,nq,touched_mean,full_reads_mean,result_mean,bytes_read_per_item".to_string()];
    for (cls, qs) in [("item", c.items.clone()), ("text", c.texts.clone())] {
        let qs: Vec<Vec<f32>> = qs.into_iter().take(nq).collect();
        // thresholds per query (exact)
        let ths: Vec<Vec<f64>> = qs.iter().map(|q| {
            let pr = prep(ix, q);
            let sc = fl.scores(&pr.q32);
            thresholds_for(&fl, &pr.q16, &sc, full_err(ix, &pr), &sizes)
        }).collect();
        // per-query setup of every group (basis product, rotated weights), 1 thread
        {
            let mut tg = Vec::new();
            for q in qs.iter().take(10) {
                let t = Timer::new();
                let pr = prep(ix, q);
                let mut st = Stats::new(ix.meta.nl());
                for g in 0..ix.groups.len() {
                    get_gq(ix, &pr, g, &mut st);
                }
                tg.push(t.ms());
            }
            out!("e2e {cls}: setting up all {} groups (incl. anchor scan) takes {:.2} ms per query, 1 thread", ix.groups.len(), median(&tg));
            let pr = prep(ix, &qs[0]);
            let t = Timer::new();
            let _ = prep(ix, &qs[0]);
            let ta = t.ms();
            let n = ix.n();
            let t = Timer::new();
            let mut qvs = Vec::new();
            for g in 0..ix.groups.len() {
                let mut qv = vec![vec![0f64; n]];
                unsafe { basis_matvec(ix_v(ix, g), n, &[&pr.q64], &mut qv) };
                qvs.push(qv.pop().unwrap());
            }
            let tb = t.ms();
            let t = Timer::new();
            for (g, qv) in qvs.into_iter().enumerate() {
                std::hint::black_box(group_q(ix, g, &pr.q32, qv));
            }
            out!("e2e {cls}: anchor scan {:.2} ms, 64 basis products {:.2} ms, 64 group weight setups {:.2} ms", ta, tb, t.ms());
        }
        for th in [1usize, 4] {
            let pl = pool(th);
            for si in 0..4 {
                let mut ts = Vec::new();
                let (mut tch, mut fr, mut rs, mut br) = (0.0, 0.0, 0.0, 0.0);
                let mut enter = vec![0f64; ix.meta.nl() + 1];
                let mut blocks = vec![0f64; ix.meta.nl() + 1];
                let (mut grp, mut opn) = (0.0, 0.0);
                for (qi, q) in qs.iter().enumerate() {
                    let o = Opts { vis: None, verify: false, want_iv: false, rec: false };
                    let mut best = f64::MAX;
                    let mut last = None;
                    for _ in 0..2 {
                        let t = Timer::new();
                        let r = pl.install(|| {
                            let r = threshold(ix, q, ths[qi][si], &o);
                            let bm = to_bitmap(ix, &r.rows);
                            (r, bm)
                        });
                        best = best.min(t.ms());
                        last = Some(r);
                    }
                    let (r, bm) = last.unwrap();
                    br += bytes_read(ix, &r.st);
                    for (e, x) in enter.iter_mut().zip(&r.st.enter) {
                        *e += *x as f64;
                    }
                    for (e, x) in blocks.iter_mut().zip(&r.st.blocks) {
                        *e += *x as f64;
                    }
                    grp += r.st.groups as f64;
                    opn += r.st.clusters_open as f64;
                    tch += r.st.touched as f64;
                    fr += r.st.full_reads as f64;
                    rs += bm.len() as f64;
                    ts.push(best);
                }
                let k = qs.len() as f64;
                let mut s = ts.clone();
                let line = format!("{},{},index_thr,{},{:.3},{:.3},{:.3},{},{:.0},{:.1},{:.0},{:.2}", cls, th, SIZES[si], median(&ts), mean(&ts), quantile(&mut s, 0.9), qs.len(), tch / k, fr / k, rs / k, br / k);
                out!("e2e {line}");
                if th == 1 {
                    out!("e2e enter-fraction by level ({cls}, {}): {:?}", SIZES[si], enter.iter().map(|x| (x / k / ix.nrows as f64 * 1e4).round() / 1e4).collect::<Vec<_>>());
                    let occ = enter[..ix.meta.nl()].iter().sum::<f64>() / (16.0 * blocks.iter().sum::<f64>()).max(1.0);
                    out!("e2e lane occupancy ({cls}, {}): {:.3} of 16 lanes busy per block-level; groups set up {:.1}, clusters opened {:.0}", SIZES[si], occ, grp / k, opn / k);
                }
                lines.push(line);
            }
            // k-NN k = 10
            let mut ts = Vec::new();
            let (mut seed, mut sweep, mut sel, mut cand) = (0.0, 0.0, 0.0, 0.0);
            for q in &qs {
                let mut best = f64::MAX;
                let mut last = None;
                for _ in 0..2 {
                    let t = Timer::new();
                    let r = pl.install(|| knn(ix, q, 10, None));
                    best = best.min(t.ms());
                    last = Some(r);
                }
                let r = last.unwrap();
                seed += r.t_seed;
                sweep += r.t_sweep;
                sel += r.t_sel;
                cand += r.cands as f64;
                ts.push(best);
            }
            let k = qs.len() as f64;
            let mut s = ts.clone();
            let line = format!("{},{},index_knn10,0,{:.3},{:.3},{:.3},{},{:.0},0,10,0", cls, th, median(&ts), mean(&ts), quantile(&mut s, 0.9), qs.len(), cand / k);
            out!("e2e {line}   (seed {:.2} / sweep {:.2} / selection {:.2} ms mean)", seed / k, sweep / k, sel / k);
            lines.push(line);
            // flat scans
            let mut tf = Vec::new();
            let mut tk = Vec::new();
            let mut t8 = Vec::new();
            for (qi, q) in qs.iter().take(nflat).enumerate() {
                let pr = prep(ix, q);
                let err = full_err(ix, &pr);
                let mut b1 = f64::MAX;
                let mut b2 = f64::MAX;
                let mut b3 = f64::MAX;
                let qmax = pr.q32.iter().fold(0f32, |a, &x| a.max(x.abs()));
                let q16i: Vec<i16> = pr.q32.iter().map(|&x| (x / qmax * 8191.0).round() as i16).collect();
                let t8thr = (ths[qi][1] * 127.0 * 8191.0 / qmax as f64) as i32;
                for _ in 0..2 {
                    let t = Timer::new();
                    let r = pl.install(|| fl.threshold(&pr.q16, &pr.q32, ths[qi][1], err, None, &c.ent_xs));
                    std::hint::black_box(r.len());
                    b1 = b1.min(t.ms());
                    let t = Timer::new();
                    let r = pl.install(|| fl.knn(&pr.q16, &pr.q32, 10, err, None, &c.ent_xs));
                    std::hint::black_box(r.len());
                    b2 = b2.min(t.ms());
                    let t = Timer::new();
                    let cnt = pl.install(|| (0..nrows).into_par_iter().with_min_len(4096).filter(|&r| unsafe { dot_i8(&q16i, &x8[r * n..(r + 1) * n]) } >= t8thr).count());
                    std::hint::black_box(cnt);
                    b3 = b3.min(t.ms());
                }
                tf.push(b1);
                tk.push(b2);
                t8.push(b3);
            }
            for (what, v) in [("flat_f16_thr", &tf), ("flat_f16_knn10", &tk), ("flat_int8_thr", &t8)] {
                let mut s = v.clone();
                let line = format!("{},{},{},0.001,{:.3},{:.3},{:.3},{},{},0,0,0", cls, th, what, median(v), mean(v), quantile(&mut s, 0.9), v.len(), nrows);
                out!("e2e {line}");
                lines.push(line);
            }
        }
    }
    std::fs::write(format!("{RES}/e2e_1m.csv"), lines.join("\n") + "\n").unwrap();
}

fn batch(c: &mut Ctx, total: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let qs: Vec<Vec<f32>> = c.texts.iter().take(total).cloned().collect();
    let mut lines = vec!["answer_frac,threads,B,qps,ms_per_batch,queries".to_string()];
    for &frac in &[1e-3, 1e-2] {
        let k = ((nrows as f64 * frac).round() as usize).max(1);
        let ths: Vec<f64> = qs.iter().map(|q| {
            let pr = prep(ix, q);
            let sc = fl.scores(&pr.q32);
            thresholds_for(&fl, &pr.q16, &sc, full_err(ix, &pr), &[k])[0]
        }).collect();
        // check batched results against single-query results once
        {
            let (rows, _) = threshold_batch::<4>(ix, &qs[..4], &ths[..4]);
            for q in 0..4 {
                let single = threshold(ix, &qs[q], ths[q], &Opts { vis: None, verify: false, want_iv: false, rec: false });
                let a = to_bitmap(ix, &rows[q]);
                let b = to_bitmap(ix, &single.rows);
                assert_eq!(a, b, "batched result differs from single");
            }
        }
        for th in [1usize, 4] {
            let pl = pool(th);
            for b in [1usize, 4, 16, 64] {
                let nb = total / b;
                let t = Timer::new();
                pl.install(|| {
                    for i in 0..nb {
                        let q = &qs[i * b..(i + 1) * b];
                        let t_ = &ths[i * b..(i + 1) * b];
                        let r = match b {
                            1 => threshold_batch::<1>(ix, q, t_).0,
                            4 => threshold_batch::<4>(ix, q, t_).0,
                            16 => threshold_batch::<16>(ix, q, t_).0,
                            _ => threshold_batch::<64>(ix, q, t_).0,
                        };
                        for rr in &r {
                            std::hint::black_box(to_bitmap(ix, rr).len());
                        }
                    }
                });
                let s = t.s();
                let line = format!("{},{},{},{:.2},{:.2},{}", frac, th, b, (nb * b) as f64 / s, s * 1e3 / nb as f64, nb * b);
                out!("batch {line}");
                lines.push(line);
            }
        }
    }
    std::fs::write(format!("{RES}/batch_1m.csv"), lines.join("\n") + "\n").unwrap();
}

fn filtered(c: &mut Ctx, nq: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let mut lines = vec!["class,filter,frac,threads,what,median_ms,mean_ms,nq,touched_mean,result_mean,viol".to_string()];
    use rand::{seq::SliceRandom, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let nent = nrows;
    for &frac in &[0.5, 0.1, 0.01, 0.001] {
        for kind in ["random", "clustered"] {
            let bm: RoaringBitmap = if kind == "random" {
                let mut ids: Vec<u32> = (0..nent as u32).collect();
                ids.shuffle(&mut rng);
                let mut v: Vec<u32> = ids[..((nent as f64 * frac) as usize).max(1)].to_vec();
                v.sort_unstable();
                RoaringBitmap::from_sorted_iter(v).unwrap()
            } else {
                let mut cls: Vec<usize> = (0..ix.k()).collect();
                cls.shuffle(&mut rng);
                let want = (nent as f64 * frac) as usize;
                let mut v = Vec::new();
                for ci in cls {
                    let cl = &ix.cl[ci];
                    for r in cl.r0..cl.r1 {
                        if v.len() >= want.max(1) {
                            break;
                        }
                        v.push(ix.row_ent[r]);
                    }
                    if v.len() >= want.max(1) {
                        break;
                    }
                }
                v.sort_unstable();
                RoaringBitmap::from_sorted_iter(v).unwrap()
            };
            let t = Timer::new();
            let vis = Vis::new(ix, bm.clone(), false);
            let t_vis = t.ms();
            let visp = Vis { bits: vis.bits.clone(), cl_cnt: vis.cl_cnt.clone(), bm: bm.clone(), probe: true };
            out!("filter {} {}: {} visible; permuting the bitmap into row order (once per session) {:.2} ms", kind, frac, bm.len(), t_vis);
            for (cls, qs) in [("item", &c.items), ("text", &c.texts)] {
                let qs: Vec<Vec<f32>> = qs.iter().take(nq).cloned().collect();
                let ths: Vec<f64> = qs.iter().map(|q| {
                    let pr = prep(ix, q);
                    let sc = fl.scores(&pr.q32);
                    thresholds_for(&fl, &pr.q16, &sc, full_err(ix, &pr), &[(nrows / 100).max(1)])[0]
                }).collect();
                for th in [1usize, 4] {
                    let pl = pool(th);
                    for (what, vv) in [("index_thr_rowbits", &vis), ("index_thr_probe", &visp)] {
                        let (mut ts, mut tch, mut rs, mut viol) = (Vec::new(), 0.0, 0.0, 0u64);
                        for (qi, q) in qs.iter().enumerate() {
                            let o = Opts { vis: Some(vv), verify: false, want_iv: false, rec: false };
                            let mut best = f64::MAX;
                            let mut last = None;
                            for _ in 0..2 {
                                let t = Timer::new();
                                let r = pl.install(|| {
                                    let r = threshold(ix, q, ths[qi], &o);
                                    let b = to_bitmap(ix, &r.rows);
                                    (r, b)
                                });
                                best = best.min(t.ms());
                                last = Some(r);
                            }
                            let (r, b) = last.unwrap();
                            if th == 1 && what == "index_thr_rowbits" {
                                let pr = prep(ix, q);
                                let truth = fl.threshold(&pr.q16, &pr.q32, ths[qi], full_err(ix, &pr), Some(&bm), &c.ent_xs);
                                let (a, bb) = sym_diff(&b, &truth);
                                viol += a + bb;
                            }
                            tch += r.st.touched as f64;
                            rs += b.len() as f64;
                            ts.push(best);
                        }
                        let k = qs.len() as f64;
                        let line = format!("{},{},{},{},{},{:.3},{:.3},{},{:.0},{:.0},{}", cls, kind, frac, th, what, median(&ts), mean(&ts), qs.len(), tch / k, rs / k, viol);
                        out!("filter {line}");
                        lines.push(line);
                    }
                    // k-NN and flat scans of the visible set
                    let (mut tk, mut tf, mut tfk, mut bad) = (Vec::new(), Vec::new(), Vec::new(), 0);
                    for (qi, q) in qs.iter().enumerate() {
                        let pr = prep(ix, q);
                        let err = full_err(ix, &pr);
                        let t = Timer::new();
                        let r = pl.install(|| knn(ix, q, 10, Some(&vis)));
                        tk.push(t.ms());
                        let t = Timer::new();
                        let f = pl.install(|| fl.threshold(&pr.q16, &pr.q32, ths[qi], err, Some(&bm), &c.ent_xs));
                        tf.push(t.ms());
                        std::hint::black_box(f.len());
                        let t = Timer::new();
                        let fk = pl.install(|| fl.knn(&pr.q16, &pr.q32, 10, err, Some(&bm), &c.ent_xs));
                        tfk.push(t.ms());
                        let mut a: Vec<u32> = r.ents.iter().map(|x| x.1).collect();
                        let mut b: Vec<u32> = fk.iter().map(|x| x.1).collect();
                        a.sort();
                        b.sort();
                        if a != b {
                            bad += 1;
                        }
                    }
                    for (what, v, vi) in [("index_knn10", &tk, bad), ("flat_visible_thr", &tf, 0), ("flat_visible_knn10", &tfk, 0)] {
                        let line = format!("{},{},{},{},{},{:.3},{:.3},{},0,0,{}", cls, kind, frac, th, what, median(v), mean(v), v.len(), vi);
                        out!("filter {line}");
                        lines.push(line);
                    }
                }
            }
        }
    }
    std::fs::write(format!("{RES}/filter_1m.csv"), lines.join("\n") + "\n").unwrap();
}

/// Disk: levels past a residency cut and the full vectors in files, laid out as in memory (level-major, per cluster,
/// blocks in sort order; full vectors in row order). Pages = distinct 4 KiB pages the query's access pattern touches.
/// Timed with O_DIRECT preads (page cache bypassed), 1 and 4 threads.
fn disk(c: &mut Ctx, nq: usize) {
    use std::os::unix::fs::{FileExt, OpenOptionsExt};
    let ix = &c.ix;
    let n = ix.n();
    let nl = ix.meta.nl();
    let fl = c.flat();
    let dir = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/diskfiles";
    std::fs::create_dir_all(dir).unwrap();
    // files: one per level (planes; th words follow), and the full vectors in row order
    let mut lvl_base = vec![0u64; nl];
    let lv_path = format!("{dir}/levels.bin");
    let fv_path = format!("{dir}/full.bin");
    if !std::path::Path::new(&fv_path).exists() {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&fv_path).unwrap());
        for r in 0..ix.nrows {
            let x = ix.full_row_f16(r);
            let b = unsafe { std::slice::from_raw_parts(x.as_ptr() as *const u8, n * 2) };
            f.write_all(b).unwrap();
        }
    }
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&lv_path).unwrap());
        let mut off = 0u64;
        for l in 0..nl {
            lvl_base[l] = off;
            let b = unsafe { std::slice::from_raw_parts(ix.planes[l].as_ptr() as *const u8, ix.planes[l].len() * 2) };
            f.write_all(b).unwrap();
            off += b.len() as u64;
            let pad = (4096 - off % 4096) % 4096;
            f.write_all(&vec![0u8; pad as usize]).unwrap();
            off += pad;
        }
    }
    let cuts = [(5usize, "32B"), (12, "96B"), (17, "192B")];
    let mut lines = vec!["class,answer_frac,resident_through,variant,pages_mean,mb_mean,io_ms_1t,io_ms_4t,items_to_disk_mean".to_string()];
    let open_direct = |p: &str| std::fs::OpenOptions::new().read(true).custom_flags(0x4000).open(p).unwrap();
    let lvf = open_direct(&lv_path);
    let fvf = open_direct(&fv_path);
    // timed on at most 4,000 pages (an evenly spaced sample, same order), scaled to the full count
    let read_pages = |all: &[(bool, u64)], th: usize| -> f64 {
        let cap = 4000;
        let step = (all.len() + cap - 1) / cap;
        let pages: Vec<(bool, u64)> = all.iter().step_by(step.max(1)).copied().collect();
        let scale = all.len() as f64 / pages.len().max(1) as f64;
        let pages = &pages[..];
        let t = Timer::new();
        let pl = pool(th);
        pl.install(|| pages.par_chunks(64.max(pages.len() / (th * 8) + 1)).for_each(|ch| {
            let lay = std::alloc::Layout::from_size_align(4096, 4096).unwrap();
            let buf = unsafe { std::alloc::alloc(lay) };
            let b = unsafe { std::slice::from_raw_parts_mut(buf, 4096) };
            for &(full, pg) in ch {
                let f = if full { &fvf } else { &lvf };
                let _ = f.read_at(b, pg * 4096);
            }
            unsafe { std::alloc::dealloc(buf, lay) };
        }));
        t.ms() * scale
    };
    let nrows = fl.nrows;
    for (cls, qs) in [("item", c.items.clone()), ("text", c.texts.clone())] {
        for &frac in &[1e-3, 1e-2] {
            let k = ((nrows as f64 * frac).round() as usize).max(1);
            let mut acc: Vec<Vec<f64>> = vec![vec![0.0; 4]; cuts.len() * 2];
            let mut cnt = 0.0;
            for q in qs.iter().take(nq) {
                let pr = prep(ix, q);
                let sc = fl.scores(&pr.q32);
                let t = thresholds_for(&fl, &pr.q16, &sc, full_err(ix, &pr), &[k])[0];
                let r = threshold_prepped(ix, &pr, t, &Opts { vis: None, verify: false, want_iv: false, rec: true });
                cnt += 1.0;
                for (ci, &(cut, _)) in cuts.iter().enumerate() {
                    // walk: every non-resident level the item enters, then its full vector
                    let mut walk = std::collections::BTreeSet::new();
                    let mut straight = std::collections::BTreeSet::new();
                    let mut ndisk = 0u64;
                    for &(l, blk, m) in &r.rec {
                        let l = l as usize;
                        if l <= cut {
                            continue;
                        }
                        let cidx = ix.cl.partition_point(|x| x.blk0 + x.nblk <= blk as usize);
                        let cl = &ix.cl[cidx];
                        let b = blk as usize - cl.blk0;
                        let np = ix.groups[cl.g].nplanes[l] as u64;
                        let off = lvl_base[l] + 2 * (ix.lvl_off[l][cidx] as u64 + b as u64 * np);
                        for pg in off / 4096..=(off + 2 * np).saturating_sub(1) / 4096 {
                            walk.insert((false, pg));
                        }
                        if l == cut + 1 {
                            for i in 0..16 {
                                if m >> i & 1 == 1 {
                                    let row = (cl.r0 + b * 16 + i) as u64;
                                    ndisk += 1;
                                    for pg in row * 2 * n as u64 / 4096..=((row + 1) * 2 * n as u64 - 1) / 4096 {
                                        straight.insert((true, pg));
                                    }
                                }
                            }
                        }
                    }
                    for &row in &r.full_rows {
                        let row = row as u64;
                        for pg in row * 2 * n as u64 / 4096..=((row + 1) * 2 * n as u64 - 1) / 4096 {
                            walk.insert((true, pg));
                            if cut + 1 >= nl {
                                straight.insert((true, pg));
                            }
                        }
                    }
                    for (vi, set) in [walk, straight].into_iter().enumerate() {
                        let pages: Vec<(bool, u64)> = set.into_iter().collect();
                        let t1 = read_pages(&pages, 1);
                        let t4 = read_pages(&pages, 4);
                        let a = &mut acc[ci * 2 + vi];
                        a[0] += pages.len() as f64;
                        a[1] += t1;
                        a[2] += t4;
                        a[3] += ndisk as f64;
                    }
                }
            }
            for (ci, &(_, name)) in cuts.iter().enumerate() {
                for (vi, vname) in ["walk_levels_then_full", "straight_to_full"].iter().enumerate() {
                    let a = &acc[ci * 2 + vi];
                    let line = format!("{},{},{},{},{:.0},{:.1},{:.2},{:.2},{:.0}", cls, frac, name, vname, a[0] / cnt, a[0] / cnt * 4096.0 / 1e6, a[1] / cnt, a[2] / cnt, a[3] / cnt);
                    out!("disk {line}");
                    lines.push(line);
                }
            }
        }
    }
    std::fs::write(format!("{RES}/disk_1m.csv"), lines.join("\n") + "\n").unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: usize| -> usize { args.iter().position(|a| a == k).map(|i| args[i + 1].parse().unwrap()).unwrap_or(d) };
    let has = |k: &str| args.iter().any(|a| a == k);
    let kk = get("--clusters", 100000);
    let nanch = get("--anchors", 100000);
    if has("checkderive") {
        let mut p = load_params(DATA);
        let pf = format!("{DATA}/prof_qmix.npy");
        let (a, b) = apply_profile(&mut p, &pf);
        println!("qmix profile re-derived: {a} frb entries and {b} rate entries differ from the numpy export");
        return;
    }
    if has("--v2") {
        if has("--scale") {
            crate::scale::run2(get("--scale", 10), get("--nq", 20));
            return;
        }
        let c = run2::setup2("run2");
        if has("verify") {
            run2::verify2(&c, get("--vitem", 200), get("--vtext", 369), get("--vcheck", 25));
        }
        if has("levels") {
            run2::levels2(&c, get("--reps", 5));
        }
        if has("setup") {
            run2::setup_cost(&c, 10);
        }
        if has("bytesq") {
            run2::bytesq2(&c, get("--nq", 100));
        }
        if has("e2e") {
            run2::e2e2(&c, get("--nq", 100), get("--nflat", 20));
        }
        if has("breakdown") {
            run2::breakdown2(&c, get("--nq", 20));
        }
        if has("filter") {
            run2::filter2(&c, get("--fnq", 30));
        }
        return;
    }
    let tag = if has("--scale") { "scale" } else { "run1m" };
    if has("--scale") {
        scale::run(get("--scale", 10), get("--nq", 20), nanch.min(1000));
        return;
    }
    let mut c = setup(kk, nanch, tag);
    if has("bandwidth") {
        bandwidth(&mut c);
    }
    if has("levels") {
        levels(&mut c);
    }
    if has("verify") {
        let (a, b) = (get("--vitem", 200), get("--vtext", 369));
        verify(&mut c, a, b, get("--vcheck", 25));
    }
    if has("bytesq") {
        bytesq(&mut c, get("--nq", 100));
    }
    if has("e2e") {
        let nq = get("--nq", 100);
        e2e(&mut c, nq, get("--nflat", 20));
    }
    if has("batch") {
        batch(&mut c, get("--batchq", 256));
    }
    if has("disk") {
        disk(&mut c, get("--dnq", 20));
    }
    if has("filter") {
        filtered(&mut c, get("--fnq", 40));
    }
}

