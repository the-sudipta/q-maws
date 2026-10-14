//! `qmaws figures`, and the figures drawn after every finished analysis run.

use qmaws_engine::figures::{self, FigureOptions, ReferenceTree, SupportChoice};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub struct FiguresArgs<'a> {
    pub output: &'a Path,
    pub reference: Option<&'a Path>,
    pub groups: Option<&'a Path>,
    pub otl: bool,
    pub s2: bool,
    pub data_dir: &'a Path,
    pub quiet: bool,
}

/// `qmaws figures`: draws the figures of a finished run again.
pub fn run(args: FiguresArgs) -> ExitCode {
    let mut opts = figures::auto_options(args.output, args.data_dir);
    if let Some(path) = args.reference {
        match read_reference(path) {
            Ok(r) => opts.reference = Some(r),
            Err(e) => {
                eprintln!(
                    "Error: the reference tree {} cannot be used: {e}",
                    path.display()
                );
                return ExitCode::from(2);
            }
        }
    }
    if let Some(g) = args.groups {
        opts.groups_file = Some(PathBuf::from(g));
    }
    opts.otl = args.otl;
    if args.s2 {
        opts.support = SupportChoice::S2;
    }
    match draw(args.output, &opts, args.quiet) {
        true => ExitCode::SUCCESS,
        false => ExitCode::from(1),
    }
}

/// Reads a user's reference tree file (`--reference`).
pub fn read_reference(path: &Path) -> Result<ReferenceTree, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let source = std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string();
    ReferenceTree::parse(&text, &format!("reference tree {name}"), &source)
}

/// Draws the figures and prints what was written; false on an error.
pub fn draw(run_dir: &Path, opts: &FigureOptions, quiet: bool) -> bool {
    let mut log = |line: &str| {
        if !quiet {
            println!("{line}");
        }
    };
    match figures::render(run_dir, opts, &mut log) {
        Ok(w) => {
            for n in &w.notes {
                println!("Note: {n}");
            }
            if !quiet {
                println!("Figures: {}", run_dir.join("figures").display());
            }
            true
        }
        Err(e) => {
            eprintln!("Error: the figures could not be drawn: {e}");
            false
        }
    }
}

/// The figures after a finished run, with the options found from the run.
/// A problem is reported but does not fail the run.
pub fn after_run(run_dir: &Path, data_dir: &Path, quiet: bool) {
    let opts = figures::auto_options(run_dir, data_dir);
    if !draw(run_dir, &opts, quiet) {
        eprintln!(
            "The run itself is complete; draw the figures again with: qmaws figures --output \"{}\"",
            run_dir.display()
        );
    }
}
