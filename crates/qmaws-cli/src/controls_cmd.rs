//! Hidden command `controls`: the positive and negative controls of
//! milestone M8, written to `results/controls/`. The first positive control,
//! the hand-calculable worksheet example, is too short for an analysis run
//! (sequences of 6 letters); it is covered by golden tests G1 and G2.
//!
//! | Control | Data | Expected |
//! |---|---|---|
//! | simulated (positive) | 16 sequences of 20,000 bases evolved along a random tree (Jukes–Cantor, branch lengths 0.01 to 0.1) | nRF near 0 to the true tree, high support |
//! | fish_shuffled (negative) | each Fish mtDNA sequence shuffled on its own | low support; nRF near that of random trees |
//! | fish (reference point) | Fish mtDNA as downloaded | |

use qmaws_core::control;
use qmaws_core::newick::{nrf, Tree};

use qmaws_engine::analysis::{self, AnalysisConfig, AnalysisOptions, WEIGHTING_SYM};
use qmaws_engine::{NullSink, Outcome};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

/// Random trees per reference for the chance level of nRF.
const RANDOM_TREES: usize = 1000;
const SIM_TAXA: usize = 16;
const SIM_LENGTH: usize = 20_000;

struct Control {
    name: &'static str,
    kind: &'static str,
    input: PathBuf,
    reference: PathBuf,
    lengths: Option<Vec<usize>>,
    strand: bool,
}

fn write_fasta(dir: &Path, name: &str, seq: &[u8]) -> std::io::Result<()> {
    let mut text = format!(">{name}\n");
    for line in seq.chunks(70) {
        text.push_str(&String::from_utf8_lossy(line));
        text.push('\n');
    }
    std::fs::write(dir.join(format!("{name}.fasta")), text)
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("Error: {msg}");
    ExitCode::from(1)
}

pub fn run(output: &Path, data_dir: &Path, seed: u64, cancel: &AtomicBool) -> ExitCode {
    let inputs = output.join("inputs");
    let runs = output.join("runs");
    if runs.exists() {
        return fail(format!(
            "{} already exists; remove it to run the controls again",
            runs.display()
        ));
    }
    // Inputs.
    let mut controls = Vec::new();
    let sim = inputs.join("simulated");
    let names: Vec<String> = (1..=SIM_TAXA).map(|i| format!("S{i:02}")).collect();
    let simulated = control::simulate_jc69(&names, SIM_LENGTH, 0.01, 0.1, seed);
    if let Err(e) = std::fs::create_dir_all(sim.join("sequences")).and_then(|_| {
        for (n, s) in names.iter().zip(&simulated.sequences) {
            write_fasta(&sim.join("sequences"), n, s)?;
        }
        std::fs::write(sim.join("true_tree.nwk"), format!("{}\n", simulated.newick))
    }) {
        return fail(e);
    }
    controls.push(Control {
        name: "simulated",
        kind: "positive",
        input: sim.join("sequences"),
        reference: sim.join("true_tree.nwk"),
        lengths: None,
        strand: true,
    });
    let reg = qmaws_data::registry::Registry::builtin();
    let Some(fish) = reg.dataset("fish_mito") else {
        return fail("the dataset fish_mito is not registered");
    };
    let fish_dir = qmaws_data::DataDir::new(data_dir).dataset_path(&reg, fish);
    let loaded = match qmaws_data::loader::load(
        &fish_dir,
        qmaws_core::input::RecordMode::ConcatenatePerFile,
    ) {
        Ok(l) => l,
        Err(e) => return fail(format!("{e} (run qmaws download --dataset fish_mito)")),
    };
    let fish_ref = data_dir.join("references").join("fish_mito.nwk");
    let shuf = inputs.join("fish_shuffled");
    if let Err(e) = std::fs::create_dir_all(&shuf).and_then(|_| {
        for (i, t) in loaded.taxa.iter().enumerate() {
            let s = control::shuffle(
                &t.cleaned.sequence,
                qmaws_core::weight::quartet_seed(seed, i as u64),
            );
            write_fasta(&shuf, &t.name, &s)?;
        }
        Ok(())
    }) {
        return fail(e);
    }
    controls.push(Control {
        name: "fish_shuffled",
        kind: "negative",
        input: shuf.clone(),
        reference: fish_ref.clone(),
        lengths: None,
        strand: true,
    });
    controls.push(Control {
        name: "fish",
        kind: "reference point",
        input: fish_dir,
        reference: fish_ref,
        lengths: None,
        strand: true,
    });
    // Runs and summary.
    let mut tsv = String::from(
        "control\tkind\ttaxa\tnrf\tsplits_differing\trandom_nrf_mean\trandom_nrf_p05\tmean_s1\tmin_s1\tmax_s1\tmean_halo\troot\n",
    );
    let mut md = String::new();
    for c in &controls {
        eprintln!("Control {} ({})...", c.name, c.kind);
        let input = std::path::absolute(&c.input).unwrap_or_else(|_| c.input.clone());
        let options = AnalysisOptions {
            config: AnalysisConfig {
                input: input.display().to_string(),
                records: "per_file".into(),
                strand: c.strand,
                lengths: c.lengths.clone(),
                seed,
                ml_max_columns: qmaws_core::matrix::MAX_ML_COLUMNS,
                weighting: WEIGHTING_SYM.into(),
                replicates: qmaws_core::weight::REPLICATES,
            },
            chunk_seconds: 3.0,
            chunk_quartets: None,
            memory_limit: None,
        };
        let dir = runs.join(c.name);
        let root = match analysis::start(&dir, &options, "terminal", &NullSink, cancel) {
            Ok(Outcome::Finished { root }) => root,
            Ok(Outcome::Stopped) => {
                return fail("stopped; resume the run, then run the controls again")
            }
            Err(e) => return fail(e),
        };
        let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap_or_default();
        let tree = match Tree::parse(read("report/tree.nwk").trim()) {
            Ok(t) => t,
            Err(e) => return fail(e),
        };
        let reference = match std::fs::read_to_string(&c.reference)
            .map_err(|e| e.to_string())
            .and_then(|t| Tree::parse(t.trim()).map_err(|e| e.to_string()))
        {
            Ok(t) => t,
            Err(e) => return fail(format!("{}: {e}", c.reference.display())),
        };
        let (diff, n) = nrf(&tree, &reference);
        let random = control::random_tree_nrf(&reference, RANDOM_TREES, seed);
        let mut sorted = random.clone();
        sorted.sort_by(f64::total_cmp);
        let rmean = random.iter().sum::<f64>() / random.len() as f64;
        let p05 = sorted[(0.05 * sorted.len() as f64) as usize];
        let column = |file: &str, col: usize| -> Vec<f64> {
            read(file)
                .lines()
                .skip(1)
                .filter_map(|l| l.split('\t').nth(col)?.parse().ok())
                .collect()
        };
        let s1 = column("report/support.tsv", 2);
        let halo = column("report/halo.tsv", 1);
        let mean = |v: &[f64]| {
            if v.is_empty() {
                f64::NAN
            } else {
                v.iter().sum::<f64>() / v.len() as f64
            }
        };
        let min = s1.iter().copied().fold(f64::INFINITY, f64::min);
        let max = s1.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let _ = writeln!(
            tsv,
            "{}\t{}\t{}\t{n:.4}\t{diff}\t{rmean:.4}\t{p05:.4}\t{:.4}\t{min:.4}\t{max:.4}\t{:.4}\t{root}",
            c.name,
            c.kind,
            tree.leaf_names().len(),
            mean(&s1),
            mean(&halo)
        );
        let _ = writeln!(
            md,
            "| {} | {} | {} | {n:.3} ({diff} splits) | {rmean:.3} / {p05:.3} | {:.3} ({min:.3} to {max:.3}) | {:.3} |",
            c.name,
            c.kind,
            tree.leaf_names().len(),
            mean(&s1),
            mean(&halo)
        );
    }
    let summary = format!(
        "# Controls\n\nProduced by `qmaws controls --seed {seed}` (hidden development command). Each control is an analysis run in `runs/<control>/` with the default settings (W2-sym, W2c with 100 resamples, wQFM-rs). The first positive control, the hand-calculable worksheet example, is covered by golden tests G1 and G2 (`cargo test`): its sequences have 6 letters, below the 100 that an analysis run accepts. Random-tree levels: nRF of {RANDOM_TREES} random binary trees (random stepwise addition, seed {seed}) to the same reference: mean and 5th percentile.\n\n| Control | Kind | Taxa | nRF to the true or reference tree | Random trees: mean / 5th percentile | Mean S1 (range) | Mean halo |\n|---|---|---|---|---|---|---|\n{md}\nAll values: `controls.tsv`.\n"
    );
    for (name, text) in [("controls.tsv", tsv), ("summary.md", summary.clone())] {
        if let Err(e) = std::fs::write(output.join(name), text) {
            return fail(e);
        }
    }
    println!("{summary}");
    ExitCode::SUCCESS
}
