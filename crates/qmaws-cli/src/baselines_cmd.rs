//! Hidden development command `baselines`: the descriptive comparison of
//! Amendment 5. Reads the outputs of `scripts/baselines.sh` (one folder per
//! tool and dataset with `provenance.txt` and, when a tree was made,
//! `comparison.tsv`) and puts them beside Q-MAWS:
//! - AFproject datasets: the Q-MAWS mean over 5 seeds (`results/summary/qmaws.tsv`);
//! - H5 datasets: Q-MAWS v2, the primary configuration and ML-MAWS
//!   (`results/h5/h5_values.tsv`, when H5 is complete).
//!
//! For each tool and group of datasets it reports the paired differences
//! Q-MAWS minus the tool with the exact Wilcoxon statistics of H1 and H5
//! (zero differences dropped), the one-sided p-values in both directions,
//! the median and the Hodges–Lehmann estimate. No criterion: Amendment 5
//! adds no hypothesis. Writes `values.tsv` and `paired.tsv`.

use qmaws_core::stats::{self, Zeros};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

pub const TOOLS: [&str; 3] = ["mash", "cdmaws", "peafowl"];

/// The AFproject datasets with a reference tree.
pub const AFPROJECT: [&str; 9] = [
    "fish_mito",
    "ecoli",
    "ecoli_shigella_hgt",
    "yersinia_hgt",
    "sim_hgt_0",
    "sim_hgt_250",
    "sim_hgt_500",
    "sim_hgt_750",
    "sim_hgt_1000",
];

/// One tool on one dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: String,
    pub nrf: Option<f64>,
    pub nqd: Option<f64>,
}

/// Reads `status:` from provenance.txt and nRF, nQD from comparison.tsv.
pub fn parse(provenance: &str, comparison: Option<&str>) -> Outcome {
    let status = provenance
        .lines()
        .find_map(|l| l.strip_prefix("status: "))
        .unwrap_or("missing")
        .trim()
        .to_string();
    let metric = |name: &str| {
        comparison.and_then(|c| {
            c.lines().find_map(|l| {
                let mut f = l.split('\t');
                (f.next() == Some(name))
                    .then(|| f.next()?.parse::<f64>().ok())
                    .flatten()
            })
        })
    };
    let (nrf, nqd) = if status == "ok" {
        (metric("nRF"), metric("nQD"))
    } else {
        (None, None)
    };
    Outcome { status, nrf, nqd }
}

/// Columns of a TSV file by header name, one map per row, keyed by the first column.
fn table(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let Some(head) = lines.next() else {
        return BTreeMap::new();
    };
    let head: Vec<&str> = head.split('\t').collect();
    lines
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let row: BTreeMap<String, String> = head
                .iter()
                .zip(&f)
                .map(|(h, v)| (h.to_string(), v.to_string()))
                .collect();
            (f[0].to_string(), row)
        })
        .collect()
}

fn value(rows: &BTreeMap<String, BTreeMap<String, String>>, ds: &str, col: &str) -> Option<f64> {
    rows.get(ds)?.get(col)?.parse().ok()
}

/// Descriptive paired statistics of `ours - theirs` over the datasets where both exist.
pub fn paired(ours: &[Option<f64>], theirs: &[Option<f64>]) -> Option<String> {
    let d: Vec<f64> = ours
        .iter()
        .zip(theirs)
        .filter_map(|(a, b)| Some((*a)? - (*b)?))
        .collect();
    if d.is_empty() {
        return None;
    }
    let neg: Vec<f64> = d.iter().map(|x| -x).collect();
    let less = stats::signed_rank_less(&d, Zeros::Wilcoxon).ok()?;
    let greater = stats::signed_rank_less(&neg, Zeros::Wilcoxon).ok()?;
    let lower = d.iter().filter(|x| **x < 0.0).count();
    let higher = d.iter().filter(|x| **x > 0.0).count();
    Some(format!(
        "{}\t{lower}\t{}\t{higher}\t{:.6}\t{:.6}\t{:.6}\t{:.6}",
        d.len(),
        d.len() - lower - higher,
        stats::median(&d).unwrap_or(f64::NAN),
        stats::hodges_lehmann(&d).unwrap_or(f64::NAN),
        less.p_less,
        greater.p_less
    ))
}

/// A group of datasets and the Q-MAWS columns compared on it.
struct Group<'a> {
    name: &'a str,
    datasets: &'a [String],
    label: &'a str,
    nrf: &'a str,
    nqd: &'a str,
    h5: bool,
}

fn fmt(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.6}")).unwrap_or_else(|| "-".into())
}

pub fn run(dir: &Path, summary: &Path, h5: &Path) -> ExitCode {
    let h5_names = crate::h5_cmd::datasets();
    let datasets: Vec<String> = AFPROJECT
        .iter()
        .map(|s| s.to_string())
        .chain(h5_names.clone())
        .collect();
    let mut got: BTreeMap<(String, String), Outcome> = BTreeMap::new();
    let mut values = String::from("dataset\ttool\tstatus\tnrf\tnqd\n");
    for ds in &datasets {
        for tool in TOOLS {
            let d = dir.join(tool).join(ds);
            let prov = std::fs::read_to_string(d.join("provenance.txt")).unwrap_or_default();
            let comp = std::fs::read_to_string(d.join("comparison.tsv")).ok();
            let o = parse(&prov, comp.as_deref());
            let _ = writeln!(
                values,
                "{ds}\t{tool}\t{}\t{}\t{}",
                o.status,
                fmt(o.nrf),
                fmt(o.nqd)
            );
            got.insert((ds.clone(), tool.to_string()), o);
        }
    }
    let q = table(&std::fs::read_to_string(summary.join("qmaws.tsv")).unwrap_or_default());
    let v = table(&std::fs::read_to_string(h5.join("h5_values.tsv")).unwrap_or_default());

    let mut out = String::from(
        "group\tqmaws\ttool\tmetric\tpairs\tqmaws_lower\tequal\tqmaws_higher\tmedian_difference\thodges_lehmann\tp_qmaws_lower\tp_qmaws_higher\n",
    );
    let afp: Vec<String> = AFPROJECT.iter().map(|s| s.to_string()).collect();
    let groups = [
        Group {
            name: "AFproject (9)",
            datasets: &afp,
            label: "primary, mean of 5 seeds",
            nrf: "nrf_mean",
            nqd: "nqd_mean",
            h5: false,
        },
        Group {
            name: "H5 (20)",
            datasets: &h5_names,
            label: "v2",
            nrf: "v2_nrf",
            nqd: "v2_nqd",
            h5: true,
        },
        Group {
            name: "H5 (20)",
            datasets: &h5_names,
            label: "primary",
            nrf: "primary_nrf",
            nqd: "primary_nqd",
            h5: true,
        },
    ];
    let mut others: Vec<(&str, String)> = TOOLS.iter().map(|t| (*t, t.to_string())).collect();
    others.push(("ml_maws", "ML-MAWS".into()));
    for g in &groups {
        let (group, ds, label, cn, cq, is_h5) =
            (g.name, g.datasets, g.label, &g.nrf, &g.nqd, &g.h5);
        let src = if *is_h5 { &v } else { &q };
        for (tool, tool_label) in &others {
            if *tool == "ml_maws" && !is_h5 {
                continue; // the AFproject ML-MAWS comparison is H1 (reported values)
            }
            for (metric, col) in [("nRF", cn), ("nQD", cq)] {
                let ours: Vec<Option<f64>> = ds.iter().map(|d| value(src, d, col)).collect();
                let theirs: Vec<Option<f64>> = ds
                    .iter()
                    .map(|d| {
                        if *tool == "ml_maws" {
                            value(&v, d, &format!("ml_maws_{}", metric.to_lowercase()))
                        } else {
                            got.get(&(d.clone(), tool.to_string())).and_then(|o| {
                                if metric == "nRF" {
                                    o.nrf
                                } else {
                                    o.nqd
                                }
                            })
                        }
                    })
                    .collect();
                if let Some(line) = paired(&ours, &theirs) {
                    let _ = writeln!(out, "{group}\t{label}\t{tool_label}\t{metric}\t{line}");
                }
            }
        }
    }
    for (name, body) in [("values.tsv", values), ("paired.tsv", out.clone())] {
        if let Err(e) = qmaws_engine::atomic::write_verified(&dir.join(name), body.as_bytes()) {
            eprintln!("Error: {name}: {e}");
            return ExitCode::from(1);
        }
    }
    print!("{out}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_run_gives_its_metrics_and_a_failed_run_none() {
        let comp = "metric\tvalue\tdetail\nnRF\t0.250000\t5 of 20\nnQD\t0.100000\tx\nMSD\t7\tx\n";
        let o = parse("tool: mash\nstatus: ok\nnote: \n", Some(comp));
        assert_eq!(
            (o.status.as_str(), o.nrf, o.nqd),
            ("ok", Some(0.25), Some(0.1))
        );
        let f = parse("status: failed\n", Some(comp));
        assert_eq!((f.nrf, f.nqd), (None, None));
        assert_eq!(parse("", None).status, "missing");
    }

    #[test]
    fn paired_counts_and_directions() {
        let ours = [Some(0.1), Some(0.2), Some(0.3), None];
        let theirs = [Some(0.2), Some(0.2), Some(0.5), Some(0.1)];
        let line = paired(&ours, &theirs).unwrap();
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(&f[..4], ["3", "2", "1", "0"]);
        assert!(f[4].starts_with("-0.1"));
        assert!(paired(&[None], &[Some(1.0)]).is_none());
    }
}
