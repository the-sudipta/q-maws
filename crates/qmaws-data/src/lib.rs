//! Benchmark registry, downloads, manifests, reference trees and input
//! reading for Q-MAWS.
//!
//! Folder layout under the data folder (default `data/`):
//!
//! | Path | Content |
//! |---|---|
//! | `raw/_archives/<file>` | Downloaded archives, kept after extraction |
//! | `raw/<download id>/` | Extracted archive, or a downloaded single file |
//! | `manifests/download_log.json` | One entry per download: URL, date, size, checksums |

pub mod download;
pub mod extract;
pub mod loader;
pub mod references;
pub mod registry;

use download::{Downloaded, Expected, Fetcher, RetryPolicy};
use registry::{Dataset, Download, DownloadKind, Registry};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Name of this crate, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Version of this crate, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default data folder, relative to the working directory.
pub const DEFAULT_DATA_DIR: &str = "data";

/// Paths inside the data folder.
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn raw(&self) -> PathBuf {
        self.root.join("raw")
    }

    pub fn archives(&self) -> PathBuf {
        self.raw().join("_archives")
    }

    pub fn log_path(&self) -> PathBuf {
        self.root.join("manifests").join("download_log.json")
    }

    /// Where a download is stored: the archive for zip downloads, the file
    /// itself for single files.
    pub fn download_file(&self, d: &Download) -> PathBuf {
        match d.kind {
            DownloadKind::Zip => self.archives().join(&d.file_name),
            DownloadKind::File => self.raw().join(&d.id).join(&d.file_name),
        }
    }

    /// Folder a zip download is extracted into (or that holds a single file).
    pub fn download_dir(&self, d: &Download) -> PathBuf {
        self.raw().join(&d.id)
    }

    /// The folder or file of a dataset.
    pub fn dataset_path(&self, registry: &Registry, ds: &Dataset) -> PathBuf {
        let d = registry
            .download(&ds.download)
            .expect("registry is checked");
        let mut p = self.download_dir(d);
        for part in ds.path.split('/') {
            p.push(part);
        }
        p
    }
}

pub fn expected(d: &Download) -> Expected {
    Expected {
        md5: d.published_md5.clone(),
        sha256: d.pinned_sha256.clone(),
    }
}

/// State of a dataset on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Downloaded, verified and (for archives) extracted.
    Ready,
    NotDownloaded,
    /// Present, but its checksums do not match the published values.
    ChecksumMismatch,
    /// Downloaded and verified, but not (completely) extracted.
    NotExtracted,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Status::Ready => "ready",
            Status::NotDownloaded => "not downloaded",
            Status::ChecksumMismatch => "checksum mismatch",
            Status::NotExtracted => "downloaded, not extracted",
        })
    }
}

/// Checks whether a download is present and verified. This reads the whole
/// file to compute its checksums.
pub fn status(data: &DataDir, d: &Download) -> Status {
    let file = data.download_file(d);
    if !file.exists() {
        return Status::NotDownloaded;
    }
    let Ok(sums) = download::checksums(&file) else {
        return Status::NotDownloaded;
    };
    if !sums.matches(&expected(d)) {
        return Status::ChecksumMismatch;
    }
    match d.kind {
        DownloadKind::File => Status::Ready,
        DownloadKind::Zip if extract::is_extracted(&data.download_dir(d), &sums.sha256) => {
            Status::Ready
        }
        DownloadKind::Zip => Status::NotExtracted,
    }
}

/// One entry of `download_log.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub download: String,
    pub url: String,
    pub date_utc: String,
    pub bytes: u64,
    pub published_md5: Option<String>,
    pub pinned_sha256: Option<String>,
    pub md5: String,
    pub sha256: String,
    /// `downloaded`, `resumed from <n> bytes`, or `already present`.
    pub result: String,
}

/// Appends an entry to the download log (written atomically).
pub fn append_log(data: &DataDir, entry: LogEntry) -> std::io::Result<()> {
    let path = data.log_path();
    let mut entries: Vec<LogEntry> = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    entries.push(entry);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(&entries).map_err(std::io::Error::other)?;
    text.push('\n');
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, path)
}

#[derive(Debug)]
pub enum FetchDataError {
    Download(download::DownloadError),
    Extract(extract::ExtractError),
    Log(std::io::Error),
}

impl fmt::Display for FetchDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchDataError::Download(e) => write!(f, "{e}"),
            FetchDataError::Extract(e) => write!(f, "{e}"),
            FetchDataError::Log(e) => write!(f, "could not write the download log: {e}"),
        }
    }
}

impl std::error::Error for FetchDataError {}

/// Result of [`fetch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub downloaded: Downloaded,
    /// True if the archive was extracted in this call.
    pub extracted: bool,
}

/// Makes a download ready: downloads (or resumes) and verifies it, extracts
/// archives, and records the download in the log. `date_utc` is the time
/// stamp written to the log.
pub fn fetch(
    data: &DataDir,
    fetcher: &dyn Fetcher,
    d: &Download,
    date_utc: &str,
    retry: RetryPolicy,
    progress: &mut dyn FnMut(download::DownloadProgress),
) -> Result<Fetched, FetchDataError> {
    let file = data.download_file(d);
    let downloaded = download::download(
        fetcher,
        &d.resolved_url,
        &file,
        &expected(d),
        retry,
        progress,
    )
    .map_err(FetchDataError::Download)?;
    let sums = downloaded.checksums().clone();
    let mut extracted = false;
    if d.kind == DownloadKind::Zip && !extract::is_extracted(&data.download_dir(d), &sums.sha256) {
        extract::extract_zip(&file, &sums.sha256, &data.download_dir(d))
            .map_err(FetchDataError::Extract)?;
        extracted = true;
    }
    let result = match &downloaded {
        Downloaded::AlreadyPresent(_) => "already present".to_string(),
        Downloaded::Fetched {
            resumed_from: 0, ..
        } => "downloaded".to_string(),
        Downloaded::Fetched { resumed_from, .. } => format!("resumed from {resumed_from} bytes"),
    };
    append_log(
        data,
        LogEntry {
            download: d.id.clone(),
            url: d.resolved_url.clone(),
            date_utc: date_utc.to_string(),
            bytes: sums.bytes,
            published_md5: d.published_md5.clone(),
            pinned_sha256: d.pinned_sha256.clone(),
            md5: sums.md5,
            sha256: sums.sha256,
            result,
        },
    )
    .map_err(FetchDataError::Log)?;
    Ok(Fetched {
        downloaded,
        extracted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(CRATE_NAME, "qmaws-data");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn data_paths() {
        let reg = Registry::builtin();
        let data = DataDir::new("data");
        let fish = reg.dataset("fish_mito").unwrap();
        assert_eq!(
            data.dataset_path(&reg, fish),
            Path::new("data")
                .join("raw")
                .join("fish_mito")
                .join("assembled-fish_mito")
        );
        let sim = reg.dataset("sim_hgt_250").unwrap();
        assert_eq!(
            data.dataset_path(&reg, sim),
            Path::new("data")
                .join("raw")
                .join("sim_hgt")
                .join("simulated-sim_hgt")
                .join("hgt_250")
        );
        let corona = reg.download("coronavirus").unwrap();
        assert_eq!(
            data.download_file(corona),
            Path::new("data")
                .join("raw")
                .join("coronavirus")
                .join("coronavirus.fasta")
        );
        assert_eq!(
            data.download_file(reg.download("fish_mito").unwrap()),
            Path::new("data")
                .join("raw")
                .join("_archives")
                .join("assembled-fish_mito.zip")
        );
    }

    #[test]
    fn every_dataset_reference_exists() {
        let reg = Registry::builtin();
        for ds in &reg.datasets {
            if !ds.reference.is_empty() {
                assert!(references::reference(&ds.reference).is_some(), "{}", ds.id);
            }
        }
    }
}
