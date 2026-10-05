//! Golden test G10 and interface switching (M9 acceptance), on the 16-taxon
//! simulated control input in `results/controls/inputs/simulated/`:
//!
//! - a run in the terminal (`qmaws run --terminal`) and a run through the
//!   GUI's controller (the engine as the window drives it, without a
//!   window) give the same root fingerprint;
//! - a run started in the terminal, stopped, and resumed in the GUI gives
//!   that root, and so does a run started in the GUI, stopped, and resumed
//!   in the terminal; `run.json` records the interface used last.
//!
//! The test that runs always uses fewer W2c resamples and bootstrap
//! replicates so that it stays short in debug builds; the ignored test
//! uses the default settings (run it with
//! `cargo test --release -p qmaws-cli --test interfaces -- --ignored`).

use qmaws_engine::analysis::{AnalysisConfig, AnalysisOptions};
use qmaws_engine::launch::RunSummary;
use qmaws_engine::{Event, Outcome};
use qmaws_gui::controller::{run_to_end, Controller, Job};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

const BIN: &str = env!("CARGO_BIN_EXE_qmaws");

/// Quartets per chunk in the interrupted runs, so that a stop lands inside
/// the weighting stage (16 taxa: 1,820 quartets).
const CHUNK: u64 = 100;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let p = std::env::temp_dir().join(format!("qmaws-it-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn input() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../results/controls/inputs/simulated/sequences");
    std::path::absolute(p).unwrap()
}

fn read_root(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("audit").join("root.txt"))
        .expect("root.txt exists after a finished run")
        .trim()
        .to_string()
}

fn last_interface(dir: &Path) -> String {
    RunSummary::read(dir).unwrap().state.last_interface
}

/// The settings of the run: W2c resamples and S2 replicates.
#[derive(Clone, Copy)]
struct Size {
    replicates: u32,
    bootstrap: u32,
}

/// `qmaws run --terminal` arguments; `chunk` fixes the chunk size.
fn terminal_args(dir: &Path, size: Size, chunk: Option<u64>, json: bool) -> Vec<String> {
    let mut a: Vec<String> = vec![if json { "--json-progress" } else { "--quiet" }.into()];
    a.extend(
        [
            "run",
            "--terminal",
            "--input",
            &input().display().to_string(),
            "--output",
            &dir.display().to_string(),
            "--replicates",
            &size.replicates.to_string(),
            "--bootstrap",
            &size.bootstrap.to_string(),
        ]
        .map(String::from),
    );
    if let Some(c) = chunk {
        a.extend(["--chunk-quartets".to_string(), c.to_string()]);
    }
    a
}

/// The same run as the GUI starts it.
fn gui_options(size: Size, chunk: Option<u64>) -> AnalysisOptions {
    AnalysisOptions {
        config: AnalysisConfig {
            input: input().display().to_string(),
            records: "per_file".into(),
            strand: true,
            lengths: None,
            seed: 1,
            ml_max_columns: qmaws_core::matrix::MAX_ML_COLUMNS,
            weighting: qmaws_engine::analysis::WEIGHTING_SYM.into(),
            replicates: size.replicates,
            bootstrap: size.bootstrap,
        },
        chunk_seconds: 3.0,
        chunk_quartets: chunk,
        memory_limit: None,
        live_tree: true,
        cores: None,
    }
}

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn quiet() -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(|| {})
}

fn run_terminal(args: &[String]) {
    let status = Command::new(BIN)
        .args(args)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "qmaws {args:?} failed: {status}");
}

/// True for a progress event inside the weighting stage after its first
/// chunk.
fn weighting_started(stage: &str, done: u64) -> bool {
    stage == qmaws_engine::analysis::QUARTET_WEIGHT && done > 0
}

fn single(results: Vec<(PathBuf, Result<Outcome, String>)>) -> Outcome {
    assert_eq!(results.len(), 1);
    results
        .into_iter()
        .next()
        .unwrap()
        .1
        .expect("the run works")
}

fn check_interfaces(size: Size) {
    let tmp = TempDir::new("g10");

    // 1. Terminal, uninterrupted.
    let terminal = tmp.0.join("terminal");
    run_terminal(&terminal_args(&terminal, size, None, false));
    let expected = read_root(&terminal);
    assert_eq!(last_interface(&terminal), "terminal");

    // 2. GUI, uninterrupted. A fixed chunk size gives the live tree chunk
    // boundaries to update at: with the calibrated size, a fast device
    // weighs all quartets in one chunk and no provisional tree is due. The
    // root does not depend on chunk boundaries (OI-4).
    let gui = tmp.0.join("gui");
    let c = Controller::spawn(
        vec![Job::Start {
            dir: gui.clone(),
            options: gui_options(size, Some(CHUNK)),
        }],
        false,
        data(),
        quiet(),
    );
    let mut provisional = 0;
    let mut worksheets = 0;
    let outcome = single(run_to_end(&c, |_, _, e| match e {
        Event::Provisional(_) => provisional += 1,
        Event::Quartet(_) => worksheets += 1,
        _ => {}
    }));
    drop(c);
    assert_eq!(
        outcome,
        Outcome::Finished {
            root: expected.clone()
        },
        "G10"
    );
    assert_eq!(read_root(&gui), expected);
    assert_eq!(last_interface(&gui), "gui");
    assert!(worksheets > 0, "the live worksheet receives quartets");
    assert!(provisional > 0, "the live tree is updated");

    // 3. Started in the terminal, stopped (hard kill inside the weighting
    // stage), resumed in the GUI.
    let t2g = tmp.0.join("terminal_to_gui");
    let mut child = Command::new(BIN)
        .args(terminal_args(&t2g, size, Some(CHUNK), true))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    for line in stdout.lines() {
        let Ok(line) = line else { break };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if v["event"] == "progress"
            && weighting_started(
                v["stage_name"].as_str().unwrap_or(""),
                v["stage_units_done"].as_u64().unwrap_or(0),
            )
        {
            break;
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let before = RunSummary::read(&t2g).expect("the run was recorded");
    assert!(!before.finished, "the run was stopped before the end");
    assert!(before.percent > 0.0 && before.percent < 100.0);
    let c = Controller::spawn(
        vec![Job::Resume { dir: t2g.clone() }],
        false,
        data(),
        quiet(),
    );
    let outcome = single(run_to_end(&c, |_, _, _| {}));
    drop(c);
    assert_eq!(
        outcome,
        Outcome::Finished {
            root: expected.clone()
        }
    );
    assert_eq!(last_interface(&t2g), "gui");

    // 4. Started in the GUI, stopped there, resumed in the terminal.
    let g2t = tmp.0.join("gui_to_terminal");
    let c = Controller::spawn(
        vec![Job::Start {
            dir: g2t.clone(),
            options: gui_options(size, Some(CHUNK)),
        }],
        false,
        data(),
        quiet(),
    );
    let outcome = single(run_to_end(&c, |c, _, e| {
        if let Event::Progress(s) = e {
            if weighting_started(&s.stage_name, s.stage_units_done) {
                c.stop();
            }
        }
    }));
    drop(c);
    assert_eq!(outcome, Outcome::Stopped);
    assert_eq!(last_interface(&g2t), "gui");
    run_terminal(&[
        "--quiet".into(),
        "resume".into(),
        "--terminal".into(),
        "--output".into(),
        g2t.display().to_string(),
    ]);
    assert_eq!(read_root(&g2t), expected);
    assert_eq!(last_interface(&g2t), "terminal");
    eprintln!("root of all four runs: {expected}");
}

#[test]
fn terminal_and_gui_give_the_same_root_and_runs_move_between_them() {
    check_interfaces(Size {
        replicates: 10,
        bootstrap: 3,
    });
}

/// G10 with the default settings (100 W2c resamples, 100 S2 replicates).
#[test]
#[ignore]
fn g10_default_settings() {
    check_interfaces(Size {
        replicates: qmaws_core::weight::REPLICATES,
        bootstrap: qmaws_engine::analysis::BOOTSTRAP_REPLICATES,
    });
}
