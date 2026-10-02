//! Acceptance tests run against the real `qmaws` binary (from M1; the
//! verification command from M8).
//!
//! - A toy run killed at random moments (hard kill, no clean shutdown) and
//!   resumed each time finishes with the same root fingerprint as an
//!   uninterrupted run.
//! - Progress output contains time estimates that are updated during the run.
//! - `qmaws verify` passes on a finished run and reports a changed input.

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

fn analysis_args(input: &Path, out: &Path, chunk: u64, extra: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--quiet".into(),
        "run".into(),
        "--input".into(),
        input.display().to_string(),
        "--chunk-quartets".into(),
        chunk.to_string(),
        "--output".into(),
        out.display().to_string(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    args
}

/// True if the run in `dir` has a chunk plan for the weighting but no root
/// yet, so a kill at this moment interrupted the weighting.
fn inside_weighting(dir: &Path) -> bool {
    let planned = std::fs::read_to_string(dir.join("run.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .is_some_and(|v| v["chunk_plans"].get("quartet_weight").is_some());
    planned && !dir.join("audit").join("root.txt").exists()
}

/// Runs an analysis with hard kills at random moments, resumes it, and
/// checks the root against an uninterrupted run with another chunk size.
fn kill_and_compare(
    label: &str,
    taxa: usize,
    chunk: u64,
    extra: &[&str],
    kill_ms: (u64, u64),
    first_until_weighting: bool,
) -> u32 {
    let tmp = TempDir::new(label);
    let input = tmp.0.join("input");
    synthetic_inputs(&input, taxa, 3000);

    let reference = tmp.0.join("reference");
    run_to_end(&analysis_args(&input, &reference, 997, extra));
    let expected = read_root(&reference);

    let dir = tmp.0.join("killed");
    let mut rng = Lcg(424242);
    let mut kills = 0;
    let mut weighting_kills = 0;
    let deadline = Instant::now() + Duration::from_secs(600);
    let next_args = |dir: &Path| {
        if dir.join("run.json").exists() {
            resume_args(dir)
        } else {
            analysis_args(&input, dir, chunk, extra)
        }
    };
    if first_until_weighting {
        // The chunk size given on the command line is not stored; let the
        // first process fix the weighting plan with it, then kill it.
        let mut child = Command::new(BIN)
            .args(next_args(&dir))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        while !inside_weighting(&dir) {
            assert!(Instant::now() < deadline, "the weighting did not start");
            assert!(
                child.try_wait().unwrap().is_none(),
                "the run ended before the weighting"
            );
            sleep(Duration::from_millis(5));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        kills += 1;
        weighting_kills += 1;
    }
    while kills < 6 {
        assert!(Instant::now() < deadline, "test took too long");
        let mut child = Command::new(BIN)
            .args(next_args(&dir))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        sleep(Duration::from_millis(rng.next_in(kill_ms.0, kill_ms.1)));
        match child.try_wait().unwrap() {
            Some(status) => {
                assert!(status.success(), "run failed before the kill: {status}");
                break;
            }
            None => {
                child.kill().unwrap();
                child.wait().unwrap();
                kills += 1;
                if inside_weighting(&dir) {
                    weighting_kills += 1;
                }
            }
        }
    }
    run_to_end(&next_args(&dir));
    eprintln!("{label} kills: {kills} ({weighting_kills} during weighting)");
    assert_eq!(
        read_root(&dir),
        expected,
        "root differs after {kills} kills"
    );
    assert!(
        kills >= 3,
        "only {kills} kills happened; the run is too short"
    );
    weighting_kills
}

#[test]
fn killed_and_resumed_analysis_gives_the_same_root() {
    let (taxa, chunk) = if cfg!(debug_assertions) {
        (40, 20)
    } else {
        (70, 100)
    };
    let _ = kill_and_compare(
        "kill_analysis",
        taxa,
        chunk,
        &["--weighting", "none"],
        (20, 250),
        false,
    );
}

#[test]
fn killed_and_resumed_weighting_gives_the_same_root() {
    let taxa = if cfg!(debug_assertions) { 14 } else { 18 };
    let during = kill_and_compare(
        "kill_weighting",
        taxa,
        9,
        &["--replicates", "5"],
        (100, 600),
        true,
    );
    assert!(during >= 1, "no kill landed during the weighting");
}

#[test]
fn verify_command_passes_and_reports_a_changed_input() {
    let tmp = TempDir::new("verify_cli");
    let input = tmp.0.join("input");
    synthetic_inputs(&input, 7, 1500);
    let run = tmp.0.join("run");
    run_to_end(&analysis_args(&input, &run, 5, &["--replicates", "3"]));
    let verify = |extra: &[&str]| {
        let mut args = vec!["verify", "--output", run.to_str().unwrap()];
        args.extend_from_slice(extra);
        let out = Command::new(BIN).args(&args).output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    };
    let (code, text) = verify(&["--seed", "4"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("drawn with seed 4"), "{text}");
    assert!(text.contains("Verdict: PASS"), "{text}");
    let (code, text) = verify(&["--quartet", "S00,S02,S04,S06"]);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("[PASS] decision"), "{text}");
    let (code, _) = verify(&["--quartet", "S00,S02"]);
    assert_eq!(code, Some(2));
    let f = input.join("S03.fasta");
    let body = std::fs::read_to_string(&f).unwrap().replace('A', "C");
    std::fs::write(&f, body).unwrap();
    let (code, text) = verify(&["--inputs"]);
    assert_eq!(code, Some(1), "{text}");
    assert!(text.contains("[FAIL] file"), "{text}");
}
