//! `qmaws verify`: checks a finished run against its audit record and
//! prints the verification report (also written to `report/`).

use qmaws_engine::verify::{self, Mode};
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

pub struct VerifyArgs<'a> {
    pub output: &'a Path,
    pub inputs: bool,
    pub full: bool,
    pub quartet: Option<Vec<String>>,
    pub seed: Option<u64>,
    pub input: Option<&'a Path>,
    pub quiet: bool,
}

pub fn run(args: VerifyArgs, cancel: &AtomicBool) -> ExitCode {
    let mode = if args.inputs {
        Mode::Inputs
    } else if args.full {
        Mode::Full
    } else if let Some(names) = args.quartet {
        match <[String; 4]>::try_from(names) {
            Ok(names) => Mode::Quartet(names),
            Err(_) => {
                eprintln!("Error: --quartet needs four taxon names separated by commas.");
                return ExitCode::from(2);
            }
        }
    } else {
        Mode::Quick(args.seed)
    };
    if !args.quiet {
        eprintln!(
            "Verifying {} ({})...",
            args.output.display(),
            match &mode {
                Mode::Inputs => "input check",
                Mode::Quick(_) => "quick check",
                Mode::Full => "full check: the whole run is recomputed",
                Mode::Quartet(_) => "single quartet",
            }
        );
    }
    match verify::verify(args.output, &mode, args.input, cancel) {
        Ok(report) => {
            if !args.quiet {
                for l in &report.lines {
                    println!("{l}");
                }
            } else {
                println!("{}", report.lines.last().cloned().unwrap_or_default());
            }
            println!("Report: {}", report.path.display());
            if report.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}
