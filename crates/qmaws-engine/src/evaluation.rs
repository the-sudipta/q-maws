//! The reference tree of a run and the comparison of the run's tree with it
//! (plan 2.10), kept as hashed files in `audit/` so that `qmaws verify`
//! can recompute nRF, nQD and MSD:
//!
//! - `audit/reference.nwk`: the reference tree as given (the user's file,
//!   or the benchmark dataset's AFproject tree), byte for byte;
//! - `audit/reference.json`: its label and source;
//! - `audit/evaluation.json`: the comparison of `report/tree.nwk` with it,
//!   with the SHA-256 of both trees.
//!
//! The comparison does not change the computation of the run: it is
//! written after the run and is not part of the root fingerprint
//! (docs/OPEN_ISSUES.md, OI-19).

use crate::atomic;
use crate::hash::sha256_hex;
use crate::rundir::RunDir;
use crate::runner::{io_err, EngineError};
use qmaws_core::newick::Tree;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const REFERENCE_FILE: &str = "reference.nwk";
pub const REFERENCE_INFO: &str = "reference.json";
pub const EVALUATION_FILE: &str = "evaluation.json";
/// The run's tree that is compared.
pub const TREE_FILE: &str = "report/tree.nwk";

/// Label and source of the stored reference tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceInfo {
    /// For example `AFproject reference tree (fish_mito)`.
    pub label: String,
    /// Where it came from: a file path or a dataset.
    pub source: String,
    /// SHA-256 of `audit/reference.nwk`.
    pub sha256: String,
    /// For a downloaded tree: the record of the queries (API, versions,
    /// date, SHA-256 of the answers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<serde_json::Value>,
}

/// A stored reference tree.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredReference {
    pub info: ReferenceInfo,
    /// The file as given.
    pub newick: Vec<u8>,
    pub tree: Tree,
}

/// The comparison of the run's tree with the reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub reference_label: String,
    pub reference_sha256: String,
    pub tree_file: String,
    pub tree_sha256: String,
    pub taxa: usize,
    /// Non-trivial splits in the symmetric difference.
    pub rf_splits_differ: usize,
    /// Denominator of nRF: 2 × (taxa − 3).
    pub rf_max: usize,
    pub nrf: f64,
    pub nqd_differ: u64,
    pub nqd_compared: u64,
    pub nqd_unresolved_in_reference: u64,
    pub nqd: f64,
    /// Raw matching split distance (not normalised; OI-15).
    pub msd: u64,
}

/// Stores `newick` as the run's reference tree. Fails if it is not a valid
/// Newick tree.
pub fn store_reference(
    run_dir: &Path,
    newick: &[u8],
    label: &str,
    source: &str,
    query: Option<&serde_json::Value>,
) -> Result<StoredReference, EngineError> {
    let text = String::from_utf8_lossy(newick);
    let tree = Tree::parse(text.trim()).map_err(|e| {
        EngineError::Invalid(format!("the reference tree is not valid Newick: {e}"))
    })?;
    let audit = RunDir::new(run_dir).audit();
    std::fs::create_dir_all(&audit).map_err(io_err(&audit))?;
    let path = audit.join(REFERENCE_FILE);
    let sha256 = atomic::write_verified(&path, newick).map_err(io_err(&path))?;
    let info = ReferenceInfo {
        label: label.to_string(),
        source: source.to_string(),
        sha256,
        query: query.cloned(),
    };
    let info_path = audit.join(REFERENCE_INFO);
    let json = serde_json::to_vec_pretty(&info).expect("serialises");
    atomic::write_verified(&info_path, &json).map_err(io_err(&info_path))?;
    Ok(StoredReference {
        info,
        newick: newick.to_vec(),
        tree,
    })
}

/// The run's stored reference tree, if any. Damaged files are an error.
pub fn stored_reference(run_dir: &Path) -> Result<Option<StoredReference>, EngineError> {
    let audit = RunDir::new(run_dir).audit();
    let path = audit.join(REFERENCE_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let damaged = |what: &Path| {
        EngineError::Invalid(format!(
            "{} is damaged (its SHA-256 does not agree)",
            what.display()
        ))
    };
    let bytes = atomic::read_verified(&path).ok_or_else(|| damaged(&path))?;
    let info_path = audit.join(REFERENCE_INFO);
    let info: ReferenceInfo = atomic::read_verified(&info_path)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| damaged(&info_path))?;
    if info.sha256 != sha256_hex(&bytes) {
        return Err(damaged(&info_path));
    }
    let tree = Tree::parse(String::from_utf8_lossy(&bytes).trim())
        .map_err(|e| EngineError::Invalid(format!("{}: {e}", path.display())))?;
    Ok(Some(StoredReference {
        info,
        newick: bytes,
        tree,
    }))
}

/// Compares the run's tree with `reference`.
pub fn evaluate(run_dir: &Path, reference: &StoredReference) -> Result<Evaluation, EngineError> {
    let path = run_dir.join(TREE_FILE);
    let bytes = atomic::read_verified(&path)
        .ok_or_else(|| EngineError::Invalid(format!("{} is missing or damaged", path.display())))?;
    let tree = Tree::parse(String::from_utf8_lossy(&bytes).trim())
        .map_err(|e| EngineError::Invalid(format!("{}: {e}", path.display())))?;
    let nqd = qmaws_core::metrics::nqd(&tree, &reference.tree).map_err(EngineError::Invalid)?;
    let msd = qmaws_core::metrics::msd(&tree, &reference.tree).map_err(EngineError::Invalid)?;
    let (rf, nrf) = qmaws_core::newick::nrf(&tree, &reference.tree);
    let taxa = tree.leaf_names().len();
    Ok(Evaluation {
        reference_label: reference.info.label.clone(),
        reference_sha256: reference.info.sha256.clone(),
        tree_file: TREE_FILE.to_string(),
        tree_sha256: sha256_hex(&bytes),
        taxa,
        rf_splits_differ: rf,
        rf_max: 2 * taxa.saturating_sub(3),
        nrf,
        nqd_differ: nqd.differ,
        nqd_compared: nqd.compared,
        nqd_unresolved_in_reference: nqd.unresolved_in_reference,
        nqd: nqd.value,
        msd,
    })
}

/// Writes `audit/evaluation.json`.
pub fn write(run_dir: &Path, evaluation: &Evaluation) -> Result<(), EngineError> {
    let path = RunDir::new(run_dir).audit().join(EVALUATION_FILE);
    let json = serde_json::to_vec_pretty(evaluation).expect("serialises");
    atomic::write_verified(&path, &json).map_err(io_err(&path))?;
    Ok(())
}

/// The stored evaluation, if any.
pub fn stored(run_dir: &Path) -> Result<Option<Evaluation>, EngineError> {
    let path = RunDir::new(run_dir).audit().join(EVALUATION_FILE);
    if !path.exists() {
        return Ok(None);
    }
    atomic::read_verified(&path)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .map(Some)
        .ok_or_else(|| {
            EngineError::Invalid(format!(
                "{} is damaged (its SHA-256 does not agree)",
                path.display()
            ))
        })
}

/// Checks of `qmaws verify`: the stored evaluation against a fresh
/// comparison of the stored tree and reference. Each item is (agrees,
/// description). Empty when the run has no reference tree.
pub fn check(run_dir: &Path, tolerance: f64) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let reference = match stored_reference(run_dir) {
        Ok(Some(r)) => r,
        Ok(None) => return out,
        Err(e) => {
            out.push((false, format!("reference tree: {e}")));
            return out;
        }
    };
    out.push((
        true,
        format!(
            "reference tree audit/{REFERENCE_FILE} ({}, SHA-256 {}) agrees with its hash",
            reference.info.label,
            &reference.info.sha256[..16]
        ),
    ));
    let stored = match stored(run_dir) {
        Ok(Some(e)) => e,
        Ok(None) => {
            out.push((
                false,
                format!("audit/{EVALUATION_FILE} is missing (draw the figures again with qmaws figures)"),
            ));
            return out;
        }
        Err(e) => {
            out.push((false, e.to_string()));
            return out;
        }
    };
    let fresh = match evaluate(run_dir, &reference) {
        Ok(e) => e,
        Err(e) => {
            out.push((false, format!("comparison with the reference: {e}")));
            return out;
        }
    };
    out.push((
        stored.reference_sha256 == fresh.reference_sha256
            && stored.tree_sha256 == fresh.tree_sha256,
        format!(
            "audit/{EVALUATION_FILE} was computed from this reference and {} (SHA-256 {})",
            fresh.tree_file,
            &fresh.tree_sha256[..16]
        ),
    ));
    out.push((
        stored.rf_splits_differ == fresh.rf_splits_differ
            && stored.rf_max == fresh.rf_max
            && (stored.nrf - fresh.nrf).abs() <= tolerance,
        format!(
            "nRF recomputed: {:.6} ({} of {} splits differ); stored {:.6}",
            fresh.nrf, fresh.rf_splits_differ, fresh.rf_max, stored.nrf
        ),
    ));
    out.push((
        stored.nqd_differ == fresh.nqd_differ
            && stored.nqd_compared == fresh.nqd_compared
            && stored.nqd_unresolved_in_reference == fresh.nqd_unresolved_in_reference
            && (stored.nqd - fresh.nqd).abs() <= tolerance,
        format!(
            "nQD recomputed: {:.6} ({} of {} quartets differ; {} unresolved in the reference); stored {:.6}",
            fresh.nqd, fresh.nqd_differ, fresh.nqd_compared, fresh.nqd_unresolved_in_reference, stored.nqd
        ),
    ));
    out.push((
        stored.msd == fresh.msd,
        format!("MSD recomputed: {}; stored {}", fresh.msd, stored.msd),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn run_with_tree(label: &str, newick: &str) -> TempDir {
        let dir = TempDir::new(label);
        let report = dir.path().join("report");
        std::fs::create_dir_all(&report).unwrap();
        atomic::write_verified(&report.join("tree.nwk"), newick.as_bytes()).unwrap();
        dir
    }

    #[test]
    fn evaluation_is_stored_and_checked() {
        let dir = run_with_tree("eval_ok", "((A,B),(C,D),(E,F));\n");
        assert!(
            check(dir.path(), 1e-6).is_empty(),
            "no reference: no checks"
        );
        let r =
            store_reference(dir.path(), b"((A,B),(C,E),(D,F));\n", "test", "x.nwk", None).unwrap();
        let e = evaluate(dir.path(), &r).unwrap();
        assert_eq!((e.rf_splits_differ, e.rf_max, e.taxa), (4, 6, 6));
        write(dir.path(), &e).unwrap();
        assert_eq!(stored(dir.path()).unwrap(), Some(e));
        let checks = check(dir.path(), 1e-6);
        assert_eq!(checks.len(), 5);
        assert!(checks.iter().all(|(ok, _)| *ok), "{checks:?}");
    }

    #[test]
    fn a_changed_tree_or_value_fails_the_check() {
        let dir = run_with_tree("eval_bad", "((A,B),(C,D),(E,F));\n");
        let r =
            store_reference(dir.path(), b"((A,B),(C,E),(D,F));", "test", "x.nwk", None).unwrap();
        let mut e = evaluate(dir.path(), &r).unwrap();
        e.msd += 1;
        write(dir.path(), &e).unwrap();
        assert!(check(dir.path(), 1e-6).iter().any(|(ok, _)| !ok));
        // A tree changed after the comparison.
        let report = dir.path().join("report");
        atomic::write_verified(&report.join("tree.nwk"), b"((A,C),(B,D),(E,F));").unwrap();
        let e = evaluate(dir.path(), &r).unwrap();
        let mut old = e.clone();
        old.tree_sha256 = "0".repeat(64);
        write(dir.path(), &old).unwrap();
        assert!(check(dir.path(), 1e-6).iter().any(|(ok, _)| !ok));
    }

    #[test]
    fn invalid_or_damaged_references_are_errors() {
        let dir = run_with_tree("eval_damaged", "((A,B),(C,D),(E,F));");
        assert!(store_reference(dir.path(), b"((A,B)", "t", "x", None).is_err());
        store_reference(dir.path(), b"((A,B),(C,D),(E,F));", "t", "x", None).unwrap();
        std::fs::write(
            dir.path().join("audit").join(REFERENCE_FILE),
            "((A,C),(B,D),(E,F));",
        )
        .unwrap();
        assert!(stored_reference(dir.path()).is_err());
        assert!(check(dir.path(), 1e-6).iter().any(|(ok, _)| !ok));
    }
}
