//! The `qmaws` binary: argument parsing and dispatch to terminal or GUI mode.
//!
//! Commands available in this development build: `toy-run` (a toy computation
//! that exercises checkpoints, resume and progress display) and `resume`.

use clap::{Parser, Subcommand};
use qmaws_engine::rundir::{find_unfinished_runs, new_run_dir, run_id, DEFAULT_RUNS_ROOT};
use qmaws_engine::{clock::UtcDateTime, resume_run, start_toy_run, Outcome, ToyOptions};
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
    after_help = "This is a development build. Analysis commands are added in later versions."
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
        /// Run folder to resume [default: the only unfinished run in results/runs]
        #[arg(long)]
        output: Option<PathBuf>,
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
}

mod data_cmd;
mod matrix_cmd;

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

fn report(outcome: Outcome, run_dir: &Path, mode: DisplayMode) -> ExitCode {
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

    let (run_dir, result) = match command {
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
        Command::Resume { output } => {
            let run_dir = match output {
                Some(dir) => dir,
                None => match choose_unfinished_run() {
                    Ok(dir) => dir,
                    Err(code) => return code,
                },
            };
            let result = resume_run(&run_dir, "terminal", &display, &cancel);
            (run_dir, result)
        }
        Command::Datasets { .. }
        | Command::Download { .. }
        | Command::Inspect { .. }
        | Command::Matrix { .. } => {
            unreachable!("data commands return above")
        }
    };

    match result {
        Ok(outcome) => report(outcome, &run_dir, mode),
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// The run to resume when no folder was given: the only unfinished run in the
/// default runs folder.
fn choose_unfinished_run() -> Result<PathBuf, ExitCode> {
    let runs = find_unfinished_runs(Path::new(DEFAULT_RUNS_ROOT));
    match runs.len() {
        0 => {
            println!("There are no unfinished runs in {DEFAULT_RUNS_ROOT}.");
            Err(ExitCode::SUCCESS)
        }
        1 => Ok(runs.into_iter().next().expect("one run").0),
        _ => {
            eprintln!("There are several unfinished runs. Choose one with --output:");
            for (dir, state) in &runs {
                eprintln!("  {}  (started {})", dir.display(), state.created_utc);
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
    fn unknown_commands_are_rejected() {
        assert!(Cli::try_parse_from(["qmaws", "frobnicate"]).is_err());
    }
}
