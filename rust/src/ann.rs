//! ANN-Benchmarks runs on the fast-scan index: k-NN of every query, exact and approximate, unfiltered and restricted
//! to visible sets, one query at a time on one thread. Writes the returned entity ids (for recall, bench/score.py) and
//! one CSV row per configuration with timings and exactness against brute force over the stored values.
//!
//! `ann-truth`: brute force over the stored values for every query and filter (all threads), saved beside the results.
//! `ann`: the timed runs (one thread, pinned to `--core N` when given). Filters are `--filters a.npy,b.npy` (sorted
//! entity ids); the unfiltered run is tagged `all`. The approximate grid is `--approx c:r:l,...` (clusters probed,
//! re-ranked items, level of the interval scores); `--flat` adds brute force over the same stored values and visible
//! rows on the same thread.
//! `--nq N` times the first N queries, `--methods exact,flat,approx` picks the methods, `--no-all` skips the unfiltered run, `--prof` prints the mean ms per phase.
//! `--dial np:d,...` adds the approximate mode with the dial (method `approx`, unfiltered unless `--approx-filtered`): exact k-NN over the first np clusters by
//! the fitted bound, each level's item half-widths cut to its (1 - d) error quantile (`fitT`: the d fitted for target
//! recall T; needs `--fit-dial` unless a fit is named); `--dial-plan` its plan. `np:d:soar` adds the second
//! assignment (needs `--soar`).
use crate::query::Flat;
use crate::run2::*;
use crate::util::*;
use crate::v2::q::*;
use rayon::prelude::*;
use roaring::RoaringBitmap;
use std::io::Write;

fn filters() -> Vec<(String, Option<RoaringBitmap>)> {
    let mut out = if args().iter().any(|a| a == "--no-all") { Vec::new() } else { vec![("all".to_string(), None)] };
    for f in arg_str("--filters", "").split(',').filter(|s| !s.is_empty()) {
        let ids: Vec<u32> = read_npy(f).i32s().iter().map(|&x| x as u32).collect();
        let tag = std::path::Path::new(f).file_stem().unwrap().to_string_lossy().to_string();
        out.push((tag, Some(RoaringBitmap::from_sorted_iter(ids).expect("filter ids must be sorted and distinct"))));
    }
    out
}

fn ids_bytes(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// Brute force over the stored values: the k best entity ids and exact scores of every query, per filter.
pub fn truth(c: &Ctx2, k: usize) {
    let fl: Flat = c.flat();
    let dir = res_dir();
    for (tag, bm) in filters() {
        let t = Timer::new();
        let res: Vec<Vec<(f64, u32)>> = c.texts.par_iter().map(|q| {
            let pr = prep2(&c.ix, q);
            fl.knn(&pr.q16, &pr.q32, k, full_err2(&c.ix, &pr), bm.as_ref(), &c.ent_xs)
        }).collect();
        let mut ids = vec![-1i32; res.len() * k];
        let mut sc = vec![f64::NEG_INFINITY; res.len() * k];
        for (i, r) in res.iter().enumerate() {
            for (j, &(s, e)) in r.iter().enumerate() {
                ids[i * k + j] = e as i32;
                sc[i * k + j] = s;
            }
        }
        write_npy(&format!("{dir}/truth_{tag}.npy"), "<i4", &[res.len(), k], &ids_bytes(&ids));
        write_npy(&format!("{dir}/truth_{tag}_s.npy"), "<f8", &[res.len(), k], &sc.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>());
        println!("truth {tag}: {} queries in {:.1}s", res.len(), t.s());
    }
}

struct Run {
    ids: Vec<i32>,
    times: Vec<f64>,
    touched: f64,
    full_reads: f64,
    bytes: f64,
    phases: [f64; 3],  // seed, sweep, selection (ms, summed)
    cyc: [f64; 12],    // time-stamp-counter cycles by phase, summed (with --prof)
}

/// Every query once per repetition, timed one at a time on one thread (pinned to `--core` when given); the
/// repetition with the least total time is kept.
fn timed(c: &Ctx2, k: usize, reps: usize, nq: usize, f: &(dyn Fn(&[f32]) -> KnnOut2 + Sync)) -> Run {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let mut best: Option<Run> = None;
    for _ in 0..reps {
        let mut r = Run { ids: vec![-1; nq * k], times: Vec::with_capacity(nq), touched: 0.0, full_reads: 0.0, bytes: 0.0, phases: [0.0; 3], cyc: [0.0; 12] };
        pool.install(|| {
            if let Some(cpu) = arg_str("--core", "").parse().ok() {
                crate::util::pin(cpu);
            }
            for (i, q) in c.texts[..nq].iter().enumerate() {
                let t = Timer::new();
                let o = f(q);
                r.times.push(t.ms());
                for (j, &(_, e)) in o.ents.iter().enumerate() {
                    r.ids[i * k + j] = e as i32;
                }
                r.touched += o.st.touched as f64;
                r.full_reads += o.st.full_reads as f64;
                r.bytes += bytes_read2(&c.ix, &o.st) * c.ix.nrows as f64;
                r.phases[0] += o.t_seed;
                r.phases[1] += o.t_sweep;
                r.phases[2] += o.t_sel;
                for (a, &b) in r.cyc.iter_mut().zip(&o.st.cyc) {
                    *a += b as f64;
                }
            }
        });
        let tot: f64 = r.times.iter().sum();
        if best.as_ref().map_or(true, |b| tot < b.times.iter().sum::<f64>()) {
            best = Some(r);
        }
    }
    best.unwrap()
}

/// Queries whose returned set differs from brute force, and those whose exact scores differ (a set can differ on ties
/// alone).
fn exactness(c: &Ctx2, k: usize, ids: &[i32], tag: &str) -> (i64, i64) {
    let dir = res_dir();
    let p = format!("{dir}/truth_{tag}.npy");
    if !std::path::Path::new(&p).exists() {
        return (-1, -1);
    }
    let tid = read_npy(&p).i32s();
    let tsc = read_npy(&format!("{dir}/truth_{tag}_s.npy")).f64s();
    let (mut wset, mut wsc) = (0, 0);
    for (i, q) in c.texts.iter().take(ids.len() / k).enumerate() {
        let mut a: Vec<i32> = ids[i * k..(i + 1) * k].to_vec();
        let mut b: Vec<i32> = tid[i * k..(i + 1) * k].to_vec();
        a.sort_unstable();
        b.sort_unstable();
        if a == b {
            continue;
        }
        wset += 1;
        let pr = prep2(&c.ix, q);
        let mut s: Vec<f64> = ids[i * k..(i + 1) * k].iter().filter(|&&e| e >= 0).map(|&e| exact_value2(&c.ix, &pr, c.ix.row_of_ent(e as u32))).collect();
        let mut t: Vec<f64> = tsc[i * k..(i + 1) * k].iter().copied().filter(|x| x.is_finite()).collect();
        s.sort_by(|x, y| y.partial_cmp(x).unwrap());
        t.sort_by(|x, y| y.partial_cmp(x).unwrap());
        if s != t {
            wsc += 1;
        }
    }
    (wset, wsc)
}

pub fn run(c: &Ctx2, k: usize) {
    let dir = res_dir();
    let prof = args().iter().any(|a| a == "--prof");
    let hz = if prof {
        PROF.store(true, std::sync::atomic::Ordering::Relaxed);
        let (t, c0) = (Timer::new(), unsafe { core::arch::x86_64::_rdtsc() });
        while t.ms() < 200.0 {}
        (unsafe { core::arch::x86_64::_rdtsc() } - c0) as f64 / t.s()
    } else {
        1.0
    };
    let name = arg_str("--name", "data");
    let reps = arg_usize("--reps", 2);
    let approx: Vec<(usize, usize, usize)> = arg_str("--approx", "").split(',').filter(|s| !s.is_empty()).map(|s| {
        let v: Vec<usize> = s.split(':').map(|x| x.parse().unwrap()).collect();
        (v[0], v[1], v[2])
    }).collect();
    let csv = format!("{dir}/ours.csv");
    let fresh = !std::path::Path::new(&csv).exists();
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&csv).unwrap();
    if fresh {
        writeln!(out, "dataset,method,filter,params,nq,k,reps,qps,mean_ms,p50_ms,p99_ms,wrong_sets,wrong_scores,touched_mean,full_reads_mean,bytes_read_mean,ids,plan").unwrap();
    }
    let nq = arg_usize("--nq", c.texts.len()).min(c.texts.len());
    let methods = arg_str("--methods", "exact,flat,approx");
    let want = |m: &str| methods.split(',').any(|x| x == m);
    for (tag, bm) in filters() {
        let vis = bm.as_ref().map(|b| Vis2::new(&c.ix, b));
        let mut cfgs: Vec<(String, String, Box<dyn Fn(&[f32]) -> KnnOut2 + Sync + '_>)> = Vec::new();
        if want("exact") {
            cfgs.push(("exact".into(), "-".into(), Box::new(|q: &[f32]| knn2(&c.ix, q, k, vis.as_ref(), c.plan))));
        }
        if want("flat") && args().iter().any(|a| a == "--flat") {
            // brute force over the same stored vectors and visible rows as the index path, without the planner
            cfgs.push(("flat".into(), "-".into(), Box::new(|q: &[f32]| {
                let (q16, q32, qn) = query_f16(q);
                let ents = scan_knn(&c.ix, &q16, &q32, qn, k, vis.as_ref());
                let mut st = crate::kernel::Stats::new(c.ix.meta.nl());
                st.full_reads = vis.as_ref().map_or(c.ix.nrows, |v| v.rows.len()) as u64;
                KnnOut2 { ents, st, cands: 0, t_seed: 0.0, t_sweep: 0.0, t_sel: 0.0, npow: c.ix.meta.npow }
            })));
        }
        if want("approx") && (bm.is_none() || args().iter().any(|a| a == "--approx-filtered")) {
            // --order centre: clusters by centre direction instead of the fitted bound
            let ap = if arg_str("--order", "bound") == "centre" { None } else { Some(&c.approx) };
            for &(np, rr, lv) in &approx {
                let v = vis.as_ref();
                cfgs.push(("approx".into(), format!("nprobe={np} rerank={rr} level={lv}"), Box::new(move |q: &[f32]| knn2_approx(&c.ix, ap, q, k, v, np, rr, lv))));
            }
        }
        // approximate mode with the dial: --dial np:d,... where d is eps (the dial's miss rate per level) or fitT (the
        // eps fitted for target recall T)
        let dial_here = bm.is_none() || args().iter().any(|a| a == "--approx-filtered");
        for spec in arg_str("--dial", "").split(',').filter(|s| !s.is_empty() && dial_here) {
            let (spec, soar) = match spec.strip_suffix(":soar") {
                Some(s) => (s, true),
                None => (spec, false),
            };
            let (np, d) = spec.split_once(':').unwrap();
            let np: usize = np.parse().unwrap();
            let dl = c.dial.as_ref();
            let factors: Option<Vec<f32>> = match d.strip_prefix("fit") {
                Some(t) => Some(dl.expect("--fit-dial").for_target(t.parse().unwrap()).to_vec()),
                None if d == "0" => None,
                None => {
                    let e: f64 = d.parse().unwrap();
                    let dl = dl.expect("--fit-dial");
                    Some(dl.factors[dl.eps.iter().position(|&x| (x - e).abs() < 1e-12).expect("eps not on the grid")].clone())
                }
            };
            let (v, plan) = (vis.as_ref(), Plan { mode: match arg_str("--dial-plan", "auto").as_str() { "index" => PlanMode::Index, "flat" => PlanMode::Flat, _ => PlanMode::Auto }, ..c.plan });
            cfgs.push(("approx".into(), format!("nprobe={np} eps={d}{}", if soar { " soar" } else { "" }), Box::new(move |q: &[f32]| knn2_dial(&c.ix, &c.approx, q, k, v, np, factors.as_deref(), soar, plan))));
        }
        for (method, params, f) in &cfgs {
            let r = timed(c, k, reps, nq, f.as_ref());
            let (wset, wsc) = if method == "exact" || method == "flat" { exactness(c, k, &r.ids, &tag) } else { (-1, -1) };
            let fname = format!("{method}_{tag}_{}.npy", params.replace([' ', '='], "_").replace("-", "x"));
            write_npy(&format!("{dir}/{fname}"), "<i4", &[nq, k], &ids_bytes(&r.ids));
            let tot: f64 = r.times.iter().sum();
            let mut ts = r.times.clone();
            let line = format!("{name},{method},{tag},{params},{nq},{k},{reps},{:.1},{:.4},{:.4},{:.4},{wset},{wsc},{:.0},{:.1},{:.0},{fname},{}",
                nq as f64 / (tot / 1e3), tot / nq as f64, quantile(&mut ts, 0.5), quantile(&mut ts, 0.99), r.touched / nq as f64, r.full_reads / nq as f64, r.bytes / nq as f64, c.plan.to_kv().trim().replace('\n', ";"));
            println!("ann {line}");
            if prof {
                let names = ["anchor", "cut", "basis", "rot", "clsetup", "tables", "repack", "decode", "bounds", "full", "init", "plan"];
                let ph: Vec<String> = r.phases.iter().map(|x| format!("{:.3}", x / nq as f64)).collect();
                let cy: Vec<String> = names.iter().zip(&r.cyc).filter(|x| *x.1 > 0.0).map(|(n, x)| format!("{n} {:.3}", x / hz * 1e3 / nq as f64)).collect();
                println!("prof seed/sweep/sel ms {} | {}", ph.join(" "), cy.join(", "));
            }
            writeln!(out, "{line}").unwrap();
            out.flush().unwrap();
        }
    }
}
