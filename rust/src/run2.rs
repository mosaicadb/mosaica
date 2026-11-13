//! Experiments on the fast-scan index (v2): build, correctness against brute force, ns per item per level, end to end
//! against flat scans, filtered queries through the planner, and the synthetic scale test.
use crate::index::*;
use crate::kernel::*;
use crate::query::{thresholds_for, Flat};
use crate::util::*;
use crate::v2::q::*;
use crate::v2::*;
use rayon::prelude::*;
use roaring::RoaringBitmap;
use std::io::Write;

pub const DATA: &str = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/rustdata/m768";
pub const RES: &str = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/research-branch/rust/results";
const SIZES: [f64; 4] = [1e-4, 1e-3, 1e-2, 5e-2];

static LOG: std::sync::OnceLock<std::sync::Mutex<std::fs::File>> = std::sync::OnceLock::new();
macro_rules! out {
    ($($a:tt)*) => {{ let s = format!($($a)*); println!("{}", s); if let Some(l) = LOG.get() { writeln!(l.lock().unwrap(), "{}", s).unwrap(); } }};
}

fn pool(t: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap()
}

pub struct Ctx2 {
    pub p: Params,
    pub ix: Index2,
    pub xs: RealSource,
    pub ent_xs: Vec<u32>,
    pub items: Vec<Vec<f32>>,
    pub texts: Vec<Vec<f32>>,
    pub plan: Plan,
}

pub fn args() -> Vec<String> {
    std::env::args().collect()
}
pub fn arg_usize(k: &str, d: usize) -> usize {
    let a = args();
    a.iter().position(|x| x == k).map(|i| a[i + 1].parse().unwrap()).unwrap_or(d)
}
pub fn arg_str(k: &str, d: &str) -> String {
    let a = args();
    a.iter().position(|x| x == k).map(|i| a[i + 1].clone()).unwrap_or(d.to_string())
}

pub fn setup2(tag: &str) -> Ctx2 {
    let log = std::fs::OpenOptions::new().create(true).append(true).open(format!("{RES}/{tag}.log")).unwrap();
    let _ = LOG.set(std::sync::Mutex::new(log));
    out!("==== run at {:.0}: {:?}", now_s(), args());
    let kk = arg_usize("--clusters", 100000);
    let prof_path = arg_str("--profile", &format!("{DATA}/prof_h0.25.npy"));
    let split = !args().iter().any(|a| a == "--u8");
    let t = Timer::new();
    let mut p = load_params(DATA);
    if kk < p.meta.k {
        let n = p.meta.n;
        p.meta.k = kk;
        p.cstart.truncate(kk + 1);
        p.grp.truncate(kk);
        p.centres.truncate(kk * n);
    }
    let prof = crate::derive::load_profile(&prof_path);
    let xs = RealSource::open(&p.xs_path);
    let src = Real2 { src: RealSource::open(&p.xs_path) };
    let f2 = RealSource::open(&p.xs_path);
    let full = Full2::Real { map: f2.map, off: f2.off, xs_row: Vec::new() };
    let mut lg = |s: String| out!("[build2] {s}");
    let mut ix = build2(&p, &prof, &src, full, p.meta.k, split, &mut lg);
    build_table2(&mut ix, p.meta.k);
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
    let plan = Plan {
        mode: match arg_str("--plan", "auto").as_str() { "index" => PlanMode::Index, "flat" => PlanMode::Flat, _ => PlanMode::Auto },
        alpha0: args().iter().any(|a| a == "--alpha0"),
        jump_level: arg_usize("--jump", 0),
        ..Plan::default()
    };
    let c = Ctx2 { p, ix, xs, ent_xs, items, texts, plan };
    out!("built v2 ({} tables, profile {}) {} rows, {} clusters in {:.1}s; stored bytes/item cumulative by level: {:?}", if split { "16-bit" } else { "8-bit" }, prof_path, c.ix.nrows, c.ix.k(), t.s(),
        c.ix.stored_bytes_per_item().iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>());
    c
}

impl Ctx2 {
    pub fn flat(&self) -> Flat<'_> {
        Flat { map: &self.xs.map, off: self.xs.off, n: self.p.meta.n, nrows: self.p.cstart[self.p.meta.k], ent: &self.p.perm }
    }
}

fn sym_diff(a: &RoaringBitmap, b: &[u32]) -> (u64, u64) {
    let bb = RoaringBitmap::from_sorted_iter(b.iter().copied()).unwrap();
    ((a - &bb).len(), (&bb - a).len())
}

pub fn bytes_read2(ix: &Index2, st: &Stats) -> f64 {
    let sb = ix.stored_bytes_per_item();
    let mut b = 0.0;
    for l in 0..ix.meta.nl() {
        b += st.enter[l] as f64 * (sb[l] - if l == 0 { 0.0 } else { sb[l - 1] });
    }
    (b + st.full_reads as f64 * 2.0 * ix.n() as f64) / ix.nrows as f64
}

/// Bytes read per item (of N) and items touched, by class and answer size; no timing.
pub fn bytesq2(c: &Ctx2, nq: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((fl.nrows as f64 * f).round() as usize).max(1)).collect();
    for (cls, qs) in [("item", &c.items), ("text", &c.texts)] {
        let k = qs.len().min(nq);
        let mut br = vec![0.0; 4];
        let mut tch = vec![0.0; 4];
        for q in qs.iter().take(nq) {
            let pr = prep2(ix, q);
            let sc = fl.scores(&pr.q32);
            let ths = thresholds_for(&fl, &pr.q16, &sc, full_err2(ix, &pr), &sizes);
            for si in 0..4 {
                let o = Opts2 { vis: None, verify: false, want_iv: false, plan: Plan { mode: PlanMode::Index, ..c.plan } };
                let r = threshold2(ix, &prep2(ix, q), ths[si], &o);
                br[si] += bytes_read2(ix, &r.st) / k as f64;
                tch[si] += r.st.touched as f64 / ix.nrows as f64 / k as f64;
            }
        }
        out!("bytesq v2 {cls} ({k} queries): bytes read per item {:?}; touched {:?}", br.iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<_>>(), tch.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>());
    }
}

/// Every threshold query and k-NN against brute force; intervals checked at every level on the first `ncheck`.
pub fn verify2(c: &Ctx2, nitem: usize, ntext: usize, ncheck: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((nrows as f64 * f).round() as usize).max(1)).collect();
    let mut qs: Vec<(&str, &Vec<f32>)> = c.items.iter().take(nitem).map(|q| ("item", q)).collect();
    let ni = qs.len();
    qs.extend(c.texts.iter().take(ntext).map(|q| ("text", q)));
    let mut tot = Stats::new(ix.meta.nl());
    let (mut wi, mut wo, mut kb) = (0u64, 0u64, 0u64);
    let mut rows = vec!["q,class,answer_frac,t,truth_size,wrong_in,wrong_out,touched,full_reads,bytes_read".to_string()];
    let t = Timer::new();
    for (qi, (cls, q)) in qs.iter().enumerate() {
        let pr = prep2(ix, q);
        let err = full_err2(ix, &pr);
        let sc = fl.scores(&pr.q32);
        let ths = thresholds_for(&fl, &pr.q16, &sc, err, &sizes);
        let chk = (qi < ncheck) || (qi >= ni && qi < ni + ncheck);
        for (si, &th) in ths.iter().enumerate() {
            let truth = fl.threshold(&pr.q16, &pr.q32, th, err, None, &c.ent_xs);
            let pr2 = prep2(ix, q);
            let o = Opts2 { vis: None, verify: chk, want_iv: false, plan: c.plan };
            let r = threshold2(ix, &pr2, th, &o);
            let bm = to_bitmap2(ix, &r.rows);
            let (a, b) = sym_diff(&bm, &truth);
            wi += a;
            wo += b;
            tot.add(&r.st);
            rows.push(format!("{},{},{},{},{},{},{},{},{},{:.3}", qi, cls, SIZES[si], th, truth.len(), a, b, r.st.touched, r.st.full_reads, bytes_read2(ix, &r.st)));
        }
        let kn = knn2(ix, q, 10, None, c.plan);
        let fk = fl.knn(&pr.q16, &pr.q32, 10, err, None, &c.ent_xs);
        let mut a: Vec<u32> = kn.ents.iter().map(|x| x.1).collect();
        let mut b: Vec<u32> = fk.iter().map(|x| x.1).collect();
        a.sort();
        b.sort();
        if a != b {
            kb += 1;
            out!("knn mismatch q{} {}", qi, cls);
        }
        if qi % 50 == 0 {
            println!("verify2 {qi}/{} {:.0}s wrong in/out {wi}/{wo} det {} rand {} checked {} maxsd {:.2}", qs.len(), t.s(), tot.viol_det, tot.viol_rand, tot.checked, tot.max_sd);
        }
    }
    out!("VERIFY2 {} queries x 4 answer sizes: wrong-in {} wrong-out {}; interval checks {} (det violations {}, rand violations {}); largest randomised error {:.2} sd (c = {:.2}); k-NN mismatches {} of {}",
        qs.len(), wi, wo, tot.checked, tot.viol_det, tot.viol_rand, tot.max_sd, ix.meta.csg, kb, qs.len());
    std::fs::write(format!("{RES}/verify2.csv"), rows.join("\n") + "\n").unwrap();
}

/// ns per item per level: all items of every cluster, no decisions (interval at each level), tables built beforehand.
pub fn levels2(c: &Ctx2, reps: usize) {
    let ix = &c.ix;
    let nl = ix.meta.nl();
    let bytes = ix.stored_bytes_per_item();
    let mut lines = vec!["query,level,level_bytes_cum,stored_bytes_cum,ns_per_item_cum,ns_per_item_level,ns_per_stored_byte_level".to_string()];
    for (name, q) in [("item", c.items[0].clone()), ("text", c.texts[0].clone())] {
        let pr = prep2(ix, &q);
        let mut st = Stats::new(nl);
        let mut cqs: Vec<CQ2> = (0..ix.k()).map(|ci| {
            let gq = get_gq2(ix, &pr, ix.cl[ci].g, &mut st);
            cluster_q2(ix, ci, gq, &pr, dot64(&pr.q64, ix.centres_f64(ci)))
        }).collect();
        // build every table once (lazily inside the first run)
        let cands: Vec<Vec<u32>> = ix.cl.iter().map(|cl| (0..(cl.r1 - cl.r0) as u32).collect()).collect();
        let mut sv = State::default();
        for ci in 0..ix.k() {
            let (mut a, mut b, mut iv) = (Vec::new(), Vec::new(), Vec::new());
            let mut out = RunOut { rows_in: &mut a, pend: &mut b, iv: Some(&mut iv) };
            unsafe { run_cluster(ix, ci, &pr, pr.gq[ix.cl[ci].g].get().unwrap(), &mut cqs[ci], &cands[ci], f64::NAN, Mode2::Interval(nl - 1), 0, false, &mut sv, &mut st, &mut out) };
        }
        let mut prev = 0.0;
        for l in 0..nl {
            let mut best = f64::MAX;
            for _ in 0..reps {
                let t = Timer::new();
                let mut st = Stats::new(nl);
                let mut iv = Vec::with_capacity(2048);
                for ci in 0..ix.k() {
                    iv.clear();
                    let (mut a, mut b) = (Vec::new(), Vec::new());
                    let mut out = RunOut { rows_in: &mut a, pend: &mut b, iv: Some(&mut iv) };
                    unsafe { run_cluster(ix, ci, &pr, pr.gq[ix.cl[ci].g].get().unwrap(), &mut cqs[ci], &cands[ci], f64::NAN, Mode2::Interval(l), 0, false, &mut sv, &mut st, &mut out) };
                }
                best = best.min(t.s());
            }
            let ns = best * 1e9 / ix.nrows as f64;
            let sb = bytes[l] - if l == 0 { 2.0 } else { bytes[l - 1] };
            let line = format!("{},{},{},{:.2},{:.2},{:.2},{:.4}", name, l, ix.meta.level_bytes(l), bytes[l], ns, ns - prev, (ns - prev) / sb);
            out!("levels2 {line}");
            lines.push(line);
            prev = ns;
        }
    }
    std::fs::write(format!("{RES}/levels2{}.csv", if ix.split { "" } else { "_u8" }), lines.join("\n") + "\n").unwrap();
}

/// Fixed setup per query: group setup (exact int8 basis product, rotated weights) and cluster setup with tables.
pub fn setup_cost(c: &Ctx2, nq: usize) {
    let ix = &c.ix;
    for (cls, qs) in [("item", &c.items), ("text", &c.texts)] {
        let (mut tg, mut tc, mut tl, mut tq, mut tw, mut trot) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for q in qs.iter().take(nq) {
            let pr = prep2(ix, q);
            let mut st = Stats::new(ix.meta.nl());
            let t = Timer::new();
            for g in 0..ix.groups.len() {
                get_gq2(ix, &pr, g, &mut st);
            }
            tg.push(t.ms());
            let t = Timer::new();
            let mut qv = vec![0f64; ix.n()];
            for g in 0..ix.groups.len() {
                let gp = &ix.groups[g];
                unsafe { crate::v2::q::qv_exact(&gp.v8t, &gp.vsc, ix.n(), &pr.qhi, &pr.qlo, &mut qv) };
                std::hint::black_box(&qv);
            }
            tq.push(t.ms());
            // the remaining parts of the group setup, timed alone (warm)
            let t = Timer::new();
            for g in 0..ix.groups.len() {
                let gp = &ix.groups[g];
                let n = ix.n();
                let mut a2 = 0.0;
                for j in 0..crate::v2::J {
                    let nbj = gp.nb[j];
                    let xs: Vec<f64> = (0..nbj).map(|b| qv[gp.ridx[gp.boff[j] + b] as usize] * gp.dt[gp.ridx[gp.boff[j] + b] as usize] as f64).collect();
                    for a in 0..nbj {
                        a2 += dot64(&ix.rots[j][a * nbj..(a + 1) * nbj], &xs);
                    }
                }
                std::hint::black_box(a2);
                let _ = n;
            }
            trot.push(t.ms());
            let t = Timer::new();
            let mut cqs: Vec<CQ2> = (0..ix.k()).map(|ci| cluster_q2(ix, ci, pr.gq[ix.cl[ci].g].get().unwrap(), &pr, dot64(&pr.q64, ix.centres_f64(ci)))).collect();
            tc.push(t.ms());
            let t = Timer::new();
            for (ci, cq) in cqs.iter_mut().enumerate() {
                let gp = &ix.groups[ix.cl[ci].g];
                for l in 0..ix.meta.nl() {
                    let mut lut = crate::v2::kern::LevelLut::default();
                    unsafe { crate::v2::kern::build_lut(&cq.w, &gp.levels[l], &ix.etab, ix.split, &mut lut) };
                    cq.luts[l] = Some(lut);
                }
            }
            tl.push(t.ms());
            // warm: the same cluster's tables 200 times
            let gp = &ix.groups[ix.cl[0].g];
            let t = Timer::new();
            let mut lut = crate::v2::kern::LevelLut::default();
            let mut nu = 0;
            for _ in 0..200 {
                for l in 0..ix.meta.nl() {
                    unsafe { crate::v2::kern::build_lut(&cqs[0].w, &gp.levels[l], &ix.etab, ix.split, &mut lut) };
                    nu += gp.levels[l].units;
                }
            }
            tw.push(t.ms() * 1e6 / nu as f64);
        }
        out!("setup2 {cls} (1 thread, {} groups, {} clusters): group setup {:.2} ms (int8 basis products alone, warm {:.2} ms; rotations alone {:.2} ms), cluster setup {:.2} ms ({:.2} us each), all tables of every cluster and level {:.2} ms ({:.2} us per cluster; warm {:.1} ns per unit)",
            ix.groups.len(), ix.k(), median(&tg), median(&tq), median(&trot), median(&tc), median(&tc) * 1e3 / ix.k() as f64, median(&tl), median(&tl) * 1e3 / ix.k() as f64, median(&tw));
    }
}

pub fn e2e2(c: &Ctx2, nq: usize, nflat: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let n = ix.n();
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((nrows as f64 * f).round() as usize).max(1)).collect();
    let x8: Vec<i8> = if nflat > 0 { (0..nrows * n).into_par_iter().map(|i| (f16_to_f32(fl.row(i / n)[i % n]) * 127.0).round().clamp(-127.0, 127.0) as i8).collect() } else { Vec::new() };
    let mut lines = vec!["class,threads,what,answer_frac,median_ms,mean_ms,p90_ms,nq,touched_mean,full_reads_mean,result_mean,bytes_read_per_item,occupancy".to_string()];
    for (cls, qs) in [("item", c.items.clone()), ("text", c.texts.clone())] {
        let qs: Vec<Vec<f32>> = qs.into_iter().take(nq).collect();
        let ths: Vec<Vec<f64>> = qs.iter().map(|q| {
            let pr = prep2(ix, q);
            let sc = fl.scores(&pr.q32);
            thresholds_for(&fl, &pr.q16, &sc, full_err2(ix, &pr), &sizes)
        }).collect();
        for th in [1usize, 4] {
            let pl = pool(th);
            for si in 0..4 {
                let mut ts = Vec::new();
                let (mut tch, mut fr, mut rs, mut br, mut ent, mut blk) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                let mut enter = vec![0f64; ix.meta.nl() + 1];
                for (qi, q) in qs.iter().enumerate() {
                    let o = Opts2 { vis: None, verify: false, want_iv: false, plan: c.plan };
                    let mut best = f64::MAX;
                    let mut last = None;
                    for _ in 0..2 {
                        let t = Timer::new();
                        let r = pl.install(|| {
                            let pr = prep2(ix, q);
                            let r = threshold2(ix, &pr, ths[qi][si], &o);
                            let bm = to_bitmap2(ix, &r.rows);
                            (r, bm)
                        });
                        best = best.min(t.ms());
                        last = Some(r);
                    }
                    let (r, bm) = last.unwrap();
                    br += bytes_read2(ix, &r.st);
                    tch += r.st.touched as f64;
                    fr += r.st.full_reads as f64;
                    rs += bm.len() as f64;
                    ent += r.st.enter[..ix.meta.nl()].iter().sum::<u64>() as f64;
                    blk += r.st.blocks.iter().sum::<u64>() as f64;
                    for (e, x) in enter.iter_mut().zip(&r.st.enter) {
                        *e += *x as f64;
                    }
                    ts.push(best);
                }
                let k = qs.len() as f64;
                let mut s = ts.clone();
                let occ = ent / (32.0 * blk).max(1.0);
                let line = format!("{},{},index2_thr,{},{:.3},{:.3},{:.3},{},{:.0},{:.1},{:.0},{:.2},{:.3}", cls, th, SIZES[si], median(&ts), mean(&ts), quantile(&mut s, 0.9), qs.len(), tch / k, fr / k, rs / k, br / k, occ);
                out!("e2e2 {line}");
                if th == 1 {
                    out!("e2e2 enter-fraction by level ({cls}, {}): {:?}", SIZES[si], enter.iter().map(|x| (x / k / ix.nrows as f64 * 1e4).round() / 1e4).collect::<Vec<_>>());
                }
                lines.push(line);
            }
            let mut ts = Vec::new();
            let (mut seed, mut sweep, mut sel) = (0.0, 0.0, 0.0);
            for q in &qs {
                let mut best = f64::MAX;
                let mut last = None;
                for _ in 0..2 {
                    let t = Timer::new();
                    let r = pl.install(|| knn2(ix, q, 10, None, c.plan));
                    best = best.min(t.ms());
                    last = Some(r);
                }
                let r = last.unwrap();
                seed += r.t_seed;
                sweep += r.t_sweep;
                sel += r.t_sel;
                ts.push(best);
            }
            let k = qs.len() as f64;
            let mut s = ts.clone();
            let line = format!("{},{},index2_knn10,0,{:.3},{:.3},{:.3},{},0,0,10,0,0", cls, th, median(&ts), mean(&ts), quantile(&mut s, 0.9), qs.len());
            out!("e2e2 {line}   (seed {:.2} / sweep {:.2} / selection {:.2} ms mean)", seed / k, sweep / k, sel / k);
            lines.push(line);
            if nflat == 0 {
                continue;
            }
            let (mut tf, mut tk, mut t8) = (Vec::new(), Vec::new(), Vec::new());
            for (qi, q) in qs.iter().take(nflat).enumerate() {
                let pr = prep2(ix, q);
                let err = full_err2(ix, &pr);
                let (mut b1, mut b2, mut b3) = (f64::MAX, f64::MAX, f64::MAX);
                let qmax = pr.q32.iter().fold(0f32, |a, &x| a.max(x.abs()));
                let q16i: Vec<i16> = pr.q32.iter().map(|&x| (x / qmax * 8191.0).round() as i16).collect();
                let t8thr = (ths[qi][1] * 127.0 * 8191.0 / qmax as f64) as i32;
                for _ in 0..2 {
                    let t = Timer::new();
                    std::hint::black_box(pl.install(|| fl.threshold(&pr.q16, &pr.q32, ths[qi][1], err, None, &c.ent_xs)).len());
                    b1 = b1.min(t.ms());
                    let t = Timer::new();
                    std::hint::black_box(pl.install(|| fl.knn(&pr.q16, &pr.q32, 10, err, None, &c.ent_xs)).len());
                    b2 = b2.min(t.ms());
                    let t = Timer::new();
                    std::hint::black_box(pl.install(|| (0..nrows).into_par_iter().with_min_len(4096).filter(|&r| unsafe { dot_i8(&q16i, &x8[r * n..(r + 1) * n]) } >= t8thr).count()));
                    b3 = b3.min(t.ms());
                }
                tf.push(b1);
                tk.push(b2);
                t8.push(b3);
            }
            for (what, v) in [("flat_f16_thr", &tf), ("flat_f16_knn10", &tk), ("flat_int8_thr", &t8)] {
                let mut s = v.clone();
                let line = format!("{},{},{},0.001,{:.3},{:.3},{:.3},{},{},0,0,0,0", cls, th, what, median(v), mean(v), quantile(&mut s, 0.9), v.len(), nrows);
                out!("e2e2 {line}");
                lines.push(line);
            }
        }
    }
    std::fs::write(format!("{RES}/e2e2_1m{}.csv", if ix.split { "" } else { "_u8" }), lines.join("\n") + "\n").unwrap();
}

/// Where the time of a threshold query goes (text queries at 1% and 5%; 1 and 4 threads), for plan variants.
pub fn breakdown2(c: &Ctx2, nq: usize) {
    use std::sync::atomic::Ordering;
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    // time-stamp-counter rate
    let (t, c0) = (Timer::new(), unsafe { core::arch::x86_64::_rdtsc() });
    while t.ms() < 200.0 {}
    let hz = (unsafe { core::arch::x86_64::_rdtsc() } - c0) as f64 / t.s();
    let names = ["anchor", "cut", "basis", "rotations", "cluster setup", "tables", "re-pack", "level decode", "bounds+decisions+compaction", "full vectors", "candidate init", "result bitmap"];
    let mut lines = vec!["variant,class,answer_frac,threads,wall_ms,cpu_ms_total,".to_string() + &names.join(",") + ",bytes_read,full_reads,wrong"];
    let qs: Vec<Vec<f32>> = c.texts.iter().take(nq).cloned().collect();
    for &frac in &[0.01, 0.05] {
        let kk = ((nrows as f64 * frac).round() as usize).max(1);
        let ths: Vec<f64> = qs.iter().map(|q| {
            let pr = prep2(ix, q);
            let sc = fl.scores(&pr.q32);
            thresholds_for(&fl, &pr.q16, &sc, full_err2(ix, &pr), &[kk])[0]
        }).collect();
        let truths: Vec<Vec<u32>> = qs.iter().zip(&ths).map(|(q, &t)| {
            let pr = prep2(ix, q);
            fl.threshold(&pr.q16, &pr.q32, t, full_err2(ix, &pr), None, &c.ent_xs)
        }).collect();
        for (vname, alpha0, jump) in [("levels", false, 0usize), ("shared_tables", true, 0), ("single_pass_64B", false, 9), ("single_pass_96B", false, 12), ("single_pass_128B", false, 14), ("shared+single_96B", true, 12)] {
            let plan = Plan { alpha0, jump_level: jump, jump_d: 0.8, ..c.plan };
            for th in [1usize, 4] {
                let pl = pool(th);
                let mut cyc = [0f64; 12];
                let (mut walls, mut br, mut fr, mut wrong) = (Vec::new(), 0.0, 0.0, 0u64);
                for (qi, q) in qs.iter().enumerate() {
                    let mut best = f64::MAX;
                    let mut keep = None;
                    for rep in 0..3 {
                        PROF.store(rep == 2, Ordering::Relaxed);
                        let tm = Timer::new();
                        let r = pl.install(|| {
                            let a0 = tick();
                            let pr = prep2(ix, q);
                            let a1 = tick();
                            let mut r = threshold2(ix, &pr, ths[qi], &Opts2 { vis: None, verify: false, want_iv: false, plan });
                            let a2 = tick();
                            let bm = to_bitmap2(ix, &r.rows);
                            r.st.cyc[P_ANCHOR] += a1 - a0;
                            r.st.cyc[P_PLAN] += tick() - a2;
                            (r, bm)
                        });
                        let w = tm.ms();
                        if rep < 2 {
                            best = best.min(w);
                        } else {
                            keep = Some(r);
                        }
                    }
                    PROF.store(false, Ordering::Relaxed);
                    let (r, bm) = keep.unwrap();
                    for k in 0..12 {
                        cyc[k] += r.st.cyc[k] as f64 / hz * 1e3 / qs.len() as f64;
                    }
                    br += bytes_read2(ix, &r.st) / qs.len() as f64;
                    fr += r.st.full_reads as f64 / qs.len() as f64;
                    let (a, b) = sym_diff(&bm, &truths[qi]);
                    wrong += a + b;
                    walls.push(best);
                }
                let tot: f64 = cyc.iter().sum();
                let line = format!("{},text,{},{},{:.2},{:.2},{},{:.2},{:.0},{}", vname, frac, th, median(&walls), tot, cyc.iter().map(|x| format!("{:.2}", x)).collect::<Vec<_>>().join(","), br, fr, wrong);
                out!("breakdown2 {line}");
                lines.push(line);
            }
        }
    }
    std::fs::write(format!("{RES}/breakdown2_text.csv"), lines.join("\n") + "\n").unwrap();
}

pub fn make_filter(ix: &Index2, kind: &str, frac: f64, rng: &mut rand::rngs::StdRng) -> RoaringBitmap {
    use rand::seq::SliceRandom;
    let nent = ix.nrows;
    if kind == "random" {
        let mut ids: Vec<u32> = (0..nent as u32).collect();
        ids.shuffle(rng);
        let mut v: Vec<u32> = ids[..((nent as f64 * frac) as usize).max(1)].to_vec();
        v.sort_unstable();
        RoaringBitmap::from_sorted_iter(v).unwrap()
    } else {
        let mut cls: Vec<usize> = (0..ix.k()).collect();
        cls.shuffle(rng);
        let want = ((nent as f64 * frac) as usize).max(1);
        let mut v = Vec::new();
        'o: for ci in cls {
            for r in ix.cl[ci].r0..ix.cl[ci].r1 {
                if v.len() >= want {
                    break 'o;
                }
                v.push(ix.row_ent[r]);
            }
        }
        v.sort_unstable();
        RoaringBitmap::from_sorted_iter(v).unwrap()
    }
}

/// Filtered threshold (at the unfiltered 1% answer) and k-NN through the planner, the forced paths, and a flat scan of
/// the visible set.
pub fn filter2(c: &Ctx2, nq: usize) {
    use rand::SeedableRng;
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let mut lines = vec!["class,filter,frac,threads,what,median_ms,mean_ms,nq,touched_mean,result_mean,wrong,index_clusters,flat_clusters".to_string()];
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    for &frac in &[0.5, 0.1, 0.01, 0.001] {
        for kind in ["random", "clustered"] {
            let bm = make_filter(ix, kind, frac, &mut rng);
            let t = Timer::new();
            let vis = Vis2::new(ix, &bm);
            out!("filter2 {kind} {frac}: {} visible; permuting into row order {:.2} ms", bm.len(), t.ms());
            for (cls, qs) in [("item", &c.items), ("text", &c.texts)] {
                let qs: Vec<Vec<f32>> = qs.iter().take(nq).cloned().collect();
                let ths: Vec<f64> = qs.iter().map(|q| {
                    let pr = prep2(ix, q);
                    let sc = fl.scores(&pr.q32);
                    thresholds_for(&fl, &pr.q16, &sc, full_err2(ix, &pr), &[(nrows / 100).max(1)])[0]
                }).collect();
                let truths: Vec<Vec<u32>> = qs.iter().zip(&ths).map(|(q, &t)| {
                    let pr = prep2(ix, q);
                    fl.threshold(&pr.q16, &pr.q32, t, full_err2(ix, &pr), Some(&bm), &c.ent_xs)
                }).collect();
                for th in [1usize, 4] {
                    let pl = pool(th);
                    for (what, mode) in [("planner", PlanMode::Auto), ("index_only", PlanMode::Index), ("range_then_full", PlanMode::Flat)] {
                        let plan = Plan { mode, ..c.plan };
                        let (mut ts, mut tch, mut rs, mut wrong, mut ic, mut fc) = (Vec::new(), 0.0, 0.0, 0u64, 0.0, 0.0);
                        for (qi, q) in qs.iter().enumerate() {
                            let o = Opts2 { vis: Some(&vis), verify: false, want_iv: false, plan };
                            let mut best = f64::MAX;
                            let mut last = None;
                            for _ in 0..2 {
                                let t = Timer::new();
                                let r = pl.install(|| {
                                    let pr = prep2(ix, q);
                                    let r = threshold2(ix, &pr, ths[qi], &o);
                                    let b = to_bitmap2(ix, &r.rows);
                                    (r, b)
                                });
                                best = best.min(t.ms());
                                last = Some(r);
                            }
                            let (r, b) = last.unwrap();
                            let (a, bb) = sym_diff(&b, &truths[qi]);
                            wrong += a + bb;
                            tch += r.st.touched as f64;
                            rs += b.len() as f64;
                            ic += r.idx_clusters as f64;
                            fc += r.flat_clusters as f64 + if r.scanned { 10000.0 } else { 0.0 };
                            ts.push(best);
                        }
                        let k = qs.len() as f64;
                        let line = format!("{},{},{},{},{},{:.3},{:.3},{},{:.0},{:.0},{},{:.0},{:.0}", cls, kind, frac, th, what, median(&ts), mean(&ts), qs.len(), tch / k, rs / k, wrong, ic / k, fc / k);
                        out!("filter2 {line}");
                        lines.push(line);
                    }
                    let (mut tk, mut tf, mut tfk, mut bad) = (Vec::new(), Vec::new(), Vec::new(), 0);
                    for (qi, q) in qs.iter().enumerate() {
                        let pr = prep2(ix, q);
                        let err = full_err2(ix, &pr);
                        let t = Timer::new();
                        let r = pl.install(|| knn2(ix, q, 10, Some(&vis), c.plan));
                        tk.push(t.ms());
                        let t = Timer::new();
                        std::hint::black_box(pl.install(|| fl.threshold(&pr.q16, &pr.q32, ths[qi], err, Some(&bm), &c.ent_xs)).len());
                        tf.push(t.ms());
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
                    for (what, v, vi) in [("planner_knn10", &tk, bad), ("flat_visible_thr", &tf, 0), ("flat_visible_knn10", &tfk, 0)] {
                        let line = format!("{},{},{},{},{},{:.3},{:.3},{},0,0,{},0,0", cls, kind, frac, th, what, median(v), mean(v), v.len(), vi);
                        out!("filter2 {line}");
                        lines.push(line);
                    }
                }
            }
        }
    }
    std::fs::write(format!("{RES}/filter2_1m.csv"), lines.join("\n") + "\n").unwrap();
}
