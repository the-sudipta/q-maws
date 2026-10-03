//! Hidden command `summary`: the comparison tables of milestone M11 in
//! `results/summary/`, built only from finished runs in `results/runs/`
//! and the published values in `baselines/published/`.
//!
//! - A run enters the tables when it is finished (`audit/root.txt`) and
//!   used the primary configuration (W2-sym, W2c with 100 resamples,
//!   strand filter on, entropy-selected lengths). Its dataset is found
//!   from the input folder recorded in `run.json`; two finished runs of
//!   the same dataset and seed are an error.
//! - Q-MAWS values are the mean and sample standard deviation over seeds.
//!   Published values are copied as they are and labelled "reported".
//! - H1 (docs/PREREGISTRATION.md) is computed only when the pairing value
//!   and the zero rule are given (docs/OPEN_ISSUES.md OI-18) and every
//!   HGT dataset has the runs that rule needs.

use qmaws_core::stats::{self, Zeros};
use qmaws_data::registry::Registry;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

/// Datasets of the benchmark, in table order.
pub const DATASETS: [&str; 14] = [
    "fish_mito",
    "ecoli",
    "ecoli_shigella_hgt",
    "yersinia_hgt",
    "sim_hgt_0",
    "sim_hgt_250",
    "sim_hgt_500",
    "sim_hgt_750",
    "sim_hgt_1000",
    "influenza_a",
    "coronavirus",
    "mammal_mtdna",
    "ebolavirus",
    "rhinovirus",
];

/// The 7 HGT datasets of H1, in the order of the pre-registration.
pub const H1_DATASETS: [&str; 7] = [
    "sim_hgt_0",
    "sim_hgt_250",
    "sim_hgt_500",
    "sim_hgt_750",
    "sim_hgt_1000",
    "ecoli_shigella_hgt",
    "yersinia_hgt",
];

/// Seeds of every stochastic configuration (pre-registration).
pub const SEEDS: u64 = 5;

/// Which Q-MAWS value enters an H1 pair (OI-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum H1Pairs {
    /// Mean over the 5 seeds
    Mean,
    /// Median over the 5 seeds
    Median,
    /// Seed 1 only
    Seed1,
}

/// How zero differences enter the H1 test (OI-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum H1Zeros {
    /// Dropped before ranking
    Wilcoxon,
    /// Ranked, then left out
    Pratt,
}

/// One finished run of the primary configuration.
#[derive(Debug, Clone)]
pub struct RunRow {
    pub dataset: String,
    pub seed: u64,
    pub folder: String,
    pub root: String,
    pub nrf: Option<f64>,
    pub nqd: Option<f64>,
    pub msd: Option<f64>,
    pub mean_s1: Option<f64>,
    pub mean_s2: Option<f64>,
    pub elapsed_seconds: f64,
}

/// Splits CSV text into records (RFC 4180 quoting; newlines inside quotes
/// are kept).
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// CSV records as maps from the header names.
fn csv_maps(path: &Path) -> Result<Vec<BTreeMap<String, String>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = parse_csv(&text).into_iter();
    let header = rows
        .next()
        .ok_or_else(|| format!("{}: empty", path.display()))?;
    Ok(rows
        .filter(|r| r.len() == header.len())
        .map(|r| header.iter().cloned().zip(r).collect())
        .collect())
}

/// Values of the `value` column of a two- or three-column TSV report by
/// its first column.
fn tsv_values(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.next()?.to_string()))
        })
        .collect()
}

/// Mean of a named numeric column of a TSV file; `None` when the file or
/// column is missing or has no rows.
fn tsv_column_mean(path: &Path, column: &str) -> Option<f64> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let idx = lines.next()?.split('\t').position(|h| h == column)?;
    let values: Vec<f64> = lines
        .filter_map(|l| l.split('\t').nth(idx)?.parse().ok())
        .collect();
    stats::mean_sd(&values).map(|(m, _)| m)
}

/// Dataset id of a recorded input path: the dataset whose
/// `<download>/<path>` the input ends with.
pub fn dataset_of_input(registry: &Registry, input: &str) -> Option<String> {
    let input = input.replace('\\', "/");
    registry
        .datasets
        .iter()
        .filter(|d| DATASETS.contains(&d.id.as_str()))
        .find(|d| {
            let tail = format!("{}/{}", d.download, d.path);
            input.ends_with(&tail) || input.ends_with(&format!("{tail}/"))
        })
        .map(|d| d.id.clone())
}

/// Reads one run folder; `Ok(None)` for unfinished runs and other
/// configurations.
fn read_run(registry: &Registry, dir: &Path) -> Result<Option<RunRow>, String> {
    let Ok(text) = std::fs::read_to_string(dir.join("run.json")) else {
        return Ok(None);
    };
    let Ok(root) = std::fs::read_to_string(dir.join("audit").join("root.txt")) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))?;
    let c = &json["config"];
    let primary = c["weighting"] == "w2_sym"
        && c["replicates"] == 100
        && c["strand"] == true
        && c["lengths"].is_null();
    if json["kind"] != "analysis" || !primary {
        return Ok(None);
    }
    let input = c["input"].as_str().unwrap_or_default();
    let Some(dataset) = dataset_of_input(registry, input) else {
        return Ok(None);
    };
    let seed = c["seed"]
        .as_u64()
        .ok_or_else(|| format!("{}: no seed in run.json", dir.display()))?;
    let report = dir.join("report");
    let cmp = tsv_values(&report.join("reference_comparison.tsv"));
    let num = |k: &str| cmp.get(k).and_then(|v| v.parse::<f64>().ok());
    Ok(Some(RunRow {
        dataset,
        seed,
        folder: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        root: root.trim().to_string(),
        nrf: num("nRF"),
        nqd: num("nQD"),
        msd: num("MSD"),
        mean_s1: tsv_column_mean(&report.join("support.tsv"), "s1"),
        mean_s2: tsv_column_mean(&report.join("bootstrap.tsv"), "s2"),
        elapsed_seconds: json["elapsed_seconds"].as_f64().unwrap_or(f64::NAN),
    }))
}

/// All finished primary-configuration runs, sorted by dataset order and
/// seed.
pub fn collect_runs(registry: &Registry, runs: &Path) -> Result<Vec<RunRow>, String> {
    let mut rows = Vec::new();
    let entries = std::fs::read_dir(runs).map_err(|e| format!("{}: {e}", runs.display()))?;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            if let Some(row) = read_run(registry, &entry.path())? {
                rows.push(row);
            }
        }
    }
    let order = |d: &str| DATASETS.iter().position(|x| *x == d).unwrap_or(usize::MAX);
    rows.sort_by(|a, b| {
        (order(&a.dataset), a.seed, &a.folder).cmp(&(order(&b.dataset), b.seed, &b.folder))
    });
    for w in rows.windows(2) {
        if w[0].dataset == w[1].dataset && w[0].seed == w[1].seed {
            return Err(format!(
                "two finished runs of {} seed {}: {} and {}",
                w[0].dataset, w[0].seed, w[0].folder, w[1].folder
            ));
        }
    }
    Ok(rows)
}

fn fmt_opt(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| "-".to_string(), |x| format!("{x:.digits$}"))
}

/// Mean and sample SD over the runs; the SD is NaN (printed "-") for a
/// single run.
fn mean_sd_of(rows: &[&RunRow], f: impl Fn(&RunRow) -> Option<f64>) -> Option<(f64, f64)> {
    let v: Option<Vec<f64>> = rows.iter().map(|r| f(r)).collect();
    let v = v?;
    stats::mean_sd(&v).map(|(m, s)| (m, if v.len() < 2 { f64::NAN } else { s }))
}

fn fmt_num(x: f64, digits: usize) -> String {
    if x.is_nan() {
        "-".to_string()
    } else {
        format!("{x:.digits$}")
    }
}

fn fmt_mean_sd(ms: Option<(f64, f64)>, digits: usize) -> String {
    match ms {
        Some((m, s)) if s.is_nan() => format!("{m:.digits$}"),
        Some((m, s)) => format!("{m:.digits$} ± {s:.digits$}"),
        None => "-".to_string(),
    }
}

/// Reported value of `method/variant` on a dataset and metric.
fn reported(
    table: &[BTreeMap<String, String>],
    dataset: &str,
    variant: &str,
    metric: &str,
) -> Option<f64> {
    table
        .iter()
        .find(|r| {
            r.get("dataset").map(String::as_str) == Some(dataset)
                && r.get("variant").is_none_or(|v| v == variant)
                && r.get("metric").map(String::as_str) == Some(metric)
        })
        .and_then(|r| r["value"].parse().ok())
}

/// Result of the H1 test for one metric.
#[derive(Debug, Clone)]
pub struct H1Metric {
    pub metric: &'static str,
    pub pairs: Vec<(String, f64, f64)>,
    pub test: stats::SignedRank,
    pub median: f64,
    pub hodges_lehmann: f64,
}

/// Q-MAWS value of a dataset under the pairing rule; `Err` names what is
/// missing.
fn h1_value(
    rows: &[&RunRow],
    pairs: H1Pairs,
    f: impl Fn(&RunRow) -> Option<f64>,
) -> Result<f64, String> {
    let values = |rs: &[&RunRow]| -> Option<Vec<f64>> { rs.iter().map(|r| f(r)).collect() };
    match pairs {
        H1Pairs::Seed1 => {
            let r: Vec<&RunRow> = rows.iter().copied().filter(|r| r.seed == 1).collect();
            values(&r)
                .and_then(|v| v.first().copied())
                .ok_or_else(|| "seed 1".to_string())
        }
        H1Pairs::Mean | H1Pairs::Median => {
            let r: Vec<&RunRow> = rows
                .iter()
                .copied()
                .filter(|r| (1..=SEEDS).contains(&r.seed))
                .collect();
            if r.len() as u64 != SEEDS {
                return Err(format!("all {SEEDS} seeds ({} finished)", r.len()));
            }
            let v = values(&r).ok_or_else(|| "a metric value".to_string())?;
            Ok(if pairs == H1Pairs::Mean {
                stats::mean_sd(&v).map(|(m, _)| m).unwrap_or(f64::NAN)
            } else {
                stats::median(&v).unwrap_or(f64::NAN)
            })
        }
    }
}

/// Computes H1 for nRF and nQD; Holm-adjusted p-values in the same order.
pub fn h1(
    runs: &[RunRow],
    table3: &[BTreeMap<String, String>],
    pairs: H1Pairs,
    zeros: Zeros,
) -> Result<(Vec<H1Metric>, Vec<f64>), String> {
    let mut out = Vec::new();
    for (metric, f) in [
        ("nRF", (|r: &RunRow| r.nrf) as fn(&RunRow) -> Option<f64>),
        ("nQD", |r: &RunRow| r.nqd),
    ] {
        let mut prs = Vec::new();
        for ds in H1_DATASETS {
            let rows: Vec<&RunRow> = runs.iter().filter(|r| r.dataset == ds).collect();
            let q = h1_value(&rows, pairs, f).map_err(|m| format!("H1 needs {m} of {ds}"))?;
            let ml = reported(table3, ds, "IQ-TREE (strand)", metric)
                .ok_or_else(|| format!("no reported ML-MAWS {metric} for {ds}"))?;
            prs.push((ds.to_string(), q, ml));
        }
        let diffs: Vec<f64> = prs.iter().map(|(_, q, ml)| q - ml).collect();
        let test = stats::signed_rank_less(&diffs, zeros)?;
        out.push(H1Metric {
            metric,
            median: stats::median(&diffs).unwrap_or(f64::NAN),
            hodges_lehmann: stats::hodges_lehmann(&diffs).unwrap_or(f64::NAN),
            pairs: prs,
            test,
        });
    }
    let adjusted = stats::holm(&out.iter().map(|m| m.test.p_less).collect::<Vec<_>>());
    Ok((out, adjusted))
}

/// AFproject datasets: id, upload page (read 2026-10-03; one Newick file
/// per dataset, leaves named by the FASTA file names without extension).
pub const AFPROJECT_UPLOADS: [(&str, &str); 9] = [
    (
        "fish_mito",
        "https://afproject.org/app/benchmark/genome/std/assembled/fish_mito/",
    ),
    (
        "ecoli",
        "https://afproject.org/app/benchmark/genome/std/assembled/ecoli/",
    ),
    (
        "ecoli_shigella_hgt",
        "https://afproject.org/app/benchmark/genome/hgt/unsimulated/ecoli_shigella/",
    ),
    (
        "yersinia_hgt",
        "https://afproject.org/app/benchmark/genome/hgt/unsimulated/yersinia/",
    ),
    (
        "sim_hgt_0",
        "https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 0)",
    ),
    (
        "sim_hgt_250",
        "https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 250)",
    ),
    (
        "sim_hgt_500",
        "https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 500)",
    ),
    (
        "sim_hgt_750",
        "https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 750)",
    ),
    (
        "sim_hgt_1000",
        "https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 1000)",
    ),
];

/// Copies the seed-1 tree (`report/tree.nwk`, topology only) of every
/// AFproject dataset to `dir` as `<dataset>.nwk` and writes its README.
fn afproject_files(runs: &[RunRow], runs_dir: &Path, dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut table = String::new();
    let mut missing = Vec::new();
    for (ds, page) in AFPROJECT_UPLOADS {
        let Some(r) = runs.iter().find(|r| r.dataset == ds && r.seed == 1) else {
            missing.push(ds.to_string());
            let _ = writeln!(
                table,
                "| - | {ds} | {page} | no finished seed-1 run yet | |"
            );
            continue;
        };
        let src = runs_dir.join(&r.folder).join("report").join("tree.nwk");
        let tree = std::fs::read_to_string(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        let file = format!("{ds}.nwk");
        write(&dir.join(&file), &tree)?;
        let _ = writeln!(
            table,
            "| `{file}` | {ds} | {page} | `results/runs/{}/report/tree.nwk` | `{}` |",
            r.folder, r.root
        );
    }
    let readme = format!(
        "# afproject_submission\n\n## Purpose\nPrediction files in the format required by each AFproject dataset upload page, and the results AFproject returned, with access dates.\n\n## Contents\n| Item | Dataset | Upload page | Copied from | Run root |\n|---|---|---|---|---|\n{table}\n## Relationships\nWritten by the hidden command `qmaws summary --afproject` (`crates/qmaws-cli/src/summary_cmd.rs`) from the seed-1 runs in `results/runs/`. The owner uploads the files manually.\n\n## Notes\nFormat (read on the upload and dataset pages, 2026-10-03): AFproject accepts a TSV of pairwise distances, a PHYLIP distance matrix, or a tree in Newick format; branch lengths are optional. The files here are Newick trees without branch lengths or support values, with leaves named by the FASTA file names of the dataset without the extension. The simulated HGT page takes one file per HGT level (0, 250, 500, 750, 1000) in one submission. Form fields: method name `Q-MAWS`; method parameters `qmaws run --dataset <id> --seed 1` (primary configuration: W2-sym, W2c with 100 resamples, wQFM-rs, strand filter on). AFproject scores with its own tools (EMBOSS ftreedist); its returned scores are recorded here with the access date once uploaded.\n"
    );
    write(&dir.join("README.md"), &readme)?;
    Ok(missing)
}

/// Writes one output file.
fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn run(
    runs_dir: &Path,
    baselines: &Path,
    output: &Path,
    h1_rule: Option<(H1Pairs, H1Zeros)>,
    afproject: Option<&Path>,
) -> ExitCode {
    match build(runs_dir, baselines, output, h1_rule, afproject) {
        Ok(lines) => {
            for l in lines {
                println!("{l}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

fn build(
    runs_dir: &Path,
    baselines: &Path,
    output: &Path,
    h1_rule: Option<(H1Pairs, H1Zeros)>,
    afproject_dir: Option<&Path>,
) -> Result<Vec<String>, String> {
    let registry = Registry::builtin();
    let runs = collect_runs(&registry, runs_dir)?;
    let table3 = csv_maps(&baselines.join("ml_maws_table3.csv"))?;
    let peafowl = csv_maps(&baselines.join("peafowl.csv"))?;
    let afproject = csv_maps(&baselines.join("afproject_results.csv"))?;
    std::fs::create_dir_all(output).map_err(|e| format!("{}: {e}", output.display()))?;
    let mut notes = Vec::new();

    // runs.tsv: every run that enters the tables.
    let mut t = String::from(
        "dataset\tseed\trun\troot\tnrf\tnqd\tmsd\tmean_s1\tmean_s2\telapsed_seconds\n",
    );
    for r in &runs {
        let _ = writeln!(
            t,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}",
            r.dataset,
            r.seed,
            r.folder,
            r.root,
            fmt_opt(r.nrf, 6),
            fmt_opt(r.nqd, 6),
            fmt_opt(r.msd, 0),
            fmt_opt(r.mean_s1, 6),
            fmt_opt(r.mean_s2, 6),
            r.elapsed_seconds
        );
    }
    write(&output.join("runs.tsv"), &t)?;

    // qmaws.tsv and comparison.tsv: one line per dataset.
    let mut q = String::from(
        "dataset\ttaxa\tseeds\tnrf_mean\tnrf_sd\tnqd_mean\tnqd_sd\tmsd_mean\tmsd_sd\ts1_mean\ts2_seed1\telapsed_mean_seconds\telapsed_sd_seconds\n",
    );
    let mut c = String::from(
        "dataset\tmetric\tqmaws_mean\tqmaws_sd\tseeds\tml_maws_strand_reported\tml_maws_nostrand_reported\tpeafowl_reported\tafproject_best_reported\tafproject_tools\tafproject_tools_lower\n",
    );
    let mut md_rows = String::new();
    for ds in DATASETS {
        let rows: Vec<&RunRow> = runs.iter().filter(|r| r.dataset == ds).collect();
        let taxa = registry.dataset(ds).map_or(0, |d| d.taxa);
        let seeds: Vec<String> = rows.iter().map(|r| r.seed.to_string()).collect();
        let seeds = if seeds.is_empty() {
            "-".to_string()
        } else {
            seeds.join(",")
        };
        let split = |ms: Option<(f64, f64)>, d: usize| match ms {
            Some((m, s)) => (format!("{m:.d$}"), fmt_num(s, d)),
            None => ("-".to_string(), "-".to_string()),
        };
        let nrf = mean_sd_of(&rows, |r| r.nrf);
        let nqd = mean_sd_of(&rows, |r| r.nqd);
        let msd = mean_sd_of(&rows, |r| r.msd);
        let s1 = mean_sd_of(&rows, |r| r.mean_s1);
        let s2 = rows.iter().find(|r| r.seed == 1).and_then(|r| r.mean_s2);
        let el = mean_sd_of(&rows, |r| Some(r.elapsed_seconds));
        let (a, b) = split(nrf, 6);
        let (cq, d) = split(nqd, 6);
        let (e, f) = split(msd, 1);
        let (g, h) = split(el, 1);
        let _ = writeln!(
            q,
            "{ds}\t{taxa}\t{seeds}\t{a}\t{b}\t{cq}\t{d}\t{e}\t{f}\t{}\t{}\t{g}\t{h}",
            fmt_opt(s1.map(|x| x.0), 6),
            fmt_opt(s2, 6)
        );
        for (metric, ms) in [("nRF", nrf), ("nQD", nqd)] {
            let strand = reported(&table3, ds, "IQ-TREE (strand)", metric);
            let nostrand = reported(&table3, ds, "NoStrand", metric);
            let pea = if metric == "nRF" {
                reported(&peafowl, ds, "", "nRF")
            } else {
                None
            };
            let col = if metric == "nRF" { "nrf" } else { "nqd" };
            let af: Vec<f64> = afproject
                .iter()
                .filter(|r| r["dataset"] == ds)
                .filter_map(|r| r[col].parse().ok())
                .collect();
            let best = af.iter().copied().reduce(f64::min);
            // AFproject lists nRF with 2 and nQD with 4 decimals; Q-MAWS is
            // rounded the same way before counting tools strictly lower.
            let places = if metric == "nRF" { 2 } else { 4 };
            let lower = ms.map(|(m, _)| {
                let scale = 10f64.powi(places);
                let qm = (m * scale).round();
                af.iter().filter(|v| (*v * scale).round() < qm).count()
            });
            let (m, s) = split(ms, 6);
            let _ = writeln!(
                c,
                "{ds}\t{metric}\t{m}\t{s}\t{seeds}\t{}\t{}\t{}\t{}\t{}\t{}",
                fmt_opt(strand, 3),
                fmt_opt(nostrand, 3),
                fmt_opt(pea, 2),
                fmt_opt(best, places as usize),
                if af.is_empty() {
                    "-".to_string()
                } else {
                    af.len().to_string()
                },
                lower.map_or("-".to_string(), |n| if af.is_empty() {
                    "-".to_string()
                } else {
                    n.to_string()
                })
            );
            let _ = writeln!(
                md_rows,
                "| {ds} | {metric} | {} | {seeds} | {} | {} | {} | {} |",
                fmt_mean_sd(ms, 3),
                fmt_opt(strand, 3),
                fmt_opt(nostrand, 3),
                fmt_opt(pea, 2),
                fmt_opt(best, places as usize),
            );
        }
        if rows.is_empty() {
            notes.push(format!("{ds}: no finished run yet"));
        } else if rows.len() as u64 != SEEDS {
            notes.push(format!("{ds}: {} of {SEEDS} seeds finished", rows.len()));
        }
    }
    write(&output.join("qmaws.tsv"), &q)?;
    write(&output.join("comparison.tsv"), &c)?;

    // h1.tsv only with an explicit rule and complete data.
    let mut h1_md = String::from(
        "H1 is not computed: the pairing value and the zero rule (docs/OPEN_ISSUES.md OI-18) were not given.\n",
    );
    let mut items = vec![
        ("runs.tsv", "Every finished run of the primary configuration: dataset, seed, folder, root hash, nRF, nQD, MSD, mean S1, mean S2, working time"),
        ("qmaws.tsv", "Q-MAWS per dataset: mean and sample SD over seeds of nRF, nQD, MSD (raw), mean S1 and working time; mean S2 of seed 1"),
        ("comparison.tsv", "Q-MAWS nRF and nQD beside the reported values of ML-MAWS (Table III), Peafowl and AFproject (best value, number of tools, tools strictly lower than Q-MAWS at AFproject's precision)"),
    ];
    if let Some((pairs, z)) = h1_rule {
        let zeros = match z {
            H1Zeros::Wilcoxon => Zeros::Wilcoxon,
            H1Zeros::Pratt => Zeros::Pratt,
        };
        match h1(&runs, &table3, pairs, zeros) {
            Ok((metrics, adjusted)) => {
                let mut h =
                    String::from("metric\tdataset\tqmaws\tml_maws_strand_reported\tdifference\n");
                let mut s = String::from("metric\tn\tzeros\tw_plus\tw_minus\tp_one_sided\tp_holm\tmedian_difference\thodges_lehmann\tcriterion_met\n");
                h1_md = format!(
                    "Pairing value: {pairs:?}; zero differences: {z:?} (OI-18). One-sided exact Wilcoxon signed-rank test of Q-MAWS minus ML-MAWS strand-aware (reported), Holm across the two metrics. Criterion: adjusted p < 0.05 for at least one metric, and median difference < 0. With 7 datasets the power of the test is limited.\n\n| Metric | n | Zeros | W+ | p (one-sided) | p (Holm) | Median difference | Hodges–Lehmann | Criterion met for this metric |\n|---|---|---|---|---|---|---|---|---|\n"
                );
                let mut any = false;
                for (m, p_adj) in metrics.iter().zip(&adjusted) {
                    let met = *p_adj < 0.05 && m.median < 0.0;
                    any |= met;
                    for (ds, qv, ml) in &m.pairs {
                        let _ = writeln!(h, "{}\t{ds}\t{qv:.6}\t{ml:.3}\t{:.6}", m.metric, qv - ml);
                    }
                    let _ = writeln!(
                        s,
                        "{}\t{}\t{}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}",
                        m.metric,
                        m.test.n,
                        m.test.zeros,
                        m.test.w_plus,
                        m.test.w_minus,
                        m.test.p_less,
                        p_adj,
                        m.median,
                        m.hodges_lehmann,
                        met
                    );
                    let _ = writeln!(
                        h1_md,
                        "| {} | {} | {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {} |",
                        m.metric,
                        m.test.n,
                        m.test.zeros,
                        m.test.w_plus,
                        m.test.p_less,
                        p_adj,
                        m.median,
                        m.hodges_lehmann,
                        if met { "yes" } else { "no" }
                    );
                }
                let _ = writeln!(
                    h1_md,
                    "\nH1 criterion met: **{}**. Per-dataset pairs: `h1_pairs.tsv`.",
                    if any { "yes" } else { "no" }
                );
                write(&output.join("h1_pairs.tsv"), &h)?;
                write(&output.join("h1.tsv"), &s)?;
                items.push(("h1.tsv", "H1 test per metric: n, zeros, W+, W-, one-sided exact p, Holm-adjusted p, median difference, Hodges–Lehmann estimate"));
                items.push(("h1_pairs.tsv", "H1 pairs: Q-MAWS value under the pairing rule, reported ML-MAWS strand-aware value, difference"));
            }
            Err(e) => {
                h1_md = format!("H1 is not computed yet: {e}.\n");
            }
        }
    }

    let mut readme = String::from(
        "# summary\n\n## Purpose\nFinal comparison tables between Q-MAWS results and published baselines, with every per-dataset value.\n\n## Contents\n| Item | Description |\n|---|---|\n",
    );
    for (item, what) in &items {
        let _ = writeln!(readme, "| `{item}` | {what} |");
    }
    let _ = write!(
        readme,
        "\n## Comparison\nQ-MAWS: mean ± sample SD over the seeds listed. Other columns are reported values (ML-MAWS: Table III of its paper; Peafowl: text of its paper; AFproject: best value among the tools on its results page). Lower is better. Datasets without a reference tree have no nRF or nQD.\n\n| Dataset | Metric | Q-MAWS | Seeds | ML-MAWS strand (reported) | ML-MAWS NoStrand (reported) | Peafowl (reported) | AFproject best (reported) |\n|---|---|---|---|---|---|---|---|\n{md_rows}\n## H1\n{h1_md}\n## Relationships\nBuilt by the hidden command `qmaws summary` (`crates/qmaws-cli/src/summary_cmd.rs`) from `results/runs/` and `baselines/published/` (milestone M11).\n\n## Notes\nOnly finished runs of the primary configuration (W2-sym, W2c with 100 resamples, strand filter on, entropy-selected lengths) enter the tables. S2 is run on seed 1 only (owner decision, 2026-10-03). Losses are reported alongside wins.\n"
    );
    if !notes.is_empty() {
        let _ = writeln!(readme, "\nIncomplete when written: {}.", notes.join("; "));
    }
    write(&output.join("README.md"), &readme)?;

    let mut lines = vec![format!(
        "{} runs in the tables; written to {}",
        runs.len(),
        output.display()
    )];
    lines.extend(notes);
    if let Some(dir) = afproject_dir {
        let missing = afproject_files(&runs, runs_dir, dir)?;
        lines.push(format!(
            "AFproject files written to {}{}",
            dir.display(),
            if missing.is_empty() {
                String::new()
            } else {
                format!("; no seed-1 run yet: {}", missing.join(", "))
            }
        ));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_handles_quotes_commas_and_newlines() {
        let rows = parse_csv("a,b,c\n1,\"x, \"\"y\"\"\",\"line\nbreak\"\r\n");
        assert_eq!(rows[0], vec!["a", "b", "c"]);
        assert_eq!(rows[1], vec!["1", "x, \"y\"", "line\nbreak"]);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn input_paths_map_to_dataset_ids() {
        let reg = Registry::builtin();
        assert_eq!(
            dataset_of_input(
                &reg,
                r"D:\q-maws\data\raw\sim_hgt\simulated-sim_hgt\hgt_250"
            )
            .as_deref(),
            Some("sim_hgt_250")
        );
        assert_eq!(
            dataset_of_input(&reg, r"D:\q-maws\data\raw\fish_mito\assembled-fish_mito").as_deref(),
            Some("fish_mito")
        );
        assert_eq!(dataset_of_input(&reg, "/tmp/my_own_folder"), None);
    }

    fn row(ds: &str, seed: u64, nrf: f64, nqd: f64) -> RunRow {
        RunRow {
            dataset: ds.to_string(),
            seed,
            folder: format!("{ds}_seed{seed}"),
            root: String::new(),
            nrf: Some(nrf),
            nqd: Some(nqd),
            msd: None,
            mean_s1: None,
            mean_s2: None,
            elapsed_seconds: 1.0,
        }
    }

    fn table3() -> Vec<BTreeMap<String, String>> {
        let mut t = Vec::new();
        for ds in H1_DATASETS {
            for metric in ["nRF", "nQD"] {
                t.push(
                    [
                        ("dataset", ds),
                        ("variant", "IQ-TREE (strand)"),
                        ("metric", metric),
                        ("value", "0.500"),
                    ]
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                );
            }
        }
        t
    }

    #[test]
    fn h1_needs_every_seed_for_the_mean() {
        let runs: Vec<RunRow> = H1_DATASETS.iter().map(|d| row(d, 1, 0.4, 0.2)).collect();
        let err = h1(&runs, &table3(), H1Pairs::Mean, Zeros::Wilcoxon).unwrap_err();
        assert!(err.contains("all 5 seeds (1 finished)"), "{err}");
        let (m, adj) = h1(&runs, &table3(), H1Pairs::Seed1, Zeros::Wilcoxon).unwrap();
        // Every difference is -0.1 or -0.3: W+ = 0, p = 1/128, Holm 2/128.
        assert_eq!(m[0].test.p_less, 1.0 / 128.0);
        assert_eq!(adj, vec![2.0 / 128.0, 2.0 / 128.0]);
        assert!((m[0].median + 0.1).abs() < 1e-12);
    }

    #[test]
    fn h1_mean_and_median_pairing() {
        let mut runs = Vec::new();
        for ds in H1_DATASETS {
            for (seed, v) in [(1, 0.1), (2, 0.2), (3, 0.3), (4, 0.4), (5, 1.0)] {
                runs.push(row(ds, seed, v, v));
            }
        }
        let (mean, _) = h1(&runs, &table3(), H1Pairs::Mean, Zeros::Wilcoxon).unwrap();
        assert!((mean[0].pairs[0].1 - 0.4).abs() < 1e-12);
        let (med, _) = h1(&runs, &table3(), H1Pairs::Median, Zeros::Wilcoxon).unwrap();
        assert!((med[0].pairs[0].1 - 0.3).abs() < 1e-12);
    }
}
