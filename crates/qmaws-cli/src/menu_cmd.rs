//! `qmaws menu` and `qmaws gui`, and the resume list of `qmaws resume`:
//! the main menu of plan 5.2 asks in the terminal, then the run is carried
//! out in the interface chosen; runs can move between the interfaces at any
//! stop (plan 5.5).

use crate::{data_cmd, report, verify_cmd, EXIT_ERROR, EXIT_STOPPED};
use qmaws_engine::launch::{self, Interface, UserConfig};
use qmaws_engine::{resume_run, Outcome};
use qmaws_tui::menu::{self, MenuAction, MenuContext, TerminalPrompter};
use qmaws_tui::{DisplayMode, TerminalDisplay};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Opens the window with these jobs (none: the main menu).
pub fn gui(jobs: Vec<qmaws_gui::Job>, queue: bool, data_dir: &Path) -> ExitCode {
    let dir = data_dir.to_path_buf();
    let download: qmaws_gui::DownloadFn =
        Arc::new(move |id| data_cmd::download_for_menu(id, &dir, DisplayMode::Quiet));
    match qmaws_gui::run(qmaws_gui::Launch {
        jobs,
        queue,
        data_dir: data_dir.to_path_buf(),
        download,
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// The runs of `resume --all`: the saved queue if it still holds unfinished
/// runs (an interrupted queue continues), otherwise every unfinished run,
/// saved as the new queue.
pub fn queue_for_all() -> Vec<PathBuf> {
    let mut config = UserConfig::load();
    let saved = config.queued_unfinished();
    if !saved.is_empty() {
        return saved;
    }
    let dirs: Vec<PathBuf> = launch::unfinished(&config.roots())
        .into_iter()
        .map(|r| r.dir)
        .collect();
    config.set_queue(&dirs);
    let _ = config.save();
    dirs
}

/// How a list of runs in the terminal ended.
enum Ended {
    AllFinished,
    Stopped(ExitCode),
    Failed,
}

/// Resumes the runs one after another in the terminal; with `queue`, each
/// finished run leaves the saved queue.
pub fn resume_terminal(
    dirs: &[PathBuf],
    queue: bool,
    display: &TerminalDisplay,
    cancel: &AtomicBool,
    mode: DisplayMode,
) -> ExitCode {
    match resume_list(dirs, queue, display, cancel, mode) {
        Ended::AllFinished => ExitCode::SUCCESS,
        Ended::Stopped(code) => code,
        Ended::Failed => ExitCode::from(EXIT_ERROR),
    }
}

fn resume_list(
    dirs: &[PathBuf],
    queue: bool,
    display: &TerminalDisplay,
    cancel: &AtomicBool,
    mode: DisplayMode,
) -> Ended {
    for (i, dir) in dirs.iter().enumerate() {
        if queue && mode != DisplayMode::Json {
            println!("Queue: run {} of {}: {}", i + 1, dirs.len(), dir.display());
        }
        match resume_run(dir, Interface::Terminal.name(), display, cancel) {
            Ok(outcome @ Outcome::Finished { .. }) => {
                let _ = report(outcome, dir, mode);
                if queue {
                    let mut config = UserConfig::load();
                    if config.dequeue(dir) {
                        let _ = config.save();
                    }
                }
            }
            Ok(Outcome::Stopped) => {
                if queue && mode != DisplayMode::Json {
                    println!("The queue stops here; 'qmaws resume --all' continues it.");
                }
                return Ended::Stopped(report(Outcome::Stopped, dir, mode));
            }
            Err(e) => {
                eprintln!("Error: {e}");
                return Ended::Failed;
            }
        }
    }
    Ended::AllFinished
}

/// Starts new runs one after another in the terminal.
fn start_terminal(
    runs: &[launch::NewRun],
    display: &TerminalDisplay,
    cancel: &AtomicBool,
    mode: DisplayMode,
) -> Ended {
    for run in runs {
        match qmaws_engine::analysis::start(
            &run.output,
            &run.options(),
            Interface::Terminal.name(),
            display,
            cancel,
        ) {
            Ok(outcome @ Outcome::Finished { .. }) => {
                let _ = report(outcome, &run.output, mode);
            }
            Ok(Outcome::Stopped) => {
                return Ended::Stopped(report(Outcome::Stopped, &run.output, mode));
            }
            Err(e) => {
                eprintln!("Error: {e}");
                return Ended::Failed;
            }
        }
    }
    Ended::AllFinished
}

/// `qmaws menu`: the main menu until the user exits (or a run is stopped
/// with Ctrl+C).
pub fn menu(
    data_dir: &Path,
    display: &TerminalDisplay,
    cancel: &AtomicBool,
    mode: DisplayMode,
) -> ExitCode {
    let mut prompter = TerminalPrompter::new();
    loop {
        let dir = data_dir.to_path_buf();
        let mut download =
            move |id: &str| data_cmd::download_for_menu(id, &dir, DisplayMode::Normal);
        let mut ctx = MenuContext {
            data_dir: data_dir.to_path_buf(),
            config: UserConfig::load(),
            download: &mut download,
        };
        let action = menu::main_menu(&mut prompter, &mut ctx);
        let ended = match action {
            MenuAction::Exit => return ExitCode::SUCCESS,
            MenuAction::NewRuns { runs, interface } => {
                let mut config = UserConfig::load();
                let mut changed = false;
                for r in &runs {
                    changed |= config.remember_run(&r.output);
                }
                if changed {
                    let _ = config.save();
                }
                match interface {
                    Interface::Gui => {
                        let jobs = runs
                            .iter()
                            .map(|r| qmaws_gui::Job::Start {
                                dir: r.output.clone(),
                                options: r.options(),
                            })
                            .collect();
                        let _ = gui(jobs, false, data_dir);
                        Ended::AllFinished
                    }
                    Interface::Terminal => start_terminal(&runs, display, cancel, mode),
                }
            }
            MenuAction::Resume {
                dirs,
                interface,
                queue,
            } => {
                if queue {
                    let mut config = UserConfig::load();
                    config.set_queue(&dirs);
                    let _ = config.save();
                }
                match interface {
                    Interface::Gui => {
                        let jobs = dirs
                            .into_iter()
                            .map(|dir| qmaws_gui::Job::Resume { dir })
                            .collect();
                        let _ = gui(jobs, queue, data_dir);
                        Ended::AllFinished
                    }
                    Interface::Terminal => {
                        let mut ended = resume_list(&dirs, queue, display, cancel, mode);
                        // After a single resumed run: offer the next one.
                        while !queue && matches!(ended, Ended::AllFinished) {
                            let roots = UserConfig::load().roots();
                            let Some(next) = menu::ask_next_run(&mut prompter, &roots) else {
                                break;
                            };
                            ended = resume_list(&[next], false, display, cancel, mode);
                        }
                        ended
                    }
                }
            }
            MenuAction::Verify {
                dir,
                mode: m,
                input,
            } => {
                let _ = verify_cmd::run_mode(&dir, &m, input.as_deref(), false, cancel);
                Ended::AllFinished
            }
        };
        // A run stopped with Ctrl+C ends the menu; otherwise it shows again.
        if cancel.load(Ordering::SeqCst) {
            return match ended {
                Ended::Stopped(code) => code,
                _ => ExitCode::from(EXIT_STOPPED),
            };
        }
    }
}
