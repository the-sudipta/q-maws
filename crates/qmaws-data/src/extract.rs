//! Safe, atomic extraction of zip archives.
//!
//! The archive is extracted into `<dest>.extracting`, then renamed to
//! `<dest>`. A marker file inside the destination records which archive
//! (by SHA-256) was extracted and how many files it held, so an interrupted
//! or outdated extraction is detected and repeated. Entries that would leave
//! the destination folder (for example `../x`) are refused.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

/// Name of the marker file written into an extracted folder.
pub const MARKER: &str = ".qmaws-extracted.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub archive_sha256: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug)]
pub enum ExtractError {
    Io(PathBuf, io::Error),
    Zip(String),
    UnsafeEntry(String),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::Io(p, e) => write!(f, "could not extract to {}: {e}", p.display()),
            ExtractError::Zip(e) => write!(f, "the archive could not be read: {e}"),
            ExtractError::UnsafeEntry(name) => write!(
                f,
                "the archive contains an entry outside its folder ({name}); it was not extracted"
            ),
        }
    }
}

impl std::error::Error for ExtractError {}

/// The marker of an extracted folder, if present and readable.
pub fn read_marker(dest: &Path) -> Option<Marker> {
    let text = fs::read_to_string(dest.join(MARKER)).ok()?;
    serde_json::from_str(&text).ok()
}

/// True if `dest` holds a complete extraction of the archive with this SHA-256.
pub fn is_extracted(dest: &Path, archive_sha256: &str) -> bool {
    read_marker(dest).is_some_and(|m| m.archive_sha256 == archive_sha256)
}

/// Extracts `archive` (with SHA-256 `archive_sha256`) into `dest`,
/// replacing any earlier extraction.
pub fn extract_zip(
    archive: &Path,
    archive_sha256: &str,
    dest: &Path,
) -> Result<Marker, ExtractError> {
    let io_err = |p: &Path| {
        let p = p.to_path_buf();
        move |e| ExtractError::Io(p, e)
    };
    let mut staging = dest.as_os_str().to_owned();
    staging.push(".extracting");
    let staging = PathBuf::from(staging);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(io_err(&staging))?;
    }
    fs::create_dir_all(&staging).map_err(io_err(&staging))?;

    let file = File::open(archive).map_err(io_err(archive))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| ExtractError::Zip(e.to_string()))?;
    let mut files = 0u64;
    let mut bytes = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| ExtractError::Zip(e.to_string()))?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(ExtractError::UnsafeEntry(entry.name().to_string()));
        };
        let target = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(io_err(&target))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let mut out = File::create(&target).map_err(io_err(&target))?;
        bytes += io::copy(&mut entry, &mut out).map_err(io_err(&target))?;
        out.sync_all().map_err(io_err(&target))?;
        files += 1;
    }
    let marker = Marker {
        archive_sha256: archive_sha256.to_string(),
        files,
        bytes,
    };
    let text = serde_json::to_string_pretty(&marker).expect("marker serialises");
    fs::write(staging.join(MARKER), text).map_err(io_err(&staging))?;

    if dest.exists() {
        fs::remove_dir_all(dest).map_err(io_err(dest))?;
    }
    fs::rename(&staging, dest).map_err(io_err(dest))?;
    Ok(marker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("qmaws-zip-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            if name.ends_with('/') {
                w.add_directory(*name, opts).unwrap();
            } else {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
        }
        w.finish().unwrap();
    }

    #[test]
    fn extracts_and_marks_the_folder() {
        let dir = tmp("ok");
        let archive = dir.join("a.zip");
        make_zip(
            &archive,
            &[
                ("set/", b""),
                ("set/A.fasta", b">A\nACGT\n"),
                ("set/B.fasta", b">B\nGG\n"),
            ],
        );
        let dest = dir.join("out");
        let m = extract_zip(&archive, "abc", &dest).unwrap();
        assert_eq!(m.files, 2);
        assert_eq!(fs::read(dest.join("set/A.fasta")).unwrap(), b">A\nACGT\n");
        assert!(is_extracted(&dest, "abc"));
        assert!(!is_extracted(&dest, "other"));
        // Extracting again replaces the folder.
        extract_zip(&archive, "def", &dest).unwrap();
        assert!(is_extracted(&dest, "def"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn entries_outside_the_folder_are_refused() {
        let dir = tmp("evil");
        let archive = dir.join("evil.zip");
        make_zip(&archive, &[("../escape.txt", b"x")]);
        let err = extract_zip(&archive, "x", &dir.join("out")).unwrap_err();
        assert!(matches!(err, ExtractError::UnsafeEntry(_)));
        assert!(!dir.join("escape.txt").exists());
        assert!(!dir.join("out").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_archive_is_reported() {
        let dir = tmp("bad");
        let archive = dir.join("bad.zip");
        fs::write(&archive, b"not a zip file").unwrap();
        assert!(matches!(
            extract_zip(&archive, "x", &dir.join("out")),
            Err(ExtractError::Zip(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }
}
