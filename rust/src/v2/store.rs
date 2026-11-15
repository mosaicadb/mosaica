//! The encoded index on disk. A build writes it; opening it maps the per-row arrays (codes, scalars, keys, row maps,
//! full vectors) and reads the per-group and per-cluster data, so no item is encoded again.
//!
//! A saved index is a directory of files: `code{l}.bin` and `scal{l}.bin` per level, `ahat.bin`, `row_ent.bin`,
//! `ent_row.bin`, `nukey.bin`, `vectors.bin` (the f16 vectors in index row order, padded to whole 4,096-byte pages so
//! that O_DIRECT reads of the last rows are not short), `offsets.bin`, `esc_key.bin` and `esc_val.bin` (the escapes),
//! `clusters.bin` (the per-cluster records and centre tables), `anchors.bin`, `hi.bin` and `lo.bin` (the support
//! table), and `index.json` (sizes, the per-group calibration, constants and basis defect, and the key of the fit and
//! options it was built from).

use super::*;
use std::io::Write;
use std::ops::Deref;

/// The version of the saved index; any other is refused.
pub const STORE_FORMAT: u64 = 5;

/// Element types that are stored as their bytes.
pub unsafe trait Plain: Copy + Send + Sync + 'static {}
unsafe impl Plain for u8 {}
unsafe impl Plain for i8 {}
unsafe impl Plain for u16 {}
unsafe impl Plain for i16 {}
unsafe impl Plain for u32 {}
unsafe impl Plain for u64 {}
unsafe impl Plain for f32 {}
unsafe impl Plain for f64 {}
unsafe impl Plain for [u16; 16] {}

fn bytes<T: Plain>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// A per-row array: on the heap after a build (and for a compacted index, which copies its rows), mapped from its
/// file once the index is saved and opened. It reads as a slice without a branch on where it is.
pub struct Arr<T: Plain> {
    ptr: *const T,
    len: usize,
    _keep: Keep<T>,
}
#[allow(dead_code)] // held only so that the buffer or mapping lives as long as the array
enum Keep<T> {
    Heap(Vec<T>),
    Map(memmap2::Mmap),
}
// the pointer is into the vector's heap buffer or the mapping, which the array owns and never changes
unsafe impl<T: Plain> Send for Arr<T> {}
unsafe impl<T: Plain> Sync for Arr<T> {}
impl<T: Plain> Deref for Arr<T> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}
impl<T: Plain> Default for Arr<T> {
    fn default() -> Self {
        Vec::new().into()
    }
}
impl<T: Plain> From<Vec<T>> for Arr<T> {
    fn from(v: Vec<T>) -> Self {
        Arr { ptr: v.as_ptr(), len: v.len(), _keep: Keep::Heap(v) }
    }
}
impl<T: Plain> FromIterator<T> for Arr<T> {
    fn from_iter<I: IntoIterator<Item = T>>(it: I) -> Self {
        it.into_iter().collect::<Vec<T>>().into()
    }
}

impl<T: Plain> Arr<T> {
    /// The array in `path`, mapped.
    pub fn map(path: &str) -> Result<Arr<T>, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let len = f.metadata().map_err(|e| format!("cannot read {path}: {e}"))?.len() as usize;
        if len % std::mem::size_of::<T>() != 0 {
            return Err(format!("{path} is {len} bytes, not a whole number of elements; rebuild the index"));
        }
        if len == 0 {
            return Ok(Arr::default());
        }
        let m = unsafe { memmap2::Mmap::map(&f).map_err(|e| format!("cannot map {path}: {e}"))? };
        Ok(Arr { ptr: m.as_ptr() as *const T, len: len / std::mem::size_of::<T>(), _keep: Keep::Map(m) })
    }
}

fn write_file<T: Plain>(path: &str, v: &[T]) {
    let mut f = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(path).unwrap_or_else(|e| panic!("cannot create {path}: {e}")));
    f.write_all(bytes(v)).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    f.flush().unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
}

/// A byte buffer of length-prefixed arrays and plain values.
#[derive(Default)]
struct Wr(Vec<u8>);
impl Wr {
    fn put<T: Plain>(&mut self, x: T) {
        self.0.extend_from_slice(bytes(std::slice::from_ref(&x)));
    }
    fn arr<T: Plain>(&mut self, v: &[T]) {
        self.put(v.len() as u64);
        self.0.extend_from_slice(bytes(v));
    }
}
struct Rd<'a>(&'a [u8]);
impl Rd<'_> {
    fn get<T: Plain>(&mut self) -> T {
        let s = std::mem::size_of::<T>();
        assert!(self.0.len() >= s, "clusters.bin ends inside a record; rebuild the index");
        let x = unsafe { std::ptr::read_unaligned(self.0.as_ptr() as *const T) };
        self.0 = &self.0[s..];
        x
    }
    fn arr<T: Plain>(&mut self) -> Vec<T> {
        let len = self.get::<u64>() as usize;
        let s = len * std::mem::size_of::<T>();
        assert!(self.0.len() >= s, "clusters.bin ends inside a record; rebuild the index");
        let mut v = Vec::<T>::with_capacity(len);
        unsafe {
            std::ptr::copy_nonoverlapping(self.0.as_ptr(), v.as_mut_ptr() as *mut u8, s);
            v.set_len(len);
        }
        self.0 = &self.0[s..];
        v
    }
}

fn put_cluster(w: &mut Wr, c: &Cl2, ctab: &[kern::CTab]) {
    for x in [c.g, c.r0, c.r1, c.nblk] {
        w.put(x as u64);
    }
    for x in [c.mn, c.btot, c.mvn, c.rmax, c.numax, c.bmin, c.bmax] {
        w.put(x);
    }
    w.put(c.mn32);
    w.arr(&c.bm);
    w.arr(&c.b2);
    w.arr(&c.l1m);
    w.arr(&c.mhat);
    w.arr(&c.mv32);
    w.arr(&c.wmr);
    w.put(ctab.len() as u64);
    for t in ctab {
        w.arr(&t.th);
        w.arr(&t.tmax);
        w.arr(&t.step);
        w.arr(&t.off);
        w.arr(&t.err);
    }
}

fn get_cluster(r: &mut Rd) -> (Cl2, Vec<kern::CTab>) {
    let [g, r0, r1, nblk] = [(); 4].map(|_| r.get::<u64>() as usize);
    let [mn, btot, mvn, rmax, numax, bmin, bmax] = [(); 7].map(|_| r.get::<f64>());
    let mn32 = r.get::<f32>();
    let bm: [f64; NDL] = r.arr::<f64>().try_into().expect("clusters.bin: a record of another level count; rebuild the index");
    let b2: [f64; J] = r.arr::<f64>().try_into().expect("clusters.bin: a record of another block count; rebuild the index");
    let l1m: [f32; J + 1] = r.arr::<f32>().try_into().expect("clusters.bin: a record of another block count; rebuild the index");
    let (mhat, mv32, wmr) = (r.arr(), r.arr(), r.arr());
    let nt = r.get::<u64>() as usize;
    let ctab = (0..nt).map(|_| kern::CTab { th: r.arr(), tmax: r.arr(), step: r.arr(), off: r.arr(), err: r.arr() }).collect();
    (Cl2 { g, r0, r1, nblk, mn, mn32, mhat: Arc::new(mhat), mv32: Arc::new(mv32), wmr: Arc::new(wmr), bm, b2, l1m, btot, mvn, rmax, numax, bmin, bmax }, ctab)
}

fn cal_json(c: &CalIn) -> serde_json::Value {
    let fin = c.y0.iter().chain(&c.ys).chain(c.k0.iter().flatten()).chain(c.ks.iter().flatten()).all(|x| x.is_finite()) && c.tq.iter().flatten().flatten().all(|x| x.is_finite());
    assert!(fin, "a calibration value is not finite and cannot be saved in JSON: {c:?}");
    serde_json::json!({
        "kmin": c.kmin, "y0": c.y0, "ys": c.ys,
        "tq": c.tq.iter().map(|a| a.iter().map(|b| b.to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "k0": c.k0, "ks": c.ks, "kb": c.kb,
    })
}

fn cal_from(v: &serde_json::Value) -> CalIn {
    let f = |x: &serde_json::Value| x.as_f64().expect("index.json: a calibration value is not a number; rebuild the index");
    let arr_f = |x: &serde_json::Value| -> [f64; J] { core::array::from_fn(|j| f(&x[j])) };
    let lr = v["k0"].as_array().map_or(0, |a| a.len());
    CalIn {
        kmin: core::array::from_fn(|l| v["kmin"][l].as_i64().expect("index.json: kmin") as i32),
        y0: arr_f(&v["y0"]),
        ys: arr_f(&v["ys"]),
        tq: (0..lr).map(|r| core::array::from_fn(|j| core::array::from_fn(|c| f(&v["tq"][r][j][c]) as f32))).collect(),
        k0: (0..lr).map(|r| arr_f(&v["k0"][r])).collect(),
        ks: (0..lr).map(|r| arr_f(&v["ks"][r])).collect(),
        kb: (0..lr).map(|r| core::array::from_fn(|j| v["kb"][r][j].as_u64().expect("index.json: kb") as u8)).collect(),
    }
}

/// The key a saved index answers to, from its `index.json`, or None when `dir` holds no finished index of this format.
pub fn saved_key(dir: &str) -> Option<String> {
    let s = std::fs::read_to_string(format!("{dir}/index.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    if v["format"].as_u64() != Some(STORE_FORMAT) {
        return None;
    }
    v["key"].as_str().map(|k| k.to_string())
}

fn index_json(ix: &Index2, dir: &str, key: Option<&str>) {
    let groups: Vec<serde_json::Value> = ix.groups.iter().map(|g| serde_json::json!({
        "e_inv": g.e_inv, "csgl": g.csgl, "cal": g.cal.as_ref().map(cal_json),
    })).collect();
    let meta = serde_json::json!({
        "format": STORE_FORMAT, "index_format": INDEX_FORMAT, "key": key, "n": ix.n(), "k": ix.k(), "nrows": ix.nrows,
        "npow": ix.meta.npow, "x2": ix.x2, "nanch": ix.nanch, "nent": ix.nent, "code_hash": format!("{:016x}", ix.code_hash()),
        "groups": groups,
    });
    let path = format!("{dir}/index.json");
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
}

/// The f16 vectors of `rows` (rows of `n`) written to `path` and padded to whole pages.
fn write_vectors(path: &str, rows: &disk::Rows, nrows: usize, n: usize) {
    let mut f = std::io::BufWriter::with_capacity(64 << 20, std::fs::File::create(path).unwrap_or_else(|e| panic!("cannot create {path}: {e}")));
    for r in 0..nrows {
        f.write_all(bytes(rows.row(r, n))).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    }
    let f = f.into_inner().unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    f.set_len(((nrows * n * 2 + 4095) & !4095) as u64).unwrap_or_else(|e| panic!("cannot size {path}: {e}"));
}

/// Write `ix` (a built index of real vectors, with its support table) into the empty directory `dir`, answering to
/// `key`. `index.json` is written last, so a directory with one is complete.
pub fn save(ix: &Index2, dir: &str, key: &str) {
    let Full2::Real { rows } = &ix.full else { panic!("a synthetic index has no stored vectors and cannot be saved") };
    assert!(!ix.compacted() && ix.vrow.is_empty(), "a compacted index is not saved; save the index it was compacted from");
    let f = |name: &str| format!("{dir}/{name}");
    for (l, (c, s)) in ix.lcode.iter().zip(&ix.lscal).enumerate() {
        write_file(&f(&format!("code{l}.bin")), c);
        write_file(&f(&format!("scal{l}.bin")), s);
    }
    write_file(&f("ahat.bin"), &ix.ahat);
    write_file(&f("row_ent.bin"), &ix.row_ent);
    write_file(&f("ent_row.bin"), &ix.ent_row);
    write_file(&f("nukey.bin"), &ix.nukey);
    write_vectors(&f("vectors.bin"), rows, ix.nrows, ix.n());
    let offs: Vec<u64> = ix.coff.iter().chain(&ix.soff).flatten().map(|&o| o as u64).collect();
    write_file(&f("offsets.bin"), &offs);
    let mut esc: Vec<(u64, f32)> = ix.esc.iter().map(|(&(l, s, r), &v)| (((l as u64) << 40) | ((s as u64) << 32) | r as u64, v)).collect();
    esc.sort_unstable_by_key(|e| e.0);
    write_file(&f("esc_key.bin"), &esc.iter().map(|e| e.0).collect::<Vec<_>>());
    write_file(&f("esc_val.bin"), &esc.iter().map(|e| e.1).collect::<Vec<_>>());
    let mut w = Wr::default();
    for (c, t) in ix.cl.iter().zip(ix.ctab.iter()) {
        put_cluster(&mut w, c, t);
    }
    write_file(&f("clusters.bin"), &w.0);
    write_file(&f("anchors.bin"), &ix.anchors);
    write_file(&f("hi.bin"), &ix.hi);
    write_file(&f("lo.bin"), &ix.lo);
    index_json(ix, dir, Some(key));
}

/// Open the index saved in `dir` for the fit `p`, profile `prof` and options `o` (with `design_c` derived from them),
/// which must be the ones it was built from (`build_key`). Nothing is encoded: the per-row arrays and the vectors are
/// mapped, the plans are derived from the fit as the build derives them, and the calibration, constants, basis defects,
/// per-cluster records and support table are read. The repack thresholds are left at their default.
pub fn open(p: &Params, prof: &[f64], o: &BuildOptions, design_c: Option<&[f64]>, dir: &str, log: &mut dyn FnMut(String)) -> Result<Index2, String> {
    let t = Timer::new();
    let f = |name: &str| format!("{dir}/{name}");
    let text = std::fs::read_to_string(f("index.json")).map_err(|e| format!("{dir} holds no saved index ({e}); rebuild it"))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{dir}/index.json: {e}; rebuild the index"))?;
    if v["format"].as_u64() != Some(STORE_FORMAT) {
        return Err(format!("{dir} holds an index of format {}, this build reads format {STORE_FORMAT}; rebuild it", v["format"]));
    }
    let us = |k: &str| v[k].as_u64().map(|x| x as usize).ok_or(format!("{dir}/index.json has no {k}; rebuild the index"));
    let (n, k, nrows, nanch, nent) = (us("n")?, us("k")?, us("nrows")?, us("nanch")?, us("nent")?);
    if n != p.meta.n || k != p.meta.k {
        return Err(format!("{dir} was built with {k} clusters of {n} dimensions, the fit has {} of {}; rebuild it", p.meta.k, p.meta.n));
    }
    let Plans { meta, rots, codes, et, mut groups, .. } = plans(p, prof, nrows, o, design_c, false, log);
    if Some(meta.npow as u64) != v["npow"].as_u64() {
        return Err(format!("{dir} was built for a guarantee of 2^{} rows, these options give 2^{}; rebuild it", v["npow"], meta.npow));
    }
    let gv = v["groups"].as_array().filter(|a| a.len() == groups.len()).ok_or(format!("{dir}/index.json does not hold {} groups; rebuild the index", groups.len()))?;
    for (gp, gv) in groups.iter_mut().zip(gv) {
        gp.e_inv = gv["e_inv"].as_f64().ok_or(format!("{dir}/index.json: a group has no e_inv; rebuild the index"))?;
        let csgl: Vec<f32> = gv["csgl"].as_array().map_or(Vec::new(), |a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect());
        if csgl.len() != gp.csgl.len() {
            return Err(format!("{dir}/index.json: a group has {} randomised constants, the plan {}; rebuild the index", csgl.len(), gp.csgl.len()));
        }
        gp.csgl = csgl;
        if !gv["cal"].is_null() {
            gp.set_calibration(cal_from(&gv["cal"]));
        }
    }
    let nl = meta.nl();
    let cbuf = std::fs::read(f("clusters.bin")).map_err(|e| format!("cannot read {dir}/clusters.bin: {e}; rebuild the index"))?;
    let mut r = Rd(&cbuf);
    let (cl, ctab): (Vec<Cl2>, Vec<Vec<kern::CTab>>) = (0..k).map(|_| get_cluster(&mut r)).unzip();
    if !r.0.is_empty() {
        return Err(format!("{dir}/clusters.bin has data past its {k} clusters; rebuild the index"));
    }
    let offs: Arr<u64> = Arr::map(&f("offsets.bin"))?;
    if offs.len() != 2 * nl * k {
        return Err(format!("{dir}/offsets.bin does not hold {nl} levels of {k} clusters; rebuild the index"));
    }
    let off = |i: usize| -> Vec<Vec<usize>> { (0..nl).map(|l| offs[(i * nl + l) * k..(i * nl + l + 1) * k].iter().map(|&o| o as usize).collect()).collect() };
    let (ek, ev): (Arr<u64>, Arr<f32>) = (Arr::map(&f("esc_key.bin"))?, Arr::map(&f("esc_val.bin"))?);
    if ek.len() != ev.len() {
        return Err(format!("{dir}: esc_key.bin and esc_val.bin differ in length; rebuild the index"));
    }
    let esc = ek.iter().zip(ev.iter()).map(|(&k, &v)| (((k >> 40) as u8, (k >> 32) as u8, k as u32), v)).collect();
    let anchors: Vec<f64> = Arr::<f64>::map(&f("anchors.bin"))?.to_vec();
    let (hi, lo): (Vec<f32>, Vec<f32>) = (Arr::<f32>::map(&f("hi.bin"))?.to_vec(), Arr::<f32>::map(&f("lo.bin"))?.to_vec());
    if anchors.len() != nanch * n || hi.len() != nanch * k || lo.len() != nanch * k {
        return Err(format!("{dir}: the support table does not hold {nanch} anchors of {n} and their bounds over {k} clusters; rebuild the index"));
    }
    let lcode: Vec<Arr<u8>> = (0..nl).map(|l| Arr::map(&f(&format!("code{l}.bin")))).collect::<Result<_, _>>()?;
    let lscal: Vec<Arr<u8>> = (0..nl).map(|l| Arr::map(&f(&format!("scal{l}.bin")))).collect::<Result<_, _>>()?;
    let (coff, soff) = (off(0), off(1));
    for l in 0..nl {
        // each level ends in 128 bytes of slack
        let (ce, se) = (coff[l].last().map_or(0, |&o| o), soff[l].last().map_or(0, |&o| o));
        if lcode[l].len() < ce + 128 || lscal[l].len() < se + 128 {
            return Err(format!("{dir}: level {l}'s codes or scalars are shorter than its offsets; rebuild the index"));
        }
    }
    let (ahat, row_ent, ent_row, nukey) = (Arr::map(&f("ahat.bin"))?, Arr::map(&f("row_ent.bin"))?, Arr::map(&f("ent_row.bin"))?, Arr::map(&f("nukey.bin"))?);
    if ahat.len() != nrows + 32 || row_ent.len() != nrows || ent_row.len() != nent || !(nukey.is_empty() || nukey.len() == nrows) || cl.last().map_or(0, |c| c.r1) != nrows {
        return Err(format!("{dir}: the row arrays do not hold {nrows} rows; rebuild the index"));
    }
    if nukey.is_empty() == o.nusort {
        return Err(format!("{dir} was built {} the norm sort, the options ask {}; rebuild it", if o.nusort { "without" } else { "with" }, if o.nusort { "for it" } else { "for none" }));
    }
    let rows = disk::Rows::open(&f("vectors.bin"), nrows * n, "normal").map_err(|e| format!("{e}; rebuild the index"))?;
    let ix = Index2 {
        nukey,
        repack: vec![0.6; nl],
        ctab: Arc::new(ctab),
        meta,
        groups: Arc::new(groups),
        codes: Arc::new(codes),
        etab: Arc::new(et),
        cl,
        centres64: Arc::new(p.centres[..k * n].iter().map(|&x| x as f64).collect()),
        rots: Arc::new(rots),
        anchors: Arc::new(anchors),
        hi: Arc::new(hi),
        lo: Arc::new(lo),
        nanch,
        ahat,
        row_ent,
        ent_row,
        lcode,
        lscal,
        coff,
        soff,
        esc,
        x2: v["x2"].as_f64().ok_or(format!("{dir}/index.json has no x2; rebuild the index"))?,
        full: Full2::Real { rows: Arc::new(rows) },
        nrows,
        vrow: Vec::new(),
        nent,
    };
    log(format!("opened {dir} in {:.1}s: {nrows} rows, {k} clusters; code hash when built {}", t.s(), v["code_hash"].as_str().unwrap_or("?")));
    Ok(ix)
}

/// The key of the index the fit `p` (with the options applied), the profile `prof` and the options `o` build: a saved
/// index is opened only when its key is this one. It covers every input of the build: the fit's parameters, the
/// profile, the identity of the vectors file (its path, length and modification time, not its contents), the options
/// that change what is built, and the formats of the encoding and of the files.
pub fn build_key(p: &Params, prof: &[f64], o: &BuildOptions) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let f64s = |v: &[f64], h: &mut std::collections::hash_map::DefaultHasher| v.iter().for_each(|x| x.to_bits().hash(h));
    let m = &p.meta;
    (STORE_FORMAT, INDEX_FORMAT, m.n, m.k, m.g, &m.dlv, &m.rlv, &m.bounds, m.csg.to_bits(), m.eps.to_bits()).hash(&mut h);
    for c in &m.certs {
        (&c.ds, c.lnd.to_bits(), c.x.to_bits()).hash(&mut h);
    }
    (&p.cstart, &p.grp, &p.perm, &p.fit).hash(&mut h);
    p.centres.iter().chain(&p.v).for_each(|x| x.to_bits().hash(&mut h));
    for v in [&p.lam, &p.tree_edges, &p.tree_cent, &p.gtree] {
        f64s(v, &mut h);
    }
    if let Some(r2) = &p.r2 {
        f64s(r2, &mut h);
    }
    f64s(prof, &mut h);
    let md = std::fs::metadata(&p.xs_path).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.xs_path));
    (&p.xs_path, md.len(), md.modified().ok()).hash(&mut h);
    // the options not already in the fit and profile above (the clusters, randomised levels and profile are)
    (o.design_exact, o.nusort, o.nguar.map(|x| x.to_bits()), o.csg_plain, o.cal_per_group).hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The calibration an opened index sets its grids from is the build's, bit for bit.
    #[test]
    fn calibration_round_trips_exactly() {
        use rand::{Rng, SeedableRng};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(5);
        let mut f = || -> f64 { rng.gen::<f64>() * 10f64.powi(rng.gen_range(-30..30)) * if rng.gen() { -1.0 } else { 1.0 } };
        let nr = 4;
        let cal = CalIn {
            kmin: core::array::from_fn(|l| l as i32 * 7 - 20),
            y0: core::array::from_fn(|_| f()),
            ys: core::array::from_fn(|_| f()),
            tq: (0..nr).map(|_| core::array::from_fn(|_| core::array::from_fn(|_| f() as f32))).collect(),
            k0: (0..nr).map(|_| core::array::from_fn(|_| f())).collect(),
            ks: (0..nr).map(|_| core::array::from_fn(|_| f())).collect(),
            kb: (0..nr).map(|_| core::array::from_fn(|j| if j % 2 == 0 { 4 } else { 8 })).collect(),
        };
        let text = serde_json::to_string(&cal_json(&cal)).unwrap();
        let back = cal_from(&serde_json::from_str(&text).unwrap());
        assert_eq!(format!("{:?}", back), format!("{:?}", cal));
        let bits = |c: &CalIn| -> Vec<u64> {
            c.y0.iter().chain(&c.ys).chain(c.k0.iter().flatten()).chain(c.ks.iter().flatten()).map(|x| x.to_bits())
                .chain(c.tq.iter().flatten().flatten().map(|x| x.to_bits() as u64)).collect()
        };
        assert_eq!(bits(&back), bits(&cal));
    }
}
