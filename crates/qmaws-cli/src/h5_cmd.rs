//! Hidden development command `h5`: the confirmatory test of hypothesis H5
//! (docs/PREREGISTRATION.md, Amendment 4). For each of the 20 simulated HGT
//! datasets it reads the error of Q-MAWS v2 (`<id>/v2/variant.json`), of the
//! primary configuration (`<id>/qmaws/audit/evaluation.json`) and of
//! ML-MAWS (`<id>/ml_maws/ML_MAWS_tree.newick` against the species tree),
//! and computes the one-sided exact Wilcoxon signed-rank test on the paired
//! differences v2 minus ML-MAWS, zero differences dropped, Holm across nRF
//! and nQD. Success: adjusted p < 0.05 for at least one metric and median
//! difference < 0. The primary configuration is compared the same way as a
//! descriptive result. The test runs only when all 20 datasets are
//! complete.

use qmaws_core::newick::Tree;
use qmaws_core::stats::{self, Zeros};
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

/// The 20 datasets in the order of Amendment 4.
pub fn datasets() -> Vec<String> {
    let mut out = Vec::new();
    for level in ["h000", "h010", "h020", "h040"] {
        for r in 1..=5 {
            out.push(format!("hgt_{level}_r{r}"));
        }
    }
    out
}

/// nRF and nQD of one method on one dataset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Errors {
    pub nrf: f64,
    pub nqd: f64,
}

fn json_errors(path: &Path, key: Option<&str>) -> Result<Errors, String> {
    let bytes = qmaws_engine::atomic::read_verified(path)
        .ok_or_else(|| format!("{} is missing or damaged", path.display()))?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let e = match key {
        Some(k) => &v[k],
        None => &v,
    };
    match (e["nrf"].as_f64(), e["nqd"].as_f64()) {
        (Some(nrf), Some(nqd)) => Ok(Errors { nrf, nqd }),
        _ => Err(format!("{} has no nRF and nQD", path.display())),
    }
}

fn tree_errors(tree: &Path, truth: &Path) -> Result<Errors, String> {
    let read = |p: &Path| -> Result<Tree, String> {
        let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        Tree::parse(t.trim()).map_err(|e| format!("{}: {e}", p.display()))
    };
    let c = qmaws_engine::figures::comparison(&read(tree)?, &read(truth)?)?;
    Ok(Errors {
        nrf: c.nrf,
        nqd: c.nqd.value,
    })
}

/// One metric of the test.
#[derive(Debug, Clone)]
pub struct MetricTest {
    pub metric: &'static str,
    pub test: stats::SignedRank,
    pub p_holm: f64,
    pub median: f64,
    pub hodges_lehmann: f64,
    pub met: bool,
}

/// The test of `ours` against `theirs` (one value pair per dataset).
pub fn paired_test(ours: &[Errors], theirs: &[Errors]) -> Result<Vec<MetricTest>, String> {
    let mut out = Vec::new();
    for (metric, f) in [
        ("nRF", (|e: &Errors| e.nrf) as fn(&Errors) -> f64),
        ("nQD", |e: &Errors| e.nqd),
    ] {
        let diffs: Vec<f64> = ours.iter().zip(theirs).map(|(a, b)| f(a) - f(b)).collect();
        out.push(MetricTest {
            metric,
            test: stats::signed_rank_less(&diffs, Zeros::Wilcoxon)?,
            p_holm: f64::NAN,
            median: stats::median(&diffs).unwrap_or(f64::NAN),
            hodges_lehmann: stats::hodges_lehmann(&diffs).unwrap_or(f64::NAN),
            met: false,
        });
    }
    let adjusted = stats::holm(&out.iter().map(|m| m.test.p_less).collect::<Vec<_>>());
    for (m, p) in out.iter_mut().zip(adjusted) {
        m.p_holm = p;
        m.met = p < 0.05 && m.median < 0.0;
    }
    Ok(out)
}

fn test_table(title: &str, tests: &[MetricTest]) -> String {
    let mut s = format!("# {title}\nmetric\tn\tzeros\tw_plus\tw_minus\tp_one_sided\tp_holm\tmedian_difference\thodges_lehmann\tcriterion_met\n");
    for m in tests {
        let _ = writeln!(
            s,
            "{}\t{}\t{}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}",
            m.metric,
            m.test.n,
            m.test.zeros,
            m.test.w_plus,
            m.test.w_minus,
            m.test.p_less,
            m.p_holm,
            m.median,
            m.hodges_lehmann,
            if m.met { "yes" } else { "no" }
        );
    }
    s
}

pub fn run(dir: &Path, trees: &Path) -> ExitCode {
    let mut rows = String::from(
        "dataset\tv2_nrf\tv2_nqd\tprimary_nrf\tprimary_nqd\tml_maws_nrf\tml_maws_nqd\n",
    );
    let (mut v2, mut primary, mut ml) = (Vec::new(), Vec::new(), Vec::new());
    let mut missing = Vec::new();
    for id in datasets() {
        let d = dir.join(&id);
        let got = (|| -> Result<(Errors, Errors, Errors), String> {
            Ok((
                json_errors(&d.join("v2/variant.json"), Some("evaluation"))?,
                json_errors(&d.join("qmaws/audit/evaluation.json"), None)?,
                tree_errors(
                    &d.join("ml_maws/ML_MAWS_tree.newick"),
                    &trees.join(format!("{id}.nwk")),
                )?,
            ))
        })();
        match got {
            Ok((a, b, c)) => {
                let _ = writeln!(
                    rows,
                    "{id}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}",
                    a.nrf, a.nqd, b.nrf, b.nqd, c.nrf, c.nqd
                );
                v2.push(a);
                primary.push(b);
                ml.push(c);
            }
            Err(e) => missing.push(format!("{id}: {e}")),
        }
    }
    if !missing.is_empty() {
        println!(
            "H5 needs all 20 datasets; {} are not complete:",
            missing.len()
        );
        for m in &missing {
            println!("  {m}");
        }
        return ExitCode::from(1);
    }
    let main = match paired_test(&v2, &ml) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    let side = paired_test(&primary, &ml).unwrap_or_default();
    let supported = main.iter().any(|m| m.met);
    let mut text = test_table("H5 (Amendment 4): Q-MAWS v2 minus ML-MAWS, one-sided exact Wilcoxon signed-rank, zeros dropped, Holm across 2 metrics", &main);
    let _ = writeln!(
        text,
        "# H5 {}",
        if supported {
            "SUPPORTED"
        } else {
            "NOT SUPPORTED"
        }
    );
    text.push('\n');
    text.push_str(&test_table(
        "Descriptive: primary configuration (W2c) minus ML-MAWS, same test",
        &side,
    ));
    let out = [("h5_values.tsv", rows), ("h5_test.tsv", text.clone())];
    for (name, body) in out {
        if let Err(e) = qmaws_engine::atomic::write_verified(&dir.join(name), body.as_bytes()) {
            eprintln!("Error: {name}: {e}");
            return ExitCode::from(1);
        }
    }
    print!("{text}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(nrf: f64, nqd: f64) -> Errors {
        Errors { nrf, nqd }
    }

    #[test]
    fn there_are_twenty_datasets_in_the_amendment_order() {
        let d = datasets();
        assert_eq!(d.len(), 20);
        assert_eq!(d[0], "hgt_h000_r1");
        assert_eq!(d[19], "hgt_h040_r5");
    }

    #[test]
    fn a_clear_improvement_meets_the_criterion_and_a_tie_does_not() {
        let ml: Vec<Errors> = (0..20).map(|i| e(0.6 + i as f64 * 0.001, 0.4)).collect();
        let better: Vec<Errors> = ml.iter().map(|m| e(m.nrf - 0.1, m.nqd - 0.05)).collect();
        let t = paired_test(&better, &ml).unwrap();
        assert!(t.iter().all(|m| m.met && m.median < 0.0 && m.p_holm < 0.05));
        let same = paired_test(&ml, &ml).unwrap();
        assert!(same.iter().all(|m| !m.met && m.test.zeros == 20));
    }
}
