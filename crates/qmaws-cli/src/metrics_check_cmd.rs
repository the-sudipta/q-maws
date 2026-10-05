//! Hidden development command `metrics-check`: golden test G9 (plan
//! 2.10.4). Draws random tree pairs (5 to 60 leaves), computes nRF, nQD and
//! MSD as every run does, checks nQD against enumeration over splits and
//! MSD against brute-force matching (for at most 8 splits), and writes
//! `pairs.tsv` so that `scripts/metrics_check.py` can compare the nRF
//! values with DendroPy. Exit status 0 when every comparison agrees.

use qmaws_core::metrics::{msd, nqd};
use qmaws_core::metrics_check::{self, MAX_LEAVES};
use qmaws_core::newick::{nrf, Tree};
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

pub fn run(output: &Path, count: usize, seed: u64) -> ExitCode {
    let mut table = String::from(
        "pair\tleaves\testimate\treference\trf_splits_differ\tnrf\tnqd_differ\tnqd_compared\tnqd_unresolved_in_reference\tnqd\tmsd\n",
    );
    let (mut nqd_checked, mut msd_checked, mut failures) = (0usize, 0usize, Vec::new());
    for (i, pair) in metrics_check::pairs(count, seed, MAX_LEAVES)
        .iter()
        .enumerate()
    {
        let parse = |s: &str| Tree::parse(s).expect("generated trees parse");
        let (a, b) = (parse(&pair.a), parse(&pair.b));
        let (rf, nrf_value) = nrf(&a, &b);
        let q = match nqd(&a, &b) {
            Ok(q) => q,
            Err(e) => {
                failures.push(format!("pair {i}: nQD failed: {e}"));
                continue;
            }
        };
        let oracle = metrics_check::nqd_by_splits(&a, &b);
        nqd_checked += 1;
        if q != oracle {
            failures.push(format!(
                "pair {i}: nQD {}/{} ({} unresolved) but enumeration over splits gives {}/{} ({} unresolved)",
                q.differ,
                q.compared,
                q.unresolved_in_reference,
                oracle.differ,
                oracle.compared,
                oracle.unresolved_in_reference
            ));
        }
        let m = msd(&a, &b).expect("same leaves");
        match metrics_check::msd_by_permutations(&a, &b) {
            Ok(Some(expected)) => {
                msd_checked += 1;
                if m != expected {
                    failures.push(format!(
                        "pair {i}: MSD {m} but brute-force matching gives {expected}"
                    ));
                }
            }
            Ok(None) => {}
            Err(e) => failures.push(format!("pair {i}: MSD oracle failed: {e}")),
        }
        let _ = writeln!(
            table,
            "{i}\t{}\t{}\t{}\t{rf}\t{nrf_value:.9}\t{}\t{}\t{}\t{:.9}\t{m}",
            pair.leaves, pair.a, pair.b, q.differ, q.compared, q.unresolved_in_reference, q.value
        );
    }
    if let Err(e) = std::fs::create_dir_all(output).and_then(|()| {
        qmaws_engine::atomic::write_atomic(&output.join("pairs.tsv"), table.as_bytes())
    }) {
        eprintln!(
            "Error: cannot write {}: {e}",
            output.join("pairs.tsv").display()
        );
        return ExitCode::from(1);
    }
    for f in &failures {
        println!("{f}");
    }
    println!(
        "G9: {count} pairs (seed {seed}, 5 to {MAX_LEAVES} leaves); nQD equal to enumeration over splits on {} of {nqd_checked}; MSD equal to brute-force matching on {} of {msd_checked} (pairs with at most {} splits); nRF written to pairs.tsv for the DendroPy check.",
        nqd_checked - failures.iter().filter(|f| f.contains("nQD")).count(),
        msd_checked - failures.iter().filter(|f| f.contains("MSD")).count(),
        metrics_check::MSD_BRUTE_FORCE_MAX
    );
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
