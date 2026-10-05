//! Tests of commands that need no network: `inspect`, `datasets`, and the
//! checks and input-warning answers of `run`.

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

#[test]
fn run_options_are_checked_before_anything_is_written() {
    let tmp = TempDir::new("runopts");
    let out = tmp.0.join("out").display().to_string();
    let (code, text) = run(&["run", "--dataset", "all", "--output", &out]);
    assert_eq!(code, 2, "{text}");
    assert!(
        text.contains("--output and --reference cannot be used"),
        "{text}"
    );
    let (code, text) = run(&["run", "--dataset", "no_such_set"]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("or use --dataset all"), "{text}");
    let (code, text) = run(&["run", "--input", "x", "--cores", "0"]);
    assert_eq!(code, 2, "{text}");
    let (code, text) = run(&["run", "--input", "x", "--memory-limit", "-1"]);
    assert_eq!(code, 2, "{text}");
    assert!(!tmp.0.join("out").exists());
}

#[test]
fn run_answers_input_warnings_from_its_options() {
    let tmp = TempDir::new("answers");
    let seqs = tmp.0.join("seqs");
    std::fs::create_dir_all(&seqs).unwrap();
    for (i, name) in ["A", "B", "C", "D", "E"].iter().enumerate() {
        let mut x = 0x9e37_79b9_7f4a_7c15u64 ^ i as u64;
        let seq: String = (0..400)
            .map(|_| {
                x = x
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                b"ACGT"[(x >> 62) as usize] as char
            })
            .collect();
        std::fs::write(
            seqs.join(format!("{name}.fasta")),
            format!(">{name}\n{seq}\n"),
        )
        .unwrap();
    }
    std::fs::write(seqs.join("Tiny.fasta"), ">Tiny\nACGTACGTAC\n").unwrap();
    let input = seqs.display().to_string();
    let first = tmp.0.join("first").display().to_string();
    let quick = ["--replicates", "2", "--bootstrap", "0", "--no-live-tree"];
    // Without an answer the run stops and names the warning.
    let mut args = vec!["--quiet", "run", "--input", &input, "--output", &first];
    args.extend(quick);
    let (code, text) = run(&args);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("Tiny is only 10 letters long") && text.contains("--skip"),
        "{text}"
    );
    // With --skip Tiny it runs on the five other taxa and records it.
    let second = tmp.0.join("second");
    let second_text = second.display().to_string();
    let mut args = vec![
        "--quiet",
        "run",
        "--input",
        &input,
        "--output",
        &second_text,
        "--skip",
        "Tiny",
    ];
    args.extend(quick);
    let (code, text) = run(&args);
    assert_eq!(code, 0, "{text}");
    let inputs: serde_json::Value =
        serde_json::from_slice(&std::fs::read(second.join("audit/inputs.json")).unwrap()).unwrap();
    assert_eq!(inputs["taxa"].as_array().unwrap().len(), 5);
    assert_eq!(inputs["skipped"], serde_json::json!(["Tiny"]));
    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(second.join("run.json")).unwrap()).unwrap();
    assert_eq!(
        state["config"]["input_choices"]["skip"],
        serde_json::json!(["Tiny"])
    );
    let (code, text) = run(&["verify", "--quick", "--seed", "1", "--output", &second_text]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn tree_compare_reads_trees_with_support_and_lengths() {
    let tmp = TempDir::new("treecmp");
    let a = tmp.0.join("a.nwk");
    let b = tmp.0.join("b.nwk");
    std::fs::write(
        &a,
        "((A:0.1,B:0.2)95:0.05,(C:0.1,E:0.3)40:0.02,(D:0.1,F:0.1)100:0.01);",
    )
    .unwrap();
    std::fs::write(&b, "((A,B),(C,D),(E,F));").unwrap();
    let (code, text) = run(&[
        "tree-compare",
        "--tree",
        &a.display().to_string(),
        "--reference",
        &b.display().to_string(),
    ]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("nRF\t0.666667\t4 of 6 splits differ"),
        "{text}"
    );
}
