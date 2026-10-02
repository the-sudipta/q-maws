//! The `qmaws` binary: argument parsing and dispatch to terminal or GUI mode.
//!
//! Status: skeleton. Only `--help` and `--version` are available. Commands are
//! added milestone by milestone.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
Q-MAWS: Quartet-based phylogeny from Minimal Absent Word Sets

Usage: qmaws [OPTIONS]

Options:
  -h, --help       Print this help
  -V, --version    Print the version

This is a development build. No analysis commands are available yet.";

/// What the program should do for a given argument list.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Help,
    Version,
    NotAvailable,
    Unknown(String),
}

fn parse(args: &[String]) -> Action {
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => Action::Help,
        Some("-V") | Some("--version") => Action::Version,
        Some(arg) if arg.starts_with('-') => Action::Unknown(arg.to_string()),
        Some(_) => Action::NotAvailable,
    }
}

/// Version line printed by `qmaws --version`, including the dependency check
/// that every workspace crate is linked into the binary.
fn version_line() -> String {
    let crates = [
        qmaws_core::VERSION,
        qmaws_engine::VERSION,
        qmaws_data::VERSION,
        qmaws_viz::VERSION,
        qmaws_tui::VERSION,
        qmaws_gui::VERSION,
    ];
    debug_assert!(crates.iter().all(|v| *v == VERSION));
    format!("qmaws {VERSION}")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Action::Help => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Action::Version => {
            println!("{}", version_line());
            ExitCode::SUCCESS
        }
        Action::NotAvailable => {
            eprintln!(
                "Error: the command '{}' is not available in this development build.",
                args[0]
            );
            eprintln!("Run 'qmaws --help' to see what is available.");
            ExitCode::from(2)
        }
        Action::Unknown(arg) => {
            eprintln!("Error: unknown option '{arg}'.");
            eprintln!("Run 'qmaws --help' to see what is available.");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_arguments_prints_help() {
        assert_eq!(parse(&args(&[])), Action::Help);
    }

    #[test]
    fn help_and_version_flags_are_recognised() {
        assert_eq!(parse(&args(&["--help"])), Action::Help);
        assert_eq!(parse(&args(&["-h"])), Action::Help);
        assert_eq!(parse(&args(&["--version"])), Action::Version);
        assert_eq!(parse(&args(&["-V"])), Action::Version);
    }

    #[test]
    fn unknown_option_is_reported() {
        assert_eq!(
            parse(&args(&["--frobnicate"])),
            Action::Unknown("--frobnicate".to_string())
        );
    }

    #[test]
    fn commands_are_not_available_yet() {
        assert_eq!(parse(&args(&["run"])), Action::NotAvailable);
    }

    #[test]
    fn version_line_uses_workspace_version() {
        assert_eq!(version_line(), format!("qmaws {VERSION}"));
    }
}
