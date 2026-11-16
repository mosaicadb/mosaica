//! The build parameters tools/fit.py writes to a fitted directory, and the mapped file of f16 vectors in cluster order.
use crate::util::*;

pub const DA: f64 = 0.5 / 32767.0 + 1e-7;
pub const SEED: u64 = 20261112;
/// The format of a fitted index directory (meta.json "format") that this build reads.
pub const FIT_FORMAT: u64 = 2;

/// The item scale from the stored sort key; build and query compute it with the same f32 operations.
#[inline]
pub fn item_scale(ah: i16, mn: f32, lamsum: f32) -> f32 {
    let a = ah as f32 / 32767.0f32;
    let s2 = (1.0f32 + mn * mn) - 2.0f32 * mn * a;
    (s2.max(1e-12f32) / lamsum).sqrt()
}

pub struct Meta {
    pub n: usize,
    pub k: usize,
    pub g: usize,
    pub dlv: Vec<usize>,
    pub rlv: Vec<usize>,
    pub bounds: Vec<usize>,
    pub csg: f64,
    /// the guarantee's failure probability per query (meta "EPS", default 1e-9)
    pub eps: f64,
    /// certified worst-case constants of the randomised levels (meta "CERTS", written by tools/certify.py)
    pub certs: Vec<crate::v2::tail::Cert>,
}
impl Meta {
}

/// Build parameters fitted by tools/fit.py.
pub struct Params {
    pub meta: Meta,
    pub cstart: Vec<usize>,
    pub grp: Vec<usize>,
    pub centres: Vec<f32>, // K x n, the stored 8-bit centres (int8 * scale) as f32 values
    pub lam: Vec<f64>,
    pub v: Vec<f32>, // G x n x n, z = V^T r
    pub r2: Option<Vec<f64>>, // G: mean |r|^2 per group (version 2 item scale)
    pub fit: String,          // the fit used, as tools/fit.py recorded it
    pub tree_edges: Vec<f64>, // 9 x 256
    pub tree_cent: Vec<f64>,
    pub gtree: Vec<f64>,
    pub perm: Vec<i64>,
    pub qi: Vec<i64>,
    /// The support table's anchors (A x n, unit; the fit's `anchors.npy`), loaded and checked by `v2::prepare`
    /// (`load_params` leaves them empty). Empty: one anchor per cluster, its centre's direction.
    pub anchors: Vec<f64>,
    pub xs_path: String, // the vectors in cluster order (a fit with an Xs.npy; empty for a partitioned fit)
    pub xs_name: String, // the vectors file as meta.json names it (relative to the fitted directory unless absolute)
    pub partition: String, // the partition a fit without an Xs.npy reads its vectors from (empty for a fit with one)
    pub partition_name: String, // the partition as meta.json names it
}

impl Params {
    /// The file whose identity stands for the fit's vectors: its Xs.npy, or its partition's manifest.json (written last
    /// by tools/partition.py, so a new partition is a new file).
    pub fn vectors_id_path(&self) -> String {
        if self.xs_path.is_empty() { format!("{}/manifest.json", self.partition) } else { self.xs_path.clone() }
    }
    /// The files holding the fit's vectors (the Xs.npy, or the partition's cell files).
    pub fn vector_files(&self) -> Vec<String> {
        if !self.xs_path.is_empty() {
            return vec![self.xs_path.clone()];
        }
        std::fs::read_dir(&self.partition).map_or(Vec::new(), |d| {
            d.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.ends_with(".f16")).collect()
        })
    }
}

/// The fit's meta.json, after checking that this build reads its format.
fn load_meta(dir: &str) -> serde_json::Value {
    let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/meta.json")).unwrap()).unwrap();
    let fmt = meta["format"].as_u64().unwrap_or(1);
    if fmt != FIT_FORMAT {
        eprintln!("{dir} was fitted in format {fmt}; this build reads format {FIT_FORMAT}. Refit it with tools/fit.py.");
        std::process::exit(2);
    }
    meta
}

/// The fit's stored centres (K x n, int8 * scale as f32 values) with n and K, from meta.json, mq8.npy and msc.npy only.
pub fn load_centres(dir: &str) -> Result<(usize, usize, Vec<f32>), String> {
    let meta = load_meta(dir);
    let (n, k) = (meta["n"].as_u64().ok_or(format!("{dir}/meta.json has no n"))? as usize, meta["K"].as_u64().ok_or(format!("{dir}/meta.json has no K"))? as usize);
    let mq8 = read_npy(&format!("{dir}/mq8.npy")).i8s();
    let msc = read_npy(&format!("{dir}/msc.npy")).f64s();
    if k.checked_mul(n) != Some(mq8.len()) || msc.len() != k {
        return Err(format!("{dir}: mq8.npy / msc.npy do not hold {k} centres of {n}; refit"));
    }
    Ok((n, k, (0..k * n).map(|i| (mq8[i] as f64 * msc[i / n]) as f32).collect()))
}

/// The fit's support-table anchors (`anchors.npy`), checked: <f8, shape (A >= 1, n), each row of unit norm within
/// 1e-12. Empty when the fit has none.
pub fn load_anchors(dir: &str, n: usize) -> Result<Vec<f64>, String> {
    let path = format!("{dir}/anchors.npy");
    if !std::path::Path::new(&path).exists() {
        return Ok(Vec::new());
    }
    let a = read_npy(&path);
    if a.descr != "<f8" {
        return Err(format!("{path}: expected <f8, found {}; write the fit's anchors again", a.descr));
    }
    if a.shape.len() != 2 || a.shape[0] < 1 || a.shape[1] != n {
        return Err(format!("{path} has shape {:?}, expected (A >= 1, {n}); write the fit's anchors again", a.shape));
    }
    let v = a.f64s();
    check_anchors(&v, n).map_err(|e| format!("{path}: {e}; write the fit's anchors again"))?;
    Ok(v)
}

/// Anchors are unit rows of n: every row's norm within 1e-12 of 1.
pub fn check_anchors(v: &[f64], n: usize) -> Result<(), String> {
    if n == 0 || v.len() % n != 0 {
        return Err(format!("{} values are not rows of {n}", v.len()));
    }
    for (i, r) in v.chunks(n).enumerate() {
        let nr = r.iter().map(|x| x * x).sum::<f64>().sqrt();
        if !((nr - 1.0).abs() <= 1e-12) {
            return Err(format!("anchor {i} has norm {nr}, not 1 within 1e-12"));
        }
    }
    Ok(())
}

pub fn load_params(dir: &str) -> Params {
    let meta = load_meta(dir);
    let us = |k: &str| meta[k].as_u64().unwrap() as usize;
    let vu = |k: &str| meta[k].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect::<Vec<_>>();
    let r = |k: &str| read_npy(&format!("{dir}/{k}.npy"));
    let opt = |k: &str| std::path::Path::new(&format!("{dir}/{k}.npy")).exists().then(|| r(k));
    let mq8 = r("mq8").i8s();
    let msc = r("msc").f64s();
    let n = us("n");
    let k = us("K");
    let centres = (0..k * n).map(|i| (mq8[i] as f64 * msc[i / n]) as f32).collect();
    // the vectors: the fit's Xs.npy (tools/fit.py), else the partition it was fitted from (fit::fit)
    let xs = meta["xs"].as_str().unwrap_or("");
    let partition = meta["partition"].as_str().unwrap_or("");
    if xs.is_empty() == partition.is_empty() {
        eprintln!("{dir}/meta.json names {}; it names either the fit's vectors file (\"xs\") or the partition it was fitted from (\"partition\"). Refit it.", if xs.is_empty() { "neither its vectors file nor a partition" } else { "both a vectors file and a partition" });
        std::process::exit(2);
    }
    Params {
        meta: Meta { n, k, g: us("G"), dlv: vu("DLV"), rlv: vu("RLV"), bounds: vu("BOUNDS"), csg: meta["CSG"].as_f64().unwrap(),
            eps: meta["EPS"].as_f64().unwrap_or(1e-9),
            certs: meta["CERTS"].as_array().map_or(Vec::new(), |a| a.iter().map(|c| crate::v2::tail::Cert {
                ds: c["ds"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect(),
                lnd: c["lnd"].as_f64().unwrap(),
                x: c["X"].as_f64().unwrap(),
            }).collect()) },
        cstart: r("cstart").i64s().iter().map(|&x| x as usize).collect(),
        grp: r("grp").i64s().iter().map(|&x| x as usize).collect(),
        centres,
        lam: r("lam").f64s(),
        v: r("V").f32s(),
        r2: opt("r2").map(|a| a.f64s()),
        fit: meta["fit"].to_string(),
        tree_edges: r("tree_edges").f64s(),
        tree_cent: r("tree_cent").f64s(),
        gtree: r("gtree").f64s(),
        perm: r("perm").i64s(),
        qi: r("qi").i64s(),
        anchors: Vec::new(),
        xs_path: if xs.is_empty() || xs.starts_with('/') { xs.to_string() } else { format!("{dir}/{xs}") },
        xs_name: xs.to_string(),
        partition: if partition.is_empty() || partition.starts_with('/') { partition.to_string() } else { format!("{dir}/{partition}") },
        partition_name: partition.to_string(),
    }
}

pub struct RealSource {
    pub map: memmap2::Mmap,
    pub off: usize,
    pub n: usize,
}
impl RealSource {
    pub fn open(path: &str) -> RealSource {
        let f = std::fs::File::open(path).unwrap();
        let map = unsafe { memmap2::Mmap::map(&f).unwrap() };
        let (hdr, off) = npy_header(&map);
        let sh = npy_shape(&hdr);
        assert!(hdr.contains("<f2"));
        RealSource { map, off, n: sh[1] }
    }
    pub fn row(&self, r: usize) -> &[u16] {
        let n = self.n;
        let b = &self.map[self.off + r * n * 2..self.off + (r + 1) * n * 2];
        unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u16, n) }
    }
}
