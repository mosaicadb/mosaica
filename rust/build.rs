//! The library's source fingerprint: a fixed hash of the compiler's version (`rustc -V`), `Cargo.lock` (the
//! dependencies' versions) and every `.rs` file under `src/` that the library compiles outside its tests, by sorted
//! path and contents. Left out: the binary's own
//! modules (`main.rs`, `run2.rs`, `ann.rs`, `scale.rs`) and `v2/e2e.rs`, which is compiled only for tests; test modules
//! inside library files stay in (comments too: stripping them would be fragile). A saved index's key includes it, so
//! an edit to the encoder (or to anything else in the library) makes a saved index stale instead of serving old
//! codes. Any library edit invalidates saved indexes, which is the safe direction.

use std::path::{Path, PathBuf};

// the binary's own modules (declared in main.rs), which do not change what the library builds
// and the test-only end-to-end module (`#[cfg(test)] mod e2e;` in v2/mod.rs)
const LEFT_OUT: [&str; 5] = ["main.rs", "run2.rs", "ann.rs", "scale.rs", "v2/e2e.rs"];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display())) {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

// the same word hash as `v2::store::Fx`
struct Fx(u64);
impl Fx {
    fn u64(&mut self, x: u64) {
        self.0 = (self.0 ^ x).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u64(b.len() as u64);
        let mut c = b.chunks_exact(8);
        for w in &mut c {
            self.u64(u64::from_le_bytes(w.try_into().unwrap()));
        }
        if !c.remainder().is_empty() {
            let mut t = [0u8; 8];
            t[..c.remainder().len()].copy_from_slice(c.remainder());
            self.u64(u64::from_le_bytes(t));
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = root.join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    let mut rel: Vec<(String, PathBuf)> = files.into_iter().map(|p| (p.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/"), p)).filter(|(r, _)| !LEFT_OUT.contains(&r.as_str())).collect();
    rel.sort();
    let mut h = Fx(0);
    let lock = root.join("Cargo.lock");
    h.bytes(b"Cargo.lock");
    h.bytes(&std::fs::read(&lock).unwrap_or_else(|e| panic!("cannot read {}: {e}; the source fingerprint covers the dependencies' versions", lock.display())));
    // the compiler's version
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = std::process::Command::new(&rustc).arg("-V").output().unwrap_or_else(|e| panic!("cannot run {rustc} -V: {e}"));
    assert!(out.status.success(), "{rustc} -V failed");
    h.bytes(&out.stdout);
    for (r, p) in &rel {
        h.bytes(r.as_bytes());
        h.bytes(&std::fs::read(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display())));
    }
    println!("cargo:rustc-env=VEXACT_LIB_FINGERPRINT={:016x}", h.0);
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build.rs");
}
