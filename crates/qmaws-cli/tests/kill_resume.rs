//! Acceptance tests for milestone M1, run against the real `qmaws` binary.
//!
//! - A toy run killed at random moments (hard kill, no clean shutdown) and
//!   resumed each time finishes with the same root fingerprint as an
//!   uninterrupted run.
//! - Progress output contains time estimates that are updated during the run.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_qmaws");

/// Size of the toy run: large enough that the run lasts several seconds, so
/// that every kill lands while it is working (release builds are faster).
const TOY_BLOCKS: u64 = if cfg!(debug_assertions) { 240 } else { 3000 };

/// Number of hard kills in the interrupted run.
const KILLS: u32 = 10;

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

/// Small deterministic generator for the kill times (64-bit LCG, Knuth MMIX).
struct Lcg(u64);

impl Lcg {
    fn next_in(&mut self, low: u64, high: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        low + (self.0 >> 33) % (high - low)
    }
}

fn toy_args(dir: &Path, chunk_blocks: u64) -> Vec<String> {
    vec![
        "--quiet".into(),
        "toy-run".into(),
        "--blocks".into(),
        TOY_BLOCKS.to_string(),
        "--seed".into(),
        "11".into(),
        "--chunk-blocks".into(),
        chunk_blocks.to_string(),
        "--output".into(),
        dir.display().to_string(),
    ]
}

fn resume_args(dir: &Path) -> Vec<String> {
    vec![
        "--quiet".into(),
        "resume".into(),
        "--output".into(),
        dir.display().to_string(),
    ]
}

fn read_root(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("audit").join("root.txt"))
        .expect("root.txt exists after a finished run")
        .trim()
        .to_string()
}

fn run_to_end(args: &[String]) {
    let status = Command::new(BIN)
        .args(args)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "qmaws {args:?} failed: {status}");
}

/// Arguments that continue the run in `dir`: resume if a valid run.json
/// exists, otherwise start it (the process may have been killed before the
/// run was first recorded).
fn continue_args(dir: &Path) -> Vec<String> {
    let has_state = dir.join("run.json").exists();
    if has_state {
        resume_args(dir)
    } else {
        toy_args(dir, 3)
    }
}

#[test]
fn killed_and_resumed_run_gives_the_same_root() {
    let tmp = TempDir::new("kill");

    let reference = tmp.0.join("reference");
    run_to_end(&toy_args(&reference, 7));
    let expected = read_root(&reference);

    let dir = tmp.0.join("killed");
    let seed = 20_261_002;
    let mut rng = Lcg(seed);
    let mut kills = 0;
    let mut finished_early = 0;
    let deadline = Instant::now() + Duration::from_secs(600);
    while kills < KILLS {
        assert!(Instant::now() < deadline, "test took too long");
        let mut child = Command::new(BIN)
            .args(continue_args(&dir))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        sleep(Duration::from_millis(rng.next_in(20, 700)));
        match child.try_wait().unwrap() {
            Some(status) => {
                // The run ended before the kill; it must have finished cleanly.
                assert!(status.success(), "run failed before the kill: {status}");
                finished_early += 1;
                break;
            }
            None => {
                child.kill().unwrap();
                child.wait().unwrap();
                kills += 1;
            }
        }
    }
    // Finish the run without interruption.
    run_to_end(&continue_args(&dir));
    assert_eq!(
        read_root(&dir),
        expected,
        "root differs after {kills} kills (kill-time seed {seed}, finished early: {finished_early})"
    );
    eprintln!("kills: {kills}, finished before a kill: {finished_early}, kill-time seed {seed}");
    assert!(
        kills >= 5,
        "only {kills} kills happened; the run is too short"
    );
}

#[test]
fn progress_reports_updated_time_estimates() {
    let tmp = TempDir::new("eta");
    let dir = tmp.0.join("run");
    let output = Command::new(BIN)
        .args([
            "--json-progress",
            "toy-run",
            "--blocks",
            "60",
            "--chunk-blocks",
            "2",
            "--output",
            &dir.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("each line is JSON"))
        .collect();

    let progress: Vec<&serde_json::Value> =
        events.iter().filter(|e| e["event"] == "progress").collect();
    assert!(progress.len() >= 30, "too few progress events");
    let remaining: Vec<f64> = progress
        .iter()
        .filter_map(|e| e["remaining_seconds"].as_f64())
        .collect();
    assert_eq!(
        remaining.len(),
        progress.len(),
        "every progress event has a remaining-time estimate"
    );
    let distinct: std::collections::BTreeSet<u64> = remaining.iter().map(|r| r.to_bits()).collect();
    assert!(
        distinct.len() > 10,
        "the estimate is updated during the run"
    );
    assert_eq!(remaining.last().copied(), Some(0.0));

    let elapsed: Vec<f64> = progress
        .iter()
        .map(|e| e["elapsed_seconds"].as_f64().unwrap())
        .collect();
    assert!(elapsed.windows(2).all(|w| w[1] >= w[0]));
    let overall: Vec<f64> = progress
        .iter()
        .map(|e| e["overall_fraction"].as_f64().unwrap())
        .collect();
    assert!(overall.windows(2).all(|w| w[1] >= w[0] - 1e-12));
    assert_eq!(overall.last().copied(), Some(1.0));

    assert_eq!(events.first().unwrap()["event"], "started");
    assert_eq!(events.last().unwrap()["event"], "finished");
}

/// Writes `m` related synthetic genomes of `len` letters into `dir`.
fn synthetic_inputs(dir: &Path, m: usize, len: usize) {
    std::fs::create_dir_all(dir).unwrap();
    let mut rng = Lcg(99);
    let base: Vec<u8> = (0..len)
        .map(|_| b"ACGT"[rng.next_in(0, 4) as usize])
        .collect();
    for t in 0..m {
        let mut s = base.clone();
        for _ in 0..len / 15 {
            let i = rng.next_in(0, len as u64) as usize;
            s[i] = b"ACGT"[rng.next_in(0, 4) as usize];
        }
        let text = format!(">S{t:02}\n{}\n", String::from_utf8(s).unwrap());
        std::fs::write(dir.join(format!("S{t:02}.fasta")), text).unwrap();
    }
}

fn analysis_args(input: &Path, out: &Path, chunk: u64) -> Vec<String> {
    vec![
        "--quiet".into(),
        "run".into(),
        "--input".into(),
        input.display().to_string(),
        "--chunk-quartets".into(),
        chunk.to_string(),
        "--output".into(),
        out.display().to_string(),
    ]
}

#[test]
fn killed_and_resumed_analysis_gives_the_same_root() {
    let tmp = TempDir::new("kill_analysis");
    let input = tmp.0.join("input");
    let (taxa, chunk) = if cfg!(debug_assertions) {
        (40, 20)
    } else {
        (70, 100)
    };
    synthetic_inputs(&input, taxa, 3000);

    let reference = tmp.0.join("reference");
    run_to_end(&analysis_args(&input, &reference, 997));
    let expected = read_root(&reference);

    let dir = tmp.0.join("killed");
    let mut rng = Lcg(424242);
    let mut kills = 0;
    let deadline = Instant::now() + Duration::from_secs(600);
    while kills < 6 {
        assert!(Instant::now() < deadline, "test took too long");
        let args = if dir.join("run.json").exists() {
            resume_args(&dir)
        } else {
            analysis_args(&input, &dir, chunk)
        };
        let mut child = Command::new(BIN)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        sleep(Duration::from_millis(rng.next_in(20, 250)));
        match child.try_wait().unwrap() {
            Some(status) => {
                assert!(status.success(), "run failed before the kill: {status}");
                break;
            }
            None => {
                child.kill().unwrap();
                child.wait().unwrap();
                kills += 1;
            }
        }
    }
    let finish = if dir.join("run.json").exists() {
        resume_args(&dir)
    } else {
        analysis_args(&input, &dir, chunk)
    };
    run_to_end(&finish);
    eprintln!("analysis kills: {kills}");
    assert_eq!(
        read_root(&dir),
        expected,
        "root differs after {kills} kills"
    );
    assert!(
        kills >= 3,
        "only {kills} kills happened; the run is too short"
    );
}
