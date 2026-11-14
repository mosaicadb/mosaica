//! The encoded index on disk. A build writes it; opening it maps the per-row arrays (codes, scalars, keys, row maps,
//! full vectors) and reads the per-group and per-cluster data, so no item is encoded again.

use super::*;
use std::io::Write;
use std::marker::PhantomData;
use std::ops::Deref;

/// The version of the saved index; any other is refused.
pub const STORE_FORMAT: u64 = 4;

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

/// A per-row array: on the heap while an index is built, mapped from its file once the index is saved.
pub enum Arr<T: Plain> {
    Heap(Vec<T>),
    Map(memmap2::Mmap, usize, PhantomData<T>),
}
impl<T: Plain> Deref for Arr<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        match self {
            Arr::Heap(v) => v,
            Arr::Map(m, len, _) => unsafe { std::slice::from_raw_parts(m.as_ptr() as *const T, *len) },
        }
    }
}
impl<T: Plain> Default for Arr<T> {
    fn default() -> Self {
        Arr::Heap(Vec::new())
    }
}
impl<T: Plain> From<Vec<T>> for Arr<T> {
    fn from(v: Vec<T>) -> Self {
        Arr::Heap(v)
    }
}
/// Whether an opened index reads its per-row arrays onto the heap instead of mapping them (`--resident`).
pub static RESIDENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl<T: Plain> Arr<T> {
    /// The array in `path`: mapped, or read onto the heap with RESIDENT.
    fn load(path: &str) -> Arr<T> {
        if !RESIDENT.load(std::sync::atomic::Ordering::Relaxed) {
            return Arr::map(path);
        }
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let s = std::mem::size_of::<T>();
        assert!(b.len() % s == 0, "{path} is {} bytes, not a whole number of elements; rebuild the index", b.len());
        let mut v = Vec::<T>::with_capacity(b.len() / s);
        unsafe {
            std::ptr::copy_nonoverlapping(b.as_ptr(), v.as_mut_ptr() as *mut u8, b.len());
            v.set_len(b.len() / s);
        }
        Arr::Heap(v)
    }
    fn map(path: &str) -> Arr<T> {
        let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("cannot open {path}: {e}"));
        let len = f.metadata().unwrap().len() as usize;
        assert!(len % std::mem::size_of::<T>() == 0, "{path} is {len} bytes, not a whole number of elements; rebuild the index");
        if len == 0 {
            return Arr::Heap(Vec::new());
        }
        let m = unsafe { memmap2::Mmap::map(&f).unwrap_or_else(|e| panic!("cannot map {path}: {e}")) };
        Arr::Map(m, len / std::mem::size_of::<T>(), PhantomData)
    }
}

fn write_file<T: Plain>(path: &str, v: &[T]) {
    let mut f = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(path).unwrap_or_else(|e| panic!("cannot create {path}: {e}")));
    f.write_all(bytes(v)).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    f.flush().unwrap();
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
        let x = unsafe { std::ptr::read_unaligned(self.0.as_ptr() as *const T) };
        self.0 = &self.0[s..];
        x
    }
    fn arr<T: Plain>(&mut self) -> Vec<T> {
        let len = self.get::<u64>() as usize;
        let s = len * std::mem::size_of::<T>();
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
    w.put(c.smax);
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
    let (mn32, smax) = (r.get::<f32>(), r.get::<f32>());
    let bm: [f64; NDL] = r.arr::<f64>().try_into().unwrap();
    let b2: [f64; J] = r.arr::<f64>().try_into().unwrap();
    let l1m: [f32; J + 1] = r.arr::<f32>().try_into().unwrap();
    let (mhat, mv32, wmr) = (r.arr(), r.arr(), r.arr());
    let nt = r.get::<u64>() as usize;
    let ctab = (0..nt).map(|_| kern::CTab { th: r.arr(), tmax: r.arr(), step: r.arr(), off: r.arr(), err: r.arr() }).collect();
    (Cl2 { g, r0, r1, nblk, mn, mn32, mhat, mv32, wmr, bm, b2, l1m, btot, mvn, rmax, smax, numax, bmin, bmax }, ctab)
}

fn cal_json(c: &CalIn) -> serde_json::Value {
    serde_json::json!({
        "kmin": c.kmin, "y0": c.y0, "ys": c.ys,
        "tq": c.tq.iter().map(|a| a.iter().map(|b| b.to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "k0": c.k0, "ks": c.ks, "kb": c.kb,
    })
}

fn cal_from(v: &serde_json::Value) -> CalIn {
    let f = |x: &serde_json::Value| x.as_f64().unwrap();
    let arr_f = |x: &serde_json::Value| -> [f64; J] { core::array::from_fn(|j| f(&x[j])) };
    let lr = v["k0"].as_array().unwrap().len();
    CalIn {
        kmin: core::array::from_fn(|l| v["kmin"][l].as_i64().unwrap() as i32),
        y0: arr_f(&v["y0"]),
        ys: arr_f(&v["ys"]),
        tq: (0..lr).map(|r| core::array::from_fn(|j| core::array::from_fn(|c| f(&v["tq"][r][j][c]) as f32))).collect(),
        k0: (0..lr).map(|r| arr_f(&v["k0"][r])).collect(),
        ks: (0..lr).map(|r| arr_f(&v["ks"][r])).collect(),
        kb: (0..lr).map(|r| core::array::from_fn(|j| v["kb"][r][j].as_u64().unwrap() as u8)).collect(),
    }
}

/// The key a saved index answers to, from its `index.json`, or None when there is no saved index in `dir`.
pub fn saved_key(dir: &str) -> Option<String> {
    let s = std::fs::read_to_string(format!("{dir}/index.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    if v["format"].as_u64() != Some(STORE_FORMAT) {
        return None;
    }
    v["key"].as_str().map(|k| k.to_string())
}

/// Write `ix` to `dir`, replacing what is there. `key` names the fit and options it was built from.
pub fn save(ix: &Index2, dir: &str, key: &str) {
    assert!(ix.col.is_none(), "an index with the columnar store is not saved; build without it to save");
    let rows = match &ix.full {
        Full2::Real { rows } => rows,
        Full2::Synth(_) => panic!("a synthetic index has no stored vectors and is not saved"),
    };
    let tmp = format!("{dir}.tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap_or_else(|e| panic!("cannot create {tmp}: {e}"));
    let f = |name: &str| format!("{tmp}/{name}");
    let nl = ix.meta.nl();
    write_file(&f("ahat.bin"), &ix.ahat);
    write_file(&f("row_ent.bin"), &ix.row_ent);
    write_file(&f("ent_row.bin"), &ix.ent_row);
    write_file(&f("nukey.bin"), &ix.nukey);
    write_file(&f("vectors.bin"), rows);
    for l in 0..nl {
        write_file(&f(&format!("code{l}.bin")), &ix.lcode[l]);
        write_file(&f(&format!("scal{l}.bin")), &ix.lscal[l]);
    }
    let offs: Vec<u64> = ix.coff.iter().chain(&ix.soff).flatten().map(|&o| o as u64).collect();
    write_file(&f("offsets.bin"), &offs);
    let mut esc: Vec<(u64, f32)> = ix.esc.iter().map(|(&(l, s, r), &v)| (((l as u64) << 40) | ((s as u64) << 32) | r as u64, v)).collect();
    esc.sort_unstable_by_key(|e| e.0);
    write_file(&f("esc_key.bin"), &esc.iter().map(|e| e.0).collect::<Vec<_>>());
    write_file(&f("esc_val.bin"), &esc.iter().map(|e| e.1).collect::<Vec<_>>());
    let mut w = Wr::default();
    for (c, t) in ix.cl.iter().zip(&ix.ctab) {
        put_cluster(&mut w, c, t);
    }
    write_file(&f("clusters.bin"), &w.0);
    write_file(&f("anchors.bin"), &ix.anchors);
    write_file(&f("hi.bin"), &ix.hi);
    write_file(&f("lo.bin"), &ix.lo);
    let groups: Vec<serde_json::Value> = ix.groups.iter().map(|g| serde_json::json!({
        "e_inv": g.e_inv, "csgl": g.csgl, "cal": g.cal.as_ref().map(cal_json),
    })).collect();
    let meta = serde_json::json!({
        "format": STORE_FORMAT, "key": key, "n": ix.n(), "k": ix.k(), "nrows": ix.nrows, "npow": ix.meta.npow,
        "x2": ix.x2, "nanch": ix.nanch, "code_hash": format!("{:016x}", ix.code_hash()), "groups": groups,
    });
    std::fs::write(f("index.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    std::fs::rename(&tmp, dir).unwrap_or_else(|e| panic!("cannot move {tmp} to {dir}: {e}"));
}

/// Open the index saved in `dir` for the fit `p` and profile `prof`, which must be the ones it was built from.
pub fn open(p: &Params, prof: &[f64], dir: &str, split: bool, log: &mut dyn FnMut(String)) -> Index2 {
    let t = Timer::new();
    let f = |name: &str| format!("{dir}/{name}");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(f("index.json")).unwrap()).unwrap();
    assert_eq!(v["format"].as_u64(), Some(STORE_FORMAT), "{dir} holds an index of another format; rebuild it");
    let us = |k: &str| v[k].as_u64().unwrap() as usize;
    let (n, k, nrows) = (us("n"), us("k"), us("nrows"));
    assert!(n == p.meta.n && k == p.meta.k, "{dir} was built with {k} clusters of {n} dimensions, the fit has {} of {}; rebuild it", p.meta.k, p.meta.n);
    let Plans { meta, rots, codes, et, mut groups } = plans(p, prof, nrows, false, log);
    assert_eq!(meta.npow as u64, v["npow"].as_u64().unwrap(), "{dir} was built for another guarantee size; rebuild it");
    for (gp, gv) in groups.iter_mut().zip(v["groups"].as_array().unwrap()) {
        gp.e_inv = gv["e_inv"].as_f64().unwrap();
        gp.csgl = gv["csgl"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect();
        if !gv["cal"].is_null() {
            gp.set_calibration(cal_from(&gv["cal"]));
        }
    }
    let nl = meta.nl();
    let cbuf = std::fs::read(f("clusters.bin")).unwrap();
    let mut r = Rd(&cbuf);
    let (cl, ctab): (Vec<Cl2>, Vec<Vec<kern::CTab>>) = (0..k).map(|_| get_cluster(&mut r)).unzip();
    assert!(r.0.is_empty(), "{dir}/clusters.bin has data past its {k} clusters; rebuild the index");
    let offs: Arr<u64> = Arr::map(&f("offsets.bin"));
    assert_eq!(offs.len(), 2 * nl * k, "{dir}/offsets.bin does not hold {nl} levels of {k} clusters; rebuild the index");
    let off = |i: usize| -> Vec<Vec<usize>> { (0..nl).map(|l| offs[(i * nl + l) * k..(i * nl + l + 1) * k].iter().map(|&o| o as usize).collect()).collect() };
    let (ek, ev): (Arr<u64>, Arr<f32>) = (Arr::map(&f("esc_key.bin")), Arr::map(&f("esc_val.bin")));
    let esc = ek.iter().zip(ev.iter()).map(|(&k, &v)| (((k >> 40) as u8, (k >> 32) as u8, k as u32), v)).collect();
    let anchors: Vec<f64> = Arr::<f64>::map(&f("anchors.bin")).to_vec();
    let ix = Index2 {
        col: None,
        nukey: Arr::load(&f("nukey.bin")),
        repack: vec![0.6; nl],
        ctab,
        meta,
        groups,
        codes,
        etab: et,
        cl,
        centres64: p.centres[..k * n].iter().map(|&x| x as f64).collect(),
        rots,
        nanch: us("nanch"),
        anchors,
        hi: Arr::<f32>::map(&f("hi.bin")).to_vec(),
        lo: Arr::<f32>::map(&f("lo.bin")).to_vec(),
        ahat: Arr::load(&f("ahat.bin")),
        row_ent: Arr::load(&f("row_ent.bin")),
        ent_row: Arr::load(&f("ent_row.bin")),
        lcode: (0..nl).map(|l| Arr::load(&f(&format!("code{l}.bin")))).collect(),
        lscal: (0..nl).map(|l| Arr::load(&f(&format!("scal{l}.bin")))).collect(),
        coff: off(0),
        soff: off(1),
        esc,
        x2: v["x2"].as_f64().unwrap(),
        full: Full2::Real { rows: Arr::load(&f("vectors.bin")) },
        nrows,
        split,
    };
    if let Full2::Real { rows } = &ix.full {
        assert_eq!(rows.len(), nrows * n, "{dir}/vectors.bin does not hold {nrows} rows of {n}; rebuild the index");
    }
    log(format!("opened {dir} in {:.1}s: {nrows} rows, {k} clusters; code hash when built {}", t.s(), v["code_hash"].as_str().unwrap_or("?")));
    ix
}

/// The key of the index `p` and `prof` build under the current options: a saved index is opened in place of a build
/// only when its key is this one. `extra` names any other option that changes what is built.
pub fn build_key(p: &Params, prof: &[f64], extra: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let f64s = |v: &[f64], h: &mut std::collections::hash_map::DefaultHasher| v.iter().for_each(|x| x.to_bits().hash(h));
    let m = &p.meta;
    (STORE_FORMAT, m.n, m.k, m.g, &m.dlv, &m.rlv, &m.bounds, m.csg.to_bits(), m.eps.to_bits()).hash(&mut h);
    for c in &m.certs {
        (&c.ds, c.lnd.to_bits(), c.x.to_bits()).hash(&mut h);
    }
    (&p.cstart, &p.grp, &p.perm, &p.fit, &p.frb, &p.ridx).hash(&mut h);
    p.centres.iter().chain(&p.v).for_each(|x| x.to_bits().hash(&mut h));
    for v in [&p.lam, &p.dl, &p.dt, &p.rates, &p.tree_edges, &p.tree_cent, &p.gtree] {
        f64s(v, &mut h);
    }
    if let Some(r2) = &p.r2 {
        f64s(r2, &mut h);
    }
    f64s(prof, &mut h);
    // the vectors: their file's identity, not its contents
    let md = std::fs::metadata(&p.xs_path).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.xs_path));
    (&p.xs_path, md.len(), md.modified().ok()).hash(&mut h);
    let cal = std::env::var("VEXACT_CAL_ITEMS").unwrap_or_default();
    let nguar = NGUAR.get().map(|x| x.to_bits());
    let plain = CSG_PLAIN.load(std::sync::atomic::Ordering::Relaxed);
    let design: Option<Vec<u64>> = DESIGN_C.get().map(|v| v.iter().map(|x| x.to_bits()).collect());
    (cal, nguar, plain, design, extra).hash(&mut h);
    format!("{:016x}", h.finish())
}
