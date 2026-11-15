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

const SIZES: [f64; 7] = [1e-4, 1e-3, 1e-2, 0.1, 0.3, 0.5, 0.75];

static LOG: std::sync::OnceLock<std::sync::Mutex<std::fs::File>> = std::sync::OnceLock::new();
macro_rules! out {
    ($($a:tt)*) => {{ let s = format!($($a)*); println!("{}", s); if let Some(l) = LOG.get() { writeln!(l.lock().unwrap(), "{}", s).unwrap(); } }};
}

/// A pool of t threads; with `--pin C`, thread i runs on CPU C + i.
fn pool(t: usize) -> rayon::ThreadPool {
    let pin0: Option<usize> = arg_str("--pin", "").parse().ok();
    rayon::ThreadPoolBuilder::new().num_threads(t).start_handler(move |i| if let Some(c) = pin0 { pin(c + i) }).build().unwrap()
}

pub struct Ctx2 {
    pub p: Params,
    pub ix: Index2,
    pub xs: RealSource,
    pub ent_xs: Vec<u32>,
    pub items: Vec<Vec<f32>>,
    pub texts: Vec<Vec<f32>>,
    pub plan: Plan,
    pub approx: Approx,
    pub pseudo: Vec<(usize, Vec<f32>, Vec<u32>)>,
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

/// The index directory written by tools/fit.py (`--index DIR`).
pub fn data_dir() -> String {
    let a = args();
    match a.iter().position(|x| x == "--index") {
        Some(i) => a[i + 1].clone(),
        None => {
            eprintln!("no index: write --index DIR, a directory made by tools/fit.py");
            std::process::exit(2);
        }
    }
}

/// Where results go (`--results DIR`, default DIR/results of the index).
pub fn res_dir() -> String {
    let d = arg_str("--results", &format!("{}/results", data_dir()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Queries from `--queries FILE` (npy, rows of the original dimension; zero-padded and normalised), else none.
pub fn load_queries(n: usize) -> Vec<Vec<f32>> {
    let path = arg_str("--queries", "");
    if path.is_empty() {
        return Vec::new();
    }
    load_queries_from(&path, n)
}

/// Rows of an npy file (f16, f32 or f64; original dimension), zero-padded to n and normalised.
pub fn load_queries_from(path: &str, n: usize) -> Vec<Vec<f32>> {
    let a = read_npy(path);
    let n0 = a.shape[1];
    let v: Vec<f32> = if a.descr.contains("f8") {
        a.f64s().iter().map(|&x| x as f32).collect()
    } else if a.descr.contains("f2") {
        a.u16s().iter().map(|&h| f16_to_f32(h)).collect()
    } else {
        a.f32s()
    };
    v.chunks(n0).map(|r| {
        let nn = r.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
        let mut q = vec![0f32; n];
        if nn > 0.0 {
            for (o, &x) in q.iter_mut().zip(r) {
                *o = (x as f64 / nn) as f32;
            }
        }
        q
    }).collect()
}

pub fn setup2(tag: &str) -> Ctx2 {
    setup2_at(tag, &data_dir(), &arg_str("--flags", ""))
}

/// The plan's switches from the command line, then from `flags` (comma-separated: int, noint, gtab, nogtab, ...).
pub fn plan_flags(mut p: Plan, flags: &str) -> Plan {
    for f in flags.split(',').map(|f| f.trim()).filter(|f| !f.is_empty()) {
        match f {
            "int" => p.int_det = true,
            "noint" => p.int_det = false,
            "gtab" => p.gtab = true,
            "nogtab" => p.gtab = false,
            "repack" => p.repack = true,
            "norepack" => p.repack = false,
            "pscan" => p.pscan = true,
            "nopscan" => p.pscan = false,
            "direct" => p.direct = true,
            "nodirect" => p.direct = false,
            "nusort" => {} // a build option, read by setup2_at
            "noscanband" => {
                p.scan_lo = 2.0;
                p.scan_hi = 2.0;
            }
            "scanband=auto" => p.scan_cal = true,
            f if f.starts_with("scanband=") => {
                let (a, b) = f[9..].split_once(':').expect("scanband=LO:HI");
                p.scan_lo = a.parse().expect("scanband=LO:HI");
                p.scan_hi = b.parse().expect("scanband=LO:HI");
            }
            f if f.starts_with("tol=") => p.tol = f[4..].parse().expect("tol=SCORE (the tolerance d, in score units)"),
            f if f.starts_with("repackx=") => {
                p.repack = true;
                p.repack_mul = f[8..].parse().expect("repackx=FACTOR");
            }
            _ => panic!("unknown plan flag {f:?}"),
        }
    }
    p
}

/// The index of `dir` with the plan of the command line changed by `flags` (for a second variant, `--b`).
pub fn setup2_at(tag: &str, dir: &str, flags: &str) -> Ctx2 {
    let log = std::fs::OpenOptions::new().create(true).append(true).open(format!("{}/{tag}.log", crate::run2::res_dir())).unwrap();
    let _ = LOG.set(std::sync::Mutex::new(log));
    out!("==== run at {:.0}: {:?}", now_s(), args());
    let profile = (dir == data_dir()).then(|| arg_str("--profile", "")).filter(|v| !v.is_empty());
    let nguar = Some(arg_str("--nguar", "")).filter(|v| !v.is_empty()).map(|v| v.parse::<f64>().expect("--nguar N (items the guarantee covers)"));
    let rlv = Some(arg_str("--rlv", "")).filter(|v| !v.is_empty()).map(|v| v.split(',').map(|x| x.parse().expect("--rlv BYTES,BYTES,...")).collect());
    let bo = BuildOptions {
        profile,
        clusters: Some(arg_usize("--clusters", 100000)),
        rlv,
        design_exact: args().iter().any(|a| a == "--design-exact"),
        nusort: flags.split(',').any(|f| f == "nusort") || (dir == data_dir() && args().iter().any(|a| a == "--nusort")),
        nguar,
        csg_plain: args().iter().any(|a| a == "--csg-plain"),
        ..BuildOptions::default()
    };
    let prof_path = bo.profile.clone().unwrap_or_else(|| format!("{dir}/profile.npy"));
    let topen = Timer::new();
    let (mut p, mut ix) = open(dir, &bo, &mut |s| out!("{s}"));
    let built_s = topen.s();
    let xs = RealSource::open(&p.xs_path);
    let n = p.meta.n;
    let mut ent_xs = vec![u32::MAX; p.perm.len()];
    for (r, &e) in p.perm.iter().enumerate() {
        ent_xs[e as usize] = r as u32;
    }
    // item queries: held-out rows from --item-queries, else stored rows listed by the fit
    let items: Vec<Vec<f32>> = if arg_str("--item-queries", "").is_empty() {
        p.qi.iter().filter(|&&r| (r as usize) < p.cstart[p.meta.k]).map(|&r| xs.row(r as usize).iter().map(|&h| f16_to_f32(h)).collect()).collect()
    } else {
        load_queries_from(&arg_str("--item-queries", ""), n)
    };
    let texts = load_queries(n);
    let plan = Plan {
        mode: match arg_str("--plan", "auto").as_str() { "index" => PlanMode::Index, "flat" => PlanMode::Flat, _ => PlanMode::Auto },
        ..Plan::default()
    };
    let plan = plan_flags(plan, flags);
    // the cost model: from --plan-file, the BioCLIP constants (--plan-default), or re-timed on this index (pinned to
    // --core when given); the constants used are written to RESULTS/plan.txt
    let plan = if let Some(f) = Some(arg_str("--plan-file", "")).filter(|f| !f.is_empty()) {
        Plan::from_kv(plan, &std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("cannot read {f}: {e}")))
    } else if args().iter().any(|a| a == "--plan-default") {
        plan
    } else {
        let t = Timer::new();
        let pc = Plan::calibrate(&ix, plan, arg_usize("--calib", 16), arg_str("--core", "").parse().ok());
        out!("[plan] calibrated in {:.1}s", t.s());
        pc
    };
    let plan = plan_flags(plan, flags); // flags win over timed or filed constants (scanband)
    out!("[plan] {}", plan.to_kv().replace('\n', " "));
    std::fs::write(format!("{}/plan.txt", res_dir()), plan.to_kv()).unwrap();
    let t = Timer::new();
    let pseudo = pseudo_queries(&ix, if args().iter().any(|a| a == "--no-approx") { 0 } else { arg_usize("--pseudo", 1000) }, 10);
    let (approx, fres) = if pseudo.is_empty() { (Approx { f: 0.0, rad: Vec::new() }, Vec::new()) } else { Approx::fit(&ix, &pseudo) };
    out!("[approx] f = {} from {} pseudo-queries in {:.1}s; mean items scanned at recall 0.95 by f: {:?}", approx.f, pseudo.len(), t.s(), fres.iter().map(|x| (x.0, x.1.round())).collect::<Vec<_>>());
    // full vectors from a file (`--full-file PATH`, mapped with `--full-advice`), for runs under a memory cap
    let ff = arg_str("--full-file", "");
    if !ff.is_empty() {
        let t = Timer::new();
        if let Full2::Real { rows, .. } = &mut ix.full {
            std::sync::Arc::get_mut(rows).expect("the full vectors are shared before --full-file").to_file(&ff, &arg_str("--full-advice", "random"));
        }
        let pol = match &ix.full {
            Full2::Real { rows, .. } => rows.policy().expect("the full vectors were just written to a file"),
            _ => panic!("--full-file needs stored vectors; a synthetic index has none"),
        };
        pol.willneed.store(args().iter().any(|a| a == "--willneed"), std::sync::atomic::Ordering::Relaxed);
        // `--full-read uring` (the default) reads the rows the index path leaves with pread through io_uring; `mmap`
        // reads them through the mapping
        let fr = arg_str("--full-read", "uring");
        assert!(fr == "uring" || fr == "mmap", "--full-read is uring or mmap");
        pol.uring.store(fr == "uring", std::sync::atomic::Ordering::Relaxed);
        // reads through the reader are priced by the planner
        pol.priced.store(fr == "uring", std::sync::atomic::Ordering::Relaxed);
        // extents of at least --direct-min bytes are read with O_DIRECT, past the page cache (0: every read; none:
        // never)
        let dm = arg_str("--direct-min", "8192");
        pol.direct_min.store(if dm == "none" { usize::MAX } else { dm.parse().expect("--direct-min is a byte count or none") }, std::sync::atomic::Ordering::Relaxed);
        // extents of at least --dontcache-min bytes and below --direct-min are buffered reads that do not stay cached
        let dc = arg_str("--dontcache-min", "none");
        pol.dontcache_min.store(if dc == "none" { usize::MAX } else { dc.parse().expect("--dontcache-min is a byte count or none") }, std::sync::atomic::Ordering::Relaxed);
        out!("[disk] full vectors written to {ff} and mapped ({}, willneed {}, read {fr}) in {:.1}s", arg_str("--full-advice", "random"), args().iter().any(|a| a == "--willneed"), t.s());
    }
    if !ff.is_empty() {
        p.v = Vec::new(); // the f32 bases are needed only by the build
    }
    let c = Ctx2 { p, ix, xs, ent_xs, items, texts, plan, approx, pseudo };
    if !ff.is_empty() {
        out!("[mem] {}", c.mem_report());
    }
    out!("[fit] {}", c.p.fit);
    out!("built v2 (index format {}, profile {}) {} rows, {} clusters in {:.1}s; stored bytes/item cumulative by level: {:?}", INDEX_FORMAT, prof_path, c.ix.nrows, c.ix.k(), built_s,
        c.ix.stored_bytes_per_item().iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>());
    c
}

impl Ctx2 {
    /// MiB held by the large parts of the parameters and the index.
    pub fn mem_report(&self) -> String {
        fn b<T>(v: &[T]) -> usize {
            std::mem::size_of_val(v)
        }
        let (p, ix) = (&self.p, &self.ix);
        let mut parts: Vec<(&str, usize)> = vec![
            ("p.v", b(&p.v)), ("p.centres", b(&p.centres)), ("p.perm", b(&p.perm)),
            ("lcode", ix.lcode.iter().map(|x| b(x)).sum()), ("lscal", ix.lscal.iter().map(|x| b(x)).sum()),
            ("coff+soff", ix.coff.iter().chain(&ix.soff).map(|x| b(x)).sum()),
            ("groups.v8t", ix.groups.iter().map(|g| b(&g.v8t)).sum()), ("groups.vinv+vq", ix.groups.iter().map(|g| b(&g.vinv) + b(&g.vq)).sum()),
            ("groups.tables", ix.groups.iter().map(|g| b(&g.ytab) + b(&g.ktab) + b(&g.ttab) + b(&g.khl)).sum()),
            ("ctab", ix.ctab.iter().flatten().map(|t| b(&t.th) + b(&t.tmax) + b(&t.step) + b(&t.off) + b(&t.err)).sum()),
            ("cl", ix.cl.iter().map(|c| b(&c.mhat) + b(&c.mv32) + b(&c.wmr)).sum()),
            ("centres64", b(&ix.centres64)), ("rots", ix.rots.iter().map(|x| b(x)).sum()), ("anchors+ahat", b(&ix.anchors) + b(&ix.ahat)),
            ("hi+lo", b(&ix.hi) + b(&ix.lo)), ("row_ent+ent_row", b(&ix.row_ent) + b(&ix.ent_row)), ("esc", ix.esc.len() * 24),
            ("nukey", b(&ix.nukey)), ("ent_xs", b(&self.ent_xs)),
            ("full rows in memory", match &ix.full { Full2::Real { rows, .. } => match &**rows { crate::v2::disk::Rows::Mem(v) => b(v), _ => 0 }, _ => 0 }),
        ];
        parts.sort_by_key(|x| std::cmp::Reverse(x.1));
        let tot: usize = parts.iter().map(|x| x.1).sum();
        format!("total {:.1}: {}", tot as f64 / 1048576.0, parts.iter().map(|(k, v)| format!("{k} {:.1}", *v as f64 / 1048576.0)).collect::<Vec<_>>().join(", "))
    }
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

/// The item scale against f64: for every row, s^2 E_g|r|^2 (s from the stored key by the build's f32 formula, E_g|r|^2
/// the group's mean squared residual recomputed here from the stored vectors) must equal |r|^2 = |x - m|^2 in f64 to
/// within what the key's rounding and |x| != 1 allow, plus 0.1% of |r|^2. Returns (rows, rows beyond, largest error
/// over E_g|r|^2, smallest and largest ratio of the build's divisor to the recomputed E_g|r|^2).
pub fn scale_check(ix: &Index2) -> (usize, usize, f64, f64, f64) {
    let n = ix.n();
    let ng = ix.groups.len();
    let r2row: Vec<(usize, f64, f64)> = (0..ix.k()).into_par_iter().flat_map_iter(|ci| {
        let c = &ix.cl[ci];
        let m = ix.centres_f64(ci);
        let mut x = vec![0f64; n];
        (c.r0..c.r1).map(move |r| {
            ix.full_row_f64(r, &mut x);
            let x2: f64 = x.iter().map(|v| v * v).sum();
            let rr: f64 = x.iter().zip(m).map(|(a, b)| (a - b) * (a - b)).sum();
            (ci, rr, x2)
        }).collect::<Vec<_>>()
    }).collect();
    let mut sum = vec![0f64; ng];
    let mut cnt = vec![0usize; ng];
    for &(ci, rr, _) in &r2row {
        sum[ix.cl[ci].g] += rr;
        cnt[ix.cl[ci].g] += 1;
    }
    let mean: Vec<f64> = (0..ng).map(|g| sum[g] / cnt[g].max(1) as f64).collect();
    let (mut bad, mut worst) = (0, 0f64);
    for (r, &(ci, rr, x2)) in r2row.iter().enumerate() {
        let c = &ix.cl[ci];
        let gp = &ix.groups[c.g];
        let s = item_scale(ix.ahat[r], c.mn32, gp.lamsum) as f64;
        let pred = s * s * mean[c.g];
        let tol = (x2 - 1.0).abs() + 2.0 * c.mn * crate::index::DA + 1e-6 + 1e-3 * rr;
        bad += ((pred - rr).abs() > tol) as usize;
        worst = worst.max((pred - rr).abs() / mean[c.g]);
    }
    let ratios: Vec<f64> = (0..ng).filter(|&g| cnt[g] > 0).map(|g| ix.groups[g].lamsum as f64 / mean[g]).collect();
    (r2row.len(), bad, worst, ratios.iter().cloned().fold(f64::MAX, f64::min), ratios.iter().cloned().fold(0.0, f64::max))
}

/// Every threshold query and k-NN against brute force, as planned and on the index path; k-NN also on full vectors
/// (k-NN compared by the exact scores of the k returned, so ties may differ). Intervals checked at every level on the
/// first `ncheck`. Then random visible sets (`--vfilt` queries each), against an exact brute force over the visible rows.
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
    if let Ok(p) = std::env::var("VEXACT_DUMP") {
        *crate::v2::q::DUMP.lock().unwrap() = Some(std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    }
    for (qi, (cls, q)) in qs.iter().enumerate() {
        let pr = prep2(ix, q);
        let err = full_err2(ix, &pr);
        let sc = fl.scores(&pr.q32);
        let ths = thresholds_for(&fl, &pr.q16, &sc, err, &sizes);
        let chk = (qi < ncheck) || (qi >= ni && qi < ni + ncheck);
        // as planned, and with the index path forced so that its intervals are checked whatever the planner picks
        let modes = [c.plan, Plan { mode: PlanMode::Index, ..c.plan }];
        for (si, &th) in ths.iter().enumerate() {
            let truth = fl.threshold(&pr.q16, &pr.q32, th, err, None, &c.ent_xs);
            for (mi, plan) in modes.into_iter().enumerate() {
                crate::v2::q::DUMP_TAG.store((qi * 64 + si * 2 + mi) as u32, std::sync::atomic::Ordering::Relaxed);
                let pr2 = prep2(ix, q);
                let o = Opts2 { vis: None, verify: chk, want_iv: false, plan };
                let r = threshold2(ix, &pr2, th, &o);
                let bm = to_bitmap2(ix, &r.rows);
                let (a, b) = sym_diff(&bm, &truth);
                wi += a;
                wo += b;
                tot.add(&r.st);
                rows.push(format!("{},{},{},{},{},{},{},{},{},{:.3}", qi, cls, SIZES[si], th, truth.len(), a, b, r.st.touched, r.st.full_reads, bytes_read2(ix, &r.st)));
            }
        }
        let fk = fl.knn(&pr.q16, &pr.q32, 10, err, None, &c.ent_xs);
        for plan in [modes[0], modes[1], Plan { mode: PlanMode::Flat, ..c.plan }] {
            let kn = knn2(ix, q, 10, None, plan);
            let mut sa: Vec<f64> = kn.ents.iter().map(|x| exact_value2(ix, &pr, ix.row_of_ent(x.1))).collect();
            sa.sort_by(|x, y| y.partial_cmp(x).unwrap());
            let sb: Vec<f64> = fk.iter().map(|x| x.0).collect();
            if sa != sb {
                kb += 1;
                out!("knn mismatch q{} {} {:?}", qi, cls, plan.mode);
            }
        }
        if qi % 50 == 0 {
            println!("verify2 {qi}/{} {:.0}s wrong in/out {wi}/{wo} det {} rand {} checked {} maxsd {:.2}", qs.len(), t.s(), tot.viol_det, tot.viol_rand, tot.checked, tot.max_sd);
        }
    }
    // filtered: random visible sets (one row in 1,000 / 100 / 10 / 2), the first `nf` queries; threshold queries at
    // the 100th best visible score and k-NN, as planned and on each forced path, against a brute force over the
    // visible rows that uses only exact integer products
    if let Some(mut f) = crate::v2::q::DUMP.lock().unwrap().take() {
        use std::io::Write;
        f.flush().unwrap();
    }
    let nf = arg_usize("--vfilt", 40).min(qs.len());
    for (fi, &den) in [1000u64, 100, 10, 2].iter().enumerate() {
        let mut ents: Vec<u32> = (0..ix.nrows).filter(|&r| ((r as u64) ^ (fi as u64 * 0x51ed27)).wrapping_mul(0x9E3779B97F4A7C15) >> 32 < (1u64 << 32) / den).map(|r| ix.row_ent[r]).collect();
        ents.sort_unstable();
        let bm = RoaringBitmap::from_sorted_iter(ents).unwrap();
        let vis = Vis2::new(ix, &bm);
        // the index compacted to the visible rows, with and without its own vectors
        let subs = [ix.compact(&vis.rows, true), ix.compact(&vis.rows, false)];
        for (qi, (_, q)) in qs.iter().step_by((qs.len() / nf.max(1)).max(1)).take(nf).enumerate() {
            let pr = prep2(ix, q);
            let mut ex: Vec<(f64, u32)> = vis.rows.par_iter().map(|&r| (exact_to_f64(exact_dot(&pr.q16, ix.full_row_f16(r as usize))), r)).collect();
            ex.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            let want: Vec<f64> = ex.iter().take(10).map(|x| x.0).collect();
            let t = ex[(100).min(ex.len()) - 1].0;
            let mut truth: Vec<u32> = ex.iter().filter(|x| x.0 >= t).map(|x| ix.row_ent[x.1 as usize]).collect();
            truth.sort_unstable();
            for plan in [c.plan, Plan { mode: PlanMode::Index, ..c.plan }, Plan { mode: PlanMode::Flat, ..c.plan }] {
                let r = threshold2(ix, &pr, t, &Opts2 { vis: Some(&vis), verify: false, want_iv: false, plan });
                let (a, b) = sym_diff(&to_bitmap2(ix, &r.rows), &truth);
                wi += a;
                wo += b;
                for sub in &subs {
                    let r = threshold2(sub, &prep2(sub, q), t, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                    let (a, b) = sym_diff(&to_bitmap2(sub, &r.rows), &truth);
                    if a + b > 0 {
                        out!("compacted index wrong q{qi} visible 1/{den} {:?}: in {a} out {b}", plan.mode);
                    }
                    wi += a;
                    wo += b;
                }
                let kn = knn2(ix, q, 10, Some(&vis), plan);
                let mut sa: Vec<f64> = kn.ents.iter().map(|x| exact_value2(ix, &pr, ix.row_of_ent(x.1))).collect();
                sa.sort_by(|x, y| y.partial_cmp(x).unwrap());
                if sa != want {
                    kb += 1;
                    out!("knn mismatch q{qi} visible 1/{den} {:?}", plan.mode);
                }
            }
        }
        println!("verify2 filtered 1/{den}: {} visible, wrong in/out {wi}/{wo}, k-NN mismatches {kb}", vis.rows.len());
    }
    out!("VERIFY2 {} queries x {} answer sizes: wrong-in {} wrong-out {}; interval checks {} (det violations {}, rand violations {}); largest randomised error {:.2} sd (group 0's constants by level {:?}, N = 2^{}); k-NN mismatches {} of {} (three plans each){}",
        qs.len(), SIZES.len(), wi, wo, tot.checked, tot.viol_det, tot.viol_rand, tot.max_sd, ix.groups[0].csgl, ix.meta.npow, kb, qs.len(), format!("; filtered: {nf} queries x 4 visible sets x 3 plans, threshold (also on the index compacted to the set, with and without its vectors) and k-NN, included in the counts"));
    out!("VERIFY2 checker: {} deterministic decisions checked against the f64 certificate, {} not implied; {} lanes outside it; entering each level (unfiltered, both plans): {:?}; decided in/out {:?} / {:?}",
        tot.imp_checked, tot.imp_fail, tot.cert_viol, tot.enter, tot.dec_in, tot.dec_out);
    let sc = scale_check(ix);
    out!("VERIFY2 item scale against f64: {} rows, {} beyond tolerance; largest |s^2 E|r|^2 - |r|^2| / E|r|^2 = {:.2e}; the build's divisor over E|r|^2 per group {:.5} to {:.5}", sc.0, sc.1, sc.2, sc.3, sc.4);
    std::fs::write(format!("{}/verify2.csv", crate::run2::res_dir()), rows.join("\n") + "\n").unwrap();
}

/// ns per item per level: all items of every cluster, tables built beforehand. By default no decisions (interval at
/// each level); with `--sweep` every item is decided against the query's 1% threshold at every level and kept, on the
/// integer deterministic levels with `--int`. Each timing is of `--passes` passes over the index.
pub fn levels2(c: &Ctx2, reps: usize) {
    let ix = &c.ix;
    let nl = ix.meta.nl();
    let sweep = args().iter().any(|a| a == "--sweep");
    let passes = arg_usize("--passes", 1); // passes over the index per timing
    if let Ok(cpu) = arg_str("--core", "").parse::<usize>() {
        pin(cpu);
    }
    let fl = c.flat();
    let bytes = ix.stored_bytes_per_item();
    let mut lines = vec!["query,level,level_bytes_cum,stored_bytes_cum,ns_per_item_level,ns_per_stored_byte_level,level_min,level_max,decided".to_string()];
    for (name, q) in [("item", c.items[0].clone()), ("text", c.texts[0].clone())] {
        let pr = prep2(ix, &q);
        let mut st = Stats::new(nl);
        let mut cqs: Vec<CQ2> = (0..ix.k()).map(|ci| {
            let gq = get_gq2(ix, &pr, ix.cl[ci].g, &mut st);
            let mut cq = cluster_q2(ix, ci, gq, &pr, dot64(&pr.q64, ix.centres_f64(ci)));
            if sweep && c.plan.int_det {
                cq.gtab = c.plan.gtab;
                cq.enable_int(ix, ci, gq);
            }
            cq
        }).collect();
        let t = if sweep {
            let sc = fl.scores(&pr.q32);
            thresholds_for(&fl, &pr.q16, &sc, full_err2(ix, &pr), &[((fl.nrows as f64 * 0.01).round() as usize).max(1)])[0]
        } else {
            f64::NAN
        };
        let mode = |l: usize| if sweep { Mode2::Sweep(l) } else { Mode2::Interval(l) };
        let mut decided = vec![0u64; nl];
        // build every table once (lazily inside the first run)
        let cands: Vec<Vec<u32>> = ix.cl.iter().map(|cl| (0..(cl.r1 - cl.r0) as u32).collect()).collect();
        let mut sv = State::default();
        for ci in 0..ix.k() {
            let (mut a, mut b, mut iv) = (Vec::new(), Vec::new(), Vec::new());
            let mut out = RunOut { rows_in: &mut a, pend: &mut b, iv: if sweep { None } else { Some(&mut iv) } };
            unsafe { run_cluster(ix, ci, &pr, pr.gq[ix.cl[ci].g].get().unwrap(), &mut cqs[ci], &cands[ci], t, mode(nl - 1), false, &mut sv, &mut st, &mut out) };
        }
        for l in 0..nl {
            decided[l] = st.dec_in[l] + st.dec_out[l];
        }
        // each repetition runs every level once; a level's time is its own blocks'
        // cycles (tables built beforehand, the key and thresholds of level 0 not included), converted at the counter's
        // rate over the repetition
        let nlv = arg_usize("--levels", nl).min(nl);
        let mut ts = vec![Vec::new(); nlv];
        let mut cum = Vec::new();
        for _ in 0..reps {
            let mut st = Stats::new(nl);
            let mut iv = Vec::with_capacity(2048);
            PROF.store(std::env::var("VEXACT_PROF").is_ok(), std::sync::atomic::Ordering::Relaxed);
            let tm = Timer::new();
            let c0 = unsafe { core::arch::x86_64::_rdtsc() };
            for ci in (0..passes).flat_map(|_| 0..ix.k()) {
                iv.clear();
                let (mut a, mut b) = (Vec::new(), Vec::new());
                let mut out = RunOut { rows_in: &mut a, pend: &mut b, iv: if sweep { None } else { Some(&mut iv) } };
                unsafe { run_cluster(ix, ci, &pr, pr.gq[ix.cl[ci].g].get().unwrap(), &mut cqs[ci], &cands[ci], t, mode(nlv - 1), false, &mut sv, &mut st, &mut out) };
            }
            let ghz = (unsafe { core::arch::x86_64::_rdtsc() } - c0) as f64 / (tm.s() * 1e9);
            cum.push(tm.s() * 1e9 / (ix.nrows * passes) as f64);
            for l in 0..nlv {
                ts[l].push(st.lcyc[l] as f64 / ghz / st.enter[l] as f64);
            }
            if std::env::var("VEXACT_PROF").is_ok() {
                out!("levels2 phases (ns per item entering level 0): repack {:.2} decode {:.2} bounds {:.2}", st.cyc[P_REPACK] as f64 / ghz / st.enter[0] as f64, st.cyc[P_DECODE] as f64 / ghz / st.enter[0] as f64, st.cyc[P_BOUNDS] as f64 / ghz / st.enter[0] as f64);
            }
        }
        out!("levels2 {name}: all {nlv} levels {:.2} ns per item (median of {reps}; {:.2} to {:.2})", median(&cum), cum.iter().cloned().fold(f64::MAX, f64::min), cum.iter().cloned().fold(0.0, f64::max));
        for l in 0..nlv {
            let mut v = ts[l].clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let sb = bytes[l] - if l == 0 { 2.0 } else { bytes[l - 1] };
            let line = format!("{},{},{},{:.2},{:.3},{:.4},{:.3},{:.3},{}", name, l, ix.meta.level_bytes(l), bytes[l], median(&v), median(&v) / sb, v[0], v[reps - 1], decided[l]);
            out!("levels2 {line}");
            lines.push(line);
        }
    }
    let tag = format!("{}{}", if sweep { "_sweep" } else { "" }, if c.plan.int_det { "_int" } else { "" });
    std::fs::write(format!("{}/levels2{tag}.csv", crate::run2::res_dir()), lines.join("\n") + "\n").unwrap();
}

/// Fixed setup per query: group setup (exact int8 basis product, rotated weights) and cluster setup with tables.
pub fn setup_cost(c: &Ctx2, nq: usize) {
    pool(1).install(|| setup_cost1(c, nq));
}

/// Bytes of every group's int8 basis summed as a plain stream (the memory floor of the basis products), ms.
fn raw_read_ms(ix: &Index2) -> f64 {
    use core::arch::x86_64::*;
    let t = Timer::new();
    let mut acc = unsafe { _mm256_setzero_si256() };
    for gp in ix.groups.iter() {
        let p = gp.v8t.as_ptr();
        let mut i = 0;
        while i + 32 <= gp.v8t.len() {
            acc = unsafe { _mm256_add_epi8(acc, _mm256_loadu_si256(p.add(i) as *const _)) };
            i += 32;
        }
    }
    std::hint::black_box(acc);
    t.ms()
}

fn setup_cost1(c: &Ctx2, nq: usize) {
    let ix = &c.ix;
    let raw: Vec<f64> = (0..10).map(|_| raw_read_ms(ix)).collect();
    out!("setup2 raw read of all {} groups' int8 bases ({:.1} MB): {:.2} ms", ix.groups.len(), ix.groups.iter().map(|g| g.v8t.len()).sum::<usize>() as f64 / 1e6, median(&raw));
    // the basis-product kernel's prefetch depth on this index's bases (one query, every group)
    if let Some(q) = c.texts.first().or(c.items.first()) {
        let pr = prep2(ix, q);
        let n = ix.n();
        let mut qv = vec![0f64; n];
        let time = |f: &dyn Fn(&[i8], &[f64], &mut [f64])| -> f64 {
            let mut qv = vec![0f64; n];
            median(&(0..10).map(|_| {
                let t = Timer::new();
                for gp in ix.groups.iter() {
                    f(&gp.v8t, &gp.vsc, &mut qv);
                    std::hint::black_box(&qv);
                }
                t.ms()
            }).collect::<Vec<_>>())
        };
        let _ = &mut qv;
        use crate::v2::setup::qv_exact_r;
        out!("setup2 basis products of all groups by prefetch: none {:.2} ms, next block {:.2} ms, two ahead {:.2} ms, three ahead {:.2} ms",
            time(&|v, s, o| unsafe { qv_exact_r::<4, 0>(v, s, n, &pr.qhi, &pr.qlo, o) }), time(&|v, s, o| unsafe { qv_exact_r::<4, 1>(v, s, n, &pr.qhi, &pr.qlo, o) }),
            time(&|v, s, o| unsafe { qv_exact_r::<4, 2>(v, s, n, &pr.qhi, &pr.qlo, o) }), time(&|v, s, o| unsafe { qv_exact_r::<4, 3>(v, s, n, &pr.qhi, &pr.qlo, o) }));
    }
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
                    unsafe { crate::v2::kern::build_lut(&cq.w, &gp.levels[l], &ix.etab, None, &mut lut) };
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
                    unsafe { crate::v2::kern::build_lut(&cqs[0].w, &gp.levels[l], &ix.etab, None, &mut lut) };
                    nu += gp.levels[l].units;
                }
            }
            tw.push(t.ms() * 1e6 / nu as f64);
        }
        out!("setup2 {cls} (1 thread, {} groups, {} clusters): group setup {:.2} ms (int8 basis products alone, warm {:.2} ms; rotations alone {:.2} ms), cluster setup {:.2} ms ({:.2} us each), all tables of every cluster and level {:.2} ms ({:.2} us per cluster; warm {:.1} ns per unit)",
            ix.groups.len(), ix.k(), median(&tg), median(&tq), median(&trot), median(&tc), median(&tc) * 1e3 / ix.k() as f64, median(&tl), median(&tl) * 1e3 / ix.k() as f64, median(&tw));
    }
}

/// End to end, threshold queries at every answer share and exact k-NN (k = 10), item and text queries, as planned, for
/// each variant (an index and plan; with two, the second from `--index-b` and `--b`). First one untimed pass on one
/// thread for bytes read per item and items entering each level (survivors); then `--runs` timed runs (after one
/// warm-up run) on each of `--threads` (default 1,4; pinned with `--pin`), one timing per query per run, the variants
/// interleaved cell by cell so that they share the machine's state; a cell's figure is the median over runs of the
/// median over queries, with the smallest and largest run.
pub fn e2e2(vs: &[(&str, &Ctx2, Plan)], nq: usize) {
    let c0 = vs[0].1;
    // answer shares from `--sizes` (fractions, comma-separated); `--knn 0` leaves out the k-NN cell
    let sizes_f: Vec<f64> = { let a = arg_str("--sizes", ""); if a.is_empty() { SIZES.to_vec() } else { a.split(',').map(|x| x.parse().unwrap()).collect() } };
    let nrows = c0.flat().nrows;
    let nl = c0.ix.meta.nl();
    let runs = arg_usize("--runs", 5);
    let threads: Vec<usize> = arg_str("--threads", "1,4").split(',').map(|s| s.parse().unwrap()).collect();
    let sizes: Vec<usize> = sizes_f.iter().map(|f| ((nrows as f64 * f).round() as usize).max(1)).collect();
    let mut lines = vec!["variant,class,threads,what,answer_frac,median_of_runs_ms,min_run_ms,max_run_ms,nq,runs,bytes_read_per_item,touched_frac,full_reads_per_item,enter_frac_by_level".to_string()];
    for (cls, qs) in [("item", c0.items.clone()), ("text", c0.texts.clone())] {
        let qs: Vec<Vec<f32>> = qs.into_iter().take(nq).collect();
        if qs.is_empty() {
            continue;
        }
        let ths: Vec<Vec<f64>> = qs.iter().map(|q| {
            let fl = c0.flat();
            let pr = prep2(&c0.ix, q);
            let sc = fl.scores(&pr.q32);
            thresholds_for(&fl, &pr.q16, &sc, full_err2(&c0.ix, &pr), &sizes)
        }).collect();
        let ncell = sizes_f.len() + if arg_usize("--knn", 1) > 0 { 1 } else { 0 }; // the last cell is k-NN
        let run_cell = |c: &Ctx2, plan: Plan, cell: usize, qi: usize| -> Stats {
            let ix = &c.ix;
            if cell < sizes_f.len() {
                let o = Opts2 { vis: None, verify: false, want_iv: false, plan };
                let pr = prep2(ix, &qs[qi]);
                let r = threshold2(ix, &pr, ths[qi][cell], &o);
                std::hint::black_box(to_bitmap2(ix, &r.rows));
                r.st
            } else {
                knn2(ix, &qs[qi], 10, None, plan).st
            }
        };
        let k = qs.len() as f64;
        let mut cellinfo = vec![Vec::new(); vs.len()];
        for (vi, &(vname, c, plan)) in vs.iter().enumerate() {
            let pl = pool(1);
            for cell in 0..ncell {
                let (mut br, mut tch, mut fr) = (0.0, 0.0, 0.0);
                let mut enter = vec![0f64; nl + 1];
                for qi in 0..qs.len() {
                    let st = pl.install(|| run_cell(c, plan, cell, qi));
                    br += bytes_read2(&c.ix, &st);
                    tch += st.touched as f64;
                    fr += st.full_reads as f64;
                    for (e, x) in enter.iter_mut().zip(&st.enter) {
                        *e += *x as f64;
                    }
                }
                let ent: Vec<String> = enter.iter().map(|x| format!("{:.5}", x / k / nrows as f64)).collect();
                cellinfo[vi].push(format!("{:.3},{:.4},{:.5},{}", br / k, tch / k / nrows as f64, fr / k / nrows as f64, ent.join(";")));
                out!("e2e2 bytes {vname} {cls} {}: bytes/item {:.3}, touched {:.4}, full reads/item {:.5}", if cell < sizes_f.len() { format!("{}", sizes_f[cell]) } else { "knn10".into() }, br / k, tch / k / nrows as f64, fr / k / nrows as f64);
            }
        }
        for &th in &threads {
            let pl = pool(th);
            let mut per = vec![vec![Vec::new(); ncell]; vs.len()];
            for run in 0..=runs {
                for cell in 0..ncell {
                    for (vi, &(_, c, plan)) in vs.iter().enumerate() {
                        let mut ts = Vec::with_capacity(qs.len());
                        for qi in 0..qs.len() {
                            let t = Timer::new();
                            pl.install(|| run_cell(c, plan, cell, qi));
                            ts.push(t.ms());
                        }
                        if run > 0 {
                            per[vi][cell].push(median(&ts));
                        }
                    }
                }
            }
            for (vi, &(vname, _, _)) in vs.iter().enumerate() {
                for cell in 0..ncell {
                    let v = &per[vi][cell];
                    let (what, frac) = if cell < sizes_f.len() { ("index2_thr", sizes_f[cell]) } else { ("index2_knn10", 0.0) };
                    let line = format!("{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{}", vname, cls, th, what, frac, median(v), v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(0.0, f64::max), qs.len(), runs, cellinfo[vi][cell]);
                    out!("e2e2 {line}");
                    lines.push(line);
                }
            }
        }
    }
    std::fs::write(format!("{}/e2e2{}.csv", crate::run2::res_dir(), arg_str("--tag", "")), lines.join("\n") + "\n").unwrap();
}

/// Where the time of a threshold query goes (item and text queries at 1% and 5%; 1 and 4 threads), for the plan as
/// given.
pub fn breakdown2(c: &Ctx2, nq: usize) {
    use std::sync::atomic::Ordering;
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    // time-stamp-counter rate
    let (t, c0) = (Timer::new(), unsafe { core::arch::x86_64::_rdtsc() });
    while t.ms() < 200.0 {}
    let hz = (unsafe { core::arch::x86_64::_rdtsc() } - c0) as f64 / t.s();
    // the phases, then what the enclosing totals leave between them: in run_cluster outside its phases (per-level
    // bookkeeping), in a cluster's work outside run_cluster, its setup and full vectors (candidates, ranges, allocation),
    // in the query outside every cluster and phase (merge, drops), and outside the query's own counters (the pool's
    // hand-off); all from the profiled run, whose wall time is prof_wall_ms (the mean over queries)
    let names = ["anchor", "cut", "basis", "rotations", "cluster setup", "tables", "re-pack", "level decode", "bounds+decisions", "full vectors", "candidate init", "result bitmap", "column order",
                 "compaction", "level setup", "group post", "level other", "cluster other", "merge", "query other", "unattributed"];
    let mut lines = vec!["variant,class,answer_frac,threads,wall_ms,prof_wall_ms,cpu_ms_total,".to_string() + &names.join(",") + ",bytes_read,full_reads,wrong"];
    let variants: &[&str] = &["levels"];
    for (cls, qs) in [("item", &c.items), ("text", &c.texts)] {
    let qs: Vec<Vec<f32>> = qs.iter().take(nq).cloned().collect();
    let bfracs: Vec<f64> = arg_str("--bfracs", "0.01,0.1").split(',').map(|s| s.parse().unwrap()).collect();
    let bthreads: Vec<usize> = arg_str("--threads", "1,4").split(',').map(|s| s.parse().unwrap()).collect();
    for &frac in &bfracs {
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
        for &vname in variants {
            let plan = c.plan;
            for &th in &bthreads {
                let pl = pool(th);
                let ms = |c: u64| c as f64 / hz * 1e3 / qs.len() as f64;
                let (mut walls, mut br, mut fr, mut wrong, mut pwall) = (Vec::new(), 0.0, 0.0, 0u64, 0.0);
                let mut parts = [0f64; 21];
                let mut cnt = [0f64; 4];
                let mut rkap = 0f64;
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
                            drop(pr);
                            r.st.cyc[P_QUERY] += tick() - a0;
                            (r, bm)
                        });
                        let w = tm.ms();
                        if rep < 2 {
                            best = best.min(w);
                        } else {
                            pwall += w / qs.len() as f64;
                            keep = Some(r);
                        }
                    }
                    PROF.store(false, Ordering::Relaxed);
                    let (r, bm) = keep.unwrap();
                    let cy = &r.st.cyc;
                    for k in 0..13 {
                        parts[k] += ms(cy[k]);
                    }
                    let inner = |ks: &[usize]| ks.iter().map(|&k| cy[k] as i64).sum::<i64>();
                    let lv = cy[P_RUNCL] as i64 - inner(&[P_INIT, P_TABLES, P_REPACK, P_DECODE, P_BOUNDS, P_COMPACT, P_LVSETUP]);
                    let clo = cy[P_CLUSTER] as i64 - cy[P_RUNCL] as i64 - inner(&[P_CLSETUP, P_FULL]);
                    let qo = cy[P_QUERY] as i64 - cy[P_CLUSTER] as i64 - cy[P_MERGE] as i64 - inner(&[P_ANCHOR, P_CUT, P_BASIS, P_ROT, P_PLAN, P_GPOST]);
                    for (k, v) in [(13, cy[P_COMPACT] as i64), (14, cy[P_LVSETUP] as i64), (15, cy[P_GPOST] as i64), (16, lv), (17, clo), (18, cy[P_MERGE] as i64), (19, qo)] {
                        parts[k] += v as f64 / hz * 1e3 / qs.len() as f64;
                    }
                    for k in 0..4 {
                        cnt[k] += cy[C_TABLES + k] as f64 / qs.len() as f64;
                    }
                    rkap += ms(cy[P_RKAP]);
                    br += bytes_read2(ix, &r.st) / qs.len() as f64;
                    fr += r.st.full_reads as f64 / qs.len() as f64;
                    let (a, b) = sym_diff(&bm, &truths[qi]);
                    wrong += a + b;
                    walls.push(best);
                }
                parts[20] = pwall - parts[..20].iter().sum::<f64>();
                let tot: f64 = parts[..20].iter().sum();
                let line = format!("{},{cls},{},{},{:.2},{:.2},{:.2},{},{:.2},{:.0},{}", vname, frac, th, median(&walls), pwall, tot, parts.iter().map(|x| format!("{:.2}", x)).collect::<Vec<_>>().join(","), br, fr, wrong);
                out!("breakdown2 {line}");
                out!("breakdown2 rand {vname} {cls} {frac}: of bounds+decisions, randomised scalars (|y|, kappa, tan decode) {:.2} ms", rkap);
                out!("breakdown2 tables {vname} {cls} {frac}: per query {:.0} (cluster, level) tables, {:.0} units, {:.1} survivors per table, {:.0} tables for at most 8", cnt[0], cnt[1], cnt[2] / cnt[0].max(1.0), cnt[3]);
                lines.push(line);
            }
        }
    }
    }
    std::fs::write(format!("{}/breakdown2{}.csv", crate::run2::res_dir(), arg_str("--tag", "")), lines.join("\n") + "\n").unwrap();
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
    std::fs::write(format!("{}/filter2_1m.csv", crate::run2::res_dir()), lines.join("\n") + "\n").unwrap();
}

/// Results for the independent float64 oracle (bench/oracle_thr.py): the first `nq` item and text queries, threshold
/// queries at every answer share as planned, and k-NN (k = 10); then the same at the 1% and 10% thresholds and k-NN
/// over a visible set of about one entity in fifty. Writes to RESULTS/oracle: queries.npy (the queries as the index
/// used them, f16 values as f32), thr.npy, res_ids.npy and res_off.npy (entity ids of each result, concatenated),
/// knn.npy, vis.npy (the visible entities).
pub fn oracle_dump(c: &Ctx2, nq: usize) {
    let ix = &c.ix;
    let fl = c.flat();
    let nrows = fl.nrows;
    let dir = format!("{}/oracle", res_dir());
    std::fs::create_dir_all(&dir).unwrap();
    let sizes: Vec<usize> = SIZES.iter().map(|f| ((nrows as f64 * f).round() as usize).max(1)).collect();
    let mut ents: Vec<u32> = (0..nrows as u32).map(|r| ix.row_ent[r as usize]).filter(|&e| (e as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 58 == 0).collect();
    ents.sort_unstable();
    let bm = RoaringBitmap::from_sorted_iter(ents.iter().copied()).unwrap();
    let vis = Vis2::new(ix, &bm);
    let qs: Vec<&Vec<f32>> = c.items.iter().take(nq).chain(c.texts.iter().take(nq)).collect();
    let (mut qv, mut thr, mut ids, mut off, mut knn) = (Vec::new(), Vec::new(), Vec::new(), vec![0i64], Vec::new());
    for q in &qs {
        let pr = prep2(ix, q);
        qv.extend_from_slice(&pr.q32);
        let ths = thresholds_for(&fl, &pr.q16, &fl.scores(&pr.q32), full_err2(ix, &pr), &sizes);
        let mut run = |t: f64, vis: Option<&Vis2>| {
            let r = threshold2(ix, &pr, t, &Opts2 { vis, verify: false, want_iv: false, plan: c.plan });
            ids.extend(to_bitmap2(ix, &r.rows).iter().map(|e| e as i64));
            off.push(ids.len() as i64);
            thr.push(t);
        };
        for &t in &ths {
            run(t, None);
        }
        for &t in &ths[2..4] {
            run(t, Some(&vis));
        }
        for v in [None, Some(&vis)] {
            let kn = knn2(ix, q, 10, v, c.plan);
            assert_eq!(kn.ents.len(), 10);
            knn.extend(kn.ents.iter().map(|x| x.1 as i64));
        }
    }
    let nqt = qs.len();
    let b = |v: &[f64]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let bi = |v: &[i64]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    write_npy(&format!("{dir}/queries.npy"), "<f4", &[nqt, ix.n()], &qv.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>());
    write_npy(&format!("{dir}/thr.npy"), "<f8", &[nqt, SIZES.len() + 2], &b(&thr));
    write_npy(&format!("{dir}/res_ids.npy"), "<i8", &[ids.len()], &bi(&ids));
    write_npy(&format!("{dir}/res_off.npy"), "<i8", &[off.len()], &bi(&off));
    write_npy(&format!("{dir}/knn.npy"), "<i8", &[nqt, 2, 10], &bi(&knn));
    write_npy(&format!("{dir}/vis.npy"), "<i8", &[ents.len()], &bi(&ents.iter().map(|&e| e as i64).collect::<Vec<_>>()));
    out!("oracle dump: {nqt} queries, {} results, {} visible entities, to {dir}", off.len() - 1, ents.len());
}

/// Filtered threshold queries (the unfiltered 1% threshold) over random visible sets of 0.1, 1 and 10% of the rows:
/// the scan of the visible set in place against the packed scan (scan_visible), and as planned with each, on each of
/// `--threads`, `--runs` interleaved runs; every variant's result is checked against the scan in place.
pub fn scanbench(c: &Ctx2, nq: usize) {
    use rand::SeedableRng;
    let ix = &c.ix;
    let fl = c.flat();
    let runs = arg_usize("--runs", 5);
    let threads: Vec<usize> = arg_str("--threads", "1,4").split(',').map(|s| s.parse().unwrap()).collect();
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let mut lines = vec!["class,visible,threads,what,median_of_runs_ms,min_run_ms,max_run_ms,planned_scans".to_string()];
    for &frac in &[0.001, 0.01, 0.1] {
        let bm = make_filter(ix, "random", frac, &mut rng);
        let vis = Vis2::new(ix, &bm);
        for (cls, qs) in [("item", &c.items), ("text", &c.texts)] {
            let qs: Vec<Vec<f32>> = qs.iter().take(nq).cloned().collect();
            let ths: Vec<f64> = qs.iter().map(|q| {
                let pr = prep2(ix, q);
                thresholds_for(&fl, &pr.q16, &fl.scores(&pr.q32), full_err2(ix, &pr), &[(fl.nrows / 100).max(1)])[0]
            }).collect();
            let variants: [(&str, bool, bool); 4] = [("scan_in_place", false, false), ("scan_packed", true, false), ("planned_in_place", false, true), ("planned_packed", true, true)];
            let mut scans = [0usize; 4];
            for &th in &threads {
                let pl = pool(th);
                let mut per = vec![Vec::new(); 4];
                for run in 0..=runs {
                    for (vi, &(_, packed, planned)) in variants.iter().enumerate() {
                        let mut ts = Vec::new();
                        for (qi, q) in qs.iter().enumerate() {
                            let pr = prep2(ix, q);
                            let t = Timer::new();
                            let r = pl.install(|| if planned {
                                threshold2(ix, &pr, ths[qi], &Opts2 { vis: Some(&vis), verify: false, want_iv: false, plan: Plan { pscan: packed, ..c.plan } })
                            } else {
                                scan_visible(ix, &pr, ths[qi], Some(&vis), false, packed)
                            });
                            ts.push(t.ms());
                            if run == 0 && th == threads[0] {
                                scans[vi] += r.scanned as usize;
                                let mut a = r.rows.clone();
                                a.sort_unstable();
                                let mut b = scan_visible(ix, &pr, ths[qi], Some(&vis), false, false).rows;
                                b.sort_unstable();
                                assert_eq!(a, b, "scan variants disagree");
                            }
                        }
                        if run > 0 {
                            per[vi].push(median(&ts));
                        }
                    }
                }
                for (vi, &(what, _, _)) in variants.iter().enumerate() {
                    let v = &per[vi];
                    let line = format!("{cls},{frac},{th},{what},{:.3},{:.3},{:.3},{}", median(v), v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(0.0, f64::max), scans[vi]);
                    out!("scanbench {line}");
                    lines.push(line);
                }
            }
        }
    }
    std::fs::write(format!("{}/scanbench.csv", res_dir()), lines.join("\n") + "\n").unwrap();
}

/// Threshold queries at explicit thresholds (`--thr FILE`, Q x S f64, one row per `--queries` row), unfiltered and
/// within each visible set (`--cmp-vis` or `--vis A.npy,B.npy`, entity ids; `--skip-all` leaves out the unfiltered run,
/// `--sets` keeps only the named sets), timed whole: query preparation, the query, and the answer as a bitmap of entity
/// ids. The variants (`--cmp-what` or `--what`, default index,flat): `index[:FLAGS]` is the planner's path, `idx[:FLAGS]`
/// (or `forced`) the index path on every cluster, `scan` LIMINAL's own scan, `cindex`, `cvindex`, `cscan` and `cvscan`
/// the same on the session's index compacted to its codes (c) or its codes and vectors (cv), `prep` and `anc` the
/// preparation alone and with the anchor search, and `flat` every stored f16 vector (the visible ones only; read from
/// --full-file when there is one). Every variant's answer must equal the first's. Every `--step`-th query runs, `--reps`
/// times, on each of `--threads` (1,4) and each of `--read-modes`, the variants interleaved per query and share, or one
/// after another with `--cmp-order variant`. `--cap BYTES` caps the process's memory after the sessions are set up, and
/// `--cold` first times query 0 at each share with the full vectors evicted. The per-query median and minimum, with full
/// vectors read, storage read, major faults and system time, go to RESULTS/dbcmp_<--tag>.csv. With `--dump DIR`, the
/// 1-thread answers go to DIR/<what>_<vis>.bin as (query, share, count, ids...) u32 records.
pub fn dbcmp(c: &Ctx2) {
    use crate::v2::disk;
    use std::sync::atomic::Ordering::Relaxed;
    let ix = &c.ix;
    let fl = c.flat();
    let thr = read_npy(&arg_str("--thr", ""));
    let ns = thr.shape[1];
    let thr = thr.f64s();
    let qs = &c.texts;
    assert_eq!(thr.len(), qs.len() * ns, "--thr must have one row per query");
    let reps = arg_usize("--reps", 5);
    let step = arg_usize("--step", 5);
    let threads: Vec<usize> = arg_str("--threads", "1,4").split(',').map(|x| x.parse().unwrap()).collect();
    let whats: Vec<String> = Some(arg_str("--cmp-what", "")).filter(|v| !v.is_empty()).unwrap_or_else(|| arg_str("--what", "index,flat")).split(',').map(|w| w.to_string()).collect();
    // `query` runs the variants interleaved per query and share; `variant` runs each over every query before the next
    let by_variant = match arg_str("--cmp-order", "query").as_str() {
        "query" => false,
        "variant" => true,
        o => panic!("--cmp-order is query or variant, not {o:?}"),
    };
    let dump = arg_str("--dump", "");
    // how full vectors on file are read, each mode in turn (`--read-modes uring,mmap:...`; the label gains @mode when
    // there are several)
    // under a cap the full vectors come from --full-file, and `flat` scans them there in row order
    let on_file = !arg_str("--full-file", "").is_empty();
    let cold = args().iter().any(|a| a == "--cold");
    let fullrows = match &ix.full {
        Full2::Real { rows, .. } => Some(&**rows),
        _ => None,
    };
    // the read policy of the vectors on file (in memory, one that nothing reads)
    let mem_policy = disk::ReadPolicy::default();
    let pol = fullrows.and_then(|r| r.policy()).unwrap_or(&mem_policy);
    let modes: Vec<String> = arg_str("--read-modes", if pol.uring.load(Relaxed) { "uring" } else { "mmap" }).split(',').map(|x| x.to_string()).collect();
    let (qd0, dm0, dc0) = (pol.qd.load(Relaxed), pol.direct_min.load(Relaxed), pol.dontcache_min.load(Relaxed));
    let mut sets: Vec<(String, Option<RoaringBitmap>)> = if args().iter().any(|a| a == "--skip-all") { vec![] } else { vec![("all".into(), None)] };
    let vf = Some(arg_str("--cmp-vis", "")).filter(|v| !v.is_empty()).unwrap_or_else(|| arg_str("--vis", ""));
    for f in vf.split(',').filter(|f| !f.is_empty()) {
        let a = read_npy(f);
        assert_eq!(a.descr, "<i4", "{f}: --vis files hold sorted int32 entity ids");
        let ids: Vec<u32> = a.data.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let name = std::path::Path::new(f).file_stem().unwrap().to_string_lossy().to_string();
        sets.push((name, Some(RoaringBitmap::from_sorted_iter(ids).unwrap())));
    }
    let only = arg_str("--sets", "");
    let wanted = |vname: &str| only.is_empty() || only.split(',').any(|x| x == vname);
    // every session's visible set and compacted indexes are made before a cap is set
    let mut sessions = Vec::new();
    for (vname, bm) in &sets {
        if !wanted(vname) {
            sessions.push(None);
            continue;
        }
        let t = Timer::new();
        let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
        out!("dbcmp {vname}: {} visible; into row order once per session {:.2} ms", bm.as_ref().map_or(ix.nrows as u64, |b| b.len()), t.ms());
        // the session's index compacted to its visible rows: codes only (c), and codes and vectors (cv)
        let rows_all: Vec<u32>;
        let vrows = match &vis {
            Some(v) => &v.rows,
            None => {
                rows_all = (0..ix.nrows as u32).collect();
                &rows_all
            }
        };
        let mut compacted: [Option<Index2>; 2] = [None, None];
        for (i, pre) in ["c", "cv"].iter().enumerate() {
            if whats.iter().any(|w| w.split(':').next().unwrap().strip_suffix("index").or_else(|| w.split(':').next().unwrap().strip_suffix("scan")) == Some(*pre)) {
                let t = Timer::new();
                let sub = ix.compact(vrows, i == 1);
                out!("dbcmp {vname}: compacted ({}) in {:.2} ms, {:.1} MB of its own", if i == 1 { "codes and vectors" } else { "codes" }, t.ms(), sub.own_bytes() as f64 / 1e6);
                compacted[i] = Some(sub);
            }
        }
        sessions.push(Some((vis, compacted)));
    }
    let cap = arg_usize("--cap", 0);
    if cap > 0 {
        out!("[cap] before: {}", disk::cg_report());
        out!("[cap] capped at {cap} bytes: {}", disk::cap_self(cap as u64));
        if let Some(r) = fullrows {
            r.evict();
            out!("[cap] full vectors resident after eviction: {:.1} MiB", r.resident() as f64 / (1 << 20) as f64);
        }
    }
    // with full vectors read through the reader, the planner's disk constants are timed here under the cap
    // (`--no-calib-disk`: as in the plan; `:plan=mem` in a read mode: reads not counted)
    let mut plan0 = c.plan;
    if pol.priced.load(Relaxed) && !args().iter().any(|a| a == "--no-calib-disk") {
        let t = Timer::new();
        plan0 = Plan::calibrate_disk(ix, plan0);
        out!("[plan] disk timed in {:.1}s: io_us={:.3} io_gbps={:.3}, so rows up to {} bytes apart are read together", t.s(), plan0.io_us, plan0.io_gbps, plan0.io_gap());
    }
    let mut lines = vec!["what,threads,vis,share_index,query,median_ms,min_ms,count,full_reads,disk_kb,majflt,sys_ms,kind".to_string()];
    // every variant's answer must equal the first variant's, bit for bit, in every read mode and at every thread count
    let mut first: std::collections::HashMap<(usize, usize, usize), RoaringBitmap> = Default::default();
    let mut ncheck = 0usize;
    let mut check = |key: (usize, usize, usize), what: &str, ans: &RoaringBitmap, th: usize, vname: &str| match first.entry(key) {
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(ans.clone());
        }
        std::collections::hash_map::Entry::Occupied(e) => {
            assert!(*e.get() == *ans, "{what} at {th} threads disagrees with the first variant on {vname}, query {}, share {}", key.1, key.2);
            ncheck += 1;
        }
    };
    let passes: Vec<Vec<usize>> = if by_variant { (0..whats.len()).map(|w| vec![w]).collect() } else { vec![(0..whats.len()).collect()] };
    for (vi, (vname, bm)) in sets.iter().enumerate() {
        let Some((vis, compacted)) = &sessions[vi] else { continue };
        let vis = vis.as_ref();
        for &th in &threads {
            let pl = pool(th);
            for mode in &modes {
                let mut plan = plan0;
                pol.uring.store(false, Relaxed);
                pol.priced.store(false, Relaxed);
                pol.qd.store(qd0, Relaxed);
                pol.direct_min.store(dm0, Relaxed);
                pol.dontcache_min.store(dc0, Relaxed);
                let bytes = |v: &str| if v == "none" { usize::MAX } else { v.parse().unwrap() };
                for kv in mode.split(':') {
                    match kv.split_once('=') {
                        None if kv == "mmap" => pol.priced.store(false, Relaxed),
                        None if kv == "uring" => {
                            pol.uring.store(true, Relaxed);
                            pol.priced.store(true, Relaxed);
                        }
                        Some(("qd", v)) => pol.qd.store(v.parse().unwrap(), Relaxed),
                        Some(("direct", v)) => pol.direct_min.store(bytes(v), Relaxed),
                        Some(("dontcache", v)) => pol.dontcache_min.store(bytes(v), Relaxed),
                        Some(("plan", "disk")) => pol.priced.store(true, Relaxed),
                        Some(("plan", "mem")) => pol.priced.store(false, Relaxed),
                        Some(("gap", v)) => plan.io_us = v.parse::<f64>().unwrap() / (plan.io_gbps * 1e3),
                        _ => panic!("--read-modes takes mmap or uring, each optionally with :qd=N, :gap=BYTES, :direct=BYTES|none, :dontcache=BYTES|none and :plan=disk or mem"),
                    }
                }
                // each mode starts from an empty cache, so that one mode's cached rows do not serve the next
                if modes.len() > 1 {
                    if let Some(r) = fullrows {
                        r.evict();
                    }
                }
                let labels: Vec<String> = whats.iter().map(|w| if modes.len() > 1 { format!("{w}@{mode}") } else { w.clone() }).collect();
                // one run of variant `wi`: its answer, the full vectors it read, and whether the planner scanned
                let run = |wi: usize, q: &Vec<f32>, t0: f64| -> (RoaringBitmap, u64, bool) {
                    let (kind, flags) = whats[wi].split_once(':').unwrap_or((whats[wi].as_str(), ""));
                    pl.install(|| match kind {
                        "index" | "idx" | "forced" => {
                            let plan = plan_flags(plan, flags);
                            let plan = if kind == "index" { plan } else { Plan { mode: PlanMode::Index, ..plan } };
                            let pr = prep2(ix, q);
                            let r = threshold2(ix, &pr, t0, &Opts2 { vis, verify: false, want_iv: false, plan });
                            (to_bitmap2(ix, &r.rows), r.st.full_reads, r.scanned)
                        }
                        "scan" => {
                            let pr = prep2(ix, q);
                            let r = scan_visible(ix, &pr, t0, vis, false, false);
                            (to_bitmap2(ix, &r.rows), r.st.full_reads, false)
                        }
                        "cindex" | "cvindex" | "cscan" | "cvscan" => {
                            let sub = compacted[kind.starts_with("cv") as usize].as_ref().unwrap();
                            let pr = prep2(sub, q);
                            if kind.ends_with("scan") {
                                let r = scan_visible(sub, &pr, t0, None, false, false);
                                (to_bitmap2(sub, &r.rows), r.st.full_reads, false)
                            } else {
                                let r = threshold2(sub, &pr, t0, &Opts2 { vis: None, verify: false, want_iv: false, plan: plan_flags(plan, flags) });
                                (to_bitmap2(sub, &r.rows), r.st.full_reads, r.scanned)
                            }
                        }
                        // timing only: the query's preparation, and with the anchor search
                        "prep" | "anc" => {
                            let pr = prep2(ix, q);
                            if kind == "anc" {
                                std::hint::black_box(pr.anc(ix));
                            }
                            (RoaringBitmap::new(), 0, false)
                        }
                        "flat" if on_file => {
                            let pr = prep2(ix, q);
                            let r = scan_visible(ix, &pr, t0, vis, false, plan.pscan);
                            (to_bitmap2(ix, &r.rows), r.st.full_reads, false)
                        }
                        "flat" => {
                            let (q16, q32, qn) = crate::v2::q::query_f16(q);
                            let mut e = fl.threshold(&q16, &q32, t0, full_err_qn(ix, qn), bm.as_ref(), &c.ent_xs);
                            e.sort_unstable();
                            let n = e.len() as u64;
                            (RoaringBitmap::from_sorted_iter(e).unwrap(), n, false)
                        }
                        k => panic!("unknown --cmp-what variant {k:?}; use flat, scan, index[:FLAGS], idx[:FLAGS] (or forced), cindex[:FLAGS], cvindex[:FLAGS], cscan, cvscan, prep or anc"),
                    })
                };
                let answers = |wi: usize| !matches!(whats[wi].split(':').next().unwrap(), "prep" | "anc");
                if cold {
                    // the first query at each answer size, after the full vectors are dropped from the page cache
                    let qi = 0;
                    for wi in 0..whats.len() {
                        for si in 0..ns {
                            if let Some(r) = fullrows {
                                r.evict();
                            }
                            let (io0, mf0, c0) = (disk::io_read_bytes(), disk::major_faults(), disk::cpu_s());
                            let t = Timer::new();
                            let (ans, fr, _) = run(wi, &qs[qi], thr[qi * ns + si]);
                            let ms = t.ms();
                            let (io1, mf1, c1) = (disk::io_read_bytes(), disk::major_faults(), disk::cpu_s());
                            if answers(wi) {
                                check((vi, qi, si), &labels[wi], &ans, th, vname);
                            }
                            lines.push(format!("{},{th},{vname},{si},{qi},{ms:.4},{ms:.4},{},{fr},{:.1},{},{:.2},cold", labels[wi], ans.len(), (io1 - io0) as f64 / 1024.0, mf1 - mf0, (c1.1 - c0.1) * 1e3));
                        }
                    }
                }
                let mut dumpf: Vec<_> = labels.iter().map(|l| (!dump.is_empty() && th == 1).then(|| std::io::BufWriter::new(std::fs::File::create(format!("{dump}/{}_{vname}.bin", l.replace([':', '@', '='], "_"))).unwrap()))).collect();
                let mut nscan = vec![0usize; whats.len()];
                for pass in &passes {
                    let (bio0, bt0, cpu0, w0, nr0, nb0) = (disk::io_read_bytes(), Timer::new(), disk::cpu_s(), disk::WAIT_NS.load(Relaxed), disk::NREADS.load(Relaxed), disk::NBYTES.load(Relaxed));
                    for (qi, q) in qs.iter().enumerate() {
                        if qi % step != 0 {
                            continue;
                        }
                        for si in 0..ns {
                            let t0 = thr[qi * ns + si];
                            for &wi in pass {
                                let mut ts = Vec::new();
                                let (mut ans, mut fr) = (RoaringBitmap::new(), 0);
                                let (io0, mf0, c0) = (disk::io_read_bytes(), disk::major_faults(), disk::cpu_s());
                                for _ in 0..reps {
                                    let t = Timer::new();
                                    let scanned;
                                    (ans, fr, scanned) = run(wi, q, t0);
                                    ts.push(t.ms());
                                    nscan[wi] += scanned as usize;
                                }
                                let (io1, mf1, c1) = (disk::io_read_bytes(), disk::major_faults(), disk::cpu_s());
                                if answers(wi) {
                                    check((vi, qi, si), &labels[wi], &ans, th, vname);
                                }
                                if let Some(w) = dumpf[wi].as_mut() {
                                    for x in [qi as u32, si as u32, ans.len() as u32].into_iter().chain(ans.iter()) {
                                        w.write_all(&x.to_le_bytes()).unwrap();
                                    }
                                }
                                let mn = ts.iter().cloned().fold(f64::MAX, f64::min);
                                lines.push(format!("{},{th},{vname},{si},{qi},{:.4},{mn:.4},{},{fr},{:.1},{:.1},{:.2},warm", labels[wi], median(&ts), ans.len(), (io1 - io0) as f64 / 1024.0 / reps as f64, (mf1 - mf0) as f64 / reps as f64, (c1.1 - c0.1) * 1e3 / reps as f64));
                            }
                        }
                    }
                    let res = fullrows.map_or(0, |r| r.resident());
                    let cpu1 = disk::cpu_s();
                    let ld = |a: &std::sync::atomic::AtomicU64, b: u64| a.load(Relaxed) - b;
                    let names: Vec<&str> = pass.iter().map(|&wi| labels[wi].as_str()).collect();
                    out!("[disk] {} vis {vname}: wall {:.1}s, cpu user {:.1}s sys {:.1}s, waiting for reads {:.1}s; {} reads of {:.0} MiB", names.join(","), bt0.s(), cpu1.0 - cpu0.0, cpu1.1 - cpu0.1, ld(&disk::WAIT_NS, w0) as f64 * 1e-9, ld(&disk::NREADS, nr0), ld(&disk::NBYTES, nb0) as f64 / (1 << 20) as f64);
                    for &wi in pass {
                        let label = &labels[wi];
                        let mine: Vec<f64> = lines.iter().filter(|l| l.starts_with(&format!("{label},{th},{vname},")) && l.ends_with(",warm")).map(|l| l.split(',').nth(5).unwrap().parse().unwrap()).collect();
                        out!("dbcmp {label} threads {th} vis {vname}: {} timings, median {:.3} ms; {} of {} runs scanned; read {:.0} MiB from storage in {:.1}s; full vectors resident {:.1} MiB; {}", mine.len(), median(&mine), nscan[wi], mine.len() * reps,
                            (disk::io_read_bytes() - bio0) as f64 / (1 << 20) as f64, bt0.s(), res as f64 / (1 << 20) as f64, disk::cg_report());
                    }
                    std::fs::write(format!("{}/dbcmp_{}.csv", res_dir(), arg_str("--tag", "run")), lines.join("\n") + "\n").unwrap();
                }
            }
        }
    }
    out!("dbcmp answers: {ncheck} compared with the first variant's, all identical");
    std::fs::write(format!("{}/dbcmp_{}.csv", res_dir(), arg_str("--tag", "run")), lines.join("\n") + "\n").unwrap();
    let st = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    out!("dbcmp resident after the queries: {}", st.lines().filter(|l| l.starts_with("VmRSS") || l.starts_with("RssFile") || l.starts_with("RssAnon")).collect::<Vec<_>>().join(", "));
}

/// Top-k on the threshold-db queries, one thread: for every `--step`-th query, each visible set in `--vis-sets`
/// (`all`, or a file of sorted int32 entity ids) and each k in `--ks`, the exact k-NN path (knn2, unless `--no-exact`)
/// and the approximate mode at the cheapest setting reaching each mean recall in `--targets`. Recall is |returned ∩
/// the first k visible entries of `--order`| / k. The approximate ladder doubles the cluster count up to every cluster
/// for each dial; each dial's first rung reaching a target is timed at `--reps` and the fastest is kept.
/// The approximate mode (knn2_fast) walks the dial off and the dials fitted for this k at each eps in `--dial-eps` on
/// the pseudo-queries. With `--params FILE` (a
/// liminal_topk.csv), the approximate mode runs only at the settings listed there and is not timed.
/// Rows go to RESULTS/liminal_topk.csv (all) or liminal_topk_<set>.csv, param "nprobe:dial"; per query, the items
/// touched, entering each level, full vectors read and bytes read of the chosen settings go to
/// RESULTS/liminal_topk_stats.csv.
pub fn dbtopk(c: &Ctx2) {
    let ix = &c.ix;
    let order = read_npy(&arg_str("--order", ""));
    let ncol = order.shape[1];
    let order = order.i32s();
    let qs = &c.texts;
    let reps = arg_usize("--reps", 5);
    let step = arg_usize("--step", 10);
    let ks: Vec<usize> = arg_str("--ks", "1,10,100,1000").split(',').map(|x| x.parse().unwrap()).collect();
    let targets: Vec<f64> = arg_str("--targets", "0.8,0.99").split(',').map(|x| x.parse().unwrap()).collect();
    let eps_list: Vec<f64> = arg_str("--dial-eps", "0.001,0.01,0.03,0.1").split(',').map(|x| x.parse().unwrap()).collect();
    let fixed: Option<Vec<(String, usize, String)>> = Some(arg_str("--params", "")).filter(|f| !f.is_empty()).map(|f| {
        std::fs::read_to_string(&f).unwrap().lines().skip(1).filter_map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            (v[0] == "liminal_approx").then(|| (v[1].to_string(), v[2].parse().unwrap(), v[4].to_string()))
        }).collect()
    });
    let k_cl = ix.k();
    let nl = ix.meta.nl();
    let pl = pool(1);
    let open = |name: &str, head: &str| {
        let path = format!("{}/{name}", res_dir());
        let fresh = !std::path::Path::new(&path).exists();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
        if fresh {
            writeln!(f, "{head}").unwrap();
        }
        f
    };
    let mut fs = open("liminal_topk_stats.csv", &format!("method,target,k,vis,query,param,recall,touched,full_reads,bytes,groups,clusters_indexed,{}",
        (0..nl).map(|l| format!("enter{l}")).collect::<Vec<_>>().join(",")));
    for vspec in arg_str("--vis-sets", "all").split(',') {
        let (vname, bm) = if vspec == "all" { ("all".to_string(), None) } else {
            let ids: Vec<u32> = read_npy(vspec).i32s().iter().map(|&x| x as u32).collect();
            (std::path::Path::new(vspec).file_stem().unwrap().to_string_lossy().to_string(), Some(RoaringBitmap::from_sorted_iter(ids).unwrap()))
        };
        let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
        let vis = vis.as_ref();
        let mut f = open(&if vname == "all" { "liminal_topk.csv".to_string() } else { format!("liminal_topk_{vname}.csv") }, "method,target,k,query,param,median_ms,recall");
        type Row = (usize, f64, f64);
        for &k in &ks {
            let exact: Vec<(usize, std::collections::HashSet<u32>)> = (0..qs.len()).step_by(step).map(|qi| {
                let e: std::collections::HashSet<u32> = order[qi * ncol..(qi + 1) * ncol].iter().map(|&x| x as u32)
                    .filter(|&x| bm.as_ref().map_or(true, |b| b.contains(x))).take(k).collect();
                assert_eq!(e.len(), k, "--order holds fewer than k visible entries for query {qi}");
                (qi, e)
            }).collect();
            let eval = |run: &(dyn Fn(&[f32]) -> KnnOut2 + Sync), r: usize| -> Vec<Row> {
                exact.iter().map(|(qi, es)| {
                    let mut ts = Vec::new();
                    let mut got = Vec::new();
                    for _ in 0..r {
                        let t = Timer::new();
                        got = pl.install(|| run(&qs[*qi])).ents;
                        ts.push(t.ms());
                    }
                    (*qi, median(&ts), got.iter().filter(|x| es.contains(&x.1)).count() as f64 / k as f64)
                }).collect()
            };
            let summ = |r: &[Row]| (median(&r.iter().map(|x| x.1).collect::<Vec<_>>()), mean(&r.iter().map(|x| x.2).collect::<Vec<_>>()));
            let mut emit = |method: &str, target: &str, param: &str, r: &[Row]| {
                let (ms, rec) = summ(r);
                out!("dbtopk {vname} {method} {target} k {k}: {param}: median {ms:.3} ms, mean recall {rec:.4}");
                for &(qi, ms, rec) in r {
                    writeln!(f, "{method},{target},{k},{qi},{param},{ms:.4},{rec:.5}").unwrap();
                }
            };
            if !args().iter().any(|a| a == "--no-exact") {
                let r = eval(&|q| knn2(ix, q, k, vis, c.plan), reps);
                emit("liminal_exact", "exact", "-", &r);
            }
            // the dials of the ladder, by name
            let dials: Vec<(String, Option<Vec<f32>>)> = {
                let t = Timer::new();
                // the fit's queries: the setup's pseudo-queries, or with --dial-from queries the queries that are not timed
                let held: Vec<(usize, Vec<f32>, Vec<u32>)> = (0..qs.len()).filter(|qi| qi % step != 0).map(|qi| (usize::MAX, qs[qi].clone(), Vec::new())).collect();
                let fq = if arg_str("--dial-from", "pseudo") == "queries" { &held[..] } else { &c.pseudo[..c.pseudo.len().min(arg_usize("--pseudo-dial", 200))] };
                let d = Dial::fit(ix, fq, c.plan, true, k, &c.approx);
                out!("[dial k {k}] fitted on {} queries in {:.1}s: eps {:?}, their recall@{k} over every cluster {:?}", fq.len(), t.s(), d.eps, d.recall.iter().map(|x| (x * 1e4).round() / 1e4).collect::<Vec<_>>());
                for (e, fa) in d.eps.iter().zip(&d.factors) {
                    out!("[dial k {k}] eps {e}: factors by level {:?}", fa.iter().map(|x| (x * 1e3).round() / 1e3).collect::<Vec<_>>());
                }
                std::iter::once(("off".to_string(), None)).chain(eps_list.iter().map(|&e| (format!("e{e}"), Some(d.for_eps(e).to_vec())))).collect()
            };
            let run = |np: usize, dial: Option<&[f32]>, q: &[f32]| knn2_fast(ix, &c.approx, q, k, vis, np, dial, c.plan);
            let dial_of = |name: &str| dials.iter().find(|d| d.0 == name).unwrap_or_else(|| panic!("no dial {name}")).1.as_deref();
            // per target: (param, rows timed or None)
            let mut chosen: Vec<Option<(String, Option<Vec<Row>>)>> = vec![None; targets.len()];
            if let Some(fx) = &fixed {
                for (ti, &tg) in targets.iter().enumerate() {
                    chosen[ti] = fx.iter().find(|x| x.1 == k && x.0.parse::<f64>().ok() == Some(tg)).map(|x| (x.2.clone(), None));
                }
            } else {
                let mut ladder: Vec<usize> = (0..).map(|i| 1usize << i).take_while(|&p| p < k_cl).collect();
                ladder.push(k_cl);
                let mut best: Vec<Option<(f64, String, Vec<Row>)>> = vec![None; targets.len()];
                for (dname, dial) in &dials {
                    let mut done = vec![false; targets.len()];
                    for &np in &ladder {
                        let (ms1, rec1) = summ(&eval(&|q| run(np, dial.as_deref(), q), 1));
                        out!("dbtopk {vname} ladder k {k}: nprobe {np} dial {dname}: median {ms1:.3} ms, mean recall {rec1:.4}");
                        let mut timed: Option<Vec<Row>> = None;
                        for (ti, &tg) in targets.iter().enumerate() {
                            if done[ti] || rec1 < tg {
                                continue;
                            }
                            done[ti] = true;
                            let r = timed.get_or_insert_with(|| eval(&|q| run(np, dial.as_deref(), q), reps)).clone();
                            let (ms, rec) = summ(&r);
                            if rec >= tg && best[ti].as_ref().map_or(true, |b| ms < b.0) {
                                best[ti] = Some((ms, format!("{np}:{dname}"), r));
                            }
                        }
                        if done.iter().all(|&d| d) {
                            break;
                        }
                    }
                }
                for (ti, b) in best.into_iter().enumerate() {
                    chosen[ti] = b.map(|(_, p, r)| (p, Some(r)));
                }
            }
            for (ti, ch) in chosen.iter().enumerate() {
                let Some((param, rows)) = ch else {
                    out!("dbtopk {vname} approx {} k {k}: not reached", targets[ti]);
                    continue;
                };
                if let Some(r) = rows {
                    emit("liminal_approx", &targets[ti].to_string(), param, r);
                }
                let (np, dname) = param.split_once(':').unwrap();
                let (np, dial): (usize, Option<&[f32]>) = (np.parse().unwrap(), dial_of(dname));
                let mut tot = vec![0f64; 6 + nl];
                // with VEXACT_PROF, the share of the query's cycles by phase
                let prof = std::env::var("VEXACT_PROF").is_ok();
                let mut cyc = [0f64; 32];
                for (qi, es) in &exact {
                    PROF.store(prof, std::sync::atomic::Ordering::Relaxed);
                    let o = pl.install(|| run(np, dial, &qs[*qi]));
                    PROF.store(false, std::sync::atomic::Ordering::Relaxed);
                    for (a, &b) in cyc.iter_mut().zip(&o.st.cyc) {
                        *a += b as f64;
                    }
                    let rec = o.ents.iter().filter(|x| es.contains(&x.1)).count() as f64 / k as f64;
                    let bytes = bytes_read2(ix, &o.st) * ix.nrows as f64;
                    let v: Vec<f64> = [rec, o.st.touched as f64, o.st.full_reads as f64, bytes, o.st.groups as f64, o.st.clusters_open as f64].into_iter().chain(o.st.enter[..nl].iter().map(|&x| x as f64)).collect();
                    writeln!(fs, "liminal_approx_new,{},{k},{vname},{qi},{param},{}", targets[ti],
                        v.iter().map(|x| format!("{x:.4}")).collect::<Vec<_>>().join(",")).unwrap();
                    for (a, b) in tot.iter_mut().zip(&v) {
                        *a += b / exact.len() as f64;
                    }
                }
                out!("dbtopk {vname} stats {} k {k} {param}: recall {:.4} touched {:.0} full {:.0} bytes {:.0} groups {:.1} clusters indexed {:.1}; entering by level {:?}", targets[ti], tot[0], tot[1], tot[2], tot[3], tot[4], tot[5],
                    tot[6..].iter().map(|x| x.round()).collect::<Vec<_>>());
                if prof {
                    let q = cyc[P_QUERY].max(1.0);
                    let lv: f64 = [P_INIT, P_TABLES, P_REPACK, P_DECODE, P_BOUNDS, P_COMPACT, P_LVSETUP].iter().map(|&p| cyc[p]).sum();
                    let sh = |x: f64| (x / q * 1000.0).round() / 10.0;
                    out!("dbtopk {vname} phases {} k {k} {param} (% of the query): order {} groups {} cluster setup {} tables {} levels {} (of which item work {}) full vectors {} rest {}", targets[ti],
                        sh(cyc[P_PLAN]), sh(cyc[P_BASIS] + cyc[P_GPOST]), sh(cyc[P_CLSETUP]), sh(cyc[P_TABLES]), sh(cyc[P_RUNCL]), sh(lv - cyc[P_TABLES]), sh(cyc[P_FULL]),
                        sh(q - cyc[P_PLAN] - cyc[P_BASIS] - cyc[P_GPOST] - cyc[P_CLSETUP] - cyc[P_RUNCL] - cyc[P_FULL]));
                }
            }
        }
    }
}

/// The approximate mode as top-m on the threshold-db queries, in ann.py's form: for every `--step`-th query and each
/// share in `--shares`, m is the size of the exact answer (the first k of `--order`, within the visible set), the query
/// asks knn2_fast for the m best, and recall is |returned ∩ exact| / m. `tight` walks a doubling ladder of cluster
/// counts (from `--ladder-from`, at one repetition) for the dial off and for the dials fitted for k = the share's answer
/// size over the whole corpus at each eps in `--dial-eps`, on the pseudo-queries or, with `--dial-from queries`, on the
/// queries that are not timed (k capped at `--dial-k-max`); the first rung of each dial reaching mean recall 0.99 is
/// timed at `--reps` and the fastest kept, and short of 0.99, the fastest rung with the best recall seen within the cap.
/// The ladder stops at the first rung whose median query exceeds `--cap-ms` (5000), or whose first query of each kind
/// each exceed twice that. A tight rung slower than `--slow-ms` (1000) per query keeps its ladder timing. Each set name
/// in `--sets` other than `all` is the file stem of one of the `--vis` files. One thread, pinned by `--pin`. Rows are
/// appended to RESULTS/liminal_approx.csv as each setting finishes, in ann.csv's columns; param is "nprobe:dial".
pub fn dbknn(c: &Ctx2) {
    let ix = &c.ix;
    let order = read_npy(&arg_str("--order", ""));
    let ncol = order.shape[1];
    let order = order.i32s();
    let qs = &c.texts;
    let n = ix.nrows;
    let ks: Vec<usize> = [1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75].iter().map(|s| (s * n as f64).round() as usize).collect();
    let reps = arg_usize("--reps", 5);
    let step = arg_usize("--step", 10);
    let k_cl = ix.k();
    let eps_list: Vec<f64> = arg_str("--dial-eps", "0.001,0.01,0.03,0.1").split(',').map(|x| x.parse().unwrap()).collect();
    let held: Vec<(usize, Vec<f32>, Vec<u32>)> = (0..qs.len()).filter(|qi| qi % step != 0).map(|qi| (usize::MAX, qs[qi].clone(), Vec::new())).collect();
    let fq = if arg_str("--dial-from", "pseudo") == "queries" { &held[..] } else { &c.pseudo[..c.pseudo.len().min(arg_usize("--pseudo-dial", 200))] };
    // the dials by fitted k, fitted once and shared by every visible set
    let mut fitted: std::collections::HashMap<usize, Vec<(String, Option<Vec<f32>>)>> = std::collections::HashMap::new();
    let pl = pool(1);
    for spec in arg_str("--sets", "all:0,1,2,3").split(';') {
        let (vname, shares) = spec.split_once(':').unwrap();
        let bm: Option<RoaringBitmap> = (vname != "all").then(|| {
            let vf = arg_str("--vis", "");
            let f = vf.split(',').find(|f| std::path::Path::new(f).file_stem().map_or(false, |s| s == vname)).unwrap_or_else(|| panic!("no --vis file named {vname}"));
            let ids: Vec<u32> = read_npy(f).i32s().iter().map(|&x| x as u32).collect();
            RoaringBitmap::from_sorted_iter(ids).unwrap()
        });
        let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
        for si in shares.split(',').map(|x| x.parse::<usize>().unwrap()) {
            let k = ks[si].min(ncol); // 0.75 N rounds half to even in prep.py, to the stored column count
            let exact: Vec<(usize, Vec<u32>)> = (0..qs.len()).step_by(step).map(|qi| {
                let e: Vec<u32> = order[qi * ncol..qi * ncol + k].iter().map(|&x| x as u32).filter(|&x| bm.as_ref().map_or(true, |b| b.contains(x))).collect();
                (qi, e)
            }).filter(|x| !x.1.is_empty()).collect();
            // (qi, median ms, recall, returned, nonmatch, m) for one setting
            let cap = arg_str("--cap-ms", "5000").parse::<f64>().unwrap();
            let slow = arg_str("--slow-ms", "1000").parse::<f64>().unwrap();
            // a probe takes one query of each kind first (image, text_q, text_insample) and gives up, returning
            // nothing, when those three each take over twice the cap
            let eval_p = |np: usize, dial: Option<&[f32]>, r: usize, probe: bool| -> Vec<(usize, f64, f64, usize, usize, usize)> {
                let mut idx: Vec<usize> = (0..exact.len()).collect();
                if probe {
                    idx.sort_by_key(|&i| (exact[i].0 % 50 != 0, exact[i].0));
                }
                let mut res = Vec::new();
                for i in idx {
                    if probe && res.len() == 3 && res.iter().all(|x: &(usize, f64, f64, usize, usize, usize)| x.1 > 2.0 * cap) {
                        return Vec::new();
                    }
                    let (qi, e) = &exact[i];
                    let m = e.len();
                    let mut ts = Vec::new();
                    let mut got = Vec::new();
                    for _ in 0..r {
                        let t = Timer::new();
                        let out = pl.install(|| knn2_fast(ix, &c.approx, &qs[*qi], m, vis.as_ref(), np, dial, c.plan));
                        got = out.ents.iter().map(|x| x.1).collect::<Vec<u32>>();
                        ts.push(t.ms());
                    }
                    let es: std::collections::HashSet<u32> = e.iter().copied().collect();
                    let hit = got.iter().filter(|x| es.contains(x)).count();
                    res.push((*qi, median(&ts), hit as f64 / m as f64, got.len(), got.len() - hit, m));
                }
                res
            };
            let eval = |np: usize, dial: Option<&[f32]>, r: usize| eval_p(np, dial, r, false);
            let summ = |r: &[(usize, f64, f64, usize, usize, usize)]| (median(&r.iter().map(|x| x.1).collect::<Vec<_>>()), mean(&r.iter().map(|x| x.2).collect::<Vec<_>>()));
            let emit = |setting: &str, np: usize, dname: &str, r: &[(usize, f64, f64, usize, usize, usize)]| {
                let (ms, rec) = summ(r);
                out!("dbknn {setting} {vname} share {si}: nprobe {np} dial {dname}: median {ms:.3} ms, mean recall {rec:.4}");
                let path = format!("{}/liminal_approx.csv", res_dir());
                let fresh = !std::path::Path::new(&path).exists();
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
                if fresh {
                    writeln!(f, "method,setting,vis,share_index,query,param,median_ms,recall,returned,nonmatch,exact").unwrap();
                }
                for &(qi, ms, rec, ret, bad, m) in r {
                    writeln!(f, "liminal,{setting},{vname},{si},{qi},{np}:{dname},{ms:.4},{rec:.5},{ret},{bad},{m}").unwrap();
                }
            };
            let mut ladder: Vec<usize> = (0..).map(|i| 1usize << i).take_while(|&p| p < k_cl).filter(|&p| p >= arg_usize("--ladder-from", 1)).collect();
            ladder.push(k_cl);
            let mut best: Option<(f64, usize, &str, Vec<(usize, f64, f64, usize, usize, usize)>)> = None;
            // the fit's verified threshold queries cost too much past a few thousand items, so larger k share the dial of `--dial-k-max`
            let kf = k.min(arg_usize("--dial-k-max", usize::MAX));
            if !fitted.contains_key(&kf) {
                let k = kf;
                let t = Timer::new();
                let d = Dial::fit(ix, fq, c.plan, false, k, &c.approx);
                out!("[dial k {k}] fitted on {} queries in {:.1}s", fq.len(), t.s());
                for &e in &eps_list {
                    out!("[dial k {k}] eps {e}: factors by level {:?}", d.for_eps(e).iter().map(|x| (x * 1e3).round() / 1e3).collect::<Vec<_>>());
                }
                fitted.insert(k, std::iter::once(("off".to_string(), None)).chain(eps_list.iter().map(|&e| (format!("e{e}"), Some(d.for_eps(e).to_vec())))).collect());
            }
            let dials: Vec<(&str, Option<&[f32]>)> = fitted[&kf].iter().map(|(n, d)| (n.as_str(), d.as_deref())).collect();
            for (dname, dial) in dials {
                // the first rung reaching 0.99, else the fastest with the best recall seen
                let mut within: Option<(usize, Vec<(usize, f64, f64, usize, usize, usize)>)> = None;
                for &np in &ladder {
                    let r1 = eval_p(np, dial, 1, true);
                    if r1.is_empty() {
                        out!("dbknn ladder {vname} share {si}: nprobe {np} dial {dname}: over the cap on its first three queries");
                        break;
                    }
                    let (ms, rec) = summ(&r1);
                    out!("dbknn ladder {vname} share {si}: nprobe {np} dial {dname}: median {ms:.3} ms, mean recall {rec:.4}");
                    if ms > cap {
                        break;
                    }
                    let better = within.as_ref().map_or(true, |w| {
                        let (wms, wrec) = summ(&w.1);
                        rec > wrec || (rec == wrec && ms < wms)
                    });
                    if better {
                        within = Some((np, r1));
                    }
                    if rec >= 0.99 {
                        break;
                    }
                }
                if let Some((np, r1)) = within {
                    let r = if summ(&r1).0 > slow {
                        out!("dbknn tight {vname} share {si}: timed at one repetition");
                        r1
                    } else {
                        eval(np, dial, reps)
                    };
                    let (ms, rec) = summ(&r);
                    // a dial reaching 0.99 wins over one that does not; short of it, the best recall wins
                    let key = if rec >= 0.99 { ms } else { 1e12 * (2.0 - rec) + ms };
                    if best.as_ref().map_or(true, |b| key < b.0) {
                        best = Some((key, np, dname, r));
                    }
                }
            }
            match best {
                Some((_, np, dname, r)) => emit("tight", np, dname, &r),
                None => out!("dbknn tight {vname} share {si}: no rung within the cap"),
            }
        }
    }
}
