//! Development commands for the IQ-TREE cross-check of the quartet
//! likelihood (plan 2.7.7, item 4). IQ-TREE is not part of Q-MAWS; the
//! workflow `.github/workflows/iqtree.yml` runs it on a GitHub runner.
//!
//! - `iqtree-export` picks quartets of a finished run by a seeded draw and
//!   writes, per quartet, a binary PHYLIP alignment of its non-constant
//!   columns (IQ-TREE's +ASC conditions on "not 0000 and not 1111"), the three
//!   topologies as Newick files without and with Q-MAWS's fitted branch
//!   lengths, and the Q-MAWS log-likelihoods in `expected.tsv`.
//! - `iqtree-compare` reads IQ-TREE's results and compares them with Q-MAWS:
//!   (A) IQ-TREE's log-likelihood at Q-MAWS's branch lengths (`-blfix`);
//!   (B) IQ-TREE's maximised log-likelihood against Q-MAWS's maximum and
//!   against Q-MAWS evaluated at IQ-TREE's branch lengths.

use qmaws_core::newick::Tree;
use qmaws_core::quartet::{self, PatternCounts};
use qmaws_core::weight::{self, Conditioning, Model, SplitMix64, PAIRS};
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

/// Tolerance of the comparison (plan 2.7.7).
const TOLERANCE: f64 = 0.0001;

/// Leaf names in the alignments, by quartet position.
const LEAVES: [&str; 4] = ["a", "b", "c", "d"];

const COND: Conditioning = Conditioning::NotConstant;

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("Error: {message}");
    ExitCode::from(1)
}

/// The two models: name, Q-MAWS model, IQ-TREE model string.
fn models(full: &qmaws_core::matrix::Matrix) -> [(&'static str, Model, String); 2] {
    let ones: u64 = full.ones.iter().map(|&o| o as u64).sum();
    let pi1 = ones as f64 / (full.taxa as f64 * full.columns_len() as f64);
    let emp = Model::with_frequency_of_one(pi1);
    [
        ("sym", Model::symmetric(), "JC2+ASC".to_string()),
        (
            "emp",
            emp,
            format!("GTR2+F{{{:.17},{:.17}}}+ASC", emp.pi[0], emp.pi[1]),
        ),
    ]
}

fn newick(topology: usize, lengths: Option<&[f64; 5]>) -> String {
    let [i, j, k, l] = PAIRS[topology];
    let len = |b: usize| {
        lengths
            .map(|v| format!(":{:.17}", v[b]))
            .unwrap_or_default()
    };
    format!(
        "({}{},{}{},({}{},{}{}){});\n",
        LEAVES[i],
        len(i),
        LEAVES[j],
        len(j),
        LEAVES[k],
        len(k),
        LEAVES[l],
        len(l),
        len(4)
    )
}

fn phylip(counts: &PatternCounts) -> String {
    let mut rows = [String::new(), String::new(), String::new(), String::new()];
    let mut n = 0;
    for (x, &count) in counts.iter().enumerate() {
        if x == 0 || x == 15 {
            continue;
        }
        for _ in 0..count {
            for (p, row) in rows.iter_mut().enumerate() {
                row.push(if (x >> (3 - p)) & 1 == 1 { '1' } else { '0' });
            }
            n += 1;
        }
    }
    let mut out = format!("4 {n}\n");
    for (p, row) in rows.iter().enumerate() {
        let _ = writeln!(out, "{} {row}", LEAVES[p]);
    }
    out
}

pub fn export(run: &Path, output: &Path, quartets: usize, seed: u64) -> ExitCode {
    let (names, full) = match qmaws_engine::analysis::load_matrix(run) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let m = names.len();
    if m < 4 {
        return fail("the run has fewer than 4 taxa");
    }
    if let Err(e) = std::fs::create_dir_all(output) {
        return fail(format!("{}: {e}", output.display()));
    }
    let q = quartet::quartet_count(m);
    let mut rng = SplitMix64::new(seed);
    let mut chosen: Vec<u64> = Vec::new();
    while chosen.len() < quartets.min(q as usize) {
        let r = rng.next_u64() % q;
        let counts =
            quartet::pattern_counts_brute(&full.rows, full.columns_len(), quartet::unrank(r));
        if !chosen.contains(&r) && weight::conditioned_total(&counts, COND) >= 10 {
            chosen.push(r);
        }
    }
    let mut list = String::from("quartet\trank\ta\tb\tc\td\tcounts\n");
    let mut expected =
        String::from("quartet\tmodel\ttopology\tiqtree_model\tqmaws_log_likelihood\tlengths\n");
    for (n, &r) in chosen.iter().enumerate() {
        let quad = quartet::unrank(r);
        let counts = quartet::pattern_counts_brute(&full.rows, full.columns_len(), quad);
        let _ = writeln!(
            list,
            "{n}\t{r}\t{}\t{}\t{}\t{}\t{}",
            names[quad[0]],
            names[quad[1]],
            names[quad[2]],
            names[quad[3]],
            counts
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let write = |name: String, text: &str| std::fs::write(output.join(name), text);
        let mut files = vec![(format!("q{n}.phy"), phylip(&counts))];
        for t in 0..3 {
            files.push((format!("q{n}_t{t}.nwk"), newick(t, None)));
        }
        for (model_name, model, iq) in models(&full) {
            for t in 0..3 {
                let fit = weight::fit(&model, COND, t, &counts).expect("at least 10 columns");
                files.push((
                    format!("q{n}_{model_name}_t{t}_fixed.nwk"),
                    newick(t, Some(&fit.lengths)),
                ));
                let _ = writeln!(
                    expected,
                    "{n}\t{model_name}\t{t}\t{iq}\t{:.10}\t{}",
                    fit.log_likelihood,
                    fit.lengths.map(|v| format!("{v:.10}")).join(",")
                );
            }
        }
        for (name, text) in files {
            if let Err(e) = write(name, &text) {
                return fail(e);
            }
        }
    }
    for (name, text) in [("quartets.tsv", list), ("expected.tsv", expected)] {
        if let Err(e) = std::fs::write(output.join(name), text) {
            return fail(e);
        }
    }
    println!("Wrote {} quartets to {}", chosen.len(), output.display());
    ExitCode::SUCCESS
}

/// IQ-TREE's log-likelihood from its `.iqtree` report.
fn iqtree_log_likelihood(report: &str) -> Option<f64> {
    report
        .lines()
        .find_map(|l| l.trim().strip_prefix("Log-likelihood of the tree:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|v| v.parse().ok())
}

/// Branch lengths (leaf of a, b, c, d; internal) of IQ-TREE's tree.
fn tree_lengths(text: &str) -> Option<[f64; 5]> {
    let tree = Tree::parse(text).ok()?;
    let mut lengths = [f64::NAN; 5];
    let mut internal = 0.0;
    for (i, node) in tree.nodes.iter().enumerate() {
        if tree.is_leaf(i) {
            let p = LEAVES
                .iter()
                .position(|l| node.label.as_deref() == Some(l))?;
            lengths[p] = node.length?;
        } else if i != tree.root {
            internal += node.length.unwrap_or(0.0);
        }
    }
    lengths[4] = internal;
    lengths.iter().all(|v| v.is_finite()).then_some(lengths)
}

pub fn compare(run: &Path, dir: &Path) -> ExitCode {
    let (_, full) = match qmaws_engine::analysis::load_matrix(run) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let read = |name: &str| std::fs::read_to_string(dir.join(name));
    let Ok(list) = read("quartets.tsv") else {
        return fail("quartets.tsv is missing; run iqtree-export first");
    };
    let mut report = String::from(
        "quartet\tmodel\ttopology\tqmaws_max\tiqtree_at_qmaws_lengths\tdiff_A\tiqtree_max\tdiff_max\tqmaws_at_iqtree_lengths\tdiff_B\n",
    );
    let mut worst: f64 = 0.0;
    let mut rows = 0;
    for line in list.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let n: usize = cols[0].parse().expect("quartet index");
        let counts: Vec<u64> = cols[6]
            .split(',')
            .map(|c| c.parse().expect("count"))
            .collect();
        let counts: PatternCounts = counts.try_into().expect("16 counts");
        for (model_name, model, _) in models(&full) {
            for t in 0..3 {
                let fit = weight::fit(&model, COND, t, &counts).expect("columns");
                let prefix = format!("q{n}_{model_name}_t{t}");
                let fixed = read(&format!("{prefix}_fixed.iqtree"))
                    .ok()
                    .and_then(|r| iqtree_log_likelihood(&r));
                let optimised = read(&format!("{prefix}_opt.iqtree"))
                    .ok()
                    .and_then(|r| iqtree_log_likelihood(&r));
                let lengths = read(&format!("{prefix}_opt.treefile"))
                    .ok()
                    .and_then(|t| tree_lengths(&t));
                let (Some(fixed), Some(optimised), Some(lengths)) = (fixed, optimised, lengths)
                else {
                    return fail(format!(
                        "IQ-TREE output for {prefix} is missing or unreadable"
                    ));
                };
                let at_iq = weight::log_likelihood(&model, COND, t, &lengths, &counts);
                let diff_a = fixed - fit.log_likelihood;
                let diff_max = optimised - fit.log_likelihood;
                let diff_b = at_iq - optimised;
                for d in [diff_a, diff_max, diff_b] {
                    worst = worst.max(d.abs());
                }
                rows += 1;
                let _ = writeln!(
                    report,
                    "{n}\t{model_name}\t{t}\t{:.6}\t{fixed:.4}\t{diff_a:.6}\t{optimised:.4}\t{diff_max:.6}\t{at_iq:.6}\t{diff_b:.6}",
                    fit.log_likelihood
                );
            }
        }
    }
    let _ = std::fs::write(dir.join("comparison.tsv"), &report);
    print!("{report}");
    println!("{rows} comparisons; largest absolute difference {worst:.6} (tolerance {TOLERANCE})");
    if worst <= TOLERANCE && rows > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_has_the_counted_columns() {
        let mut c = [0u64; 16];
        c[0] = 5;
        c[15] = 4;
        c[0b1100] = 2;
        c[0b0001] = 1;
        // Columns in pattern order: 0001, then 1100 twice.
        assert_eq!(phylip(&c), "4 3\na 011\nb 011\nc 000\nd 100\n");
    }

    #[test]
    fn newick_and_lengths_round_trip() {
        let l = [0.1, 0.2, 0.3, 0.4, 0.05];
        for t in 0..3 {
            assert_eq!(tree_lengths(&newick(t, Some(&l))), Some(l));
        }
        // IQ-TREE writes the tree with another root.
        assert_eq!(
            tree_lengths("(a:0.1,(b:0.2,c:0.3):0.05,d:0.4);"),
            Some([0.1, 0.2, 0.3, 0.4, 0.05])
        );
        assert_eq!(newick(1, None), "(a,c,(b,d));\n");
    }

    #[test]
    fn reads_the_log_likelihood_line() {
        let r = "Some text\nLog-likelihood of the tree: -1234.5678 (s.e. 12.3456)\n";
        assert_eq!(iqtree_log_likelihood(r), Some(-1234.5678));
    }
}
