//! Hidden development command `s2-cost`: runs a few S2 bootstrap
//! replicates of a finished run and reports their measured cost, for
//! decision D7. Writes `report/s2_cost.txt` in the run folder.

use qmaws_core::newick::Tree;
use qmaws_engine::bootstrap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

pub fn run(run_dir: &Path, replicates: u64, w2c: bool) -> ExitCode {
    let tree = match bootstrap::run_tree(run_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    let splits = Tree::parse(&tree).map(|t| t.splits()).unwrap_or_default();
    let mut found = vec![0u64; splits.len()];
    let mut text = String::new();
    let _ = writeln!(
        text,
        "S2 cost measurement of {}: {replicates} replicates, {} weights; {} threads",
        run_dir.display(),
        if w2c { "W2c" } else { "W2b" },
        std::thread::available_parallelism().map_or(1, |n| n.get())
    );
    let _ = writeln!(
        text,
        "replicate\tseed\tcolumns_used\ttables_s\tweigh_s\tamalgamate_s\ttotal_s\ttable_MB"
    );
    print!("{text}");
    let mut totals = Vec::new();
    for b in 0..replicates {
        let r = match bootstrap::replicate(run_dir, b, w2c) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::from(1);
            }
        };
        let total = r.tables_seconds + r.weigh_seconds + r.amalgamate_seconds;
        totals.push(total);
        let line = format!(
            "{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.1}",
            b,
            r.seed,
            r.columns_used,
            r.tables_seconds,
            r.weigh_seconds,
            r.amalgamate_seconds,
            total,
            r.table_bytes as f64 / 1e6
        );
        println!("{line}");
        let _ = writeln!(text, "{line}");
        if let Ok(t) = Tree::parse(&r.newick) {
            let rs = t.splits();
            for (i, s) in splits.iter().enumerate() {
                if rs.contains(s) {
                    found[i] += 1;
                }
            }
        }
    }
    let mean = totals.iter().sum::<f64>() / totals.len().max(1) as f64;
    let summary = format!(
        "Mean per replicate: {mean:.1} s. At this mean, 100 replicates take {:.1} h on this device (arithmetic from the measured mean).\nSplits of the run's tree found in the replicates: {}",
        mean * 100.0 / 3600.0,
        found
            .iter()
            .map(|c| format!("{c}/{replicates}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("{summary}");
    let _ = writeln!(text, "{summary}");
    let out = run_dir.join("report").join("s2_cost.txt");
    if let Err(e) = std::fs::write(&out, text) {
        eprintln!("Error: {}: {e}", out.display());
        return ExitCode::from(1);
    }
    println!("Written: {}", out.display());
    ExitCode::SUCCESS
}
