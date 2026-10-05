//! The `qmaws` binary: argument parsing and dispatch to terminal or GUI mode.
//!
//! `qmaws menu` (also `run.bat` and `run.sh` without arguments) shows the
//! interactive main menu; `qmaws gui` opens the window; `run` and `resume`
//! take `--gui` or `--terminal` (default: terminal).

use clap::{Parser, Subcommand};
use qmaws_engine::launch::{self, UserConfig};
use qmaws_engine::rundir::{new_run_dir, run_id, DEFAULT_RUNS_ROOT};
use qmaws_engine::{clock::UtcDateTime, start_toy_run, Outcome, ToyOptions};
use qmaws_tui::{DisplayMode, TerminalDisplay};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Exit status when a run stopped on request and can be resumed.
const EXIT_STOPPED: u8 = 3;
/// Exit status for errors.
const EXIT_ERROR: u8 = 1;
/// Exit status for usage problems.
const EXIT_USAGE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "qmaws",
    version,
    about = "Q-MAWS: Quartet-based phylogeny from Minimal Absent Word Sets",
    after_help = "Start the interactive menu with 'qmaws menu', or the window with 'qmaws gui'."
)]
struct Cli {
    /// Show only the final result and errors
    #[arg(long, global = true, conflicts_with = "json_progress")]
    quiet: bool,

    /// Print progress as one JSON object per line on standard output
    #[arg(long, global = true)]
    json_progress: bool,

    /// Do not use colours
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the interactive main menu: start, resume or verify a run
    Menu {
        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Open the graphical interface (main menu in a window)
    Gui {
        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Analyse a folder or a benchmark dataset: tree, S1 and S2 support, halo values, comparison with a reference tree, figures
    Run {
        /// Folder of sequence files, or one multi-FASTA file
        #[arg(long, conflicts_with = "dataset", required_unless_present = "dataset")]
        input: Option<PathBuf>,

        /// Benchmark dataset id, or "all" (one run per dataset, one after another); missing datasets are downloaded
        #[arg(long)]
        dataset: Option<String>,

        /// Run folder [default: results/runs/<name>_<date>_<time>; with --dataset all, one such folder per dataset]
        #[arg(long)]
        output: Option<PathBuf>,

        /// Reference tree (Newick) to compare the result with [default: the benchmark dataset's reference, if any]
        #[arg(long)]
        reference: Option<PathBuf>,

        /// Input warning "same name": rename duplicate names by appending _2, _3, ...
        #[arg(long)]
        rename_duplicates: bool,

        /// Input warnings: leave out these taxa (empty file, the second of two identical sequences, short sequence)
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,

        /// Input warnings: keep these taxa (the second of two identical sequences, short sequence)
        #[arg(long, value_delimiter = ',')]
        keep: Vec<String>,

        /// Number of CPU cores to use [default: all]
        #[arg(long)]
        cores: Option<usize>,

        /// Memory limit in GB [default: 70% of the available memory]
        #[arg(long)]
        memory_limit: Option<f64>,

        /// Do not apply the strand filter
        #[arg(long)]
        no_strand: bool,

        /// Fixed MAW lengths, for example 7,8,9, instead of the entropy selection
        #[arg(long, value_delimiter = ',')]
        lengths: Option<Vec<usize>>,

        /// Random seed of the quartet processing order
        #[arg(long, default_value_t = 1)]
        seed: u64,

        /// Target duration of one chunk of quartets, in seconds
        #[arg(long, default_value_t = 3.0)]
        chunk_seconds: f64,

        /// Fixed number of quartets per chunk instead of calibration (testing)
        #[arg(long, hide = true)]
        chunk_quartets: Option<u64>,

        /// Quartet weighting model
        #[arg(long, value_enum, default_value_t = Weighting::W2Sym)]
        weighting: Weighting,

        /// Number of W2c resamples per quartet (0: no W2c)
        #[arg(long, default_value_t = qmaws_core::weight::REPLICATES)]
        replicates: u32,

        /// Number of S2 column-bootstrap replicates, W2b inside (0: no S2)
        #[arg(long, default_value_t = qmaws_engine::analysis::BOOTSTRAP_REPLICATES)]
        bootstrap: u32,

        /// Do not draw the live provisional tree while quartets are weighed
        #[arg(long)]
        no_live_tree: bool,

        /// Show the run in the graphical interface
        #[arg(long, conflicts_with = "terminal")]
        gui: bool,

        /// Show the run in the terminal (the default)
        #[arg(long)]
        terminal: bool,

        /// For a folder: one taxon per file (records joined) or one per record
        #[arg(long, value_enum, default_value_t = data_cmd::Records::PerFile)]
        records: data_cmd::Records,

        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Run a toy computation that exercises checkpoints, resume and progress display
    ToyRun {
        /// Run folder [default: results/runs/toy_<date>_<time>]
        #[arg(long)]
        output: Option<PathBuf>,

        /// Number of work blocks (each block is 1,048,576 steps)
        #[arg(long, default_value_t = 20_000)]
        blocks: u64,

        /// Random seed
        #[arg(long, default_value_t = 1)]
        seed: u64,

        /// Target duration of one chunk, in seconds
        #[arg(long, default_value_t = 3.0)]
        chunk_seconds: f64,

        /// Fixed number of blocks per chunk instead of calibration (testing)
        #[arg(long, hide = true)]
        chunk_blocks: Option<u64>,
    },

    /// Resume an unfinished run
    Resume {
        /// Run folder to resume [default: the only unfinished run]
        #[arg(long, conflicts_with = "all")]
        output: Option<PathBuf>,

        /// Resume every unfinished run, one after another (the queue survives interruptions)
        #[arg(long)]
        all: bool,

        /// Show the run in the graphical interface
        #[arg(long, conflicts_with = "terminal")]
        gui: bool,

        /// Show the run in the terminal (the default)
        #[arg(long)]
        terminal: bool,

        /// Data folder: benchmark reference trees for the figures [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Check a finished run against its audit record (default: input check and quick check)
    Verify {
        /// Run folder to verify
        #[arg(long)]
        output: PathBuf,

        /// Only the input check
        #[arg(long, conflicts_with_all = ["full", "quartet", "quick"])]
        inputs: bool,

        /// Input check and quick check (the default)
        #[arg(long, conflicts_with_all = ["full", "quartet"])]
        quick: bool,

        /// Recompute the whole run and compare the root fingerprint
        #[arg(long, conflicts_with = "quartet")]
        full: bool,

        /// Recompute one quartet, e.g. A,B,C,D, print its worksheet and compare its decision
        #[arg(long, value_delimiter = ',')]
        quartet: Option<Vec<String>>,

        /// Seed of the quick check's chunk draw [default: a fresh seed, printed]
        #[arg(long)]
        seed: Option<u64>,

        /// Input folder or file, if the data are no longer where the run read them
        #[arg(long)]
        input: Option<PathBuf>,
    },

    /// Draw the figures of a finished run again (Halo Tree, rectangular tree, tanglegram, HTML, animation)
    Figures {
        /// Run folder
        #[arg(long)]
        output: PathBuf,

        /// Reference tree (Newick) for the tanglegram [default: the benchmark dataset's reference, if any]
        #[arg(long)]
        reference: Option<PathBuf>,

        /// Group file for the group bands: one "taxon<TAB>group" per line [default: groups.tsv in the input folder]
        #[arg(long)]
        groups: Option<PathBuf>,

        /// Take the groups from the Open Tree of Life taxonomy (needs the internet)
        #[arg(long)]
        otl: bool,

        /// Colour the branches by S2 instead of S1
        #[arg(long)]
        s2: bool,

        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// List the benchmark datasets and whether they are downloaded
    Datasets {
        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Download, verify and extract benchmark datasets
    Download {
        /// Dataset id, or "all"
        #[arg(long)]
        dataset: String,

        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Read a sequence folder or file (or a downloaded dataset) and report taxa and problems
    Inspect {
        /// Folder of sequence files, or one multi-FASTA file
        #[arg(long, conflicts_with = "dataset", required_unless_present = "dataset")]
        input: Option<PathBuf>,

        /// Downloaded benchmark dataset id
        #[arg(long)]
        dataset: Option<String>,

        /// Reference tree (Newick) whose leaf names are compared with the taxa
        #[arg(long)]
        reference: Option<PathBuf>,

        /// Downloaded dataset whose cleaned sequences are compared with these
        #[arg(long)]
        compare_with: Option<String>,

        /// For a folder: one taxon per file (records joined) or one per record
        #[arg(long, value_enum, default_value_t = data_cmd::Records::PerFile)]
        records: data_cmd::Records,

        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Build the MAW matrices (M_full and M_ml) of a folder or downloaded dataset
    Matrix {
        /// Folder of sequence files, or one multi-FASTA file
        #[arg(long, conflicts_with = "dataset", required_unless_present = "dataset")]
        input: Option<PathBuf>,

        /// Downloaded benchmark dataset id
        #[arg(long)]
        dataset: Option<String>,

        /// Folder for the output files
        #[arg(long)]
        output: PathBuf,

        /// Do not apply the strand filter
        #[arg(long)]
        no_strand: bool,

        /// Fixed MAW lengths, for example 7,8,9, instead of the entropy selection
        #[arg(long, value_delimiter = ',')]
        lengths: Option<Vec<usize>>,

        /// For a folder: one taxon per file (records joined) or one per record
        #[arg(long, value_enum, default_value_t = data_cmd::Records::PerFile)]
        records: data_cmd::Records,

        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Print the hand-calculable teaching worksheet
    Teach {
        /// Use the built-in example (taxa K, L, M, N, P)
        #[arg(long, conflicts_with = "input", required_unless_present = "input")]
        example: bool,

        /// A small folder of your own sequences (at most 8 taxa, 200 letters each)
        #[arg(long)]
        input: Option<PathBuf>,

        /// Reference tree (Newick) for the nRF step
        #[arg(long, requires = "input")]
        reference: Option<PathBuf>,
    },

    /// Run the long-branch simulation of hypothesis H3
    SimulateH3 {
        /// Folder for recovery.csv, replicates.csv, recovery.svg and evaluation.txt
        #[arg(long, default_value = "results/h3")]
        output: PathBuf,

        /// Global seed; each replicate's seed is derived from it
        #[arg(long, default_value_t = 1)]
        seed: u64,

        /// Replicates per setting
        #[arg(long, default_value_t = qmaws_core::sim::REPLICATES)]
        replicates: u32,

        /// W2c resamples per replicate (supplementary result)
        #[arg(long, default_value_t = qmaws_core::weight::REPLICATES)]
        w2c_replicates: u32,
    },

    /// Development: export weighted-quartet inputs for the wQFM jar comparison
    #[command(hide = true)]
    WqfmExport {
        /// Folder for the .wqrts files and inputs.tsv
        #[arg(long)]
        output: PathBuf,
        /// Finished analysis runs whose W2c quartets are exported too
        #[arg(long)]
        run: Vec<PathBuf>,
        /// Seed of the simulated trees and weights
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },

    /// Development: compare wQFM-rs with the trees of the wQFM jar
    #[command(hide = true)]
    WqfmCompare {
        /// The folder written by wqfm-export, with the jar's .jar.tre files
        #[arg(long)]
        dir: PathBuf,
    },

    /// Development: run the positive and negative controls into results/controls
    #[command(hide = true)]
    Controls {
        /// Output folder
        #[arg(long, default_value = "results/controls")]
        output: PathBuf,

        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,

        /// Seed of the simulation, the shuffles, the runs and the random trees
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },

    /// Development: draw the figures for the taxon names of a dataset on a random tree (readability check)
    #[command(hide = true)]
    FigureCheck {
        /// Downloaded dataset id
        #[arg(long)]
        dataset: String,
        /// Output folder
        #[arg(long)]
        output: PathBuf,
        /// Seed of the random tree and values
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Data folder [default: data]
        #[arg(long, default_value = qmaws_data::DEFAULT_DATA_DIR)]
        data_dir: PathBuf,
    },

    /// Development: golden test G9, the tree metrics against independent oracles (nRF values for DendroPy)
    #[command(hide = true)]
    MetricsCheck {
        /// Folder for pairs.tsv
        #[arg(long)]
        output: PathBuf,
        /// Number of tree pairs
        #[arg(long, default_value_t = 500)]
        pairs: usize,
        /// Seed of the random trees
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },

    /// Development: comparison tables of the benchmark (milestone M11)
    #[command(hide = true)]
    Summary {
        /// Folder of the runs
        #[arg(long, default_value = "results/runs")]
        runs: PathBuf,
        /// Folder of the published values
        #[arg(long, default_value = "baselines/published")]
        baselines: PathBuf,
        /// Folder for the tables
        #[arg(long, default_value = "results/summary")]
        output: PathBuf,
        /// H1: Q-MAWS value of a pair (OI-18); H1 is skipped without it
        #[arg(long, value_enum, requires = "h1_zeros")]
        h1_pairs: Option<summary_cmd::H1Pairs>,
        /// H1: treatment of zero differences (OI-18)
        #[arg(long, value_enum, requires = "h1_pairs")]
        h1_zeros: Option<summary_cmd::H1Zeros>,
        /// Also write the AFproject upload files (seed-1 trees) to this folder
        #[arg(long)]
        afproject: Option<PathBuf>,
    },

    /// Development: time S2 bootstrap replicates of a finished run (decision D7)
    #[command(hide = true)]
    S2Cost {
        /// Finished run folder
        #[arg(long)]
        run: PathBuf,

        /// Number of replicates to run
        #[arg(long, default_value_t = 2)]
        replicates: u64,

        /// Use W2b weights instead of W2c in the replicates
        #[arg(long)]
        w2b: bool,
    },

    /// Development: export quartets of a run for the IQ-TREE cross-check
    #[command(hide = true)]
    IqtreeExport {
        /// Finished run folder (its matrix stage must be done)
        #[arg(long)]
        run: PathBuf,
        /// Folder for the alignments, trees and expected values
        #[arg(long)]
        output: PathBuf,
        /// Number of quartets
        #[arg(long, default_value_t = 5)]
        quartets: usize,
        /// Seed of the quartet draw
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },

    /// Development: compare IQ-TREE's results with Q-MAWS
    #[command(hide = true)]
    IqtreeCompare {
        /// The run folder given to iqtree-export
        #[arg(long)]
        run: PathBuf,
        /// The folder written by iqtree-export, with IQ-TREE's outputs
        #[arg(long)]
        dir: PathBuf,
    },
}

mod controls_cmd;
mod data_cmd;
mod figure_check_cmd;
mod figures_cmd;
mod h3_cmd;
mod iqtree_cmd;
mod matrix_cmd;
mod menu_cmd;
mod metrics_check_cmd;
mod s2_cmd;
mod summary_cmd;
mod teach_cmd;
mod verify_cmd;
mod wqfm_cmd;

/// Installs the Ctrl+C handler: the first press asks the engine to stop after
/// the current unit; the second exits immediately (safe because every output
/// is written atomically).
fn install_interrupt_handler(cancel: Arc<AtomicBool>) {
    let presses = AtomicU32::new(0);
    let result = ctrlc::set_handler(move || {
        if presses.fetch_add(1, Ordering::SeqCst) == 0 {
            cancel.store(true, Ordering::SeqCst);
            eprintln!("\nStopping after the current step. Press Ctrl+C again to exit immediately.");
        } else {
            eprintln!("\nExiting now. The run can be resumed.");
            std::process::exit(130);
        }
    });
    if let Err(e) = result {
        eprintln!("Warning: Ctrl+C handling is not available: {e}");
    }
}

pub(crate) fn report(outcome: Outcome, run_dir: &Path, mode: DisplayMode) -> ExitCode {
    match outcome {
        Outcome::Finished { root } => {
            if mode != DisplayMode::Json {
                println!("Run finished: {}", run_dir.display());
                println!("Root fingerprint: {root}");
            }
            ExitCode::SUCCESS
        }
        Outcome::Stopped => {
            if mode != DisplayMode::Json {
                println!("Run stopped: {}", run_dir.display());
                println!(
                    "Resume it with: qmaws resume --output \"{}\"",
                    run_dir.display()
                );
            }
            ExitCode::from(EXIT_STOPPED)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mode = if cli.json_progress {
        DisplayMode::Json
    } else if cli.quiet {
        DisplayMode::Quiet
    } else {
        DisplayMode::Normal
    };

    let Some(command) = cli.command else {
        use clap::CommandFactory;
        let _ = Cli::command().print_help();
        println!();
        return ExitCode::SUCCESS;
    };

    // Data commands need no Ctrl+C handling: an interrupted download leaves
    // a .part file that the next download continues.
    let color = !cli.no_color;
    let command = match command {
        Command::Datasets { data_dir } => return data_cmd::datasets(&data_dir),
        Command::SimulateH3 {
            output,
            seed,
            replicates,
            w2c_replicates,
        } => {
            return h3_cmd::run(h3_cmd::H3Args {
                output: &output,
                seed,
                replicates,
                w2c_replicates,
                quiet: mode != DisplayMode::Normal,
            })
        }
        Command::IqtreeExport {
            run,
            output,
            quartets,
            seed,
        } => return iqtree_cmd::export(&run, &output, quartets, seed),
        Command::IqtreeCompare { run, dir } => return iqtree_cmd::compare(&run, &dir),
        Command::WqfmExport { output, run, seed } => return wqfm_cmd::export(&output, &run, seed),
        Command::WqfmCompare { dir } => return wqfm_cmd::compare(&dir),
        Command::S2Cost {
            run,
            replicates,
            w2b,
        } => return s2_cmd::run(&run, replicates, !w2b),
        Command::Summary {
            runs,
            baselines,
            output,
            h1_pairs,
            h1_zeros,
            afproject,
        } => {
            return summary_cmd::run(
                &runs,
                &baselines,
                &output,
                h1_pairs.zip(h1_zeros),
                afproject.as_deref(),
            )
        }
        Command::Teach {
            example,
            input,
            reference,
        } => return teach_cmd::run(example, input.as_deref(), reference.as_deref()),
        Command::Matrix {
            input,
            dataset,
            output,
            no_strand,
            lengths,
            records,
            data_dir,
        } => {
            return matrix_cmd::run(matrix_cmd::MatrixArgs {
                input: input.as_deref(),
                dataset: dataset.as_deref(),
                output: &output,
                no_strand,
                lengths,
                records,
                data_dir: &data_dir,
                quiet: mode != DisplayMode::Normal,
            })
        }
        Command::Download { dataset, data_dir } => {
            return data_cmd::download(&dataset, &data_dir, mode, color)
        }
        Command::Gui { data_dir } => return menu_cmd::gui(Vec::new(), false, &data_dir),
        Command::MetricsCheck {
            output,
            pairs,
            seed,
        } => return metrics_check_cmd::run(&output, pairs, seed),
        Command::FigureCheck {
            dataset,
            output,
            seed,
            data_dir,
        } => return figure_check_cmd::run(&dataset, &data_dir, &output, seed),
        Command::Figures {
            output,
            reference,
            groups,
            otl,
            s2,
            data_dir,
        } => {
            return figures_cmd::run(figures_cmd::FiguresArgs {
                output: &output,
                reference: reference.as_deref(),
                groups: groups.as_deref(),
                otl,
                s2,
                data_dir: &data_dir,
                quiet: mode != DisplayMode::Normal,
            })
        }
        // The menu makes its own progress display for each run, so that no
        // empty bars are drawn above the menu.
        Command::Menu { data_dir } => {
            let cancel = Arc::new(AtomicBool::new(false));
            install_interrupt_handler(Arc::clone(&cancel));
            return menu_cmd::menu(&data_dir, &cancel, mode, color);
        }
        Command::Inspect {
            input,
            dataset,
            reference,
            compare_with,
            records,
            data_dir,
        } => {
            return data_cmd::inspect(
                input.as_deref(),
                dataset.as_deref(),
                reference.as_deref(),
                compare_with.as_deref(),
                records,
                &data_dir,
            )
        }
        other => other,
    };

    let display = TerminalDisplay::new(mode, color);
    let cancel = Arc::new(AtomicBool::new(false));
    install_interrupt_handler(Arc::clone(&cancel));

    let command = match command {
        Command::Controls {
            output,
            data_dir,
            seed,
        } => return controls_cmd::run(&output, &data_dir, seed, &cancel),
        Command::Verify {
            output,
            inputs,
            quick: _,
            full,
            quartet,
            seed,
            input,
        } => {
            return verify_cmd::run(
                verify_cmd::VerifyArgs {
                    output: &output,
                    inputs,
                    full,
                    quartet,
                    seed,
                    input: input.as_deref(),
                    quiet: mode != DisplayMode::Normal,
                },
                &cancel,
            )
        }
        other => other,
    };

    // The data folder, for the figures after an analysis run.
    let mut figures_data: Option<PathBuf> = None;
    let (run_dir, result) = match command {
        Command::Run {
            input,
            dataset,
            output,
            no_strand,
            lengths,
            seed,
            chunk_seconds,
            chunk_quartets,
            weighting,
            replicates,
            bootstrap,
            no_live_tree,
            gui,
            terminal: _,
            records,
            data_dir,
            reference,
            rename_duplicates,
            skip,
            keep,
            cores,
            memory_limit,
        } => {
            let input_choices = qmaws_engine::analysis::InputChoices {
                rename_duplicates,
                skip,
                keep,
            };
            let memory_limit = match memory_limit {
                Some(gb) if gb.is_finite() && gb > 0.0 => Some((gb * 1e9) as u64),
                Some(gb) => {
                    eprintln!("Error: --memory-limit must be a positive number of GB, not {gb}");
                    return ExitCode::from(EXIT_USAGE);
                }
                None => None,
            };
            if cores == Some(0) {
                eprintln!("Error: --cores must be at least 1");
                return ExitCode::from(EXIT_USAGE);
            }
            let reg = qmaws_data::registry::Registry::builtin();
            if dataset.as_deref() == Some("all") {
                if output.is_some() || reference.is_some() {
                    eprintln!("Error: with --dataset all, every dataset gets its own folder and its own reference tree; --output and --reference cannot be used");
                    return ExitCode::from(EXIT_USAGE);
                }
                let mut runs = Vec::new();
                for ds in &reg.datasets {
                    if let Err(code) = data_cmd::ensure_downloaded(&ds.id, &data_dir, mode) {
                        return code;
                    }
                    runs.push(qmaws_engine::launch::NewRun {
                        input: qmaws_data::DataDir::new(&data_dir).dataset_path(&reg, ds),
                        records: data_cmd::records_name(ds).to_string(),
                        output: new_run_dir(
                            Path::new(DEFAULT_RUNS_ROOT),
                            &run_id(&ds.id, UtcDateTime::now()),
                        ),
                        settings: qmaws_engine::launch::Settings {
                            weighting: weighting.config_name().to_string(),
                            lengths: lengths.clone(),
                            strand: !no_strand,
                            live_tree: !no_live_tree,
                            seed,
                            cores,
                            memory_limit,
                            replicates,
                            bootstrap,
                        },
                        reference: None,
                        input_choices: input_choices.clone(),
                    });
                }
                if mode != DisplayMode::Json {
                    println!(
                        "Queue: {} datasets, one after another (resume an interrupted queue with: qmaws resume --all).",
                        runs.len()
                    );
                }
                return menu_cmd::run_new(&runs, gui, &display, &cancel, mode, &data_dir);
            }
            let (path, records, name) = match (input, dataset) {
                (Some(p), _) => {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "run".into());
                    let r = match records {
                        data_cmd::Records::PerFile => "per_file",
                        data_cmd::Records::PerRecord => "per_record",
                    };
                    (p, r, name)
                }
                (None, Some(id)) => {
                    let Some(ds) = reg.dataset(&id) else {
                        eprintln!(
                            "Error: unknown dataset {id}; run 'qmaws datasets' to see the list (or use --dataset all)"
                        );
                        return ExitCode::from(EXIT_USAGE);
                    };
                    if let Err(code) = data_cmd::ensure_downloaded(&ds.id, &data_dir, mode) {
                        return code;
                    }
                    (
                        qmaws_data::DataDir::new(&data_dir).dataset_path(&reg, ds),
                        data_cmd::records_name(ds),
                        id,
                    )
                }
                (None, None) => unreachable!("clap requires --input or --dataset"),
            };
            let absolute = std::path::absolute(&path).unwrap_or(path);
            let run_dir = output.unwrap_or_else(|| {
                new_run_dir(
                    Path::new(DEFAULT_RUNS_ROOT),
                    &run_id(&name, UtcDateTime::now()),
                )
            });
            // A reference tree of the user: checked against the taxa, then
            // stored in the run folder (plan 5.4).
            if let Some(path) = &reference {
                let r = match qmaws_engine::launch::RunReference::from_file(path) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "Error: the reference tree {} cannot be used: {e}",
                            path.display()
                        );
                        return ExitCode::from(EXIT_USAGE);
                    }
                };
                if let Err(e) =
                    data_cmd::check_reference_names(&absolute, records, &input_choices, &r)
                {
                    eprintln!("Error: {e}");
                    return ExitCode::from(EXIT_USAGE);
                }
                if qmaws_engine::analysis::run_exists(&run_dir) {
                    eprintln!("Error: {} already holds a run; resume it with qmaws resume, or choose another --output", run_dir.display());
                    return ExitCode::from(EXIT_USAGE);
                }
                if let Err(e) = qmaws_engine::evaluation::store_reference(
                    &run_dir,
                    r.newick.as_bytes(),
                    &r.label,
                    &r.source,
                    None,
                ) {
                    eprintln!("Error: {e}");
                    return ExitCode::from(EXIT_ERROR);
                }
            }
            let options = qmaws_engine::analysis::AnalysisOptions {
                config: qmaws_engine::analysis::AnalysisConfig {
                    input: absolute.display().to_string(),
                    records: records.to_string(),
                    strand: !no_strand,
                    lengths,
                    seed,
                    ml_max_columns: qmaws_core::matrix::MAX_ML_COLUMNS,
                    weighting: weighting.config_name().to_string(),
                    replicates,
                    bootstrap,
                    input_choices: input_choices.clone(),
                },
                chunk_seconds,
                chunk_quartets,
                memory_limit,
                live_tree: !no_live_tree,
                cores,
            };
            let mut config = UserConfig::load();
            if config.remember_run(&run_dir) {
                let _ = config.save();
            }
            if gui {
                let job = qmaws_gui::Job::Start {
                    dir: run_dir,
                    options,
                };
                return menu_cmd::gui(vec![job], false, &data_dir);
            }
            let result =
                qmaws_engine::analysis::start(&run_dir, &options, "terminal", &display, &cancel);
            figures_data = Some(data_dir);
            (run_dir, result)
        }
        Command::ToyRun {
            output,
            blocks,
            seed,
            chunk_seconds,
            chunk_blocks,
        } => {
            let run_dir = output.unwrap_or_else(|| {
                new_run_dir(
                    Path::new(DEFAULT_RUNS_ROOT),
                    &run_id("toy", UtcDateTime::now()),
                )
            });
            let options = ToyOptions {
                seed,
                blocks,
                chunk_seconds,
                chunk_blocks,
            };
            let result = start_toy_run(&run_dir, &options, "terminal", &display, &cancel);
            (run_dir, result)
        }
        Command::Resume {
            output,
            all,
            gui,
            terminal: _,
            data_dir,
        } => {
            let (dirs, queue) = if all {
                let dirs = menu_cmd::queue_for_all();
                if dirs.is_empty() {
                    println!("There are no unfinished runs.");
                    return ExitCode::SUCCESS;
                }
                (dirs, true)
            } else {
                match output {
                    Some(dir) => (vec![dir], false),
                    None => match choose_unfinished_run() {
                        Ok(dir) => (vec![dir], false),
                        Err(code) => return code,
                    },
                }
            };
            if gui {
                let jobs = dirs
                    .into_iter()
                    .map(|dir| qmaws_gui::Job::Resume { dir })
                    .collect();
                return menu_cmd::gui(jobs, queue, &data_dir);
            }
            return menu_cmd::resume_terminal(&dirs, queue, &display, &cancel, mode, &data_dir);
        }
        Command::Datasets { .. }
        | Command::Download { .. }
        | Command::Inspect { .. }
        | Command::Matrix { .. }
        | Command::Teach { .. }
        | Command::SimulateH3 { .. }
        | Command::IqtreeExport { .. }
        | Command::IqtreeCompare { .. }
        | Command::WqfmExport { .. }
        | Command::WqfmCompare { .. }
        | Command::S2Cost { .. }
        | Command::Summary { .. }
        | Command::Controls { .. }
        | Command::Menu { .. }
        | Command::Gui { .. }
        | Command::Figures { .. }
        | Command::FigureCheck { .. }
        | Command::MetricsCheck { .. }
        | Command::Verify { .. } => {
            unreachable!("data commands return above")
        }
    };

    match result {
        Ok(outcome) => {
            let finished = matches!(outcome, Outcome::Finished { .. });
            let code = report(outcome, &run_dir, mode);
            if let (true, Some(data_dir)) = (finished, figures_data) {
                figures_cmd::after_run(&run_dir, &data_dir, mode != DisplayMode::Normal);
            }
            code
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// The run to resume when no folder was given: the only unfinished run in
/// the results folders (`results/runs` and the remembered ones).
fn choose_unfinished_run() -> Result<PathBuf, ExitCode> {
    let runs = launch::unfinished(&UserConfig::load().roots());
    match runs.len() {
        0 => {
            println!("There are no unfinished runs.");
            Err(ExitCode::SUCCESS)
        }
        1 => Ok(runs.into_iter().next().expect("one run").dir),
        _ => {
            eprintln!(
                "There are several unfinished runs. Choose one with --output, or resume all with --all:"
            );
            for r in &runs {
                eprintln!("  {}  ({})", r.dir.display(), r.line());
            }
            Err(ExitCode::from(EXIT_USAGE))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_definition_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn toy_run_defaults() {
        let cli = Cli::try_parse_from(["qmaws", "toy-run"]).unwrap();
        match cli.command {
            Some(Command::ToyRun {
                blocks,
                seed,
                chunk_seconds,
                chunk_blocks,
                output,
            }) => {
                assert_eq!(blocks, 20_000);
                assert_eq!(seed, 1);
                assert_eq!(chunk_seconds, 3.0);
                assert_eq!(chunk_blocks, None);
                assert_eq!(output, None);
            }
            other => panic!("unexpected parse: {other:?}"),
        }
    }

    #[test]
    fn global_flags_work_after_the_command() {
        let cli = Cli::try_parse_from(["qmaws", "resume", "--quiet", "--no-color"]).unwrap();
        assert!(cli.quiet && cli.no_color && !cli.json_progress);
    }

    #[test]
    fn quiet_and_json_progress_conflict() {
        assert!(Cli::try_parse_from(["qmaws", "--quiet", "--json-progress", "resume"]).is_err());
    }

    #[test]
    fn interface_flags_parse_and_conflict() {
        let cli = Cli::try_parse_from(["qmaws", "run", "--input", "x", "--gui"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Run { gui: true, .. })));
        assert!(
            Cli::try_parse_from(["qmaws", "run", "--input", "x", "--gui", "--terminal"]).is_err()
        );
        let cli = Cli::try_parse_from(["qmaws", "resume", "--all", "--terminal"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Resume {
                all: true,
                gui: false,
                ..
            })
        ));
        assert!(Cli::try_parse_from(["qmaws", "resume", "--all", "--output", "x"]).is_err());
        assert!(Cli::try_parse_from(["qmaws", "verify", "--output", "x", "--quick"]).is_ok());
        assert!(
            Cli::try_parse_from(["qmaws", "verify", "--output", "x", "--quick", "--full"]).is_err()
        );
        assert!(Cli::try_parse_from(["qmaws", "menu"]).is_ok());
        assert!(Cli::try_parse_from(["qmaws", "gui"]).is_ok());
    }

    #[test]
    fn unknown_commands_are_rejected() {
        assert!(Cli::try_parse_from(["qmaws", "frobnicate"]).is_err());
    }
}

/// Quartet weighting of `qmaws run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Weighting {
    /// Conditioned likelihood under the two-state symmetric model
    W2Sym,
    /// Conditioned likelihood with the frequencies of 0 and 1 of the full matrix
    W2Emp,
    /// No weighting (pattern counts only)
    None,
}

impl Weighting {
    fn config_name(self) -> &'static str {
        match self {
            Self::W2Sym => qmaws_engine::analysis::WEIGHTING_SYM,
            Self::W2Emp => qmaws_engine::analysis::WEIGHTING_EMP,
            Self::None => qmaws_engine::analysis::WEIGHTING_NONE,
        }
    }
}
