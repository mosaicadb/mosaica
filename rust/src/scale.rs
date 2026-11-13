//! Scale beyond 1M: synthetic items from the fitted cluster model (see synth.rs), same build and query code.
use crate::index::*;
use crate::kernel::*;
use crate::query::*;
use crate::synth::*;
use crate::util::*;
use rayon::prelude::*;

const DATA: &str = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/rustdata/m768";
const RES: &str = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/research-branch/rust/results";

fn log(s: String) {
    println!("{s}");
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(format!("{RES}/scale.log")).unwrap();
    writeln!(f, "{s}").unwrap();
}

/// support table for synthetic items: p.x = p.m + (V^T p).z
fn synth_table(ix: &mut Index, src: &SynthSource, p: &Params, nanch: usize) {
    let n = ix.n();
    let k = ix.k();
    let step = k as f64 / nanch as f64;
    let mut anchors = vec![0f64; nanch * n];
    for a in 0..nanch {
        let c = (a as f64 * step) as usize;
        anchors[a * n..(a + 1) * n].copy_from_slice(&ix.cl[c].mhat);
    }
    // V_g^T p for every group and anchor
    let pv: Vec<Vec<f64>> = (0..ix.groups.len()).into_par_iter().map(|g| {
        let v = &ix.groups[g].vf64;
        let mut out = vec![0f64; nanch * n];
        unsafe { matrixmultiply::dgemm(nanch, n, n, 1.0, anchors.as_ptr(), n as isize, 1, v.as_ptr(), n as isize, 1, 0.0, out.as_mut_ptr(), n as isize, 1) };
        out
    }).collect();
    let cols: Vec<(Vec<f32>, Vec<f32>)> = (0..k).into_par_iter().map(|c| {
        let g = ix.cl[c].g;
        let it = src.items(c, p, &ix.groups[g], &ix.cl[c].mhat);
        let nc = it.ah.len();
        let mut px = vec![0f64; nc * nanch];
        unsafe { matrixmultiply::dgemm(nc, n, nanch, 1.0, it.z.as_ptr(), n as isize, 1, pv[g].as_ptr(), 1, n as isize, 0.0, px.as_mut_ptr(), nanch as isize, 1) };
        let m = ix.centres_f64(c);
        let mut hi = vec![0f32; nanch];
        let mut lo = vec![0f32; nanch];
        for a in 0..nanch {
            let pm = dot64(&anchors[a * n..(a + 1) * n], m);
            let (mut mx, mut mi) = (f64::NEG_INFINITY, f64::INFINITY);
            for i in 0..nc {
                mx = mx.max(px[i * nanch + a]);
                mi = mi.min(px[i * nanch + a]);
            }
            hi[a] = ((((pm + mx) + 1e-9) * 32768.0).ceil() / 32768.0) as f32;
            lo[a] = ((((pm + mi) - 1e-9) * 32768.0).floor() / 32768.0) as f32;
        }
        (hi, lo)
    }).collect();
    ix.hi = vec![0f32; nanch * k];
    ix.lo = vec![0f32; nanch * k];
    for c in 0..k {
        for a in 0..nanch {
            ix.hi[a * k + c] = cols[c].0[a];
            ix.lo[a * k + c] = cols[c].1[a];
        }
    }
    ix.anchors = anchors;
    ix.nanch = nanch;
}

pub fn run(scale: usize, nq: usize, nanch: usize) {
    let t = Timer::new();
    log(format!("==== scale run x{scale} at {:.0}", now_s()));
    let p = load_params(DATA);
    let real = build(&p, Box::new(RealSource::open(&p.xs_path)), 1, &mut |s| log(format!("[real] {s}")));
    let sy = Synth::new(&p, &real, scale);
    let groups_ref: Vec<GroupPlan> = (0..p.meta.g).map(|g| GroupPlan::new(&p, &real.tree, g)).collect();
    let src = SynthSource { sy, groups: &groups_ref };
    let n = p.meta.n;
    // the build consumes the source; keep a second one for the table and truth passes
    let src2 = SynthSource { sy: Synth::new(&p, &real, scale), groups: &groups_ref };
    let texts: Vec<Vec<f32>> = p.text_q.chunks(n).map(|v| {
        let nn = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / nn).collect()
    }).collect();
    drop(real);
    let src_box: Box<dyn Source + '_> = Box::new(src);
    let src_static: Box<dyn Source> = unsafe { std::mem::transmute(src_box) };
    let mut ix = build(&p, src_static, nanch, &mut |s| log(format!("[synth] {s}")));
    synth_table(&mut ix, &src2, &p, nanch);
    log(format!("synthetic index: {} rows, {} clusters, {} anchors, built in {:.0}s; stored B/item {:?}", ix.nrows, ix.k(), nanch, t.s(),
        ix.stored_bytes_per_item().iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>()));
    // queries: text, and synthetic items (their x in the ambient space)
    let mut qs: Vec<(&str, Vec<f32>)> = Vec::new();
    for i in 0..nq {
        let row = (i * 7919 * 104729) % ix.nrows;
        let mut x = vec![0f64; n];
        if let Full::Synth(sy) = &ix.full {
            sy.x_f64(&ix, row, &mut x);
        }
        qs.push(("item", x.iter().map(|&v| v as f32).collect()));
    }
    for i in 0..nq {
        qs.push(("text", texts[i * 9 % texts.len()].clone()));
    }
    // truth: one pass, exact scores of every row for every query
    let preps: Vec<Prep> = qs.iter().map(|q| prep(&ix, &q.1)).collect();
    let mut st0 = Stats::new(ix.meta.nl());
    for pr in &preps {
        for g in 0..ix.groups.len() {
            get_gq(&ix, pr, g, &mut st0);
        }
    }
    let tt = Timer::new();
    let nqq = qs.len();
    let scores: Vec<Vec<f64>> = {
        let per_cl: Vec<Vec<f64>> = (0..ix.k()).into_par_iter().map(|c| {
            let cl = &ix.cl[c];
            let g = cl.g;
            let mut z = vec![0f64; n];
            let mut out = vec![0f64; (cl.r1 - cl.r0) * nqq];
            let qms: Vec<f64> = preps.iter().map(|pr| dot64(&pr.q64, ix.centres_f64(c))).collect();
            if let Full::Synth(sy) = &ix.full {
                for r in cl.r0..cl.r1 {
                    sy.gen(c, g, sy.ext[r], &mut z);
                    for (qi, pr) in preps.iter().enumerate() {
                        out[(r - cl.r0) * nqq + qi] = qms[qi] + dot64(&pr.gq[g].get().unwrap().qv, &z);
                    }
                }
            }
            out
        }).collect();
        let mut sc = vec![vec![0f64; ix.nrows]; nqq];
        for (c, v) in per_cl.into_iter().enumerate() {
            let r0 = ix.cl[c].r0;
            for i in 0..v.len() / nqq {
                for qi in 0..nqq {
                    sc[qi][r0 + i] = v[i * nqq + qi];
                }
            }
        }
        sc
    };
    log(format!("truth pass over {} rows x {} queries: {:.0}s", ix.nrows, nqq, tt.s()));
    let sizes: Vec<usize> = [1e-4, 1e-3, 1e-2, 5e-2].iter().map(|f| (ix.nrows as f64 * f) as usize).collect();
    let mut lines = vec!["class,threads,what,answer_frac,median_ms,mean_ms,nq,touched_mean,full_reads_mean,fallback_ms_mean,result_mean,wrong".to_string()];
    for th in [1usize, 4] {
        let pl = rayon::ThreadPoolBuilder::new().num_threads(th).build().unwrap();
        for cls in ["item", "text"] {
            for (si, &k) in sizes.iter().enumerate() {
                let (mut ts, mut tch, mut fr, mut rs, mut wrong, mut fbt) = (Vec::new(), 0.0, 0.0, 0.0, 0u64, 0.0);
                let mut cnt = 0;
                for (qi, (c2, q)) in qs.iter().enumerate() {
                    if *c2 != cls {
                        continue;
                    }
                    let mut s: Vec<f64> = scores[qi].clone();
                    s.select_nth_unstable_by(k, |a, b| b.partial_cmp(a).unwrap());
                    let sk1 = s[k];
                    let sk = s[..k].iter().cloned().fold(f64::INFINITY, f64::min);
                    let thr = (sk + sk1) / 2.0;
                    let mut best = f64::MAX;
                    let mut last = None;
                    for _ in 0..2 {
                        let pr = prep(&ix, q);
                        let t0 = Timer::new();
                        let r = pl.install(|| {
                            let r = threshold_prepped(&ix, &pr, thr, &Opts { vis: None, verify: false, want_iv: false, rec: false });
                            let b = to_bitmap(&ix, &r.rows);
                            (r, b)
                        });
                        best = best.min(t0.ms());
                        last = Some(r);
                    }
                    let (r, b) = last.unwrap();
                    // fallback cost: regenerating the full vectors (not representative of a 1,536 B read)
                    let pr = prep(&ix, q);
                    let t1 = Timer::new();
                    let mut stx = Stats::new(0);
                    for i in 0..r.st.full_reads.min(2000) as usize {
                        std::hint::black_box(full_score(&ix, &pr, (i * 7) % ix.nrows, &mut stx));
                    }
                    let per = if r.st.full_reads > 0 { t1.ms() / r.st.full_reads.min(2000) as f64 } else { 0.0 };
                    fbt += per * r.st.full_reads as f64;
                    let truth: u64 = scores[qi].iter().filter(|&&v| v >= thr).count() as u64;
                    let inres: u64 = b.iter().filter(|&e| scores[qi][ix.ent_row[e as usize] as usize] >= thr).count() as u64;
                    wrong += (b.len() - inres) + (truth - inres);
                    ts.push(best);
                    tch += r.st.touched as f64;
                    fr += r.st.full_reads as f64;
                    rs += b.len() as f64;
                    cnt += 1;
                }
                let kq = cnt as f64;
                let line = format!("{},{},index_thr,{},{:.2},{:.2},{},{:.0},{:.1},{:.2},{:.0},{}", cls, th, [1e-4, 1e-3, 1e-2, 5e-2][si], median(&ts), mean(&ts), cnt, tch / kq, fr / kq, fbt / kq, rs / kq, wrong);
                log(format!("scale {line}"));
                lines.push(line);
            }
            // k-NN k = 10
            let (mut ts, mut bad) = (Vec::new(), 0);
            for (qi, (c2, q)) in qs.iter().enumerate() {
                if *c2 != cls {
                    continue;
                }
                let t0 = Timer::new();
                let r = pl.install(|| knn(&ix, q, 10, None));
                ts.push(t0.ms());
                let mut idx: Vec<usize> = (0..ix.nrows).collect();
                idx.select_nth_unstable_by(10, |a, b| scores[qi][*b].partial_cmp(&scores[qi][*a]).unwrap());
                let mut want: Vec<u32> = idx[..10].iter().map(|&r| ix.row_ent[r]).collect();
                let mut got: Vec<u32> = r.ents.iter().map(|x| x.1).collect();
                want.sort();
                got.sort();
                if want != got {
                    bad += 1;
                }
            }
            let line = format!("{},{},index_knn10,0,{:.2},{:.2},{},0,0,0,10,{}", cls, th, median(&ts), mean(&ts), ts.len(), bad);
            log(format!("scale {line}"));
            lines.push(line);
        }
    }
    std::fs::write(format!("{RES}/scale_{}m.csv", ix.nrows / 1_000_000), lines.join("\n") + "\n").unwrap();
    log(format!("scale run done in {:.0}s", t.s()));
}

/// Scale test of the fast-scan index: synthetic items defined in each group's int8 basis.
pub fn run2(scale: usize, nq: usize) {
    use crate::v2::q::*;
    use crate::v2::*;
    let t = Timer::new();
    log(format!("==== v2 scale run x{scale} at {:.0}", now_s()));
    let p = load_params(DATA);
    let n = p.meta.n;
    let k = p.meta.k;
    let prof = crate::derive::load_profile(&format!("{DATA}/prof_h0.25.npy"));
    let split = !std::env::args().any(|a| a == "--u8");
    // a real build supplies the int8 bases, residual norms and centres
    let real = {
        let src = Real2 { src: RealSource::open(&p.xs_path) };
        let f = RealSource::open(&p.xs_path);
        build2(&p, &prof, &src, Full2::Real { map: f.map, off: f.off, xs_row: Vec::new() }, 1, split, &mut |s| log(format!("[real] {s}")))
    };
    let vq: Vec<Vec<f64>> = real.groups.iter().map(|g| {
        let mut v = vec![0f64; n * n];
        for c in 0..n {
            for i in 0..n {
                v[i * n + c] = g.v8t[c * n + i] as f64 * g.vsc[c];
            }
        }
        v
    }).collect();
    let eg: Vec<f64> = vq.iter().map(|v| {
        let mut gm = vec![0f64; n * n];
        unsafe { matrixmultiply::dgemm(n, n, n, 1.0, v.as_ptr(), 1, n as isize, v.as_ptr(), n as isize, 1, 0.0, gm.as_mut_ptr(), n as isize, 1) };
        let mut s = 0.0;
        for i in 0..n {
            for c in 0..n {
                let e = gm[i * n + c] - if i == c { 1.0 } else { 0.0 };
                s += e * e;
            }
        }
        s.sqrt() * (1.0 + 1e-6) + 1e-12
    }).collect();
    log(format!("int8 bases: largest |V_q^T V_q - I|_F {:.3e}", eg.iter().cloned().fold(0.0, f64::max)));
    let sizes: Vec<usize> = (0..k).map(|c| (p.cstart[c + 1] - p.cstart[c]) * scale).collect();
    let mut coff = vec![0usize; k + 1];
    for c in 0..k {
        coff[c + 1] = coff[c] + sizes[c];
    }
    let mk = || Synth2 {
        sizes: sizes.clone(),
        rnorms: real.cl.iter().map(|c| (c.r0..c.r1).map(|r| {
            let a = real.ahat[r] as f64 / 32767.0;
            (1.0 + c.mn * c.mn - 2.0 * c.mn * a).max(1e-12).sqrt() as f32
        }).collect()).collect(),
        mv: (0..k).map(|c| {
            let g = &real.groups[p.grp[c]];
            let m = &p.centres[c * n..(c + 1) * n];
            (0..n).map(|cc| g.vsc[cc] * g.v8t[cc * n..(cc + 1) * n].iter().zip(m).map(|(&v, &x)| v as f64 * x as f64).sum::<f64>()).collect()
        }).collect(),
        mn: real.cl.iter().map(|c| c.mn).collect(),
        lamsc: (0..p.meta.g).map(|g| {
            let l = &p.lam[g * n..(g + 1) * n];
            let s: f64 = l.iter().sum();
            l.iter().map(|&x| (x / s).sqrt()).collect()
        }).collect(),
        eg: eg.clone(),
        ext: Vec::new(),
        row_cl: Vec::new(),
        n,
        coff: coff.clone(),
        grp: p.grp.clone(),
        vq: vq.clone(),
    };
    let sy_src = mk();
    let texts: Vec<Vec<f32>> = p.text_q.chunks(n).map(|v| {
        let nn = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / nn).collect()
    }).collect();
    let real_mhat: Vec<Vec<f64>> = real.cl.iter().map(|c| c.mhat.clone()).collect();
    let sy_full = mk();
    drop(mk);
    drop(real);
    let mut ix = build2(&p, &prof, &SynthSrc { sy: &sy_src }, Full2::Synth(sy_full), 1, split, &mut |s| log(format!("[synth2] {s}")));
    // support table for synthetic items: p.x = p.m + (V_q^T p).z
    let nanch = 1000.min(k);
    let mut anchors = vec![0f64; nanch * n];
    for a in 0..nanch {
        let c = (a as f64 * k as f64 / nanch as f64) as usize;
        anchors[a * n..(a + 1) * n].copy_from_slice(&real_mhat[c]);
    }
    let pv: Vec<Vec<f64>> = vq.par_iter().map(|v| {
        let mut out = vec![0f64; nanch * n];
        unsafe { matrixmultiply::dgemm(nanch, n, n, 1.0, anchors.as_ptr(), n as isize, 1, v.as_ptr(), n as isize, 1, 0.0, out.as_mut_ptr(), n as isize, 1) };
        out
    }).collect();
    let cols: Vec<(Vec<f32>, Vec<f32>)> = (0..k).into_par_iter().map(|c| {
        let g = p.grp[c];
        let it = SynthSrc { sy: &sy_src }.items(c, &p, &ix.groups[g], &[]);
        let nc = it.ah.len();
        let mut px = vec![0f64; nc * nanch];
        unsafe { matrixmultiply::dgemm(nc, n, nanch, 1.0, it.z.as_ptr(), n as isize, 1, pv[g].as_ptr(), 1, n as isize, 0.0, px.as_mut_ptr(), nanch as isize, 1) };
        let m = ix.centres_f64(c);
        let (mut hi, mut lo) = (vec![0f32; nanch], vec![0f32; nanch]);
        for a in 0..nanch {
            let pm = dot64(&anchors[a * n..(a + 1) * n], m);
            let (mut mx, mut mi) = (f64::NEG_INFINITY, f64::INFINITY);
            for i in 0..nc {
                mx = mx.max(px[i * nanch + a]);
                mi = mi.min(px[i * nanch + a]);
            }
            hi[a] = ((((pm + mx) + 1e-9) * 32768.0).ceil() / 32768.0) as f32;
            lo[a] = ((((pm + mi) - 1e-9) * 32768.0).floor() / 32768.0) as f32;
        }
        (hi, lo)
    }).collect();
    ix.hi = vec![0f32; nanch * k];
    ix.lo = vec![0f32; nanch * k];
    for c in 0..k {
        for a in 0..nanch {
            ix.hi[a * k + c] = cols[c].0[a];
            ix.lo[a * k + c] = cols[c].1[a];
        }
    }
    ix.anchors = anchors;
    ix.nanch = nanch;
    log(format!("synthetic v2 index: {} rows built in {:.0}s; x2 bound {:.3e}; stored B/item {:?}", ix.nrows, t.s(), ix.x2 - 1.0, ix.stored_bytes_per_item().iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>()));
    let mut qs: Vec<(&str, Vec<f32>)> = Vec::new();
    for i in 0..nq {
        let row = (i * 7919 * 104729) % ix.nrows;
        let mut x = vec![0f64; n];
        if let Full2::Synth(sy) = &ix.full {
            sy.x_f64(&ix, row, &mut x);
        }
        qs.push(("item", x.iter().map(|&v| v as f32).collect()));
    }
    for i in 0..nq {
        qs.push(("text", texts[i * 9 % texts.len()].clone()));
    }
    let preps: Vec<Prep2> = qs.iter().map(|q| prep2(&ix, &q.1)).collect();
    let mut st0 = Stats::new(ix.meta.nl());
    for pr in &preps {
        for g in 0..ix.groups.len() {
            get_gq2(&ix, pr, g, &mut st0);
        }
    }
    let tt = Timer::new();
    let nqq = qs.len();
    let scores: Vec<Vec<f64>> = {
        let per_cl: Vec<Vec<f64>> = (0..k).into_par_iter().map(|c| {
            let cl = &ix.cl[c];
            let g = cl.g;
            let mut z = vec![0f64; n];
            let mut out = vec![0f64; (cl.r1 - cl.r0) * nqq];
            let qms: Vec<f64> = preps.iter().map(|pr| dot64(&pr.q64, ix.centres_f64(c))).collect();
            if let Full2::Synth(sy) = &ix.full {
                for r in cl.r0..cl.r1 {
                    sy.gen(c, sy.ext[r], &mut z);
                    for (qi, pr) in preps.iter().enumerate() {
                        out[(r - cl.r0) * nqq + qi] = qms[qi] + dot64(&pr.gq[g].get().unwrap().qv, &z);
                    }
                }
            }
            out
        }).collect();
        let mut sc = vec![vec![0f64; ix.nrows]; nqq];
        for (c, v) in per_cl.into_iter().enumerate() {
            let r0 = ix.cl[c].r0;
            for i in 0..v.len() / nqq {
                for qi in 0..nqq {
                    sc[qi][r0 + i] = v[i * nqq + qi];
                }
            }
        }
        sc
    };
    log(format!("truth pass over {} rows x {} queries: {:.0}s", ix.nrows, nqq, tt.s()));
    let szs: Vec<usize> = [1e-4, 1e-3, 1e-2, 5e-2].iter().map(|f| (ix.nrows as f64 * f) as usize).collect();
    let mut lines = vec!["class,threads,what,answer_frac,median_ms,mean_ms,nq,touched_mean,full_reads_mean,fallback_ms_mean,result_mean,wrong".to_string()];
    let plan = Plan::default();
    for th in [1usize, 4] {
        let pl = rayon::ThreadPoolBuilder::new().num_threads(th).build().unwrap();
        for cls in ["item", "text"] {
            for (si, &kk) in szs.iter().enumerate() {
                let (mut ts, mut tch, mut fr, mut rs, mut wrong, mut fbt) = (Vec::new(), 0.0, 0.0, 0.0, 0u64, 0.0);
                let mut cnt = 0;
                for (qi, (c2, q)) in qs.iter().enumerate() {
                    if *c2 != cls {
                        continue;
                    }
                    let mut s: Vec<f64> = scores[qi].clone();
                    s.select_nth_unstable_by(kk, |a, b| b.partial_cmp(a).unwrap());
                    let sk1 = s[kk];
                    let sk = s[..kk].iter().cloned().fold(f64::INFINITY, f64::min);
                    let thr = (sk + sk1) / 2.0;
                    let mut best = f64::MAX;
                    let mut last = None;
                    for _ in 0..2 {
                        let t0 = Timer::new();
                        let r = pl.install(|| {
                            let pr = prep2(&ix, q);
                            let r = threshold2(&ix, &pr, thr, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                            let b = to_bitmap2(&ix, &r.rows);
                            (r, b)
                        });
                        best = best.min(t0.ms());
                        last = Some(r);
                    }
                    let (r, b) = last.unwrap();
                    let pr = prep2(&ix, q);
                    let t1 = Timer::new();
                    for i in 0..r.st.full_reads.min(2000) as usize {
                        std::hint::black_box(exact_value2(&ix, &pr, (i * 7) % ix.nrows));
                    }
                    let per = if r.st.full_reads > 0 { t1.ms() / r.st.full_reads.min(2000) as f64 } else { 0.0 };
                    fbt += per * r.st.full_reads as f64;
                    let truth: u64 = scores[qi].iter().filter(|&&v| v >= thr).count() as u64;
                    let inres: u64 = b.iter().filter(|&e| scores[qi][ix.ent_row[e as usize] as usize] >= thr).count() as u64;
                    wrong += (b.len() - inres) + (truth - inres);
                    ts.push(best);
                    tch += r.st.touched as f64;
                    fr += r.st.full_reads as f64;
                    rs += b.len() as f64;
                    cnt += 1;
                }
                let kq = cnt as f64;
                let line = format!("{},{},index2_thr,{},{:.2},{:.2},{},{:.0},{:.1},{:.2},{:.0},{}", cls, th, [1e-4, 1e-3, 1e-2, 5e-2][si], median(&ts), mean(&ts), cnt, tch / kq, fr / kq, fbt / kq, rs / kq, wrong);
                log(format!("scale2 {line}"));
                lines.push(line);
            }
            let (mut ts, mut bad) = (Vec::new(), 0);
            for (qi, (c2, q)) in qs.iter().enumerate() {
                if *c2 != cls {
                    continue;
                }
                let t0 = Timer::new();
                let r = pl.install(|| knn2(&ix, q, 10, None, plan));
                ts.push(t0.ms());
                let mut idx: Vec<usize> = (0..ix.nrows).collect();
                idx.select_nth_unstable_by(10, |a, b| scores[qi][*b].partial_cmp(&scores[qi][*a]).unwrap());
                let mut want: Vec<u32> = idx[..10].iter().map(|&r| ix.row_ent[r]).collect();
                let mut got: Vec<u32> = r.ents.iter().map(|x| x.1).collect();
                want.sort();
                got.sort();
                if want != got {
                    bad += 1;
                }
            }
            let line = format!("{},{},index2_knn10,0,{:.2},{:.2},{},0,0,0,10,{}", cls, th, median(&ts), mean(&ts), ts.len(), bad);
            log(format!("scale2 {line}"));
            lines.push(line);
        }
    }
    std::fs::write(format!("{RES}/scale2_{}m.csv", ix.nrows / 1_000_000), lines.join("\n") + "\n").unwrap();
    log(format!("v2 scale run done in {:.0}s", t.s()));
}
