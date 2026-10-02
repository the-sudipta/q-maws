//! Tests of the data commands that need no network: `inspect` and `datasets`.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_qmaws");

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let p = std::env::temp_dir().join(format!("qmaws-dc-{label}-{}", std::process::id()));
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

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(BIN).args(args).output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.code().unwrap_or(-1), text)
}

#[test]
fn inspect_reports_summary_findings_and_reference_match() {
    let tmp = TempDir::new("inspect");
    let seqs = tmp.0.join("seqs");
    std::fs::create_dir_all(&seqs).unwrap();
    let long = "ACGT".repeat(40);
    for (name, extra) in [("K", "NN"), ("L", ""), ("M", ""), ("N", "")] {
        let body = format!(">{name}\n{long}{extra}{}\n", name.len());
        std::fs::write(seqs.join(format!("{name}.fasta")), body).unwrap();
    }
    std::fs::write(seqs.join("P.fasta"), format!(">P\n{long}\n")).unwrap();
    let tree = tmp.0.join("ref.nwk");
    std::fs::write(&tree, "((K,L),M,(N,Q));\n").unwrap();

    let (code, text) = run(&[
        "inspect",
        "--input",
        seqs.to_str().unwrap(),
        "--reference",
        tree.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("Found 5 files, 5 taxa:"), "{text}");
    assert!(text.contains("2 (N)"), "{text}");
    // L, M and N are identical (P too): the first copy is named in each warning.
    assert!(
        text.contains("Warning: K and") || text.contains("Warning: L and"),
        "{text}"
    );
    assert!(
        text.contains("Choices: keep both / keep one / abort"),
        "{text}"
    );
    assert!(text.contains("Taxa not in the tree: P"), "{text}");
    assert!(text.contains("Tree leaves without a taxon: Q"), "{text}");
}

#[test]
fn inspect_fails_for_too_few_taxa_and_missing_folders() {
    let tmp = TempDir::new("few");
    std::fs::write(tmp.0.join("a.fa"), ">a\nACGT\n>b\nAAAA\n").unwrap();
    let (code, text) = run(&["inspect", "--input", tmp.0.join("a.fa").to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(
        text.contains("Quartet methods need at least 4 taxa"),
        "{text}"
    );

    let (code, text) = run(&["inspect", "--input", tmp.0.join("nope").to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(text.contains("does not exist"), "{text}");
}

#[test]
fn datasets_lists_every_registered_dataset() {
    let tmp = TempDir::new("datasets");
    let (code, text) = run(&["datasets", "--data-dir", tmp.0.to_str().unwrap()]);
    assert_eq!(code, 0, "{text}");
    for id in [
        "fish_mito",
        "ecoli",
        "ecoli_shigella_hgt",
        "yersinia_hgt",
        "sim_hgt_0",
        "sim_hgt_1000",
        "coronavirus",
        "rhinovirus",
    ] {
        assert!(text.contains(id), "{id} missing:\n{text}");
    }
    assert_eq!(text.matches("not downloaded").count(), 19, "{text}");
}

#[test]
fn unknown_datasets_are_usage_errors() {
    let tmp = TempDir::new("unknown");
    let (code, text) = run(&[
        "download",
        "--dataset",
        "nope",
        "--data-dir",
        tmp.0.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(text.contains("unknown dataset nope"), "{text}");
}
