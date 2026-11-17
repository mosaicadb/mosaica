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
    pub row_ent: Vec<i64>, // index row -> entity
    pub ent_xs: Vec<u32>,  // entity -> index row
    pub items: Vec<Vec<f32>>,
    pub texts: Vec<Vec<f32>>,
    pub plan: Plan,
    pub approx: Approx,
    pub pseudo: Vec<(usize, Vec<f32>, Vec<u32>)>,
}

/// The flags that take a value, the flags that stand alone, and the modes. Anything else on the command line is refused,
/// so that a removed or misspelt flag cannot be ignored silently, and so is a flag given where it would do nothing
/// (`check_args_of`). (`--int` and `--gtab` restate the default.)
const VALUE_FLAGS: &[&str] = &[
    "--anchors", "--b", "--basis", "--bfracs", "--c", "--cal-items", "--cell-sample", "--cells", "--cert-time", "--cap", "--cap-ms", "--clusters", "--cmp-order",
    "--cmp-vis", "--cmp-what", "--core", "--cross", "--d", "--ddec", "--dims", "--ds", "--dial", "--dial-eps", "--dial-from", "--dial-k-max", "--direct-min", "--dlv",
    "--dontcache-min", "--dump", "--filters", "--flags", "--fnq", "--full-advice", "--full-file", "--full-read",
    "--groups", "--index", "--index-b", "--index-c", "--index-d", "--item-queries", "--k", "--knn", "--knn-k", "--ks",
    "--ladder-from", "--levels", "--lnd", "--methods", "--n", "--name", "--nguar", "--niter", "--nq", "--onq",
    "--order", "--out", "--params", "--passes", "--per-cluster", "--pin", "--plan", "--plan-file", "--profile", "--pseudo",
    "--pseudo-dial", "--queries", "--query-sample", "--read-modes", "--reps", "--results", "--rlv", "--runs", "--sample-per-centre", "--scale",
    "--seed", "--sets", "--sizes", "--slow-ms", "--step", "--tag", "--targets", "--text-anchors", "--text-count", "--thr", "--threads", "--time-cap",
    "--vcheck", "--vectors", "--vfilt", "--vis", "--vis-sets", "--vitem", "--vtext", "--what",
];
const BOOL_FLAGS: &[&str] = &[
    "--approx-filtered", "--cold", "--csg-plain", "--design-exact", "--flat", "--gtab", "--hash", "--in-memory", "--int", "--keep-index",
    "--no-all", "--no-approx", "--no-exact", "--nusort", "--prof", "--rebuild",
    "--skip-all", "--sweep", "--v2", "--willneed",
];
const MODES: &[&str] = &[
    "ann", "ann-truth", "breakdown", "bytesq", "cert-layouts", "certify-layout", "create", "dbcmp", "dbknn", "dbtopk", "e2e",
    "filter", "levels", "oracle", "partition", "qvbench", "scanbench", "setup", "verify",
];
/// The flags that make an index (`v2::BuildOptions`): given only where one is made, with `create`, `--rebuild` or
/// `--in-memory`.
const BUILD_FLAGS: &[&str] = &[
    "--per-cluster", "--groups", "--basis", "--query-sample", "--seed", "--cert-time", "--anchors", "--text-anchors", "--text-count",
    "--profile", "--clusters", "--rlv", "--dlv", "--design-exact", "--nusort", "--nguar", "--csg-plain", "--cal-items",
];
/// Of those, the ones that make the partition and the fit: `create` only (a rebuild re-encodes the fit as it is).
const FIT_FLAGS: &[&str] = &["--per-cluster", "--groups", "--basis", "--query-sample", "--cert-time"];
/// Of those, the ones that make the support table's anchors (with `--seed`, the only thing a rebuild seeds).
const ANCHOR_FLAGS: &[&str] = &["--anchors", "--text-anchors", "--text-count"];
/// The certificate's tools (`cert_cmd`): each mode with the flags it takes (it runs alone, with these only).
const CERT_MODES: &[(&str, &[&str])] = &[("certify-layout", &["--ds", "--lnd", "--time-cap"]), ("cert-layouts", &["--dims"])];
/// The partition alone (`partition_cmd`): the flags it takes, and of those the ones that are its only.
const PARTITION_FLAGS: &[&str] = &["--vectors", "--out", "--cells", "--cross", "--per-cluster", "--seed", "--sample-per-centre", "--cell-sample", "--niter"];
const PARTITION_ONLY: &[&str] = &["--out", "--cells", "--cross", "--sample-per-centre", "--cell-sample", "--niter"];
/// The flags `create` takes besides the build flags.
const CREATE_FLAGS: &[&str] = &["--vectors", "--index", "--hash"];

/// Exits with a message naming any argument that `check_args_of` refuses.
pub fn check_args() {
    if let Err(e) = check_args_of(&args()) {
        exit2(&e);
    }
}

/// `a` (the command line, the program first) checked: every argument a mode, a flag or a flag's value (no positional
/// arguments); `create` alone with `--vectors` and `--index`; build flags only where an index is made; the removed
/// commands named as such.
pub fn check_args_of(a: &[String]) -> Result<(), String> {
    let (mut bad, mut modes, mut flags, mut i) = (Vec::new(), Vec::new(), Vec::new(), 1);
    while i < a.len() {
        let x = a[i].as_str();
        if VALUE_FLAGS.contains(&x) {
            if i + 1 >= a.len() {
                return Err(format!("{x} needs a value"));
            }
            flags.push(x);
            i += 2;
            continue;
        }
        if BOOL_FLAGS.contains(&x) {
            flags.push(x);
        } else if MODES.contains(&x) {
            modes.push(x);
        } else {
            bad.push(x.to_string());
        }
        i += 1;
    }
    if bad.iter().any(|b| ["fit", "build", "anchors"].contains(&b.as_str())) {
        return Err(format!("unknown arguments {bad:?}: the fit, build and anchors commands are gone. vexact --vectors VECTORS.npy --index DIR create [build flags] makes an index (partition, fit, anchors, encoding); vexact --v2 --index DIR --rebuild [build flags] re-encodes one (anchors with --anchors, --text-anchors, --text-count)"));
    }
    if !bad.is_empty() {
        return Err(format!("unknown arguments {bad:?}: give modes from {MODES:?}, and flags from {BOOL_FLAGS:?} or, each followed by its value, {VALUE_FLAGS:?}"));
    }
    let has = |f: &str| flags.contains(&f);
    for (m, takes) in CERT_MODES {
        if modes.contains(m) {
            if modes.len() > 1 || flags.iter().any(|f| !takes.contains(f)) {
                return Err(format!("{m} runs alone, with {takes:?} only"));
            }
            if *m == "certify-layout" && (!has("--ds") || !has("--lnd")) {
                return Err("certify-layout needs --ds D1,D2,.. (2 to 4 block dimensions) and --lnd LN_DELTA".into());
            }
            let val = |f: &str| a.iter().position(|x| x == f).map(|i| a[i + 1].as_str());
            if let Some(v) = val("--time-cap") {
                if !v.parse::<f64>().is_ok_and(|c| c >= 0.0) {
                    return Err(format!("--time-cap is a number of seconds, at least 0, not {v}"));
                }
            }
            if let Some(v) = val("--ds") {
                if !v.split(',').all(|d| d.trim().parse::<usize>().is_ok_and(|d| d >= 4)) {
                    return Err(format!("--ds is 2 to 4 block dimensions of at least 4 each, not {v}"));
                }
            }
            if let Some(v) = val("--lnd") {
                if !v.parse::<f64>().is_ok_and(|l| l < 0.0 && l.is_finite()) {
                    return Err(format!("--lnd is ln delta, a negative number, not {v}"));
                }
            }
        } else if let Some(f) = flags.iter().find(|f| takes.contains(f)) {
            return Err(format!("{f} is {m}'s (vexact {m} ...)"));
        }
    }
    if modes.contains(&"partition") {
        if modes.len() > 1 || flags.iter().any(|f| !PARTITION_FLAGS.contains(f)) {
            return Err(format!("partition runs alone, with {PARTITION_FLAGS:?} only"));
        }
        if !has("--vectors") || !has("--out") {
            return Err("partition needs --vectors VECTORS.npy (N x n0, row i is entity i) and --out DIR".into());
        }
        let val = |f: &str| a.iter().position(|x| x == f).map(|i| a[i + 1].as_str());
        for f in ["--cells", "--cross", "--sample-per-centre", "--cell-sample", "--niter"] {
            if let Some(v) = val(f) {
                if !v.parse::<usize>().is_ok_and(|x| x >= 1 || f == "--niter") {
                    return Err(format!("{f} is a whole number{}, not {v}", if f == "--niter" { "" } else { " of at least 1" }));
                }
            }
        }
        if let Some(v) = val("--per-cluster") {
            PerCluster::parse(v).map_err(|e| format!("--per-cluster: {e}"))?;
        }
        if let Some(v) = val("--seed") {
            v.parse::<u64>().map_err(|_| format!("--seed is a whole number, not {v}"))?;
        }
        return Ok(());
    }
    if let Some(f) = flags.iter().find(|f| PARTITION_ONLY.contains(f)) {
        return Err(format!("{f} is partition's (vexact --vectors VECTORS.npy partition --out DIR {f} ...); create partitions at the defaults"));
    }
    let create = modes.contains(&"create");
    if create {
        if modes.len() > 1 || has("--v2") || has("--rebuild") || has("--in-memory") {
            return Err(format!("create makes an index and does nothing else (not with {:?}); run experiments on it afterwards (vexact --v2 --index DIR MODE)", modes.iter().copied().filter(|m| *m != "create").chain(flags.iter().copied().filter(|f| ["--v2", "--rebuild", "--in-memory"].contains(f))).collect::<Vec<_>>()));
        }
        if !has("--vectors") || !has("--index") {
            return Err("create needs --vectors VECTORS.npy (N x n0, row i is entity i) and --index DIR (a new directory)".into());
        }
        let other: Vec<&str> = flags.iter().copied().filter(|f| !BUILD_FLAGS.contains(f) && !CREATE_FLAGS.contains(f)).collect();
        if !other.is_empty() {
            return Err(format!("create takes build flags and {CREATE_FLAGS:?} only, not {other:?}: they do nothing there; give them to a run on the index afterwards (vexact --v2 --index DIR MODE)"));
        }
    } else if has("--vectors") {
        return Err("--vectors is create's: vexact --vectors VECTORS.npy --index DIR create".into());
    }
    if has("--rebuild") && has("--in-memory") {
        return Err("--rebuild saves the index it encodes, --in-memory saves nothing: give one".into());
    }
    // the seed is the partition's, the fit's and the anchors': a rebuild uses it only to make anchors, a build in
    // memory not at all
    if has("--seed") && (has("--in-memory") || !create && !ANCHOR_FLAGS.iter().any(|f| has(f))) {
        return Err(if has("--in-memory") {
            "--seed does nothing with --in-memory (the seed is the partition's, the fit's and the anchors'; a build in memory takes the fit's anchors)".to_string()
        } else {
            format!("--seed does nothing here: a rebuild uses it only to make anchors ({ANCHOR_FLAGS:?}); the partition's and the fit's seed is create's")
        });
    }
    // qvbench takes --groups (the groups it times), and nothing else of the build
    let qv = modes.contains(&"qvbench");
    let builds = create || has("--rebuild") || has("--in-memory");
    for f in &flags {
        if (qv && *f == "--groups") || !BUILD_FLAGS.contains(f) {
            continue;
        }
        if FIT_FLAGS.contains(f) && !create {
            return Err(format!("{f} makes the partition or the fit, so it is create's (vexact --vectors VECTORS.npy --index DIR create {f} ...); a rebuild re-encodes the fit as it is"));
        }
        if !builds {
            return Err(format!("{f} is a build option: it applies where an index is made (create, --rebuild or --in-memory); an open takes the options the index was built with"));
        }
    }
    Ok(())
}

/// The certificate's tools: `certify-layout --ds D1,D2,.. --lnd LN_DELTA [--time-cap SECONDS]` certifies a layout
/// (`certify::certify_layout`, 600 s by default; its progress on stderr) and prints {"ds", "lnd", "X"} as certify.py
/// does (X null when not certified); `cert-layouts --dims N0,N0,..` prints, per dimension, the fit's layout and its
/// budgets at N = 2^10 .. 2^34, or the fit's error for it (tools/certify_fixture.py and tools/certs_table.py read
/// them, so the layout rule lives only in the fit).
pub fn cert_cmd() {
    let list = |k: &str| -> Vec<usize> {
        arg_str(k, "").split(',').filter(|s| !s.is_empty()).map(|s| s.trim().parse().unwrap_or_else(|_| exit2(&format!("{k}: {s} is not a whole number")))).collect()
    };
    if has_mode("certify-layout") {
        let ds = list("--ds");
        let lnd: f64 = arg_str("--lnd", "").parse().unwrap_or_else(|_| exit2("--lnd is a number (ln delta)"));
        let cap: f64 = arg_str("--time-cap", "600").parse().ok().filter(|c: &f64| *c >= 0.0).unwrap_or_else(|| exit2("--time-cap is a number of seconds, at least 0"));
        if let Some(d) = ds.iter().find(|&&d| d < 4) {
            exit2(&format!("--ds: a block has {d} dimensions; a certificate is for blocks of at least 4"));
        }
        let x = vexact::certify::certify_layout(&ds, lnd, cap, &mut |s| eprintln!("{s}"));
        println!("{}", serde_json::json!({"ds": ds, "lnd": lnd, "X": x}));
    } else {
        for n0 in list("--dims") {
            match vexact::fit::cert_layout(n0) {
                Ok((ds, levels)) => {
                    let lnd: Vec<(u32, f64)> = (10..=34u32).map(|p| (p, vexact::certify::ln_delta(1usize << p, levels, 1e-9))).collect();
                    println!("{}", serde_json::json!({"n0": n0, "ds": ds, "levels": levels, "lnd": lnd}));
                }
                Err(e) => println!("{}", serde_json::json!({"n0": n0, "error": e})),
            }
        }
    }
}

/// Whether `m` is given as a mode (not as a flag's value, such as a directory named like a mode).
pub fn has_mode(m: &str) -> bool {
    let a = args();
    let mut i = 1;
    while i < a.len() {
        if VALUE_FLAGS.contains(&a[i].as_str()) {
            i += 2;
            continue;
        }
        if a[i] == m {
            return true;
        }
        i += 1;
    }
    false
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

/// The index directory (`--index DIR`): made by `create`, or a fit (tools/fit.py) with its index saved by `--rebuild`.
pub fn data_dir() -> String {
    let a = args();
    match a.iter().position(|x| x == "--index") {
        Some(i) => a[i + 1].clone(),
        None => {
            eprintln!("no index: write --index DIR, a directory made by create (vexact --vectors VECTORS.npy --index DIR create) or a fit (tools/fit.py)");
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
    let (v, n0) = read_npy(path).rows_f32(path).unwrap_or_else(|e| panic!("{e}"));
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

/// A comma-separated list of byte counts from the flag `k` (none when not given).
fn arg_list(k: &str, what: &str) -> Option<Vec<usize>> {
    Some(arg_str(k, "")).filter(|v| !v.is_empty()).map(|v| v.split(',').map(|x| x.parse().unwrap_or_else(|_| exit2(&format!("{k} {what}, not {v}")))).collect())
}

/// The build options of the command line (`v2::BuildOptions`; `check_args_of` allows them only where an index is
/// made). Anchors are given when any of `--anchors`, `--text-anchors`, `--text-count` is.
pub fn build_options() -> BuildOptions {
    let d = BuildOptions::default();
    let given = |k: &str| args().iter().any(|a| a == k);
    let parse = |k: &str, what: &str| -> Option<f64> { Some(arg_str(k, "")).filter(|v| !v.is_empty()).map(|v| v.parse().unwrap_or_else(|_| exit2(&format!("{k} {what}, not {v}")))) };
    let anchors = ["--anchors", "--text-anchors", "--text-count"].iter().any(|k| given(k)).then(|| {
        let da = Anchors::default();
        Anchors { image: arg_usize("--anchors", da.image), text: Some(arg_str("--text-anchors", "")).filter(|s| !s.is_empty()), text_count: arg_usize("--text-count", da.text_count) }
    });
    BuildOptions {
        per_cluster: if given("--per-cluster") { PerCluster::parse(&arg_str("--per-cluster", "")).unwrap_or_else(|e| exit2(&format!("--per-cluster: {e}"))) } else { d.per_cluster },
        groups: if given("--groups") { crate::vexact_fit::Groups::parse(&arg_str("--groups", "")).unwrap_or_else(|e| exit2(&format!("--groups: {e}"))) } else { d.groups },
        basis: arg_str("--basis", &d.basis),
        query_sample: Some(arg_str("--query-sample", "")).filter(|s| !s.is_empty()),
        seed: arg_str("--seed", &d.seed.to_string()).parse().unwrap_or_else(|_| exit2("--seed N (a whole number)")),
        cert_time: parse("--cert-time", "SECONDS").unwrap_or(d.cert_time),
        anchors,
        profile: Some(arg_str("--profile", "")).filter(|v| !v.is_empty()),
        clusters: given("--clusters").then(|| arg_usize("--clusters", 0)),
        rlv: arg_list("--rlv", "BYTES,BYTES,... (cumulative bytes of the randomised levels)"),
        // --dlv fit: the fit's own levels
        dlv: if arg_str("--dlv", "") == "fit" { None } else { arg_list("--dlv", "BYTES,BYTES,... (cumulative bytes of the deterministic levels), or fit").or(d.dlv) },
        design_exact: given("--design-exact"),
        nusort: given("--nusort"),
        nguar: parse("--nguar", "N (items the guarantee covers)"),
        csg_plain: given("--csg-plain"),
        cal_items: arg_usize("--cal-items", d.cal_items),
        // glibc's mmap threshold fixed for the build: only in a run that builds and then runs no experiment (the
        // setting stays for the rest of the process)
        tune_malloc: !MODES.iter().any(|m| *m != "create" && has_mode(m)),
    }
}

/// The open options of the command line (`v2::OpenOptions`): `--ddec`, `--keep-index`.
pub fn open_options() -> OpenOptions {
    OpenOptions { ddec: parse_ddec(&arg_str("--ddec", "all")), keep_index: args().iter().any(|a| a == "--keep-index") }
}

/// `vexact --vectors VECTORS.npy --index DIR create [build flags] [--hash]`: make the index directory DIR from the
/// vectors (`v2::create`: partition, fit, anchors, encoding); `vexact --v2 --index DIR MODE` then opens it.
pub fn create_cmd() {
    let (vectors, dir) = (arg_str("--vectors", ""), data_dir());
    let t = Timer::new();
    let (_, ix) = create(&vectors, &dir, &build_options(), &mut |s| println!("{s}")).unwrap_or_else(|e| exit2(&e));
    if args().iter().any(|a| a == "--hash") {
        println!("[index] code hash {:016x}", ix.code_hash());
        println!("[index] store hash {:016x}", ix.store_hash());
    }
    println!("created {dir} from {vectors}: {} rows, {} clusters in {:.1}s; stored bytes/item cumulative by level: {:?}", ix.nrows, ix.k(), t.s(),
        ix.stored_bytes_per_item().iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>());
}

/// The partition alone: `vexact --vectors VECTORS.npy partition --out DIR [--cells C] [--cross T] [--per-cluster
/// by-n|M] [--seed S] [--sample-per-centre 200] [--cell-sample 1000] [--niter 20]` (`partition::partition`; the flags
/// of tools/partition.py, with its defaults).
pub fn partition_cmd() {
    let d = vexact::partition::PartitionOptions::default();
    let num = |k: &str, dv: usize| -> usize { Some(arg_str(k, "")).filter(|v| !v.is_empty()).map_or(dv, |v| v.parse().unwrap_or_else(|_| exit2(&format!("{k} is a whole number, not {v}")))) };
    let o = vexact::partition::PartitionOptions {
        cells: num("--cells", d.cells),
        cross: num("--cross", d.cross),
        per_cluster: Some(arg_str("--per-cluster", "")).filter(|v| !v.is_empty()).map_or(d.per_cluster, |v| PerCluster::parse(&v).unwrap_or_else(|e| exit2(&format!("--per-cluster: {e}")))),
        sample_per_centre: num("--sample-per-centre", d.sample_per_centre),
        cell_sample: num("--cell-sample", d.cell_sample),
        niter: num("--niter", d.niter),
        seed: arg_str("--seed", &d.seed.to_string()).parse().unwrap_or_else(|_| exit2("--seed N (a whole number)")),
    };
    let (vectors, out) = (arg_str("--vectors", ""), arg_str("--out", ""));
    vexact::partition::partition(&vectors, &out, &o, &mut |s| println!("[partition] {s}")).unwrap_or_else(|e| exit2(&e));
}

pub fn setup2(tag: &str) -> Ctx2 {
    setup2_at(tag, &data_dir(), &arg_str("--flags", ""))
}

/// The plan's switches from the command line, then from `flags` (comma-separated: int, noint, gtab, nogtab, ...); an
/// error names a flag that is not one or a value that does not parse.
pub fn plan_flags(mut p: Plan, flags: &str) -> Result<Plan, String> {
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
            // build options: an index records what it was built with, so a variant's other build needs its own
            // index directory, rebuilt with them
            f if f == "nusort" || f.starts_with("dlv=") => return Err(format!("the flag {f} changes what is built: give this variant an index directory of its own, rebuilt with it (vexact --v2 --index DIR --rebuild {}), and name it with --index-X DIR", if f == "nusort" { "--nusort".to_string() } else { format!("--dlv {}", f[4..].replace('.', ",")) })),
            f if f.starts_with("ddec=") => p.ddec = Some(parse_ddec(&f[5..])), // per query: checked by Plan::check
            // the planner's mode for a variant (as --plan for the first): auto, index (every open cluster on the index
            // path), flat (every candidate from its full vector) or scan (every query scans the visible set)
            "mode=auto" => p.mode = PlanMode::Auto,
            "mode=index" => p.mode = PlanMode::Index,
            "mode=flat" => p.mode = PlanMode::Flat,
            "mode=scan" => p.mode = PlanMode::Scan,
            // the rule's rows in memory (Plan::r_rows, r_kappa, r_seed), as a plan file gives them
            f if f.starts_with("r=") => p = Plan::from_kv(p, &format!("r_rows={}", &f[2..]))?,
            f if f.starts_with("rk=") => p = Plan::from_kv(p, &format!("r_kappa={}", &f[3..]))?,
            f if f.starts_with("rseed=") => p = Plan::from_kv(p, &format!("r_seed={}", &f[6..]))?,
            // the planner's constants from a file of name=value lines (as RESULTS/plan.txt), for a variant
            f if f.starts_with("pf=") => p = read_plan_file(p, &f[3..])?.0,
            f if f.starts_with("tol=") => p.tol = f[4..].parse().ok().filter(|x: &f64| x.is_finite() && *x >= 0.0).ok_or_else(|| format!("{f}: tol=SCORE (the tolerance d, in score units, at least 0)"))?,
            f if f.starts_with("repackx=") => {
                p.repack = true;
                p.repack_mul = f[8..].parse().ok().filter(|x: &f32| x.is_finite() && *x >= 0.0).ok_or_else(|| format!("{f}: repackx=FACTOR (a number of at least 0)"))?;
            }
            _ => return Err(format!("unknown plan flag {f:?}")),
        }
    }
    Ok(p)
}

/// `p` with the constants of the file `f` (name=value lines, as `Plan::to_kv` writes them), and whether the file gives
/// both disk constants (so that none need be timed).
pub fn read_plan_file(p: Plan, f: &str) -> Result<(Plan, bool), String> {
    let t = std::fs::read_to_string(f).map_err(|e| format!("cannot read {f}: {e}"))?;
    Ok((Plan::from_kv(p, &t).map_err(|e| format!("{f}: {e}"))?, Plan::kv_gives_disk(&t)))
}

/// `plan` (with `flags` applied, `plan_flags`) with `--plan-file`'s constants, and where `priced` (the vectors' reads
/// are priced) the disk constants from the cache of `dir` (or timed, `disk_constants`) unless the file, or a `pf=`
/// file among `flags`, gives both, so that nothing is timed or evicted then; the file's constants take precedence. A
/// plan file that cannot be read or holds another name stops the run.
pub fn plan_with_file(ix: &Index2, plan: Plan, priced: bool, dir: &str, flags: &str) -> Plan {
    let file = arg_str("--plan-file", "");
    plan_with_file_by(plan, priced, &file, flags, &mut |s| out!("{s}"), &mut |p, log| disk_constants(ix, p, dir, log))
}

/// `plan_with_file` with the plan file `file` ("": none), its lines logged by `log` and the disk constants timed (or
/// read from the cache) by `time`.
fn plan_with_file_by(plan: Plan, priced: bool, file: &str, flags: &str, log: &mut dyn FnMut(String), time: &mut dyn FnMut(Plan, &mut dyn FnMut(String)) -> Plan) -> Plan {
    let (fp, gives_disk) = if file.is_empty() { (plan, false) } else { read_plan_file(plan, file).unwrap_or_else(|e| exit2(&e)) };
    if !priced {
        return fp;
    }
    let pf = flags.split(',').filter_map(|f| f.trim().strip_prefix("pf=")).find(|f| std::fs::read_to_string(f).is_ok_and(|t| Plan::kv_gives_disk(&t)));
    if gives_disk || pf.is_some() {
        log(format!("[plan] disk constants io_us={} io_gbps={} from {}; not timed", fp.io_us, fp.io_gbps, if gives_disk { file } else { pf.unwrap() }));
        return fp;
    }
    let timed = time(plan, log);
    if file.is_empty() { timed } else { read_plan_file(timed, file).unwrap_or_else(|e| exit2(&e)).0 }
}

/// Prints `e` and stops the run with status 2.
fn exit2(e: &str) -> ! {
    eprintln!("{e}");
    std::process::exit(2);
}

/// The deterministic levels that decide, as `--ddec` and the flag `ddec=` give them: LEVEL.LEVEL... from 0, all or
/// none, as a bit set (u32::MAX: all).
pub fn parse_ddec(v: &str) -> u32 {
    match v {
        "all" => u32::MAX,
        "none" => 0,
        v => v.split('.').map(|x| match x.parse::<u32>() {
            Ok(l) if l < 32 => 1u32 << l,
            _ => {
                eprintln!("--ddec LEVEL.LEVEL... (deterministic levels from 0 that decide), all or none; not {v}");
                std::process::exit(2);
            }
        }).fold(0, |a, b| a | b),
    }
}

/// A variant's plan on another variant's opened index (`e2e`'s `--b` without `--index-b`): `base` changed by `flags`,
/// checked against `ix`. A build option among the flags (`dlv=`, `nusort`) needs another index, so it is an error.
pub fn shared_plan(base: Plan, ix: &Index2, flags: &str) -> Result<Plan, String> {
    let p = plan_flags(base, flags)?;
    p.check(ix)?;
    Ok(p)
}

/// The index of `dir` with the plan of the command line changed by `flags` (for a second variant, `--b`). The index
/// of `--index` is opened (`v2::open`), or with `--rebuild` encoded with the command line's build options and saved
/// first (`v2::rebuild`), or with `--in-memory` encoded and not saved (`v2::build`); another variant's (`--index-b`)
/// is opened.
pub fn setup2_at(tag: &str, dir: &str, flags: &str) -> Ctx2 {
    let log = std::fs::OpenOptions::new().create(true).append(true).open(format!("{}/{tag}.log", crate::run2::res_dir())).unwrap();
    let _ = LOG.set(std::sync::Mutex::new(log));
    out!("==== run at {:.0}: {:?}", now_s(), args());
    let main = dir == data_dir();
    let has = |f: &str| main && args().iter().any(|a| a == f);
    let oo = if main { open_options() } else { OpenOptions::default() };
    let topen = Timer::new();
    let mut lg = |s: String| out!("{s}");
    let (mut p, mut ix) = if has("--in-memory") {
        build(dir, &build_options(), &mut lg).and_then(|(p, mut ix)| ix.set_ddec(oo.ddec).map(|_| (p, ix)))
    } else if has("--rebuild") {
        rebuild(dir, &build_options(), &mut lg).and_then(|(p, mut ix)| ix.set_ddec(oo.ddec).map(|_| (p, ix)))
    } else {
        open(dir, &oo, &mut lg)
    }
    .unwrap_or_else(|e| exit2(&e));
    let built_s = topen.s();
    if args().iter().any(|a| a == "--hash") {
        out!("[index] code hash {:016x}", ix.code_hash());
        out!("[index] store hash {:016x}", ix.store_hash());
    }
    if args().iter().any(|a| a == "--hash") || has_mode("verify") {
        if let Err(e) = ix.check_maps() {
            eprintln!("{e}");
            std::process::exit(2);
        }
        out!("[index] row and entity maps checked");
    }
    // the experiments read the index's vectors (in index row order), not the fit's Xs.npy
    let n = p.meta.n;
    let ent_xs = ix.ent_row.to_vec();
    let row_ent: Vec<i64> = ix.row_ent.iter().map(|&e| e as i64).collect();
    // item queries: held-out rows from --item-queries, else rows of the fit listed by it (Xs.npy rows, read from the
    // index through the fit's row-to-entity map)
    let items: Vec<Vec<f32>> = if arg_str("--item-queries", "").is_empty() {
        // the fit's perm.npy, which an open does not read (a build has read it), checked against the hash the saved
        // index recorded
        if p.perm.is_empty() {
            load_perm_checked(&mut p, &saved_dir(dir)).unwrap_or_else(|e| {
                eprintln!("{e} (item queries from the fit's qi.npy need it; or give --item-queries)");
                std::process::exit(2);
            });
        }
        p.qi.iter().filter(|&&r| (r as usize) < p.cstart[p.meta.k]).map(|&r| ix.full_row_f16(ix.row_of_ent(p.perm[r as usize] as u32)).iter().map(|&h| f16_to_f32(h)).collect()).collect()
    } else {
        load_queries_from(&arg_str("--item-queries", ""), n)
    };
    let texts = load_queries(n);
    let plan = Plan {
        mode: match arg_str("--plan", "auto").as_str() {
            "auto" => PlanMode::Auto,
            "index" => PlanMode::Index,
            "flat" => PlanMode::Flat,
            "scan" => PlanMode::Scan,
            m => {
                eprintln!("--plan is auto, index, flat or scan, not {m}");
                std::process::exit(2);
            }
        },
        ..Plan::default()
    };
    // nothing is timed here: in memory the planner needs no timed constant; the disk constants are read from the
    // index directory's cache (or timed once) where the vectors' reads are priced, below
    let plan = plan_flags(plan, flags).unwrap_or_else(|e| exit2(&e));
    if let Err(e) = plan.check(&ix) {
        eprintln!("{e}");
        std::process::exit(2);
    }
    let t = Timer::new();
    let pseudo = pseudo_queries(&ix, if args().iter().any(|a| a == "--no-approx") { 0 } else { arg_usize("--pseudo", 1000) }, 10);
    let (approx, fres) = if pseudo.is_empty() { (Approx { f: 0.0, rad: Vec::new() }, Vec::new()) } else { Approx::fit(&ix, &pseudo) };
    out!("[approx] f = {} from {} pseudo-queries in {:.1}s; mean items scanned at recall 0.95 by f: {:?}", approx.f, pseudo.len(), t.s(), fres.iter().map(|x| (x.0, x.1.round())).collect::<Vec<_>>());
    // full vectors from a file (`--full-file PATH`, mapped with `--full-advice`), for runs under a memory cap
    let ff = arg_str("--full-file", "");
    if !ff.is_empty() {
        // the copy is written from the index's vectors, which a saved index maps from its own file; to_file refuses
        // a path inside a saved index, the fit's vectors and any file this process maps, and never truncates the file
        // it replaces
        let t = Timer::new();
        if let Full2::Real { rows, .. } = &mut ix.full {
            let r = std::sync::Arc::get_mut(rows).expect("the full vectors are shared before --full-file").to_file(&ff, &arg_str("--full-advice", "random"), &p.vector_files().iter().map(|s| s.as_str()).collect::<Vec<_>>());
            if let Err(e) = r {
                eprintln!("{e}");
                std::process::exit(2);
            }
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
    // --plan-file's constants (name=value lines: r_rows, r_kappa, r_seed, io_us, io_gbps), and where reads are priced
    // the planner's disk constants unless the file gives them (cached in the index directory, keyed by the machine,
    // the vectors' device and the read policy; `disk_key`); the constants used are written to RESULTS/plan.txt
    let priced = ix.own_rows().is_some_and(|r| r.priced());
    let plan = plan_with_file(&ix, plan, priced, dir, flags);
    out!("[plan] {}", plan.to_kv().replace('\n', " "));
    std::fs::write(format!("{}/plan.txt", res_dir()), plan.to_kv()).unwrap();
    let c = Ctx2 { p, ix, row_ent, ent_xs, items, texts, plan, approx, pseudo };
    if !ff.is_empty() {
        out!("[mem] {}", c.mem_report());
    }
    out!("[fit] {}", c.p.fit);
    out!("built v2 (index format {}, {}) {} rows, {} clusters in {:.1}s; stored bytes/item cumulative by level: {:?}", INDEX_FORMAT, saved_dir(dir), c.ix.nrows, c.ix.k(), built_s,
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
            ("nukey", b(&ix.nukey)), ("ent_xs+row_ent", b(&self.ent_xs) + b(&self.row_ent)),
            ("full rows in memory", match &ix.full { Full2::Real { rows, .. } => match &**rows { crate::v2::disk::Rows::Mem(v) => b(v), _ => 0 }, _ => 0 }),
        ];
        parts.sort_by_key(|x| std::cmp::Reverse(x.1));
        let tot: usize = parts.iter().map(|x| x.1).sum();
        format!("total {:.1}: {}", tot as f64 / 1048576.0, parts.iter().map(|(k, v)| format!("{k} {:.1}", *v as f64 / 1048576.0)).collect::<Vec<_>>().join(", "))
    }
    /// Flat scans over the index's vectors in index row order.
    pub fn flat(&self) -> Flat<'_> {
        let Full2::Real { rows, .. } = &self.ix.full else { panic!("a synthetic index has no vectors to scan") };
        let n = self.p.meta.n;
        Flat { map: rows.as_bytes(self.ix.nrows * n), off: 0, n, nrows: self.ix.nrows, ent: &self.row_ent }
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
        // the last cells are exact k-NN, one per k of `--knn-k` (default 10)
        let knn_ks: Vec<usize> = if arg_usize("--knn", 1) > 0 {
            arg_str("--knn-k", "10").split(',').map(|x| x.parse().ok().filter(|&k| k > 0).unwrap_or_else(|| {
                eprintln!("--knn-k K,K,... (each at least 1), not {x}");
                std::process::exit(2);
            })).collect()
        } else {
            Vec::new()
        };
        let ncell = sizes_f.len() + knn_ks.len();
        let cell_name = |cell: usize| if cell < sizes_f.len() { format!("{}", sizes_f[cell]) } else { format!("knn{}", knn_ks[cell - sizes_f.len()]) };
        let run_cell = |c: &Ctx2, plan: Plan, cell: usize, qi: usize| -> Stats {
            let ix = &c.ix;
            if cell < sizes_f.len() {
                let o = Opts2 { vis: None, verify: false, want_iv: false, plan };
                let pr = prep2(ix, &qs[qi]);
                let r = threshold2(ix, &pr, ths[qi][cell], &o);
                std::hint::black_box(to_bitmap2(ix, &r.rows));
                r.st
            } else {
                knn2(ix, &qs[qi], knn_ks[cell - sizes_f.len()], None, plan).st
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
                // the planner's choice per query (untimed, one thread): a scan of the visible set, or per query the clusters
                // scored directly, run on the index path and decided from full vectors
                let (mut nscan, mut ndir, mut nidx, mut nfull) = (0usize, 0usize, 0usize, 0usize);
                for qi in 0..qs.len() {
                    let pr = prep2(&c.ix, &qs[qi]);
                    let t = if cell < sizes_f.len() { ths[qi][cell] } else { let mut st = Stats::new(nl); seed2(&c.ix, &pr, knn_ks[cell - sizes_f.len()], None, &mut st) };
                    let o = Opts2 { vis: None, verify: false, want_iv: cell >= sizes_f.len(), plan };
                    match pl.install(|| plan2(&c.ix, &pr, t, &o)) {
                        Planned::Scan => nscan += 1,
                        Planned::Run { pre, use_idx, .. } => {
                            for (ci, p) in pre.iter().enumerate() {
                                match p {
                                    Pre::Direct => ndir += 1,
                                    Pre::Open { ncand, .. } if *ncand > 0 => if use_idx[ci] { nidx += 1 } else { nfull += 1 },
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                out!("e2e2 path {vname} {cls} {}: scan {nscan}/{}, per query clusters direct {:.1} index {:.1} full {:.1}", cell_name(cell), qs.len(), ndir as f64 / k, nidx as f64 / k, nfull as f64 / k);
                let ent: Vec<String> = enter.iter().map(|x| format!("{:.5}", x / k / nrows as f64)).collect();
                cellinfo[vi].push(format!("{:.3},{:.4},{:.5},{}", br / k, tch / k / nrows as f64, fr / k / nrows as f64, ent.join(";")));
                out!("e2e2 bytes {vname} {cls} {}: bytes/item {:.3}, touched {:.4}, full reads/item {:.5}", cell_name(cell), br / k, tch / k / nrows as f64, fr / k / nrows as f64);
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
                    let (what, frac) = if cell < sizes_f.len() { ("index2_thr".to_string(), sizes_f[cell]) } else { let kk = knn_ks[cell - sizes_f.len()]; (format!("index2_knn{kk}"), if kk == 10 { 0.0 } else { kk as f64 }) };
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
                // per query: clusters on the index path, on full vectors, items entering the first level, item-levels,
                // groups, full reads
                let mut opened = [0f64; 6];
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
                    let nqf = qs.len() as f64;
                    opened[0] += r.idx_clusters as f64 / nqf;
                    opened[1] += r.flat_clusters as f64 / nqf;
                    opened[2] += r.st.enter[0] as f64 / nqf;
                    opened[3] += r.st.enter[..r.st.enter.len() - 1].iter().sum::<u64>() as f64 / nqf;
                    opened[4] += r.st.groups as f64 / nqf;
                    opened[5] += r.st.full_reads as f64 / nqf;
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
                out!("breakdown2 counts {vname} {cls} {frac}: per query {:.1} clusters on the index path, {:.1} on full vectors, {:.0} items entering the first level, {:.0} item-levels, {:.1} groups, {:.0} full reads", opened[0], opened[1], opened[2], opened[3], opened[4], opened[5]);
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
    // every variant's plan flags are checked before anything runs
    for w in &whats {
        if let Err(e) = plan_flags(c.plan, w.split_once(':').map_or("", |x| x.1)) {
            exit2(&format!("--what {w}: {e}"));
        }
    }
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
    // the planner's disk constants: the setup's where it priced reads; else, when a read mode prices them, from the
    // index directory's cache (timed once per machine and device; `:plan=mem` in a read mode: reads not counted)
    let plan0 = if !pol.priced.load(Relaxed) && modes.iter().any(|m| m.split(':').any(|kv| kv == "uring" || kv == "plan=disk")) {
        plan_with_file(ix, c.plan, true, &data_dir(), &arg_str("--flags", ""))
    } else {
        c.plan
    };
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
                        // a gap in bytes, as io_us at this io_gbps, through the plan's own check (a number above 0)
                        Some(("gap", v)) => plan = Plan::from_kv(plan, &format!("io_us={}", v.parse::<f64>().unwrap_or(f64::NAN) / (plan.io_gbps * 1e3))).unwrap_or_else(|e| exit2(&format!("--read-modes :gap={v}: {e}"))),
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
                            let plan = plan_flags(plan, flags).expect("checked before the runs");
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
                                let r = threshold2(sub, &pr, t0, &Opts2 { vis: None, verify: false, want_iv: false, plan: plan_flags(plan, flags).expect("checked before the runs") });
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
                let mut dumpf: Vec<_> = labels.iter().map(|l| (!dump.is_empty() && th == 1).then(|| std::io::BufWriter::new(std::fs::File::create(format!("{dump}/{}_{vname}.bin", l.replace([':', '@', '='], "_"))).unwrap()))).collect();
                let mut nscan = vec![0usize; whats.len()];
                for pass in &passes {
                    if cold {
                        // the first query at each answer size, after the full vectors are dropped from the page cache, just before
                        // the variants of this pass run warm
                        let qi = 0;
                        for &wi in pass {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `e2e`'s per-variant flags on a variant that shares the first index: `ddec=` sets the variant's plan (checked
    /// against the index), a build option (`dlv=`, `nusort`) is an error instead of being dropped.
    #[test]
    fn e2e_variant_flags_on_a_shared_index() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
        let (_, ix) = build(dir, &BuildOptions { dlv: None, ..BuildOptions::default() }, &mut |_| {}).unwrap();
        let base = Plan::default();
        assert_eq!(shared_plan(base, &ix, "ddec=0.5").unwrap().ddec, Some(0b100001));
        let p = shared_plan(base, &ix, "nogtab, ddec=none").unwrap();
        assert_eq!((p.ddec, p.gtab), (Some(0), false));
        assert_eq!(shared_plan(base, &ix, "ddec=all").unwrap().ddec, Some(u32::MAX));
        assert_eq!(shared_plan(base, &ix, "noint").unwrap().ddec, None, "no ddec= keeps the index's choice");
        assert_eq!(shared_plan(Plan { ddec: Some(0b10), ..base }, &ix, "noint").unwrap().ddec, Some(0b10), "the first variant's choice carries over");
        assert!(shared_plan(base, &ix, "ddec=6").is_err(), "a level beyond the index's six");
        for f in ["dlv=16.32", "noint,dlv=16.32", "nusort"] {
            assert!(shared_plan(base, &ix, f).is_err(), "{f} on a shared index");
        }
    }

    /// The command line's registry: the removed commands (fit, build, anchors) and positional arguments are refused,
    /// `create` takes `--vectors` and `--index` and nothing else to do, and build flags are taken only where an index
    /// is made (the fit's only by `create`).
    #[test]
    fn the_command_line_is_checked() {
        let check = |s: &str| check_args_of(&std::iter::once("vexact").chain(s.split_whitespace()).map(String::from).collect::<Vec<_>>());
        let ok = |s: &str| assert!(check(s).is_ok(), "{s}: {:?}", check(s));
        let refused = |s: &str, says: &str| {
            let e = check(s).unwrap_err();
            assert!(e.contains(says), "{s}: {e}");
        };
        ok("--vectors v.npy --index D create");
        ok("--vectors v.npy --index D create --per-cluster by-n --groups by-n --dlv 16,32 --anchors 512 --text-anchors t.npy --hash");
        ok("--v2 --index D e2e --nq 10");
        ok("--v2 --index D --rebuild --dlv 16,32 --anchors 64");
        ok("--vectors v.npy --index D create --seed 7");
        ok("--v2 --index D --rebuild --anchors 64 --seed 7");
        ok("--v2 --index D --rebuild --text-anchors t.npy --text-count 32 --seed 7 e2e");
        // the certificate's tools run alone, with their own flags
        ok("certify-layout --ds 15,15,15,15 --lnd -29.04 --time-cap 60");
        ok("cert-layouts --dims 64,768");
        refused("certify-layout --ds 15,15", "needs --ds");
        refused("certify-layout --ds 1,15 --lnd -30", "at least 4");
        refused("certify-layout --ds 15,x --lnd -30", "at least 4");
        refused("certify-layout --ds 15,15 --lnd -30 --time-cap nan", "--time-cap");
        refused("certify-layout --ds 15,15 --lnd -30 --time-cap -1", "--time-cap");
        refused("certify-layout --ds 15,15 --lnd nan", "--lnd");
        ok("certify-layout --ds 15,15 --lnd -30 --time-cap inf");
        refused("certify-layout --ds 15,15 --lnd -30 --index D", "runs alone");
        refused("--v2 --index D certify-layout --ds 15,15 --lnd -30", "runs alone");
        refused("--v2 --index D e2e --lnd -30", "certify-layout's");
        refused("--vectors v.npy --index D create --dims 64", "cert-layouts's");
        // the partition alone, with its own flags
        ok("--vectors v.npy partition --out P");
        ok("--vectors v.npy partition --out P --cells 32 --cross 2 --per-cluster by-n --seed 7 --sample-per-centre 100 --cell-sample 500 --niter 0");
        refused("--vectors v.npy partition", "--out");
        refused("partition --out P", "--vectors");
        refused("--vectors v.npy partition --out P --cells 0", "--cells");
        refused("--vectors v.npy partition --out P --cross x", "--cross");
        refused("--vectors v.npy partition --out P --per-cluster 0", "--per-cluster");
        refused("--vectors v.npy partition --out P --index D", "runs alone");
        refused("--vectors v.npy partition --out P create", "runs alone");
        refused("--vectors v.npy partition --out P --groups by-n", "runs alone");
        refused("--vectors v.npy --index D create --cells 4", "partition's");
        refused("--v2 --index D e2e --out P", "partition's");
        ok("--v2 --index D --in-memory --nusort verify");
        ok("--v2 --index D --ddec 0.1 --keep-index bytesq");
        ok("qvbench --groups 8");
        for gone in ["fit", "build", "anchors"] {
            refused(&format!("--index D {gone}"), "gone");
        }
        refused("--index D fit --partition P", "gone");
        refused("--v2 --index D verify extra", "unknown arguments");
        refused("--v2 --index D /some/path e2e", "unknown arguments");
        refused("--index D create", "--vectors");
        refused("--vectors v.npy create", "--index");
        refused("--vectors v.npy --index D create e2e", "does nothing else");
        refused("--vectors v.npy --index D create --rebuild", "does nothing else");
        refused("--v2 --vectors v.npy --index D e2e", "create's");
        refused("--v2 --index D --dlv 16,32 e2e", "build option");
        refused("--v2 --index D --anchors 64 e2e", "build option");
        refused("--v2 --index D --rebuild --per-cluster by-n", "create's");
        refused("--v2 --index D --rebuild --basis dual", "create's");
        refused("--v2 --index D --rebuild --in-memory", "give one");
        refused("--v2 --index D e2e --nq", "needs a value");
        // create takes build flags, --vectors, --index and --hash only
        refused("--vectors v.npy --index D create --ddec 0", "create takes");
        refused("--vectors v.npy --index D create --keep-index", "create takes");
        refused("--vectors v.npy --index D create --nq 5", "create takes");
        refused("--vectors v.npy --index D create --results R", "create takes");
        // --seed only where it seeds something
        refused("--v2 --index D --rebuild --seed 7", "does nothing");
        refused("--v2 --index D --rebuild --dlv 16,32 --seed 7 e2e", "does nothing");
        refused("--v2 --index D --in-memory --seed 7 verify", "--in-memory");
        refused("--v2 --index D --in-memory --anchors 64 --seed 7 verify", "--in-memory");
        refused("--v2 --index D --seed 7 e2e", "does nothing");
    }

    /// A variant's `pf=` file that gives both disk constants is used as given: nothing is timed (the timer panics) and
    /// the run logs that; one that gives only one of them leaves them to the timer.
    #[test]
    fn disk_constants_from_a_pf_file_are_not_timed() {
        struct Rm(std::path::PathBuf); // removed when dropped, also when the test fails
        impl Drop for Rm {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let tmp = Rm(std::env::temp_dir().join(format!("vexact-pf-{}-{}", std::process::id(), now_s() as u64)));
        let tmp = &tmp.0;
        std::fs::create_dir_all(tmp).unwrap();
        let (both, one) = (tmp.join("plan.txt"), tmp.join("half.txt"));
        std::fs::write(&both, "io_us=7.25\nio_gbps=1.5\n").unwrap();
        std::fs::write(&one, "io_us=7.25\n").unwrap();
        let flags = format!("noint,pf={}", both.display());
        let plan = plan_flags(Plan::default(), &flags).unwrap();
        let mut lines = Vec::new();
        let p = plan_with_file_by(plan, true, "", &flags, &mut |s| lines.push(s), &mut |_, _| panic!("the disk constants were timed"));
        assert_eq!((p.io_us, p.io_gbps), (7.25, 1.5));
        assert!(lines.iter().any(|l| l.contains("io_us=7.25 io_gbps=1.5") && l.contains("not timed")), "{lines:?}");
        // in memory (reads not priced) nothing is timed either
        plan_with_file_by(plan, false, "", "", &mut |_| {}, &mut |_, _| panic!("timed in memory"));
        // a pf= file with one constant: timed
        let flags = format!("pf={}", one.display());
        let plan = plan_flags(Plan::default(), &flags).unwrap();
        let mut timed = false;
        let p = plan_with_file_by(plan, true, "", &flags, &mut |_| {}, &mut |p, _| {
            timed = true;
            Plan { io_gbps: 9.0, ..p }
        });
        assert!(timed && p.io_gbps == 9.0);
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
