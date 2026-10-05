//! Hidden development commands of milestone M12 (plan Part 8, item 6, and
//! 2.10.5; docs/PREREGISTRATION.md, Amendment 3):
//!
//! - `variant`: the tree of a finished run made again with W1, W2a, W2b or
//!   W2c weights from its stored quartet results (`qmaws_engine::variant`);
//! - `calibration`: the reliability table and the expected calibration
//!   error (ECE) of the support values of one or more trees against their
//!   true trees, pooled.

use qmaws_core::calibration;
use qmaws_core::newick::Tree;
use qmaws_engine::variant::{self, Weights};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum WeightChoice {
    W1,
    W2a,
    W2b,
    W2c,
}

pub fn variant(run: &Path, weights: WeightChoice, output: &Path) -> ExitCode {
    let w = match weights {
        WeightChoice::W1 => Weights::W1,
        WeightChoice::W2a => Weights::W2a,
        WeightChoice::W2b => Weights::W2b,
        WeightChoice::W2c => Weights::W2c,
    };
    match variant::make(run, w, output) {
        Ok(s) => {
            println!(
                "Variant {}: {} weighted quartets ({} quartets without weight); tree in {}",
                s.weights,
                s.weighted_quartets,
                s.quartets_without_weight,
                output.join("tree.nwk").display()
            );
            match &s.evaluation {
                Some(e) => println!(
                    "Against {}: nRF {:.6} ({} of {} splits differ), nQD {:.6}, MSD {}",
                    e.reference_label, e.nrf, e.rf_splits_differ, e.rf_max, e.nqd, e.msd
                ),
                None => println!("The run has no stored reference tree; no comparison."),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

fn read(path: &Path) -> Result<Tree, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Tree::parse(text.trim()).map_err(|e| format!("{}: {e}", path.display()))
}

/// `pairs` are `estimate=truth` file pairs; `scale` divides the labels
/// (1 for S1 and S2, 100 for UFBoot).
pub fn calibration(
    pairs: &[String],
    scale: f64,
    bins: usize,
    output: Option<&PathBuf>,
) -> ExitCode {
    let mut points = Vec::new();
    let mut lines = String::from("# estimate\ttruth\tedges_with_support\tedges_in_true_tree\n");
    for p in pairs {
        let Some((est, truth)) = p.split_once('=') else {
            eprintln!("Error: --pair takes ESTIMATE=TRUTH, not {p}");
            return ExitCode::from(2);
        };
        let result = read(Path::new(est))
            .and_then(|e| read(Path::new(truth)).map(|t| (e, t)))
            .and_then(|(e, t)| calibration::pairs(&e, &t, scale));
        match result {
            Ok(ps) => {
                let right = ps.iter().filter(|(_, y)| *y).count();
                let _ = writeln!(lines, "# {est}\t{truth}\t{}\t{right}", ps.len());
                points.extend(ps);
            }
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::from(1);
            }
        }
    }
    let c = match calibration::calibration(&points, bins) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    let opt = |v: Option<f64>| v.map_or("NA".to_string(), |x| format!("{x:.6}"));
    let mut table = lines;
    let _ = writeln!(
        table,
        "# edges {}; ECE {:.6} ({bins} equal-width bins)",
        c.n, c.ece
    );
    table.push_str("bin_lower\tbin_upper\tedges\tmean_support\tfraction_in_true_tree\n");
    for b in &c.bins {
        let _ = writeln!(
            table,
            "{:.2}\t{:.2}\t{}\t{}\t{}",
            b.lower,
            b.upper,
            b.n,
            opt(b.mean_support),
            opt(b.fraction_true)
        );
    }
    print!("{table}");
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = qmaws_engine::atomic::write_verified(path, table.as_bytes()) {
            eprintln!("Error: {}: {e}", path.display());
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
