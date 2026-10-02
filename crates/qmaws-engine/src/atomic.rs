//! Atomic, hash-verified file writes (see `docs/DESIGN.md`).
//!
//! Every output file `<name>` is written as `<name>.tmp`, flushed and synced to
//! disk, then renamed to `<name>`. Its SHA-256 is written the same way to
//! `<name>.sha256`. A file is valid only if both exist and the hash matches, so
//! a process killed at any moment never leaves a file that looks valid but is
//! not.

use crate::hash::sha256_hex;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Suffix of temporary files written before the final rename.
pub const TMP_SUFFIX: &str = ".tmp";
/// Suffix of the file that holds the SHA-256 of its companion file.
pub const HASH_SUFFIX: &str = ".sha256";

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Path of the hash file belonging to `path`.
pub fn hash_path(path: &Path) -> PathBuf {
    with_suffix(path, HASH_SUFFIX)
}

/// Writes `bytes` to `path` atomically: temporary file, flush, sync, rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = with_suffix(path, TMP_SUFFIX);
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    sync_parent(path);
    Ok(())
}

/// Writes `bytes` to `path` and its SHA-256 to `<path>.sha256`, both
/// atomically. The data file is written first, so a missing or stale hash file
/// always makes the pair invalid. Returns the hash.
pub fn write_verified(path: &Path, bytes: &[u8]) -> io::Result<String> {
    let hash = sha256_hex(bytes);
    // Remove an old hash first: if the process dies after the data rename, the
    // pair is then invalid rather than pairing new data with an old hash.
    match fs::remove_file(hash_path(path)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    write_atomic(path, bytes)?;
    write_atomic(&hash_path(path), format!("{hash}\n").as_bytes())?;
    Ok(hash)
}

/// Reads the stored hash of `path`, if its hash file exists and is well formed.
pub fn stored_hash(path: &Path) -> Option<String> {
    let text = fs::read_to_string(hash_path(path)).ok()?;
    let hash = text.trim();
    let well_formed = hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit());
    well_formed.then(|| hash.to_ascii_lowercase())
}

/// Reads `path` and returns its contents only if they match the stored hash.
pub fn read_verified(path: &Path) -> Option<Vec<u8>> {
    let expected = stored_hash(path)?;
    let bytes = fs::read(path).ok()?;
    (sha256_hex(&bytes) == expected).then_some(bytes)
}

/// True if `path` exists and matches its stored hash.
pub fn is_valid(path: &Path) -> bool {
    read_verified(path).is_some()
}

/// Deletes leftover `*.tmp` files in `dir` (not recursive). They are the
/// remains of writes interrupted before their rename and are never used.
pub fn remove_stale_tmp(dir: &Path) -> io::Result<usize> {
    let mut removed = 0;
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let path = entry?.path();
        let is_tmp = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(TMP_SUFFIX));
        if is_tmp && path.is_file() {
            fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// On Unix, syncs the parent directory so the rename itself is durable.
/// Directory sync is not available on Windows, where the rename is already
/// durable once it returns.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn write_then_read_verified() {
        let dir = TempDir::new("atomic_rw");
        let path = dir.path().join("out.bin");
        let hash = write_verified(&path, b"hello").unwrap();
        assert_eq!(hash, sha256_hex(b"hello"));
        assert_eq!(read_verified(&path).unwrap(), b"hello");
        assert!(!with_suffix(&path, TMP_SUFFIX).exists());
    }

    #[test]
    fn overwrite_replaces_content_and_hash() {
        let dir = TempDir::new("atomic_overwrite");
        let path = dir.path().join("out.bin");
        write_verified(&path, b"first").unwrap();
        write_verified(&path, b"second").unwrap();
        assert_eq!(read_verified(&path).unwrap(), b"second");
    }

    #[test]
    fn corrupted_or_unhashed_files_are_invalid() {
        let dir = TempDir::new("atomic_invalid");
        let path = dir.path().join("out.bin");
        write_verified(&path, b"data").unwrap();
        fs::write(&path, b"datA").unwrap();
        assert!(!is_valid(&path));

        let bare = dir.path().join("bare.bin");
        fs::write(&bare, b"no hash file").unwrap();
        assert!(!is_valid(&bare));

        let garbage = dir.path().join("garbage.bin");
        fs::write(&garbage, b"x").unwrap();
        fs::write(hash_path(&garbage), b"not a hash").unwrap();
        assert!(!is_valid(&garbage));
    }

    #[test]
    fn stale_tmp_files_are_removed() {
        let dir = TempDir::new("atomic_stale");
        fs::write(dir.path().join("a.bin.tmp"), b"partial").unwrap();
        fs::write(dir.path().join("keep.bin"), b"keep").unwrap();
        assert_eq!(remove_stale_tmp(dir.path()).unwrap(), 1);
        assert!(dir.path().join("keep.bin").exists());
        assert_eq!(remove_stale_tmp(&dir.path().join("missing")).unwrap(), 0);
    }
}
