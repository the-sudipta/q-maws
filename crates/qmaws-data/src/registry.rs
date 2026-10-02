//! The benchmark dataset registry (`data/manifests/benchmarks.toml`),
//! compiled into the program so that it works without the repository.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;

/// The registry file, as committed.
pub const REGISTRY_TOML: &str = include_str!("../../../data/manifests/benchmarks.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    /// A zip archive, extracted after download.
    Zip,
    /// A single file, used as downloaded.
    File,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Registry {
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
            if !d.resolved_url.ends_with(&d.file_name) {
                return Err(RegistryError(format!(
                    "download {}: the resolved URL does not end with the file name",
                    d.id
                )));
            }
            if d.published_md5.is_none() && d.pinned_sha256.is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_is_valid_and_complete() {
        let r = Registry::builtin();
        assert_eq!(r.downloads.len(), 10);
        assert_eq!(r.datasets.len(), 14);
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
        assert_eq!(r.select("all").unwrap().len(), 14);
        assert!(r.select("nonexistent").is_err());
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
