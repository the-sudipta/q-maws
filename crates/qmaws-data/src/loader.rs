//! Reading sequence files from disk into taxa.

use qmaws_core::input::{self, Finding, RecordMode, Taxon};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Fingerprint of one input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFingerprint {
    pub path: PathBuf,
    pub bytes: u64,
    /// SHA-256 of the file as stored (compressed, for `.gz`).
    pub raw_sha256: String,
}

/// Taxa read from a folder or file, with fingerprints and findings.
#[derive(Debug, Clone)]
pub struct LoadedInput {
    pub taxa: Vec<Taxon>,
    /// SHA-256 of each taxon's cleaned sequence, in taxon order.
    pub cleaned_sha256: Vec<String>,
    pub files: Vec<FileFingerprint>,
    pub findings: Vec<Finding>,
}

#[derive(Debug)]
pub enum LoadError {
    Missing(PathBuf),
    Unreadable(PathBuf, io::Error),
    NoSequenceFiles(PathBuf),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Missing(p) => write!(f, "{} does not exist", p.display()),
            LoadError::Unreadable(p, e) => write!(f, "{} could not be read: {e}", p.display()),
            LoadError::NoSequenceFiles(p) => write!(
                f,
                "{} contains no sequence files (accepted: {}, optionally compressed as .gz)",
                p.display(),
                input::SEQUENCE_EXTENSIONS
                    .iter()
                    .map(|e| format!(".{e}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for LoadError {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Reads a file, decompressing `.gz` files.
fn read_file(path: &Path) -> Result<(Vec<u8>, FileFingerprint), LoadError> {
    let stored = fs::read(path).map_err(|e| LoadError::Unreadable(path.to_path_buf(), e))?;
    let fingerprint = FileFingerprint {
        path: path.to_path_buf(),
        bytes: stored.len() as u64,
        raw_sha256: hex(&Sha256::digest(&stored)),
    };
    let is_gz = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gz"));
    let content = if is_gz {
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(stored.as_slice())
            .read_to_end(&mut out)
            .map_err(|e| LoadError::Unreadable(path.to_path_buf(), e))?;
        out
    } else {
        stored
    };
    Ok((content, fingerprint))
}

/// Sequence files directly inside `folder`, sorted by file name.
pub fn sequence_files(folder: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let entries =
        fs::read_dir(folder).map_err(|e| LoadError::Unreadable(folder.to_path_buf(), e))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(input::is_sequence_file)
        })
        .collect();
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

/// Reads `path`: a folder of sequence files (each file one taxon, or one
/// taxon per record, depending on `mode`), or a single sequence file (one
/// taxon per record).
pub fn load(path: &Path, mode: RecordMode) -> Result<LoadedInput, LoadError> {
    if !path.exists() {
        return Err(LoadError::Missing(path.to_path_buf()));
    }
    let (paths, mode) = if path.is_dir() {
        (sequence_files(path)?, mode)
    } else {
        (vec![path.to_path_buf()], RecordMode::OneTaxonPerRecord)
    };
    if paths.is_empty() {
        return Err(LoadError::NoSequenceFiles(path.to_path_buf()));
    }
    let mut taxa = Vec::new();
    let mut files = Vec::new();
    for p in &paths {
        let (content, fingerprint) = read_file(p)?;
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        taxa.extend(input::taxa_from_file(&name, &content, mode));
        files.push(fingerprint);
    }
    let cleaned_sha256 = taxa
        .iter()
        .map(|t| hex(&Sha256::digest(&t.cleaned.sequence)))
        .collect();
    let findings = input::validate(&taxa);
    Ok(LoadedInput {
        taxa,
        cleaned_sha256,
        files,
        findings,
    })
}

/// Folder summary in the format shown before a run:
///
/// ```text
/// Found 6 files, 6 taxa:
///
///   Taxon      Length    Removed characters
///   Salmon     16,580    12 (N)
/// ```
pub fn summary(input: &LoadedInput) -> String {
    let name_width = input
        .taxa
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(5)
        .max(5);
    let lengths: Vec<String> = input
        .taxa
        .iter()
        .map(|t| group_thousands(t.cleaned.cleaned_length()))
        .collect();
    let len_width = lengths.iter().map(|l| l.len()).max().unwrap_or(6).max(6);
    let files = input.files.len();
    let mut out = format!(
        "Found {files} {}, {} taxa:\n\n",
        if files == 1 { "file" } else { "files" },
        input.taxa.len()
    );
    out.push_str(&format!(
        "  {:<name_width$}  {:>len_width$}    Removed characters\n",
        "Taxon", "Length"
    ));
    for (t, len) in input.taxa.iter().zip(&lengths) {
        out.push_str(&format!(
            "  {:<name_width$}  {:>len_width$}    {}\n",
            t.name,
            len,
            input::removed_summary(&t.cleaned.removed)
        ));
    }
    out
}

/// 16580 → "16,580".
pub fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("qmaws-load-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn loads_a_folder_with_mixed_files() {
        let dir = tmp("folder");
        fs::write(dir.join("Salmon.txt"), "ACGTNNNN\nACGT 123\n").unwrap();
        fs::write(dir.join("Trout.fasta"), ">Trout_mt\nACGTACGT\n").unwrap();
        fs::write(dir.join("README.md"), "not a sequence").unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(b">Carp\nAACCGGTT\n").unwrap();
        fs::write(dir.join("carp.fa.gz"), gz.finish().unwrap()).unwrap();
        fs::write(dir.join("eel.fna"), ">e1\nAAAA\n>e2\nCCCC\n").unwrap();

        let loaded = load(&dir, RecordMode::ConcatenatePerFile).unwrap();
        let names: Vec<&str> = loaded.taxa.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Salmon", "Trout_mt", "Carp", "eel"]);
        assert_eq!(loaded.files.len(), 4);
        assert_eq!(loaded.taxa[0].cleaned.sequence, b"ACGTACGT".to_vec());
        assert_eq!(loaded.taxa[3].cleaned.sequence, b"AAAACCCC".to_vec());
        assert_eq!(loaded.cleaned_sha256.len(), 4);
        // All four are shorter than 100 letters.
        assert_eq!(
            loaded
                .findings
                .iter()
                .filter(|f| matches!(f, Finding::ShortSequence { .. }))
                .count(),
            4
        );
        let s = summary(&loaded);
        assert!(s.starts_with("Found 4 files, 4 taxa:"));
        assert!(s.contains("Salmon"));
        assert!(s.contains("4 (N)"));

        let per_record = load(&dir, RecordMode::OneTaxonPerRecord).unwrap();
        assert_eq!(per_record.taxa.len(), 5);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_single_file_gives_one_taxon_per_record() {
        let dir = tmp("single");
        let f = dir.join("all.fasta");
        fs::write(&f, ">a\nAC\n>b\nGT\n>c\nAA\n").unwrap();
        let loaded = load(&f, RecordMode::ConcatenatePerFile).unwrap();
        assert_eq!(loaded.taxa.len(), 3);
        assert!(loaded.findings.contains(&Finding::TooFewTaxa { count: 3 }));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_and_empty_folders_are_errors() {
        let dir = tmp("empty");
        assert!(matches!(
            load(&dir.join("nope"), RecordMode::ConcatenatePerFile),
            Err(LoadError::Missing(_))
        ));
        assert!(matches!(
            load(&dir, RecordMode::ConcatenatePerFile),
            Err(LoadError::NoSequenceFiles(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(16_580), "16,580");
        assert_eq!(group_thousands(4_641_652), "4,641,652");
    }
}
