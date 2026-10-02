//! Development commands for comparing `wQFM-rs` with the original wQFM jar
//! (plan 2.8, item 3). Java and the jar are never needed by Q-MAWS itself;
//! the workflow `.github/workflows/wqfm.yml` runs the jar on a runner.
//!
//! - `wqfm-export` writes weighted-quartet inputs in wQFM's format
//!   (`<name>.wqrts`): the worksheet example, quartets induced from seeded
//!   random trees without noise and with noise, and the W2c quartets of
//!   finished analysis runs. `inputs.tsv` lists them with their kind and,
//!   for the simulated ones, the true tree.
//! - `wqfm-compare` runs `wQFM-rs` on every input, reads the jar's tree
//!   (`<name>.jar.tre`) and compares: on noise-free inputs the topologies
//!   must be identical; on the others the `wQFM-rs` consistency score must
//!   be at least 99.9% of the jar's.

use qmaws_core::amalgamate::{self, Quartet, Topology};
use qmaws_core::newick::{self, Tree};
use qmaws_core::quartet;
use qmaws_core::weight::SplitMix64;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Required score ratio on noisy and real inputs (plan 2.8).
const SCORE_RATIO: f64 = 0.999;

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("Error: {message}");
    ExitCode::from(1)
}

fn names(m: usize) -> Vec<String> {
    (0..m).map(|i| format!("T{i:02}")).collect()
}

/// Weighted quartets of a random tree: every 4-set with its true topology
/// (weight 1 to 10); with noise, also the two other topologies (weight 0 to
/// 6), so that the true topology is not always the heaviest.
fn simulated(m: usize, noisy: bool, rng: &mut SplitMix64) -> (String, Vec<Quartet>) {
    let nm = names(m);
    let tree = amalgamate::random_binary_tree(&nm, rng);
    let topo = Topology::from_newick(&tree, &nm).expect("own tree");
    let mut out = Vec::new();
    for q in amalgamate::induced_quartets(&topo) {
        out.push(Quartet {
            weight: round3(1.0 + 9.0 * rng.next_f64()),
            ..q
        });
        if noisy {
            let [a, b] = q.l;
            let [c, d] = q.r;
            for other in [Quartet::new(a, c, b, d, 0.0), Quartet::new(a, d, b, c, 0.0)] {
                out.push(Quartet {
                    weight: round3(6.0 * rng.next_f64()),
                    ..other
                });
            }
        }
    }
    out.retain(|q| q.weight > 0.0);
    (tree, out)
}

/// Rounds to 3 decimals, so the weights in the files are exact.
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

pub fn export(output: &Path, runs: &[PathBuf], seed: u64) -> ExitCode {
    if let Err(e) = std::fs::create_dir_all(output) {
        return fail(format!("{}: {e}", output.display()));
    }
    let mut list = String::from("name\tkind\ttaxa\tquartets\ttrue_tree\n");
    let mut write = |name: &str,
                     kind: &str,
                     nm: &[String],
                     qs: &[Quartet],
                     truth: &str|
     -> Result<(), String> {
        let path = output.join(format!("{name}.wqrts"));
        std::fs::write(&path, amalgamate::to_wqfm_input(nm, qs))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let _ = writeln!(list, "{name}\t{kind}\t{}\t{}\t{truth}", nm.len(), qs.len());
        Ok(())
    };
    // The teaching example with its W1 weights.
    let w = qmaws_core::teach::example();
    let ex_names: Vec<String> = qmaws_core::teach::EXAMPLE
        .iter()
        .map(|(n, _)| n.to_string())
        .collect();
    let mut ex = Vec::new();
    for (qi, q) in w.quartets.iter().enumerate() {
        let mut counts = [0u64; 16];
        for row in &w.patterns {
            counts[usize::from_str_radix(&row[qi], 2).expect("pattern")] += 1;
        }
        if let Some(w1) = qmaws_core::weight::w1(&counts) {
            ex.extend(amalgamate::quartets_of(*q, w1));
        }
    }
    let mut results = vec![write(
        "worksheet",
        "noise-free",
        &ex_names,
        &ex,
        "((K,L),M,(N,P));",
    )];
    let mut rng = SplitMix64::new(seed);
    for (i, m) in [6, 8, 10, 12, 15, 20, 25, 30, 40, 50]
        .into_iter()
        .enumerate()
    {
        let (tree, qs) = simulated(m, false, &mut rng);
        results.push(write(
            &format!("clean_{i:02}_m{m}"),
            "noise-free",
            &names(m),
            &qs,
            &tree,
        ));
    }
    for (i, m) in [6, 8, 10, 12, 15, 20, 25, 30, 40, 50]
        .into_iter()
        .enumerate()
    {
        let (tree, qs) = simulated(m, true, &mut rng);
        results.push(write(
            &format!("noisy_{i:02}_m{m}"),
            "noisy",
            &names(m),
            &qs,
            &tree,
        ));
    }
    for (i, run) in runs.iter().enumerate() {
        let loaded = qmaws_engine::analysis::load_matrix(run)
            .and_then(|(nm, _)| qmaws_engine::analysis::read_weights(run).map(|w| (nm, w)));
        let (nm, mut weights) = match loaded {
            Ok(v) => v,
            Err(e) => return fail(e),
        };
        weights.sort_by_key(|w| w.rank);
        let qs: Vec<Quartet> = weights
            .iter()
            .filter(|w| w.resampled())
            .flat_map(|w| amalgamate::quartets_of(quartet::unrank(w.rank), w.w2c))
            .collect();
        results.push(write(&format!("real_{i:02}"), "real", &nm, &qs, ""));
    }
    if let Some(Err(e)) = results.into_iter().find(|r| r.is_err()) {
        return fail(e);
    }
    if let Err(e) = std::fs::write(output.join("inputs.tsv"), &list) {
        return fail(e);
    }
    println!(
        "Wrote {} inputs to {}",
        list.lines().count() - 1,
        output.display()
    );
    ExitCode::SUCCESS
}

pub fn compare(dir: &Path) -> ExitCode {
    let Ok(list) = std::fs::read_to_string(dir.join("inputs.tsv")) else {
        return fail("inputs.tsv is missing; run wqfm-export first");
    };
    let mut report = String::from(
        "name\tkind\ttaxa\tquartets\trust_score\tjar_score\ttotal_weight\tratio\tnrf_rust_jar\tnrf_rust_truth\tnrf_jar_truth\tresult\n",
    );
    let (mut rows, mut failed) = (0, 0);
    for line in list.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (name, kind, truth) = (cols[0], cols[1], cols.get(4).copied().unwrap_or(""));
        let read =
            |p: String| std::fs::read_to_string(dir.join(&p)).map_err(|e| format!("{p}: {e}"));
        let mut nm = Vec::new();
        let parsed =
            read(format!("{name}.wqrts")).and_then(|t| amalgamate::parse_wqfm_input(&t, &mut nm));
        let qs = match parsed {
            Ok(q) => q,
            Err(e) => return fail(e),
        };
        let jar_text = match read(format!("{name}.jar.tre")) {
            Ok(t) => t.trim().to_string(),
            Err(e) => return fail(e),
        };
        let rust = amalgamate::wqfm(&nm, &qs, &Default::default());
        let jar_topo = match Topology::from_newick(&jar_text, &nm) {
            Ok(t) => t,
            Err(e) => return fail(format!("{name}: jar tree: {e}")),
        };
        let (jar_score, total) = jar_topo.score(&qs);
        let ratio = if jar_score > 0.0 {
            rust.score / jar_score
        } else {
            1.0
        };
        let tree = |t: &str| Tree::parse(t).expect("valid Newick");
        let nrf = |a: &str, b: &str| newick::nrf(&tree(a), &tree(b)).1;
        let rust_jar = nrf(&rust.newick, &jar_text);
        let (rust_truth, jar_truth) = if truth.is_empty() {
            ("NA".to_string(), "NA".to_string())
        } else {
            (
                format!("{:.4}", nrf(&rust.newick, truth)),
                format!("{:.4}", nrf(&jar_text, truth)),
            )
        };
        let ok = if kind == "noise-free" {
            rust_jar == 0.0
        } else {
            ratio >= SCORE_RATIO
        };
        rows += 1;
        if !ok {
            failed += 1;
        }
        let _ = writeln!(
            report,
            "{name}\t{kind}\t{}\t{}\t{:.6}\t{jar_score:.6}\t{total:.6}\t{ratio:.6}\t{rust_jar:.4}\t{rust_truth}\t{jar_truth}\t{}",
            nm.len(),
            qs.len(),
            rust.score,
            if ok { "pass" } else { "FAIL" }
        );
    }
    let _ = std::fs::write(dir.join("comparison.tsv"), &report);
    print!("{report}");
    println!(
        "{rows} inputs; {failed} failed (noise-free: identical topology; noisy and real: score at least {SCORE_RATIO} of the jar's)"
    );
    if rows >= 20 && failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
