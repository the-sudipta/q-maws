//! A run with the user's own reference tree (plan 5.4 and 2.10): the tree
//! is checked against the taxa, stored with the run, compared after the
//! run, and the comparison is recomputed by `qmaws verify` (OI-19).
//! Input: the 16-taxon simulated control and its true tree.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_qmaws");

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let p = std::env::temp_dir().join(format!("qmaws-ref-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn control() -> PathBuf {
    std::path::absolute(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../results/controls/inputs/simulated"),
    )
    .unwrap()
}

fn qmaws(args: &[&str]) -> (bool, String) {
    let out = Command::new(BIN).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn the_users_reference_is_stored_compared_and_verified() {
    let tmp = TempDir::new("own");
    let run = tmp.0.join("run");
    let input = control().join("sequences");
    let truth = control().join("true_tree.nwk");

    // A tree whose leaves are not the taxa is refused before the run.
    let wrong = tmp.0.join("wrong.nwk");
    std::fs::write(&wrong, "((S01,S02),S03,(S04,X99));").unwrap();
    let (ok, text) = qmaws(&[
        "--quiet",
        "run",
        "--input",
        &input.display().to_string(),
        "--output",
        &run.display().to_string(),
        "--reference",
        &wrong.display().to_string(),
    ]);
    assert!(!ok && text.contains("leaves do not match"), "{text}");
    assert!(!run.join("run.json").exists());

    let (ok, text) = qmaws(&[
        "--quiet",
        "run",
        "--input",
        &input.display().to_string(),
        "--output",
        &run.display().to_string(),
        "--reference",
        &truth.display().to_string(),
        "--replicates",
        "4",
        "--bootstrap",
        "0",
        "--no-live-tree",
        "--cores",
        "2",
    ]);
    assert!(ok, "{text}");
    let audit = run.join("audit");
    assert_eq!(
        std::fs::read(audit.join("reference.nwk")).unwrap(),
        std::fs::read(&truth).unwrap(),
        "the reference is stored byte for byte"
    );
    let eval: serde_json::Value =
        serde_json::from_slice(&std::fs::read(audit.join("evaluation.json")).unwrap()).unwrap();
    assert_eq!(eval["taxa"], 16);
    assert_eq!(eval["rf_max"], 26);
    assert!(run.join("figures/tanglegram.svg").exists());

    let (ok, text) = qmaws(&[
        "verify",
        "--quick",
        "--seed",
        "1",
        "--output",
        &run.display().to_string(),
    ]);
    assert!(ok, "{text}");
    assert!(text.contains("nRF recomputed"), "{text}");

    // A changed number in the stored comparison is found.
    let mut changed = eval.clone();
    changed["msd"] = serde_json::json!(eval["msd"].as_u64().unwrap() + 1);
    // Written with a matching hash, so only the recomputation can find it.
    qmaws_engine::atomic::write_verified(
        &audit.join("evaluation.json"),
        &serde_json::to_vec_pretty(&changed).unwrap(),
    )
    .unwrap();
    let (ok, text) = qmaws(&[
        "verify",
        "--quick",
        "--seed",
        "1",
        "--output",
        &run.display().to_string(),
    ]);
    assert!(!ok && text.contains("FAIL"), "{text}");
}
