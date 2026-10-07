//! The benchmark dataset registry (`data/manifests/benchmarks.toml`),
//! compiled into the program so that it works without the repository.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;

/// The registry file, as committed.
pub const REGISTRY_TOML: &str = include_str!("../../../data/manifests/benchmarks.toml");

/// Accession lists (`data/manifests/accessions/<id>.tsv`), compiled in.
pub fn accession_list(id: &str) -> Option<&'static str> {
    macro_rules! list {
        ($name:literal) => {
            include_str!(concat!(
                "../../../data/manifests/accessions/",
                $name,
                ".tsv"
            ))
        };
    }
    Some(match id {
        "coronavirus" => list!("coronavirus"),
        "ebolavirus" => list!("ebolavirus"),
        "influenza_a" => list!("influenza_a"),
        "mammal_mtdna" => list!("mammal_mtdna"),
        "rhinovirus" => list!("rhinovirus"),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    /// A zip archive, extracted after download.
    Zip,
    /// A single file, used as downloaded.
    File,
    /// Records fetched from NCBI by accession (see `ncbi.rs`), written as
    /// one multi-FASTA file.
    Ncbi,
}

/// One file fetched from the internet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Download {
    pub id: String,
    pub source: String,
    pub page_url: String,
    pub file_name: String,
    /// Direct link, found on the page and verified by download.
    pub resolved_url: String,
    pub kind: DownloadKind,
    pub published_size: String,
    /// MD5 published by the source, if it publishes one.
    #[serde(default)]
    pub published_md5: Option<String>,
    /// SHA-256 fixed in this registry, for sources pinned to an exact
    /// version (such as a repository commit).
    #[serde(default)]
    pub pinned_sha256: Option<String>,
    /// For NCBI downloads: id of the accession list in
    /// `data/manifests/accessions/`.
    #[serde(default)]
    pub accessions: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// A folder with one sequence file per taxon.
    FilePerTaxon,
    /// One multi-FASTA file with one record per taxon.
    RecordPerTaxon,
}

/// One benchmark dataset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Dataset {
    pub id: String,
    pub name: String,
    /// Id of the download that contains it.
    pub download: String,
    /// Folder or file inside the extracted download.
    pub path: String,
    pub layout: Layout,
    /// Expected number of taxa.
    pub taxa: usize,
    /// Id of the reference tree in `data/references/`, or empty.
    #[serde(default)]
    pub reference: String,
    pub citation: String,
}

/// Settings sent with every NCBI E-utilities request.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NcbiSettings {
    pub tool: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub ncbi: Option<NcbiSettings>,
    #[serde(rename = "download")]
    pub downloads: Vec<Download>,
    #[serde(rename = "dataset")]
    pub datasets: Vec<Dataset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError(pub String);

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "benchmark registry: {}", self.0)
    }
}

impl std::error::Error for RegistryError {}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Registry {
    /// The registry compiled into the program.
    pub fn builtin() -> Registry {
        Registry::parse(REGISTRY_TOML).expect("the committed registry is valid")
    }

    /// Parses and checks a registry.
    pub fn parse(text: &str) -> Result<Registry, RegistryError> {
        let reg: Registry = toml::from_str(text).map_err(|e| RegistryError(e.to_string()))?;
        reg.check()?;
        Ok(reg)
    }

    fn check(&self) -> Result<(), RegistryError> {
        let mut ids = BTreeSet::new();
        for d in &self.downloads {
            if !ids.insert(d.id.as_str()) {
                return Err(RegistryError(format!("download id {} is used twice", d.id)));
            }
            if d.kind == DownloadKind::Ncbi {
                let list = d.accessions.as_deref().unwrap_or_default();
                if accession_list(list).is_none() {
                    return Err(RegistryError(format!(
                        "download {}: unknown accession list {list:?}",
                        d.id
                    )));
                }
                if self.ncbi.is_none() {
                    return Err(RegistryError(
                        "NCBI downloads need an [ncbi] section with tool and email".into(),
                    ));
                }
            } else if !d.resolved_url.ends_with(&d.file_name) {
                return Err(RegistryError(format!(
                    "download {}: the resolved URL does not end with the file name",
                    d.id
                )));
            }
            if d.kind != DownloadKind::Ncbi
                && d.published_md5.is_none()
                && d.pinned_sha256.is_none()
            {
                return Err(RegistryError(format!(
                    "download {}: neither a published MD5 nor a pinned SHA-256 is given",
                    d.id
                )));
            }
            if d.published_md5.as_deref().is_some_and(|m| !is_hex(m, 32))
                || d.pinned_sha256.as_deref().is_some_and(|s| !is_hex(s, 64))
            {
                return Err(RegistryError(format!("download {}: malformed hash", d.id)));
            }
        }
        let mut dataset_ids = BTreeSet::new();
        for s in &self.datasets {
            if !dataset_ids.insert(s.id.as_str()) || s.id == "all" {
                return Err(RegistryError(format!("dataset id {} is not unique", s.id)));
            }
            if !ids.contains(s.download.as_str()) {
                return Err(RegistryError(format!(
                    "dataset {} refers to unknown download {}",
                    s.id, s.download
                )));
            }
            if s.path.contains("..") || s.path.starts_with('/') || s.path.contains('\\') {
                return Err(RegistryError(format!("dataset {}: unsafe path", s.id)));
            }
        }
        Ok(())
    }

    pub fn download(&self, id: &str) -> Option<&Download> {
        self.downloads.iter().find(|d| d.id == id)
    }

    pub fn dataset(&self, id: &str) -> Option<&Dataset> {
        self.datasets.iter().find(|d| d.id == id)
    }

    /// Datasets selected by `id`, or all datasets for `all`.
    pub fn select(&self, id: &str) -> Result<Vec<&Dataset>, RegistryError> {
        if id == "all" {
            return Ok(self.datasets.iter().collect());
        }
        self.dataset(id).map(|d| vec![d]).ok_or_else(|| {
            RegistryError(format!(
                "unknown dataset {id}; run 'qmaws datasets' to see the list"
            ))
        })
    }
}

/// A published size for display: an exact byte count ("951066 bytes") is
/// shown in the units of the other sizes (1,024-based KB and MB, one
/// decimal); every other text is shown as published.
pub fn size_label(published: &str) -> String {
    let Some(n) = published
        .strip_suffix(" bytes")
        .and_then(|n| n.trim().parse::<u64>().ok())
    else {
        return published.to_string();
    };
    let n = n as f64;
    if n >= 1024.0 * 1024.0 {
        format!("{:.1} MB", n / (1024.0 * 1024.0))
    } else if n >= 1024.0 {
        format!("{:.1} KB", n / 1024.0)
    } else {
        format!("{n} bytes")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn byte_counts_are_shown_like_the_other_sizes() {
        assert_eq!(super::size_label("140249 bytes"), "137.0 KB");
        assert_eq!(super::size_label("114629704 bytes"), "109.3 MB");
        assert_eq!(super::size_label("42.1 MB"), "42.1 MB");
        assert_eq!(
            super::size_label("34 records (Table S5)"),
            "34 records (Table S5)"
        );
    }

    use super::*;

    #[test]
    fn builtin_registry_is_valid_and_complete() {
        let r = Registry::builtin();
        assert_eq!(r.downloads.len(), 15);
        assert_eq!(r.datasets.len(), 19);
        let afproject: Vec<&str> = r
            .datasets
            .iter()
            .filter(|d| r.download(&d.download).unwrap().source == "AFproject")
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(
            afproject,
            vec![
                "fish_mito",
                "ecoli",
                "ecoli_shigella_hgt",
                "yersinia_hgt",
                "sim_hgt_0",
                "sim_hgt_250",
                "sim_hgt_500",
                "sim_hgt_750",
                "sim_hgt_1000"
            ]
        );
        assert_eq!(r.dataset("fish_mito").unwrap().taxa, 25);
        assert_eq!(
            r.download("fish_mito").unwrap().published_md5.as_deref(),
            Some("8ec0391b78c9dc13fc38f6de0aed92b5")
        );
        assert_eq!(r.select("all").unwrap().len(), 19);
        assert!(r.select("nonexistent").is_err());
    }

    #[test]
    fn ncbi_accession_lists_match_their_datasets() {
        let r = Registry::builtin();
        let ncbi: Vec<&Download> = r
            .downloads
            .iter()
            .filter(|d| d.kind == DownloadKind::Ncbi)
            .collect();
        assert_eq!(ncbi.len(), 5);
        let mut all = BTreeSet::new();
        for d in ncbi {
            let rows = crate::ncbi::parse_accessions(
                accession_list(d.accessions.as_deref().unwrap()).unwrap(),
            )
            .unwrap();
            let ds = r.datasets.iter().find(|s| s.download == d.id).unwrap();
            assert_eq!(rows.len(), ds.taxa, "{}", d.id);
            assert!(d.pinned_sha256.is_some(), "{} is pinned", d.id);
            for row in &rows {
                let v = row.version.as_deref().expect("every version is pinned");
                assert!(v.starts_with(&format!("{}.", row.accession)), "{v}");
                assert!(
                    all.insert(row.accession.clone()),
                    "{} listed twice",
                    row.accession
                );
            }
            let names: BTreeSet<&str> = rows.iter().map(|r| r.name.as_str()).collect();
            assert_eq!(names.len(), rows.len(), "{}: names are unique", d.id);
        }
        assert_eq!(all.len(), 34 + 59 + 38 + 41 + 116);
    }

    #[test]
    fn invalid_registries_are_rejected() {
        let base = r#"
[[download]]
id = "a"
source = "s"
page_url = "p"
file_name = "a.zip"
resolved_url = "https://x/a.zip"
kind = "zip"
published_size = "1 KB"
published_md5 = "8ec0391b78c9dc13fc38f6de0aed92b5"

[[dataset]]
id = "d"
name = "n"
download = "a"
path = "folder"
layout = "file_per_taxon"
taxa = 4
citation = "c"
"#;
        assert!(Registry::parse(base).is_ok());
        assert!(Registry::parse(&base.replace("download = \"a\"", "download = \"b\"")).is_err());
        assert!(Registry::parse(&base.replace("https://x/a.zip", "https://x/b.zip")).is_err());
        assert!(Registry::parse(&base.replace("path = \"folder\"", "path = \"../up\"")).is_err());
        assert!(Registry::parse(&base.replace("8ec0391b", "zzzz")).is_err());
        assert!(Registry::parse(
            &base.replace("published_md5 = \"8ec0391b78c9dc13fc38f6de0aed92b5\"", "")
        )
        .is_err());
    }
}
