//! The build parameters tools/fit.py writes to a fitted directory, and the mapped file of f16 vectors in cluster order.
use crate::util::*;

pub const DA: f64 = 0.5 / 32767.0 + 1e-7;

/// The stored key of a row whose x . mhat the build computes as `a`, and how far `a` lies outside [-1, 1], where the
/// key is clamped (rounded up; 0 inside). The key is within DA plus that excess of x . mhat.
#[inline]
pub fn key_of(a: f64) -> (i16, f64) {
    ((a.clamp(-1.0, 1.0) * 32767.0).round() as i16, (a.abs() - 1.0).max(0.0) * (1.0 + 2.0 * f64::EPSILON))
}
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
    /// certified worst-case constants of the randomised levels (meta "CERTS", written by the fit, `crate::certify`,
    /// or tools/certify.py)
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
    /// Fit row -> entity (perm.npy): empty until `load_perm`, which a build calls. An open never reads it (8 bytes a
    /// row); the index holds its own row-to-entity map, and the build records perm.npy's hash and identity.
    pub perm: Vec<i64>,
    pub perm_path: String,
    /// The fit's rows (its cstart's last entry, before `BuildOptions::clusters` keeps only some clusters).
    pub fit_rows: usize,
    pub qi: Vec<i64>,
    /// The support table's anchors (A x n, unit; the fit's `anchors.npy`), loaded and checked by `v2::prepare`
    /// (`load_params` leaves them empty). Empty: one anchor per cluster, its centre's direction.
    pub anchors: Vec<f64>,
    pub xs_path: String, // the vectors in cluster order (a fit with an Xs.npy; empty for a partitioned fit)
    pub xs_name: String, // the vectors file as meta.json names it (relative to the fitted directory unless absolute)
    pub partition: String, // the partition a fit without an Xs.npy reads its vectors from (empty for a fit with one)
    pub partition_name: String, // the partition as meta.json names it
    /// The partition's identity when it was fitted (meta.json "partition_id", fit::partition_id): a hash of its
    /// manifest.json and that file's identity. Empty for a fit with an Xs.npy.
    pub partition_id: String,
    /// The identity of the partition's manifest.json file when it was fitted (meta.json "partition_file"): a build
    /// warns when it changed while the content (`partition_id`) did not.
    pub partition_file: Option<crate::v2::store::XsId>,
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
fn load_meta(dir: &str) -> Result<serde_json::Value, String> {
    let path = format!("{dir}/meta.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}; is {dir} a fitted directory?"))?;
    let meta: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}; refit it"))?;
    let fmt = meta["format"].as_u64().unwrap_or(1);
    if fmt != FIT_FORMAT {
        return Err(format!("{dir} was fitted in format {fmt}; this build reads format {FIT_FORMAT}. Refit it with tools/fit.py."));
    }
    Ok(meta)
}

/// The fit's stored centres (K x n, int8 * scale as f32 values) with n and K, from meta.json, mq8.npy and msc.npy only.
pub fn load_centres(dir: &str) -> Result<(usize, usize, Vec<f32>), String> {
    let meta = load_meta(dir)?;
    let (n, k) = (meta["n"].as_u64().ok_or(format!("{dir}/meta.json has no n"))? as usize, meta["K"].as_u64().ok_or(format!("{dir}/meta.json has no K"))? as usize);
    centres_of(dir, n, k)
}

fn centres_of(dir: &str, n: usize, k: usize) -> Result<(usize, usize, Vec<f32>), String> {
    let mq8 = try_read_npy(&format!("{dir}/mq8.npy"))?.i8s();
    let msc = try_read_npy(&format!("{dir}/msc.npy"))?.f64s();
    if k.checked_mul(n) != Some(mq8.len()) || msc.len() != k {
        return Err(format!("{dir}: mq8.npy / msc.npy do not hold {k} centres of {n}; refit"));
    }
    Ok((n, k, (0..k * n).map(|i| (mq8[i] as f64 * msc[i / n]) as f32).collect()))
}

/// A fixed hash of a fit's stored centres (K x n), which `anchors.json` records for the fit its anchors were made for.
pub fn centres_hash(n: usize, k: usize, centres: &[f32]) -> String {
    let mut h = crate::v2::store::Fx::default();
    h.str("vexact centres");
    h.usizes(&[n, k]);
    h.pod(&centres[..(k * n).min(centres.len())]);
    format!("{:016x}", h.finish())
}

/// A fixed hash of a set of anchors, which `anchors.json` records.
pub fn anchors_hash(a: &[f64]) -> String {
    let mut h = crate::v2::store::Fx::default();
    h.str("vexact anchors");
    h.pod(a);
    format!("{:016x}", h.finish())
}

/// The fit's support-table anchors (`anchors.npy`), checked: <f8, shape (A >= 1, n), each row of unit norm within
/// 1e-12, and made for this fit (its sidecar `anchors.json`, written by `fit::save_anchors`, records the hash of the
/// centres `centres` (K x n) and of the anchors themselves). An anchors.npy without a sidecar (written before it
/// existed, or by hand) is refused with what to do. Empty when the fit has none.
pub fn load_anchors(dir: &str, n: usize, k: usize, centres: &[f32]) -> Result<Vec<f64>, String> {
    let path = format!("{dir}/anchors.npy");
    if !std::path::Path::new(&path).exists() {
        return Ok(Vec::new());
    }
    let again = format!("write them again for this fit (a rebuild with anchors: vexact --v2 --index {dir} --rebuild --anchors A, v2::BuildOptions::anchors), or remove {path} to use one anchor per cluster");
    let side = format!("{dir}/anchors.json");
    let sv: serde_json::Value = match std::fs::read_to_string(&side) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{side}: {e}; {again}"))?,
        Err(_) => return Err(format!("{path} has no {side} saying which fit it was made for; {again}")),
    };
    if sv["fit"].as_str() != Some(centres_hash(n, k, centres).as_str()) {
        return Err(format!("{path} was made for another fit (the fit was redone since); {again}"));
    }
    let a = try_read_npy(&path)?;
    if a.descr != "<f8" {
        return Err(format!("{path}: expected <f8, found {}; write the fit's anchors again", a.descr));
    }
    if a.shape.len() != 2 || a.shape[0] < 1 || a.shape[1] != n {
        return Err(format!("{path} has shape {:?}, expected (A >= 1, {n}); write the fit's anchors again", a.shape));
    }
    let v = a.f64s();
    check_anchors(&v, n).map_err(|e| format!("{path}: {e}; write the fit's anchors again"))?;
    if sv["anchors"].as_str() != Some(anchors_hash(&v).as_str()) {
        return Err(format!("{path} is not the file {side} was written with (replaced alone?); {again}"));
    }
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

/// The parameters of the fitted directory `dir`, their sizes checked against each other; an error (naming what to
/// do) on a missing, unreadable or inconsistent file.
pub fn load_params(dir: &str) -> Result<Params, String> {
    let meta = load_meta(dir)?;
    let us = |k: &str| meta[k].as_u64().map(|x| x as usize).ok_or(format!("{dir}/meta.json has no {k}; refit it"));
    let vu = |k: &str| -> Result<Vec<usize>, String> {
        meta[k].as_array().and_then(|a| a.iter().map(|x| x.as_u64().map(|v| v as usize)).collect::<Option<Vec<_>>>()).ok_or(format!("{dir}/meta.json has no list {k}; refit it"))
    };
    let r = |k: &str| try_read_npy(&format!("{dir}/{k}.npy"));
    let n = us("n")?;
    let k = us("K")?;
    let g = us("G")?;
    let (_, _, centres) = centres_of(dir, n, k)?;
    // the vectors: the fit's Xs.npy (tools/fit.py), else the partition it was fitted from (fit::fit)
    let xs = meta["xs"].as_str().unwrap_or("");
    let partition = meta["partition"].as_str().unwrap_or("");
    if xs.is_empty() == partition.is_empty() {
        return Err(format!("{dir}/meta.json names {}; it names either the fit's vectors file (\"xs\") or the partition it was fitted from (\"partition\"). Refit it.", if xs.is_empty() { "neither its vectors file nor a partition" } else { "both a vectors file and a partition" }));
    }
    let certs = match meta["CERTS"].as_array() {
        None => Vec::new(),
        Some(a) => a.iter().map(|c| -> Option<crate::v2::tail::Cert> {
            Some(crate::v2::tail::Cert {
                ds: c["ds"].as_array()?.iter().map(|x| x.as_u64().map(|v| v as usize)).collect::<Option<Vec<_>>>()?,
                lnd: c["lnd"].as_f64()?,
                x: c["X"].as_f64()?,
            })
        }).collect::<Option<Vec<_>>>().ok_or(format!("{dir}/meta.json: a certificate (CERTS) lacks ds, lnd or X; certify the fit again"))?,
    };
    let opt = |name: &str| -> Result<Option<Npy>, String> { if std::path::Path::new(&format!("{dir}/{name}.npy")).exists() { r(name).map(Some) } else { Ok(None) } };
    let p = Params {
        meta: Meta { n, k, g, dlv: vu("DLV")?, rlv: vu("RLV")?, bounds: vu("BOUNDS")?, csg: meta["CSG"].as_f64().ok_or(format!("{dir}/meta.json has no CSG; refit it"))?,
            eps: meta["EPS"].as_f64().unwrap_or(1e-9), certs },
        cstart: r("cstart")?.i64s().iter().map(|&x| x as usize).collect(),
        grp: r("grp")?.i64s().iter().map(|&x| x as usize).collect(),
        centres,
        lam: r("lam")?.f64s(),
        v: r("V")?.f32s(),
        r2: opt("r2")?.map(|a| a.f64s()),
        fit: meta["fit"].to_string(),
        tree_edges: r("tree_edges")?.f64s(),
        tree_cent: r("tree_cent")?.f64s(),
        gtree: r("gtree")?.f64s(),
        perm: Vec::new(),
        perm_path: format!("{dir}/perm.npy"),
        fit_rows: 0,
        qi: r("qi")?.i64s(),
        anchors: Vec::new(),
        xs_path: if xs.is_empty() || xs.starts_with('/') { xs.to_string() } else { format!("{dir}/{xs}") },
        xs_name: xs.to_string(),
        partition: if partition.is_empty() || partition.starts_with('/') { partition.to_string() } else { format!("{dir}/{partition}") },
        partition_name: partition.to_string(),
        partition_id: meta["partition_id"].as_str().unwrap_or("").to_string(),
        partition_file: crate::v2::store::XsId::from_json(&meta["partition_file"]),
    };
    // the sizes the build and the open index by
    let ok = p.cstart.len() == k + 1 && p.cstart.first() == Some(&0) && p.cstart.windows(2).all(|w| w[0] <= w[1]) && p.grp.len() == k && p.grp.iter().all(|&x| x < g)
        && Some(p.lam.len()) == g.checked_mul(n) && g.checked_mul(n).and_then(|x| x.checked_mul(n)) == Some(p.v.len())
        && p.r2.as_ref().map_or(true, |r| r.len() == g) && p.tree_edges.len() == p.tree_cent.len();
    if !ok {
        return Err(format!("{dir}: the fit's arrays (cstart, grp, lam, V, r2, tree tables) do not agree with its {k} clusters, {g} groups and {n} dimensions; refit it"));
    }
    let fit_rows = p.cstart[k];
    Ok(Params { fit_rows, ..p })
}

impl Params {
    /// Read perm.npy (fit row -> entity) into `perm`, checked: one entity per fit row, each a u32. A build needs it; an
    /// open does not.
    pub fn load_perm(&mut self) -> Result<(), String> {
        let rows = self.fit_rows;
        let a = try_read_npy(&self.perm_path).map_err(|e| format!("{e}; a build reads the fit's perm.npy"))?;
        if a.descr != "<i8" || a.shape.len() != 1 || a.shape[0] != rows {
            return Err(format!("{}: expected {rows} int64 entities (<i8, 1-D), found {} of shape {:?}; refit", self.perm_path, a.descr, a.shape));
        }
        let perm = a.i64s();
        if let Some(i) = perm.iter().position(|&e| !(0..=u32::MAX as i64).contains(&e)) {
            return Err(format!("{}: fit row {i} is entity {}, outside 0..2^32; refit", self.perm_path, perm[i]));
        }
        self.perm = perm;
        Ok(())
    }
}

pub struct RealSource {
    pub map: memmap2::Mmap,
    pub off: usize,
    pub n: usize,
}
impl RealSource {
    pub fn open(path: &str) -> RealSource {
        RealSource::try_open(path).unwrap_or_else(|e| panic!("{e}"))
    }
    /// The vectors file mapped, its header checked: f16 rows of n, the data as long as its shape says.
    pub fn try_open(path: &str) -> Result<RealSource, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        // SAFETY: the fit's vectors file is only read, and not changed while a build reads it (its identity is checked)
        let map = unsafe { memmap2::Mmap::map(&f).map_err(|e| format!("cannot map {path}: {e}"))? };
        if map.len() < 12 || &map[..6] != b"\x93NUMPY" {
            return Err(format!("{path} is not an .npy file"));
        }
        let (hdr, off) = npy_header(&map);
        let sh = npy_shape(&hdr);
        if !hdr.contains("<f2") || sh.len() != 2 || sh[0].checked_mul(sh[1]).and_then(|x| x.checked_mul(2)).and_then(|x| x.checked_add(off)).map_or(true, |e| e > map.len()) {
            return Err(format!("{path}: expected rows of float16 (<f2, 2-D) as long as its shape; refit"));
        }
        Ok(RealSource { map, off, n: sh[1] })
    }
    pub fn row(&self, r: usize) -> &[u16] {
        let n = self.n;
        let b = &self.map[self.off + r * n * 2..self.off + (r + 1) * n * 2];
        unsafe { std::slice::from_raw_parts(b.as_ptr() as *const u16, n) }
    }
}
