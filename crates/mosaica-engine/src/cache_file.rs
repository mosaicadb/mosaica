//! Entries in the engine's cache directory: a magic, the payload's length and its SHA-256, then
//! the payload, so a torn or altered file is a miss. Each is named by a stem and a version, and
//! written through [`mosaica_authz::write_private_atomically`], as the fragments are. The
//! directories are held under byte bounds: after a write the files least recently written or read
//! go first.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const HEADER: usize = 8 + 8 + 32;

/// The file of the entry `stem` at `at`.
pub(crate) fn path_of(dir: &Path, stem: &str, at: u64, extension: &str) -> PathBuf {
    dir.join(format!("{stem}-{at}.{extension}"))
}

/// Remove the files under `root` least recently written or read until they hold at most `bound`
/// bytes.
pub(crate) fn hold_under(root: &Path, bound: u64) {
    fn walk(dir: &Path, out: &mut Vec<(std::time::SystemTime, u64, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                walk(&entry.path(), out);
            } else {
                let at = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                out.push((at, meta.len(), entry.path()));
            }
        }
    }
    let mut files = Vec::new();
    walk(root, &mut files);
    let mut held: u64 = files.iter().map(|(_, len, _)| len).sum();
    files.sort();
    for (_, len, path) in files {
        if held <= bound {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            held -= len;
        }
    }
}

/// The bytes the files under `root` hold.
pub(crate) fn bytes_under(root: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => bytes_under(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Write `payload` as the entry `stem` at `at`, and remove the entry's other versions.
pub(crate) fn write(
    dir: &Path,
    stem: &str,
    at: u64,
    extension: &str,
    magic: &[u8; 8],
    payload: &[u8],
) {
    if !write_beside(dir, stem, at, extension, magic, payload) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let prefix = format!("{stem}-");
    let suffix = format!(".{extension}");
    let current = format!("{stem}-{at}.{extension}");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(&suffix) && name != current {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Write `payload` as the entry `stem` at `at`, beside the entry's other versions. `false` where
/// it could not be written.
pub(crate) fn write_beside(
    dir: &Path,
    stem: &str,
    at: u64,
    extension: &str,
    magic: &[u8; 8],
    payload: &[u8],
) -> bool {
    let mut bytes = Vec::with_capacity(HEADER + payload.len());
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&Sha256::digest(payload));
    bytes.extend_from_slice(payload);
    let path = path_of(dir, stem, at, extension);
    if let Err(error) = mosaica_authz::write_private_atomically(&path, &bytes) {
        tracing::warn!(
            path = %path.display(),
            %error,
            "an entry could not be written to the cache directory; a restart computes it again"
        );
        return false;
    }
    true
}

/// The payload of the entry `stem` at `at`, where a whole and unaltered one is on disk. A read marks
/// the file used, so the byte bound removes it after the files nobody has read.
pub(crate) fn read(
    dir: &Path,
    stem: &str,
    at: u64,
    extension: &str,
    magic: &[u8; 8],
) -> Option<Vec<u8>> {
    let path = path_of(dir, stem, at, extension);
    let bytes = std::fs::read(&path).ok()?;
    if let Ok(file) = std::fs::File::options().append(true).open(&path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
    payload_of(&bytes, magic)
}

/// The payload of a file's `bytes`, where they are whole, unaltered and of `magic`.
pub(crate) fn payload_of(bytes: &[u8], magic: &[u8; 8]) -> Option<Vec<u8>> {
    if bytes.len() < HEADER || &bytes[..8] != magic {
        return None;
    }
    let length = u64::from_le_bytes(bytes[8..16].try_into().ok()?) as usize;
    let payload = &bytes[HEADER..];
    if payload.len() != length || Sha256::digest(payload)[..] != bytes[16..48] {
        return None;
    }
    Some(payload.to_vec())
}

