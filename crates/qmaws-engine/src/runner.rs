//! Running, stopping and resuming runs.
//!
//! A run is a sequence of stages recorded in `run.json`. Chunked stages write
//! one verified file per chunk; a chunk is done exactly when its file and hash
//! file exist and agree. Resuming therefore needs no other bookkeeping: verify
//! what exists, then continue with the first unfinished unit.
//!
//! In this milestone the only kind of run is the toy run (`toy` module).

use crate::atomic;
use crate::clock::UtcDateTime;
use crate::hash::{sha256_hex, StreamHasher};
use crate::progress::{Estimator, Event, ProgressSink, Snapshot};
use crate::rundir::RunDir;
use crate::state::{ChunkPlan, RunState, StageState, StageStatus, StateError, FORMAT_VERSION};
use crate::toy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const TOY_COUNT: &str = "toy_count";
const FINALIZE: &str = "finalize";
const TOY_STAGES: [&str; 2] = [TOY_COUNT, FINALIZE];

/// Settings of a toy run.
#[derive(Debug, Clone, PartialEq)]
pub struct ToyOptions {
    pub seed: u64,
    /// Number of blocks of `toy::BLOCK_SIZE` counter values.
    pub blocks: u64,
    /// Target duration of one chunk in seconds, used with the calibration.
    pub chunk_seconds: f64,
    /// Fixed number of blocks per chunk instead of calibration (for tests).
    pub chunk_blocks: Option<u64>,
}

/// The part of the toy settings that determines the result. Chunk sizes are
/// not part of it: the result does not depend on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToyConfig {
    pub seed: u64,
    pub blocks: u64,
    pub block_size: u64,
}

/// How a call to the engine ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// All stages are done; the root fingerprint of the run.
    Finished { root: String },
    /// Stopped on request; resumable.
    Stopped,
}

#[derive(Debug)]
pub enum EngineError {
    Io { path: PathBuf, error: io::Error },
    State { path: PathBuf, error: StateError },
    RunExists(PathBuf),
    InputChanged(String),
    Invalid(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Io { path, error } => {
                write!(f, "could not read or write {}: {error}", path.display())
            }
            EngineError::State { path, error } => {
                write!(f, "cannot use the run in {}: {error}", path.display())
            }
            EngineError::RunExists(path) => write!(
                f,
                "a run already exists in {}; resume it, or choose another output folder",
                path.display()
            ),
            EngineError::InputChanged(what) => write!(
                f,
                "an input file changed since the run started ({what}); the run cannot be resumed with different data"
            ),
            EngineError::Invalid(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for EngineError {}

pub(crate) fn io_err(path: &Path) -> impl FnOnce(io::Error) -> EngineError + '_ {
    move |error| EngineError::Io {
        path: path.to_path_buf(),
        error,
    }
}

/// Starts a new toy run in `run_dir`, which must not contain a run yet.
pub fn start_toy_run(
    run_dir: &Path,
    options: &ToyOptions,
    interface: &str,
    sink: &dyn ProgressSink,
    cancel: &AtomicBool,
) -> Result<Outcome, EngineError> {
    if options.blocks == 0 {
        return Err(EngineError::Invalid(
            "the number of blocks must be at least 1".into(),
        ));
    }
    if !(options.chunk_seconds.is_finite() && options.chunk_seconds > 0.0) {
        return Err(EngineError::Invalid(
            "the chunk duration must be a positive number of seconds".into(),
        ));
    }
    let dir = RunDir::new(run_dir);
    if dir.run_json().exists() {
        return Err(EngineError::RunExists(run_dir.to_path_buf()));
    }
    dir.create_layout().map_err(io_err(run_dir))?;

    let config = ToyConfig {
        seed: options.seed,
        blocks: options.blocks,
        block_size: toy::BLOCK_SIZE,
    };
    let config_value = serde_json::to_value(config).expect("toy configuration serialises");
    let config_sha256 = sha256_hex(config_value.to_string().as_bytes());

    let calibration = calibrate(options.seed);
    let units_per_chunk = options
        .chunk_blocks
        .unwrap_or_else(|| (calibration.blocks_per_second * options.chunk_seconds).round() as u64);
    let plan = ChunkPlan::new(options.blocks, units_per_chunk);
    let mut throughput = BTreeMap::new();
    throughput.insert(TOY_COUNT.to_string(), calibration.blocks_per_second);
    throughput.insert(
        FINALIZE.to_string(),
        calibration.hash_bytes_per_second / (8.0 * plan.units_per_chunk as f64),
    );

    let created = UtcDateTime::now();
    let run_id = run_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("toy_{}", created.run_id_part()));
    let state = RunState {
        format_version: FORMAT_VERSION,
        program_version: env!("CARGO_PKG_VERSION").to_string(),
        run_id,
        created_utc: created.iso8601(),
        kind: "toy".to_string(),
        config: config_value,
        config_sha256,
        inputs: Vec::new(),
        stages: TOY_STAGES.iter().map(|s| StageState::pending(s)).collect(),
        chunk_plans: BTreeMap::from([(TOY_COUNT.to_string(), plan)]),
        throughput,
        elapsed_seconds: calibration.seconds,
        last_interface: interface.to_string(),
    };
    state
        .save(&dir.run_json())
        .map_err(io_err(&dir.run_json()))?;

    let mut session = Session::new(dir, state, sink, cancel);
    session.emit_started(false);
    session.log(&format!(
        "Started toy run: {} blocks, seed {}, {} chunks of up to {} blocks (calibration: {:.0} blocks per second).",
        options.blocks,
        options.seed,
        plan.chunk_count(),
        plan.units_per_chunk,
        calibration.blocks_per_second
    ))?;
    session.execute()
}

/// Resumes the run stored in `run_dir`.
pub fn resume_run(
    run_dir: &Path,
    interface: &str,
    sink: &dyn ProgressSink,
    cancel: &AtomicBool,
) -> Result<Outcome, EngineError> {
    let dir = RunDir::new(run_dir);
    let mut state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    if state.kind == crate::analysis::KIND {
        return crate::analysis::resume(run_dir, state, interface, sink, cancel);
    }
    if state.kind != "toy" {
        return Err(EngineError::Invalid(format!(
            "runs of kind '{}' are not supported by this version",
            state.kind
        )));
    }
    verify_inputs(&state)?;
    state.last_interface = interface.to_string();
    let mut session = Session::new(dir, state, sink, cancel);
    session.emit_started(true);
    session.log("Resuming run.")?;
    session.execute()
}

/// Recomputes the fingerprint of every input file and compares it with the
/// fingerprint stored when the run started.
pub(crate) fn verify_inputs(state: &RunState) -> Result<(), EngineError> {
    for input in &state.inputs {
        let bytes = std::fs::read(&input.path).map_err(io_err(Path::new(&input.path)))?;
        if sha256_hex(&bytes) != input.sha256 {
            return Err(EngineError::InputChanged(input.path.clone()));
        }
    }
    Ok(())
}

struct Calibration {
    blocks_per_second: f64,
    hash_bytes_per_second: f64,
    seconds: f64,
}

/// Short measurement of this device's speed (well under 10 seconds).
fn calibrate(seed: u64) -> Calibration {
    let start = Instant::now();
    let mut blocks = 0u64;
    let mut sink = 0u64;
    while blocks == 0 || (start.elapsed().as_secs_f64() < 0.25 && blocks < 4096) {
        sink = sink.wrapping_add(toy::block_sum(seed, blocks));
        blocks += 1;
    }
    let block_seconds = start.elapsed().as_secs_f64().max(1e-9);
    std::hint::black_box(sink);

    let data = vec![0x5au8; 4 << 20];
    let hash_start = Instant::now();
    std::hint::black_box(sha256_hex(&data));
    let hash_seconds = hash_start.elapsed().as_secs_f64().max(1e-9);

    Calibration {
        blocks_per_second: blocks as f64 / block_seconds,
        hash_bytes_per_second: data.len() as f64 / hash_seconds,
        seconds: start.elapsed().as_secs_f64(),
    }
}

/// One process's work on a run: from start or resume until finished,
/// stopped or failed.
struct Session<'a> {
    dir: RunDir,
    state: RunState,
    sink: &'a dyn ProgressSink,
    cancel: &'a AtomicBool,
    started: Instant,
    elapsed_before: f64,
    estimator: Estimator,
}

impl<'a> Session<'a> {
    fn new(
        dir: RunDir,
        state: RunState,
        sink: &'a dyn ProgressSink,
        cancel: &'a AtomicBool,
    ) -> Self {
        let plan = state.chunk_plans[TOY_COUNT];
        let mut estimator = Estimator::new(&[
            (TOY_COUNT, plan.units_total),
            (FINALIZE, plan.chunk_count()),
        ]);
        for (stage, rate) in &state.throughput {
            estimator.set_rate(stage, *rate);
        }
        let elapsed_before = state.elapsed_seconds;
        Self {
            dir,
            state,
            sink,
            cancel,
            started: Instant::now(),
            elapsed_before,
            estimator,
        }
    }

    fn elapsed(&self) -> f64 {
        self.elapsed_before + self.started.elapsed().as_secs_f64()
    }

    fn config(&self) -> ToyConfig {
        serde_json::from_value(self.state.config.clone()).expect("toy configuration in run.json")
    }

    fn plan(&self) -> ChunkPlan {
        self.state.chunk_plans[TOY_COUNT]
    }

    fn save(&mut self) -> Result<(), EngineError> {
        self.state.elapsed_seconds = self.elapsed();
        self.state.throughput = self.estimator.rates();
        let path = self.dir.run_json();
        self.state.save(&path).map_err(io_err(&path))
    }

    fn log(&self, message: &str) -> Result<(), EngineError> {
        let path = self.dir.run_log();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(io_err(&path))?;
        writeln!(file, "{} {message}", UtcDateTime::now().iso8601()).map_err(io_err(&path))?;
        self.sink.event(&Event::Log {
            message: message.to_string(),
        });
        Ok(())
    }

    fn emit_started(&self, resumed: bool) {
        self.sink.event(&Event::Started {
            run_id: self.state.run_id.clone(),
            run_dir: self.dir.root().display().to_string(),
            resumed,
        });
    }

    fn emit_progress(&self, stage: &str, done: u64, total: u64, item: String) {
        let stage_index = TOY_STAGES.iter().position(|s| *s == stage).unwrap_or(0) + 1;
        self.sink.event(&Event::Progress(Snapshot {
            stage_index,
            stage_count: TOY_STAGES.len(),
            stage_name: stage.to_string(),
            stage_units_done: done,
            stage_units_total: total,
            overall_fraction: self.estimator.overall_fraction(),
            elapsed_seconds: self.elapsed(),
            remaining_seconds: self.estimator.remaining_seconds(),
            current_item: item,
        }));
    }

    fn set_status(&mut self, stage: &str, status: StageStatus) {
        if let Some(s) = self.state.stage_mut(stage) {
            s.status = status;
        }
    }

    fn status(&self, stage: &str) -> StageStatus {
        self.state
            .stage(stage)
            .map(|s| s.status)
            .unwrap_or(StageStatus::Pending)
    }

    /// Removes interrupted writes and resets every unit whose output is
    /// missing or damaged.
    fn verify_existing(&mut self) -> Result<u64, EngineError> {
        for d in self.dir.output_dirs() {
            atomic::remove_stale_tmp(&d).map_err(io_err(&d))?;
        }
        let plan = self.plan();
        let valid = (0..plan.chunk_count())
            .filter(|&i| atomic::is_valid(&self.dir.chunk_file(TOY_COUNT, i)))
            .count() as u64;
        if valid < plan.chunk_count() && self.status(TOY_COUNT) == StageStatus::Done {
            self.log(&format!(
                "{} of {} chunk files are missing or damaged; they will be recomputed.",
                plan.chunk_count() - valid,
                plan.chunk_count()
            ))?;
            self.set_status(TOY_COUNT, StageStatus::Running);
            self.set_status(FINALIZE, StageStatus::Pending);
        }
        if self.status(FINALIZE) == StageStatus::Done && !self.outputs_valid(FINALIZE) {
            self.log("Final outputs are missing or damaged; they will be recomputed.")?;
            self.set_status(FINALIZE, StageStatus::Pending);
        }
        let units_done: u64 = (0..plan.chunk_count())
            .filter(|&i| atomic::is_valid(&self.dir.chunk_file(TOY_COUNT, i)))
            .map(|i| {
                let (s, e) = plan.range(i);
                e - s
            })
            .sum();
        self.estimator.set_done(TOY_COUNT, units_done);
        if self.status(FINALIZE) == StageStatus::Done {
            self.estimator.set_done(FINALIZE, plan.chunk_count());
        }
        Ok(valid)
    }

    fn outputs_valid(&self, stage: &str) -> bool {
        let Some(s) = self.state.stage(stage) else {
            return false;
        };
        !s.outputs.is_empty()
            && s.outputs.iter().all(|(rel, hash)| {
                let path = self.dir.root().join(rel);
                atomic::stored_hash(&path).as_deref() == Some(hash.as_str())
                    && atomic::is_valid(&path)
            })
    }

    fn execute(&mut self) -> Result<Outcome, EngineError> {
        self.verify_existing()?;
        self.save()?;

        if self.status(TOY_COUNT) != StageStatus::Done {
            if let Some(outcome) = self.run_toy_count()? {
                return Ok(outcome);
            }
        }
        if self.status(FINALIZE) != StageStatus::Done {
            if self.cancel.load(Ordering::SeqCst) {
                return self.stop();
            }
            self.run_finalize()?;
        }
        let root = self.root_fingerprint()?;
        self.log(&format!("Run finished. Root fingerprint: {root}"))?;
        self.save()?;
        self.sink.event(&Event::Finished { root: root.clone() });
        Ok(Outcome::Finished { root })
    }

    fn stop(&mut self) -> Result<Outcome, EngineError> {
        self.save()?;
        self.log("Stopped on request. The run can be resumed.")?;
        self.sink.event(&Event::Stopped);
        Ok(Outcome::Stopped)
    }

    /// Returns `Some(Stopped)` if stopped on request.
    fn run_toy_count(&mut self) -> Result<Option<Outcome>, EngineError> {
        let plan = self.plan();
        let config = self.config();
        self.set_status(TOY_COUNT, StageStatus::Running);
        self.save()?;
        let total_chunks = plan.chunk_count();
        let mut done_units = (0..total_chunks)
            .filter(|&i| atomic::is_valid(&self.dir.chunk_file(TOY_COUNT, i)))
            .map(|i| plan.range(i).1 - plan.range(i).0)
            .sum::<u64>();
        self.emit_progress(
            TOY_COUNT,
            done_units,
            plan.units_total,
            "checking finished chunks".into(),
        );

        for index in 0..total_chunks {
            let path = self.dir.chunk_file(TOY_COUNT, index);
            if atomic::is_valid(&path) {
                continue;
            }
            if self.cancel.load(Ordering::SeqCst) {
                return self.stop().map(Some);
            }
            let (start, end) = plan.range(index);
            self.emit_progress(
                TOY_COUNT,
                done_units,
                plan.units_total,
                format!(
                    "chunk {} of {} (blocks {} to {})",
                    index + 1,
                    total_chunks,
                    start,
                    end - 1
                ),
            );
            let t = Instant::now();
            let payload = toy::chunk_payload(config.seed, start, end);
            atomic::write_verified(&path, &payload).map_err(io_err(&path))?;
            self.estimator
                .record(TOY_COUNT, end - start, t.elapsed().as_secs_f64());
            done_units += end - start;
            self.save()?;
        }

        let stage_hash = self.chunked_stage_hash()?;
        if let Some(s) = self.state.stage_mut(TOY_COUNT) {
            s.status = StageStatus::Done;
            s.outputs.clear();
        }
        self.save()?;
        self.emit_progress(
            TOY_COUNT,
            plan.units_total,
            plan.units_total,
            format!("all {total_chunks} chunks done"),
        );
        self.log(&format!(
            "Stage toy_count done: {total_chunks} chunks, content hash {stage_hash}."
        ))?;
        Ok(None)
    }

    /// SHA-256 over the payloads of all chunks in order. It does not depend on
    /// where the chunk boundaries are.
    fn chunked_stage_hash(&self) -> Result<String, EngineError> {
        let plan = self.plan();
        let mut hasher = StreamHasher::new();
        for i in 0..plan.chunk_count() {
            let path = self.dir.chunk_file(TOY_COUNT, i);
            let bytes = atomic::read_verified(&path).ok_or_else(|| {
                EngineError::Invalid(format!(
                    "chunk file {} is missing or damaged",
                    path.display()
                ))
            })?;
            hasher.update(&bytes);
        }
        Ok(hasher.finish_hex())
    }

    fn run_finalize(&mut self) -> Result<(), EngineError> {
        self.set_status(FINALIZE, StageStatus::Running);
        self.save()?;
        let plan = self.plan();
        let t = Instant::now();

        let mut hasher = StreamHasher::new();
        let mut total = 0u64;
        let mut chunks = Vec::with_capacity(plan.chunk_count() as usize);
        for i in 0..plan.chunk_count() {
            let path = self.dir.chunk_file(TOY_COUNT, i);
            let bytes = atomic::read_verified(&path).ok_or_else(|| {
                EngineError::Invalid(format!(
                    "chunk file {} is missing or damaged",
                    path.display()
                ))
            })?;
            hasher.update(&bytes);
            total = total.wrapping_add(toy::payload_total(&bytes));
            let (start, end) = plan.range(i);
            chunks.push(serde_json::json!({
                "index": i,
                "start": start,
                "end": end,
                "sha256": sha256_hex(&bytes),
            }));
        }
        let stage_hash = hasher.finish_hex();
        let config = self.config();

        let chunks_json = serde_json::json!({
            "stage": TOY_COUNT,
            "units_per_chunk": plan.units_per_chunk,
            "chunks": chunks,
        });
        let result_json = serde_json::json!({
            "kind": "toy",
            "seed": config.seed,
            "blocks": config.blocks,
            "block_size": config.block_size,
            "total": total,
            "toy_count_content_sha256": stage_hash,
        });
        let root = root_from(&self.state.config_sha256, &stage_hash);

        let mut outputs = BTreeMap::new();
        for (rel, text) in [
            ("audit/chunks.json", pretty(&chunks_json)),
            ("report/toy_result.json", pretty(&result_json)),
            ("audit/root.txt", format!("{root}\n")),
        ] {
            let path = self.dir.root().join(rel);
            let hash = atomic::write_verified(&path, text.as_bytes()).map_err(io_err(&path))?;
            outputs.insert(rel.to_string(), hash);
        }
        if let Some(s) = self.state.stage_mut(FINALIZE) {
            s.status = StageStatus::Done;
            s.outputs = outputs;
        }
        self.estimator
            .record(FINALIZE, plan.chunk_count(), t.elapsed().as_secs_f64());
        self.save()?;
        self.emit_progress(
            FINALIZE,
            plan.chunk_count(),
            plan.chunk_count(),
            "final outputs written".into(),
        );
        Ok(())
    }

    fn root_fingerprint(&self) -> Result<String, EngineError> {
        let path = self.dir.audit().join("root.txt");
        let bytes = atomic::read_verified(&path).ok_or_else(|| {
            EngineError::Invalid(format!("{} is missing or damaged", path.display()))
        })?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_string())
    }
}

fn pretty(value: &serde_json::Value) -> String {
    let mut s = serde_json::to_string_pretty(value).expect("JSON serialises");
    s.push('\n');
    s
}

/// Root fingerprint: SHA-256 over an ordered, line-based list of the
/// configuration fingerprint and every stage's content hash.
fn root_from(config_sha256: &str, toy_count_hash: &str) -> String {
    let text = format!(
        "qmaws-root-v1\nkind toy\nconfig {config_sha256}\nstage {TOY_COUNT} {toy_count_hash}\n"
    );
    sha256_hex(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::NullSink;
    use crate::testutil::TempDir;
    use std::cell::Cell;

    fn options(chunk_blocks: u64) -> ToyOptions {
        ToyOptions {
            seed: 7,
            blocks: 12,
            chunk_seconds: 3.0,
            chunk_blocks: Some(chunk_blocks),
        }
    }

    fn run_to_end(dir: &Path, chunk_blocks: u64) -> String {
        let cancel = AtomicBool::new(false);
        match start_toy_run(dir, &options(chunk_blocks), "terminal", &NullSink, &cancel).unwrap() {
            Outcome::Finished { root } => root,
            Outcome::Stopped => panic!("unexpected stop"),
        }
    }

    /// Requests a stop after a given number of progress events.
    struct StopAfter<'a> {
        remaining: Cell<u32>,
        cancel: &'a AtomicBool,
    }

    impl ProgressSink for StopAfter<'_> {
        fn event(&self, event: &Event) {
            if let Event::Progress(_) = event {
                match self.remaining.get() {
                    0 => self.cancel.store(true, Ordering::SeqCst),
                    n => self.remaining.set(n - 1),
                }
            }
        }
    }

    #[test]
    fn root_does_not_depend_on_chunk_size() {
        let tmp = TempDir::new("runner_chunks");
        let a = run_to_end(&tmp.path().join("a"), 1);
        let b = run_to_end(&tmp.path().join("b"), 5);
        let c = run_to_end(&tmp.path().join("c"), 12);
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn stop_and_resume_gives_the_same_root() {
        let tmp = TempDir::new("runner_resume");
        let reference = run_to_end(&tmp.path().join("ref"), 2);

        let dir = tmp.path().join("stopped");
        let cancel = AtomicBool::new(false);
        let sink = StopAfter {
            remaining: Cell::new(3),
            cancel: &cancel,
        };
        let first = start_toy_run(&dir, &options(2), "terminal", &sink, &cancel).unwrap();
        assert_eq!(first, Outcome::Stopped);
        let state = RunState::load(&RunDir::new(&dir).run_json()).unwrap();
        assert!(!state.is_finished());

        let cancel = AtomicBool::new(false);
        let resumed = resume_run(&dir, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(
            resumed,
            Outcome::Finished {
                root: reference.clone()
            }
        );
        // Resuming a finished run verifies it and returns the same root.
        let again = resume_run(&dir, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(again, Outcome::Finished { root: reference });
    }

    #[test]
    fn damaged_outputs_are_recomputed() {
        let tmp = TempDir::new("runner_damage");
        let dir = tmp.path().join("r");
        let reference = run_to_end(&dir, 3);
        let run = RunDir::new(&dir);
        std::fs::write(run.chunk_file(TOY_COUNT, 1), b"damaged").unwrap();
        std::fs::remove_file(run.chunk_file(TOY_COUNT, 2)).unwrap();
        std::fs::write(run.audit().join("root.txt"), b"0\n").unwrap();
        std::fs::write(run.chunks().join("x.bin.tmp"), b"partial").unwrap();

        let cancel = AtomicBool::new(false);
        let outcome = resume_run(&dir, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(outcome, Outcome::Finished { root: reference });
        assert!(!run.chunks().join("x.bin.tmp").exists());
    }

    #[test]
    fn existing_run_is_not_overwritten() {
        let tmp = TempDir::new("runner_exists");
        let dir = tmp.path().join("r");
        run_to_end(&dir, 4);
        let cancel = AtomicBool::new(false);
        let err = start_toy_run(&dir, &options(4), "terminal", &NullSink, &cancel).unwrap_err();
        assert!(matches!(err, EngineError::RunExists(_)));
    }

    #[test]
    fn the_root_is_fixed_for_fixed_settings() {
        // Fixed expected value, first produced on Windows x86-64 (2026-10-02).
        // The same value on every operating system shows that the toy run is
        // deterministic across platforms; a change of the toy computation or
        // of the fingerprint format also changes it.
        const EXPECTED: &str = "d7b6750a957a985f9c0fa3f39041bf1303179593fc13ea7da41f1d5bdd6da1ab";
        let tmp = TempDir::new("runner_fixed");
        assert_eq!(run_to_end(&tmp.path().join("r"), 4), EXPECTED);
        assert_eq!(run_to_end(&tmp.path().join("s"), 12), EXPECTED);
    }
}
