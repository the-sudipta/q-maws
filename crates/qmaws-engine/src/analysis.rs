//! The analysis run: from sequence files to a tree, as a
//! resumable sequence of stages. (Evaluation and figures are added in
//! later milestones.)
//!
//! | Stage | Unit of work | Output |
//! |---|---|---|
//! | `ingest` | whole stage | `audit/inputs.json` |
//! | `maw_extract` | one taxon (strand filter included) | `work/maws/<taxon>.bin` |
//! | `length_select` | whole stage | `work/matrix/selection.json` |
//! | `matrix_build` | whole stage | `work/matrix/m_full.bin`, `report/m_ml.phy` |
//! | `quartet_count` | one chunk of quartets in Feistel order | `work/chunks/quartet_count_<n>.bin` |
//! | `quartet_weight` | one chunk of quartets in Feistel order (counts recomputed from `M_full`) | `work/chunks/quartet_weight_<n>.bin` |
//! | `amalgamate` | whole stage: `wQFM-rs` on the weighted quartets in rank order | `report/tree.nwk`, `work/amalgamation.json` |
//! | `support` | whole stage: S1 per internal edge and halo value per taxon on the same weights | `trees/tree_s1.nwk`, `report/support.tsv`, `report/halo.tsv` |
//! | `bootstrap` | one S2 replicate (Poisson(1) column weights, W2b, wQFM-rs); skipped when `bootstrap` is 0 | `work/bootstrap/replicate_<b>.nwk`, then `trees/bootstrap_trees.nwk`, `trees/tree_s2.nwk`, `report/bootstrap.tsv` |
//! | `finalize` | whole stage | `audit/results.json`, `audit/quartet_decisions.bin.zst`, `audit/sample_worksheets.txt`, `audit/environment.json`, `audit/stages.json`, `audit/chunks.json`, `audit/root.txt` |
//!
//! Every file is written atomically with a hash file; a unit is done exactly
//! when its file is valid. The root fingerprint covers each stage's content
//! hash; for chunked stages the content hash is taken over all outputs in
//! order, so it does not depend on chunk boundaries. Weights are hashed as
//! text with every floating-point value rounded to 9 significant digits.

use crate::atomic;
use crate::clock::UtcDateTime;
use crate::hash::{sha256_hex, StreamHasher};
use crate::matrix_pipeline::{extraction_bytes, measured_memory_limit, EntropyRow};
use crate::progress::{Estimator, Event, LiveQuartet, ProgressSink, ProvisionalTree, Snapshot};
use crate::provisional::{self, Live};
use crate::rundir::RunDir;
use crate::runner::{io_err, verify_inputs, EngineError, Outcome};
use crate::state::{
    ChunkPlan, InputFingerprint, RunState, StageState, StageStatus, FORMAT_VERSION,
};
use crate::store;
use qmaws_core::amalgamate;
use qmaws_core::input::RecordMode;
use qmaws_core::matrix::{self, Matrix};
use qmaws_core::maw::{self, MawSet};
use qmaws_core::quartet::{self, CoCounts, Permutation, PopcountPath};
use qmaws_core::support;
use qmaws_core::weight::{self, Conditioning, Model};
use qmaws_core::worksheet;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// `kind` of an analysis run in `run.json`.
pub const KIND: &str = "analysis";

pub const INGEST: &str = "ingest";
pub const MAW_EXTRACT: &str = "maw_extract";
pub const LENGTH_SELECT: &str = "length_select";
pub const MATRIX_BUILD: &str = "matrix_build";
pub const QUARTET_COUNT: &str = "quartet_count";
pub const QUARTET_WEIGHT: &str = "quartet_weight";
pub const AMALGAMATE: &str = "amalgamate";
pub const SUPPORT: &str = "support";
pub const BOOTSTRAP: &str = "bootstrap";
pub const FINALIZE: &str = "finalize";
/// Quartets per parallel block when counting.
pub(crate) const COUNT_BLOCK: u64 = 4096;
/// Quartets per parallel block when weighting.
pub(crate) const WEIGHT_BLOCK: u64 = 16;

/// Largest chunk of records held in memory (bytes).
const MAX_CHUNK_BYTES: usize = 64 << 20;

/// Description of a chunked stage for [`Session::run_chunks`].
struct ChunkedStage {
    stage: &'static str,
    /// Name in the log.
    label: &'static str,
    /// Quartets per parallel block.
    block: u64,
    /// Bytes per record.
    record: usize,
    /// Extra text for the calibration log line.
    note: String,
}

pub const STAGES: [&str; 10] = [
    INGEST,
    MAW_EXTRACT,
    LENGTH_SELECT,
    MATRIX_BUILD,
    QUARTET_COUNT,
    QUARTET_WEIGHT,
    AMALGAMATE,
    SUPPORT,
    BOOTSTRAP,
    FINALIZE,
];

/// Settings of an analysis run; stored in `run.json` and never changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Folder of sequence files or one multi-FASTA file (absolute path).
    pub input: String,
    /// `per_file` or `per_record`.
    pub records: String,
    pub strand: bool,
    /// Fixed MAW lengths instead of the entropy selection.
    pub lengths: Option<Vec<usize>>,
    /// Seed of the quartet processing order and of the W2c resamples.
    pub seed: u64,
    pub ml_max_columns: usize,
    /// Quartet weighting: `w2_sym`, `w2_emp` or `none` (counts only).
    #[serde(default = "default_weighting")]
    pub weighting: String,
    /// Number of W2c resamples per quartet (0: no W2c).
    #[serde(default = "default_replicates")]
    pub replicates: u32,
    /// Number of S2 column-bootstrap replicates with W2b inside (0: no S2;
    /// decision D7). Left out of `run.json` when 0, so the configuration of
    /// a run without S2 reads and hashes as before.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bootstrap: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// Default number of S2 replicates of a run (decision D7).
pub const BOOTSTRAP_REPLICATES: u32 = 100;

fn default_weighting() -> String {
    WEIGHTING_SYM.to_string()
}

fn default_replicates() -> u32 {
    weight::REPLICATES
}

/// Weighting with the two-state symmetric model (the default).
pub const WEIGHTING_SYM: &str = "w2_sym";
/// Weighting with the frequencies of 0 and 1 of `M_full`.
pub const WEIGHTING_EMP: &str = "w2_emp";
/// No weighting stage.
pub const WEIGHTING_NONE: &str = "none";

/// Settings of how the work is split; they do not change results.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisOptions {
    pub config: AnalysisConfig,
    /// Target duration of one quartet chunk in seconds.
    pub chunk_seconds: f64,
    /// Fixed number of quartets per chunk instead of calibration (tests).
    pub chunk_quartets: Option<u64>,
    /// Memory limit in bytes; `None` measures 70% of the available memory.
    pub memory_limit: Option<u64>,
    /// Draw the live provisional tree while quartets are weighed (plan 4.7).
    /// A resumed run keeps the setting it started with.
    pub live_tree: bool,
}

impl AnalysisConfig {
    pub(crate) fn mode(&self) -> RecordMode {
        if self.records == "per_record" {
            RecordMode::OneTaxonPerRecord
        } else {
            RecordMode::ConcatenatePerFile
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaxonRecord {
    pub name: String,
    pub original_name: String,
    pub source_file: String,
    pub original_length: u64,
    pub cleaned_length: u64,
    pub removed: BTreeMap<char, u64>,
    pub cleaned_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InputsRecord {
    pub taxa: Vec<TaxonRecord>,
    pub files: Vec<InputFingerprint>,
    pub lmin: usize,
    pub lmax: usize,
    pub average_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Selection {
    entropies: Vec<EntropyRow>,
    selected: Vec<usize>,
}

/// Starts a new analysis run in `run_dir`.
pub fn start(
    run_dir: &Path,
    options: &AnalysisOptions,
    interface: &str,
    sink: &dyn ProgressSink,
    cancel: &AtomicBool,
) -> Result<Outcome, EngineError> {
    start_session(run_dir, options, interface, sink, cancel, false)
}

/// Runs only the stages up to `matrix_build` in a new folder `run_dir` and
/// returns the full matrix (`None` if stopped). Used by verification.
pub fn build_matrix_only(
    run_dir: &Path,
    options: &AnalysisOptions,
    sink: &dyn ProgressSink,
    cancel: &AtomicBool,
) -> Result<Option<Matrix>, EngineError> {
    match start_session(run_dir, options, "terminal", sink, cancel, true)? {
        Outcome::Stopped => Ok(None),
        Outcome::Finished { .. } => {
            let bytes =
                atomic::read_verified(&RunDir::new(run_dir).work().join("matrix/m_full.bin"))
                    .ok_or_else(|| EngineError::Invalid("the rebuilt matrix is missing".into()))?;
            store::matrix_from_bytes(&bytes)
                .map(Some)
                .map_err(|e| EngineError::Invalid(e.to_string()))
        }
    }
}

fn start_session(
    run_dir: &Path,
    options: &AnalysisOptions,
    interface: &str,
    sink: &dyn ProgressSink,
    cancel: &AtomicBool,
    until_matrix: bool,
) -> Result<Outcome, EngineError> {
    let dir = RunDir::new(run_dir);
    if dir.run_json().exists() {
        return Err(EngineError::RunExists(run_dir.to_path_buf()));
    }
    if !(options.chunk_seconds.is_finite() && options.chunk_seconds > 0.0) {
        return Err(EngineError::Invalid(
            "the chunk duration must be a positive number of seconds".into(),
        ));
    }
    if ![WEIGHTING_SYM, WEIGHTING_EMP, WEIGHTING_NONE].contains(&options.config.weighting.as_str())
    {
        return Err(EngineError::Invalid(format!(
            "unknown weighting {}",
            options.config.weighting
        )));
    }
    dir.create_layout().map_err(io_err(run_dir))?;
    let created = UtcDateTime::now();
    let config_value = serde_json::to_value(&options.config).expect("configuration serialises");
    let state = RunState {
        format_version: FORMAT_VERSION,
        program_version: env!("CARGO_PKG_VERSION").to_string(),
        run_id: run_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("run_{}", created.run_id_part())),
        created_utc: created.iso8601(),
        kind: KIND.to_string(),
        config_sha256: sha256_hex(config_value.to_string().as_bytes()),
        config: config_value,
        inputs: Vec::new(),
        stages: STAGES.iter().map(|s| StageState::pending(s)).collect(),
        chunk_plans: BTreeMap::new(),
        throughput: BTreeMap::new(),
        elapsed_seconds: 0.0,
        last_interface: interface.to_string(),
    };
    state
        .save(&dir.run_json())
        .map_err(io_err(&dir.run_json()))?;
    let mut s = Session::new(dir, state, options.clone(), sink, cancel);
    s.until_matrix = until_matrix;
    s.emit(Event::Started {
        run_id: s.state.run_id.clone(),
        run_dir: s.dir.root().display().to_string(),
        resumed: false,
    });
    s.log(&format!("Started analysis of {}.", options.config.input))?;
    s.execute()
}

/// Resumes an analysis run (called by [`crate::runner::resume_run`]).
pub(crate) fn resume(
    run_dir: &Path,
    mut state: RunState,
    interface: &str,
    sink: &dyn ProgressSink,
    cancel: &AtomicBool,
) -> Result<Outcome, EngineError> {
    verify_inputs(&state)?;
    state.last_interface = interface.to_string();
    let config: AnalysisConfig = serde_json::from_value(state.config.clone())
        .map_err(|e| EngineError::Invalid(format!("run.json configuration: {e}")))?;
    let options = AnalysisOptions {
        config,
        chunk_seconds: 3.0,
        chunk_quartets: None,
        memory_limit: None,
        live_tree: true,
    };
    let mut s = Session::new(RunDir::new(run_dir), state, options, sink, cancel);
    s.emit(Event::Started {
        run_id: s.state.run_id.clone(),
        run_dir: s.dir.root().display().to_string(),
        resumed: true,
    });
    s.log("Resuming run.")?;
    s.execute()
}

struct Session<'a> {
    dir: RunDir,
    state: RunState,
    options: AnalysisOptions,
    sink: &'a dyn ProgressSink,
    cancel: &'a AtomicBool,
    started: Instant,
    elapsed_before: f64,
    /// Seconds paused in this session, not counted as working time.
    paused: f64,
    estimator: Estimator,
    /// Stop after `matrix_build` (verification).
    until_matrix: bool,
    /// Live provisional tree, while `quartet_weight` runs.
    live: Option<Live>,
}

fn maws_dir(dir: &RunDir) -> PathBuf {
    dir.work().join("maws")
}
fn matrix_dir(dir: &RunDir) -> PathBuf {
    dir.work().join("matrix")
}
fn maw_file(dir: &RunDir, taxon: usize) -> PathBuf {
    maws_dir(dir).join(format!("taxon_{taxon:04}.bin"))
}
fn bootstrap_dir(dir: &RunDir) -> PathBuf {
    dir.work().join("bootstrap")
}
fn replicate_file(dir: &RunDir, b: u64) -> PathBuf {
    bootstrap_dir(dir).join(format!("replicate_{b:04}.nwk"))
}

impl<'a> Session<'a> {
    fn new(
        dir: RunDir,
        state: RunState,
        options: AnalysisOptions,
        sink: &'a dyn ProgressSink,
        cancel: &'a AtomicBool,
    ) -> Self {
        let elapsed_before = state.elapsed_seconds;
        Self {
            dir,
            state,
            options,
            sink,
            cancel,
            started: Instant::now(),
            elapsed_before,
            paused: 0.0,
            estimator: Estimator::new(&[]),
            until_matrix: false,
            live: None,
        }
    }

    fn elapsed(&self) -> f64 {
        self.elapsed_before + self.started.elapsed().as_secs_f64() - self.paused
    }

    fn emit(&self, e: Event) {
        self.sink.event(&e);
    }

    fn log(&self, message: &str) -> Result<(), EngineError> {
        let path = self.dir.run_log();
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(io_err(&path))?;
        writeln!(f, "{} {message}", UtcDateTime::now().iso8601()).map_err(io_err(&path))?;
        self.emit(Event::Log {
            message: message.to_string(),
        });
        Ok(())
    }

    fn save(&mut self) -> Result<(), EngineError> {
        self.state.elapsed_seconds = self.elapsed();
        for (k, v) in self.estimator.rates() {
            self.state.throughput.insert(k, v);
        }
        let p = self.dir.run_json();
        self.state.save(&p).map_err(io_err(&p))
    }

    fn status(&self, stage: &str) -> StageStatus {
        self.state
            .stage(stage)
            .map(|s| s.status)
            .unwrap_or(StageStatus::Pending)
    }

    fn set_done(
        &mut self,
        stage: &str,
        outputs: BTreeMap<String, String>,
    ) -> Result<(), EngineError> {
        let now = UtcDateTime::now().iso8601();
        if let Some(s) = self.state.stage_mut(stage) {
            s.status = StageStatus::Done;
            s.outputs = outputs;
            s.started_utc.get_or_insert_with(|| now.clone());
            s.finished_utc = Some(now);
        }
        self.save()
    }

    fn set_running(&mut self, stage: &str) -> Result<(), EngineError> {
        if let Some(s) = self.state.stage_mut(stage) {
            s.status = StageStatus::Running;
            s.started_utc
                .get_or_insert_with(|| UtcDateTime::now().iso8601());
            s.finished_utc = None;
        }
        self.save()
    }

    fn reset_from(&mut self, stage: &str) {
        let mut on = false;
        for s in &mut self.state.stages {
            if s.name == stage {
                on = true;
            }
            if on && s.status == StageStatus::Done {
                s.status = StageStatus::Running;
                s.finished_utc = None;
            }
        }
    }

    fn progress(&self, stage: &str, done: u64, total: u64, item: String) {
        let index = STAGES.iter().position(|s| *s == stage).unwrap_or(0) + 1;
        self.emit(Event::Progress(Snapshot {
            stage_index: index,
            stage_count: STAGES.len(),
            stage_name: stage.to_string(),
            stage_units_done: done,
            stage_units_total: total,
            overall_fraction: self.estimator.overall_fraction(),
            elapsed_seconds: self.elapsed(),
            remaining_seconds: self.estimator.remaining_seconds(),
            current_item: item,
        }));
    }

    fn stop(&mut self) -> Result<Outcome, EngineError> {
        self.save()?;
        self.log("Stopped on request. The run can be resumed.")?;
        self.emit(Event::Stopped);
        Ok(Outcome::Stopped)
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Called between units of work: waits while the interface asks for a
    /// pause, then returns true when the run must stop.
    fn should_stop(&mut self) -> Result<bool, EngineError> {
        if self.sink.pause_requested() && !self.cancelled() {
            self.save()?;
            self.log("Paused.")?;
            self.paused += crate::progress::wait_while_paused(self.sink, self.cancel);
            if !self.cancelled() {
                self.log("Continuing.")?;
            }
        }
        Ok(self.cancelled())
    }

    fn outputs_valid(&self, stage: &str) -> bool {
        self.state.stage(stage).is_some_and(|s| {
            s.outputs.iter().all(|(rel, hash)| {
                let p = self.dir.root().join(rel);
                atomic::stored_hash(&p).as_deref() == Some(hash.as_str()) && atomic::is_valid(&p)
            })
        })
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> Result<String, EngineError> {
        let p = self.dir.root().join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        atomic::write_verified(&p, bytes).map_err(io_err(&p))
    }

    fn read(&self, rel: &str) -> Result<Vec<u8>, EngineError> {
        let p = self.dir.root().join(rel);
        atomic::read_verified(&p)
            .ok_or_else(|| EngineError::Invalid(format!("{} is missing or damaged", p.display())))
    }

    fn inputs(&self) -> Result<InputsRecord, EngineError> {
        serde_json::from_slice(&self.read("audit/inputs.json")?)
            .map_err(|e| EngineError::Invalid(format!("audit/inputs.json: {e}")))
    }

    fn execute(&mut self) -> Result<Outcome, EngineError> {
        for d in [
            self.dir.root().to_path_buf(),
            maws_dir(&self.dir),
            matrix_dir(&self.dir),
            self.dir.chunks(),
            self.dir.audit(),
            self.dir.report(),
            bootstrap_dir(&self.dir),
        ] {
            atomic::remove_stale_tmp(&d).map_err(io_err(&d))?;
        }
        // Whole-stage outputs that are damaged reset that stage and the ones
        // after it.
        for stage in [
            INGEST,
            LENGTH_SELECT,
            MATRIX_BUILD,
            AMALGAMATE,
            SUPPORT,
            BOOTSTRAP,
            FINALIZE,
        ] {
            if self.status(stage) == StageStatus::Done && !self.outputs_valid(stage) {
                self.log(&format!(
                    "Outputs of {stage} are missing or damaged; recomputing from there."
                ))?;
                self.reset_from(stage);
            }
        }
        if self.status(INGEST) != StageStatus::Done {
            self.ingest()?;
        }
        let inputs = self.inputs()?;
        let m = inputs.taxa.len();
        let q = quartet::quartet_count(m);
        let mut units = vec![(MAW_EXTRACT, m as u64), (QUARTET_COUNT, q)];
        if self.weighted() {
            units.push((QUARTET_WEIGHT, q));
        }
        if self.bootstrapped() {
            units.push((BOOTSTRAP, self.options.config.bootstrap as u64));
        }
        self.estimator = Estimator::new(&units);
        for (stage, rate) in self.state.throughput.clone() {
            if units.iter().any(|(s, _)| *s == stage) {
                self.estimator.set_rate(&stage, rate);
            }
        }
        if let Some(outcome) = self.maw_extract(&inputs)? {
            return Ok(outcome);
        }
        if self.should_stop()? {
            return self.stop();
        }
        if self.status(LENGTH_SELECT) != StageStatus::Done {
            self.length_select(&inputs)?;
        }
        if self.status(MATRIX_BUILD) != StageStatus::Done {
            self.matrix_build(&inputs)?;
        }
        if self.until_matrix {
            self.save()?;
            return Ok(Outcome::Finished {
                root: String::new(),
            });
        }
        if let Some(outcome) = self.quartet_count(m)? {
            return Ok(outcome);
        }
        if let Some(outcome) = self.quartet_weight(m)? {
            return Ok(outcome);
        }
        if self.status(AMALGAMATE) != StageStatus::Done {
            self.amalgamate(m)?;
            self.write_convergence()?;
        }
        if self.status(SUPPORT) != StageStatus::Done {
            self.support(m)?;
        }
        if self.status(BOOTSTRAP) != StageStatus::Done {
            if let Some(outcome) = self.bootstrap()? {
                return Ok(outcome);
            }
        }
        if self.status(FINALIZE) != StageStatus::Done {
            self.finalize(m)?;
        }
        let root = String::from_utf8_lossy(&self.read("audit/root.txt")?)
            .trim()
            .to_string();
        self.log(&format!("Run finished. Root fingerprint: {root}"))?;
        crate::readme::write_run_readmes(self.dir.root()).map_err(io_err(self.dir.root()))?;
        self.save()?;
        self.emit(Event::Finished { root: root.clone() });
        Ok(Outcome::Finished { root })
    }

    fn ingest(&mut self) -> Result<(), EngineError> {
        self.set_running(INGEST)?;
        let cfg = &self.options.config;
        let loaded = qmaws_data::loader::load(Path::new(&cfg.input), cfg.mode())
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        let problems: Vec<String> = loaded
            .findings
            .iter()
            .filter(|f| f.is_error() || f.is_warning())
            .map(|f| f.message())
            .collect();
        if !problems.is_empty() {
            return Err(EngineError::Invalid(format!(
                "the input has problems that must be resolved first (see qmaws inspect): {}",
                problems.join(" ")
            )));
        }
        let total: u64 = loaded.taxa.iter().map(|t| t.cleaned.cleaned_length()).sum();
        let m = loaded.taxa.len() as u64;
        let average_length = total / m;
        let (lmin, lmax) = match &cfg.lengths {
            Some(l) if !l.is_empty() => (*l.iter().min().unwrap(), *l.iter().max().unwrap()),
            _ => matrix::adaptive_range(average_length, m as usize),
        };
        let files: Vec<InputFingerprint> = loaded
            .files
            .iter()
            .map(|f| InputFingerprint {
                path: f.path.display().to_string(),
                sha256: f.raw_sha256.clone(),
                bytes: f.bytes,
            })
            .collect();
        let record = InputsRecord {
            taxa: loaded
                .taxa
                .iter()
                .zip(&loaded.cleaned_sha256)
                .map(|(t, h)| TaxonRecord {
                    name: t.name.clone(),
                    original_name: t.original_name.clone(),
                    source_file: t.source_file.clone(),
                    original_length: t.cleaned.original_length,
                    cleaned_length: t.cleaned.cleaned_length(),
                    removed: t.cleaned.removed.clone(),
                    cleaned_sha256: h.clone(),
                })
                .collect(),
            files: files.clone(),
            lmin,
            lmax,
            average_length,
        };
        let text = serde_json::to_string_pretty(&record).expect("inputs serialise") + "\n";
        let hash = self.write("audit/inputs.json", text.as_bytes())?;
        self.state.inputs = files;
        self.log(&format!(
            "Ingest: {} taxa, average length {average_length}; MAW lengths {lmin} to {lmax}.",
            record.taxa.len()
        ))?;
        self.set_done(
            INGEST,
            BTreeMap::from([("audit/inputs.json".to_string(), hash)]),
        )
    }

    /// Returns `Some(Stopped)` when stopped on request.
    fn maw_extract(&mut self, inputs: &InputsRecord) -> Result<Option<Outcome>, EngineError> {
        let m = inputs.taxa.len();
        let pending: Vec<usize> = (0..m)
            .filter(|&t| !atomic::is_valid(&maw_file(&self.dir, t)))
            .collect();
        self.estimator
            .set_done(MAW_EXTRACT, (m - pending.len()) as u64);
        if pending.is_empty() {
            if self.status(MAW_EXTRACT) != StageStatus::Done {
                self.set_done(MAW_EXTRACT, BTreeMap::new())?;
            }
            return Ok(None);
        }
        if self.status(MAW_EXTRACT) == StageStatus::Done {
            self.log(&format!(
                "{} MAW files are missing or damaged; recomputing them.",
                pending.len()
            ))?;
        }
        // Everything after a recomputed extraction must be recomputed too.
        self.reset_from(MAW_EXTRACT);
        self.set_running(MAW_EXTRACT)?;
        let cfg = self.options.config.clone();
        let loaded = qmaws_data::loader::load(Path::new(&cfg.input), cfg.mode())
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        for (t, rec) in loaded.taxa.iter().zip(&inputs.taxa) {
            if t.name != rec.name || sha256_hex(&t.cleaned.sequence) != rec.cleaned_sha256 {
                return Err(EngineError::InputChanged(rec.source_file.clone()));
            }
        }
        let mut seqs: Vec<Option<Vec<u8>>> = loaded
            .taxa
            .into_iter()
            .map(|t| Some(t.cleaned.sequence))
            .collect();
        let limit = self
            .options
            .memory_limit
            .unwrap_or_else(measured_memory_limit);
        let longest = seqs.iter().flatten().map(|s| s.len()).max().unwrap_or(0);
        let total: u64 = seqs.iter().flatten().map(|s| s.len() as u64).sum();
        let per = extraction_bytes(longest).max(1);
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let workers = ((limit.saturating_sub(total) / per) as usize).clamp(1, cores);
        self.log(&format!(
            "MAW extraction: {} taxa to do, {workers} at a time, strand filter {}.",
            pending.len(),
            if cfg.strand { "on" } else { "off" }
        ))?;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .expect("thread pool");
        let mut done = (m - pending.len()) as u64;
        for batch in pending.chunks(workers) {
            if self.should_stop()? {
                return self.stop().map(Some);
            }
            let jobs: Vec<(usize, Vec<u8>)> = batch
                .iter()
                .map(|&t| (t, seqs[t].take().expect("sequence present")))
                .collect();
            let t0 = Instant::now();
            let results: Vec<(usize, Result<MawSet, maw::MawError>)> = pool.install(|| {
                jobs.into_par_iter()
                    .map(|(t, s)| {
                        let r = if cfg.strand {
                            maw::extract_strand_aware(&s, inputs.lmin, inputs.lmax)
                        } else {
                            maw::extract(&s, inputs.lmin, inputs.lmax)
                        };
                        (t, r)
                    })
                    .collect()
            });
            for (t, r) in results {
                let set = r.map_err(|e| EngineError::Invalid(e.to_string()))?;
                let path = maw_file(&self.dir, t);
                atomic::write_verified(&path, &store::maws_to_bytes(&set))
                    .map_err(io_err(&path))?;
                self.log(&format!("  {}: {} MAWs", inputs.taxa[t].name, set.count()))?;
            }
            done += batch.len() as u64;
            self.estimator
                .record(MAW_EXTRACT, batch.len() as u64, t0.elapsed().as_secs_f64());
            self.save()?;
            self.progress(MAW_EXTRACT, done, m as u64, format!("{} of {m} taxa", done));
        }
        self.set_done(MAW_EXTRACT, BTreeMap::new())?;
        Ok(None)
    }

    fn load_maws(&self, m: usize) -> Result<Vec<MawSet>, EngineError> {
        (0..m)
            .map(|t| {
                let p = maw_file(&self.dir, t);
                let bytes = atomic::read_verified(&p).ok_or_else(|| {
                    EngineError::Invalid(format!("{} is missing or damaged", p.display()))
                })?;
                store::maws_from_bytes(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))
            })
            .collect()
    }

    fn length_select(&mut self, inputs: &InputsRecord) -> Result<(), EngineError> {
        self.set_running(LENGTH_SELECT)?;
        let sets = self.load_maws(inputs.taxa.len())?;
        let entropies = matrix::length_entropies(&sets, inputs.lmin, inputs.lmax);
        let selected = match &self.options.config.lengths {
            Some(l) if !l.is_empty() => {
                let mut l = l.clone();
                l.sort_unstable();
                l.dedup();
                l
            }
            _ => matrix::select_lengths(&entropies, matrix::TOP_K, matrix::MIN_CHARS),
        };
        let sel = Selection {
            entropies: entropies.iter().map(EntropyRow::from).collect(),
            selected: selected.clone(),
        };
        let text = serde_json::to_string_pretty(&sel).expect("selection serialises") + "\n";
        let hash = self.write("work/matrix/selection.json", text.as_bytes())?;
        self.log(&format!("Selected MAW lengths: {selected:?}"))?;
        self.set_done(
            LENGTH_SELECT,
            BTreeMap::from([("work/matrix/selection.json".to_string(), hash)]),
        )
    }

    fn matrix_build(&mut self, inputs: &InputsRecord) -> Result<(), EngineError> {
        self.set_running(MATRIX_BUILD)?;
        let sel: Selection = serde_json::from_slice(&self.read("work/matrix/selection.json")?)
            .map_err(|e| EngineError::Invalid(format!("selection.json: {e}")))?;
        let m = inputs.taxa.len();
        let mut sets = self.load_maws(m)?;
        for s in &mut sets {
            s.keep_lengths(&sel.selected);
        }
        let columns = matrix::count_columns(&sets, &sel.selected);
        let wide = sel.selected.iter().any(|&l| l > 32);
        let estimated = Matrix::estimate_bytes(m, columns, wide);
        let limit = self
            .options
            .memory_limit
            .unwrap_or_else(measured_memory_limit);
        self.log(&format!(
            "Full matrix: {m} taxa x {columns} columns, estimated {} MB (memory limit {} MB).",
            estimated.div_ceil(1_000_000),
            limit / 1_000_000
        ))?;
        if estimated > limit {
            return Err(EngineError::Invalid(format!(
                "the full matrix would need about {} MB, more than the memory limit of {} MB (decision D3)",
                estimated / 1_000_000,
                limit / 1_000_000
            )));
        }
        if columns as u64 >= u32::MAX as u64 {
            return Err(EngineError::Invalid(
                "the matrix has too many columns for 32-bit counts".into(),
            ));
        }
        let full = matrix::build_full(&sets, &sel.selected);
        drop(sets);
        let keep = matrix::ml_columns(&full, self.options.config.ml_max_columns);
        let names: Vec<String> = inputs.taxa.iter().map(|t| t.name.clone()).collect();
        let phy = matrix::to_phylip(&full.select(&keep), &names);
        let h_full = self.write("work/matrix/m_full.bin", &store::matrix_to_bytes(&full))?;
        let h_phy = self.write("report/m_ml.phy", phy.as_bytes())?;
        self.log(&format!(
            "M_full {m} x {}; M_ml {m} x {}.",
            full.columns_len(),
            keep.len()
        ))?;
        self.set_done(
            MATRIX_BUILD,
            BTreeMap::from([
                ("work/matrix/m_full.bin".to_string(), h_full),
                ("report/m_ml.phy".to_string(), h_phy),
            ]),
        )
    }

    fn weighted(&self) -> bool {
        self.options.config.weighting != WEIGHTING_NONE
    }

    fn bootstrapped(&self) -> bool {
        self.weighted() && self.options.config.bootstrap > 0
    }

    fn load_full(&self) -> Result<Matrix, EngineError> {
        store::matrix_from_bytes(&self.read("work/matrix/m_full.bin")?)
            .map_err(|e| EngineError::Invalid(e.to_string()))
    }

    fn quartet_count(&mut self, m: usize) -> Result<Option<Outcome>, EngineError> {
        let q = quartet::quartet_count(m);
        let full = self.load_full()?;
        let cc = CoCounts::new(&full.rows, full.columns_len() as u64);
        let path = PopcountPath::detect();
        let perm = Permutation::new(q, self.options.config.seed);
        let count_range =
            |s: u64, e: u64| -> Vec<u8> { count_positions(&full, &cc, path, &perm, s, e) };
        self.run_chunks(
            ChunkedStage {
                stage: QUARTET_COUNT,
                label: "Quartet counting",
                block: COUNT_BLOCK,
                record: store::COUNT_RECORD,
                note: format!("; popcount {path:?}"),
            },
            q,
            &count_range,
            None,
        )
    }

    fn quartet_weight(&mut self, m: usize) -> Result<Option<Outcome>, EngineError> {
        if !self.weighted() {
            if self.status(QUARTET_WEIGHT) != StageStatus::Done {
                self.set_done(QUARTET_WEIGHT, BTreeMap::new())?;
            }
            return Ok(None);
        }
        let q = quartet::quartet_count(m);
        let full = self.load_full()?;
        let cc = CoCounts::new(&full.rows, full.columns_len() as u64);
        let perm = Permutation::new(q, self.options.config.seed);
        let weigher = Weigher {
            full: &full,
            cc: &cc,
            path: PopcountPath::detect(),
            perm: &perm,
            model: weight_model(&self.options.config, &full)?,
            seed: self.options.config.seed,
            replicates: self.options.config.replicates,
        };
        let weigh_range = |s: u64, e: u64| -> Vec<u8> { weigher.positions(s, e) };
        let counts = |quad: [usize; 4]| weigher.counts(quad);
        self.live = Some(Live::open(&self.dir, self.options.live_tree));
        let outcome = self.run_chunks(
            ChunkedStage {
                stage: QUARTET_WEIGHT,
                label: "Quartet weighting",
                block: WEIGHT_BLOCK,
                record: store::WEIGHT_RECORD,
                note: format!(
                    "; model pi1 = {:.6}, {} resamples",
                    weigher.model.pi[1], weigher.replicates
                ),
            },
            q,
            &weigh_range,
            Some(&counts),
        );
        self.live = None;
        outcome
    }

    /// Sends the last quartet of a weighed chunk to the interface.
    fn live_quartet(
        &self,
        chunk: &[u8],
        counts: &dyn Fn([usize; 4]) -> quartet::PatternCounts,
        done: u64,
        q: u64,
    ) -> Result<(), EngineError> {
        let records =
            store::weight_records(chunk).map_err(|e| EngineError::Invalid(e.to_string()))?;
        let Some(r) = records.last() else {
            return Ok(());
        };
        let quad = quartet::unrank(r.rank);
        let c = counts(quad);
        let names = self.names()?;
        self.emit(Event::Quartet(Box::new(LiveQuartet {
            taxa: quad.map(|i| names[i].clone()),
            counts: c,
            w1: weight::w1(&c),
            log_likelihoods: r.fitted().then_some(r.log_likelihoods),
            weights: r.tree_weights(),
            weights_name: if r.resampled() { "w2c" } else { "w2b" }.to_string(),
            quartets_done: done,
            quartets_total: q,
        })));
        Ok(())
    }

    /// Weights of the quartets whose chunk is finished, by rank.
    fn finished_weights(&self) -> Result<Vec<(u64, [f64; 3])>, EngineError> {
        let plan = self.state.chunk_plans[QUARTET_WEIGHT];
        let mut out = Vec::new();
        for i in 0..plan.chunk_count() {
            let Some(bytes) = atomic::read_verified(&self.dir.chunk_file(QUARTET_WEIGHT, i)) else {
                continue;
            };
            for r in
                store::weight_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?
            {
                if let Some(w) = r.tree_weights() {
                    out.push((r.rank, w));
                }
            }
        }
        out.sort_unstable_by_key(|&(rank, _)| rank);
        Ok(out)
    }

    /// A provisional tree from the finished quartets, when one is due.
    fn provisional_update(&mut self, done: u64, q: u64) -> Result<(), EngineError> {
        if !self.live.as_ref().is_some_and(|l| l.due(done, q)) {
            return Ok(());
        }
        let started = Instant::now();
        let weights = self.finished_weights()?;
        if weights.is_empty() {
            return Ok(());
        }
        let names = self.names()?;
        let quartets: Vec<amalgamate::Quartet> = weights
            .iter()
            .flat_map(|&(rank, w)| amalgamate::quartets_of(quartet::unrank(rank), w))
            .collect();
        let result = amalgamate::wqfm(&names, &quartets, &amalgamate::Settings::default());
        drop(quartets);
        let mut acc =
            support::Accumulator::new(&names, &result.newick).map_err(EngineError::Invalid)?;
        for &(rank, w) in &weights {
            acc.add(quartet::unrank(rank), w);
        }
        let halo: Vec<(String, Option<f64>)> = acc
            .finish()
            .halo
            .into_iter()
            .map(|h| (h.taxon, h.value))
            .collect();
        let percent = 100.0 * done as f64 / q as f64;
        let frame = self
            .live
            .as_ref()
            .map_or(0, |l| l.state.frames.len() as u32)
            + 1;
        let layout = qmaws_viz::tree::TreeLayout::from_newick(&result.newick)
            .map_err(EngineError::Invalid)?;
        let style = qmaws_viz::tree::HaloTreeStyle {
            title: vec![
                format!("Provisional Halo Tree: {}", self.state.run_id),
                format!(
                    "{} taxa, {done} of {q} quartets ({percent:.0}%), update {frame}",
                    names.len()
                ),
            ],
            watermark: Some(format!("PROVISIONAL \u{2014} {percent:.0}% of quartets")),
            size: 800.0,
        };
        let svg = qmaws_viz::tree::halo_tree_svg(&layout, &halo.iter().cloned().collect(), &style);
        let render = |r: Result<Vec<u8>, String>| r.map_err(EngineError::Invalid);
        let png = render(qmaws_viz::render::png(&svg, provisional::LATEST_WIDTH))?;
        let pdf = render(qmaws_viz::render::pdf(&svg))?;
        let small = render(qmaws_viz::render::png(&svg, provisional::FRAME_WIDTH))?;
        for (path, bytes) in [
            (provisional::latest_file(&self.dir, "svg"), svg.as_bytes()),
            (provisional::latest_file(&self.dir, "png"), &png[..]),
            (provisional::latest_file(&self.dir, "pdf"), &pdf[..]),
            (provisional::frame_file(&self.dir, frame), &small[..]),
        ] {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(io_err(parent))?;
            }
            atomic::write_atomic(&path, bytes).map_err(io_err(&path))?;
        }
        let seconds = started.elapsed().as_secs_f64();
        let live = self.live.as_mut().expect("live tree");
        let note = live.record(provisional::Frame {
            frame,
            percent,
            quartets: done,
            newick: result.newick.clone(),
            seconds,
        });
        live.save(&self.dir)?;
        self.log(&format!(
            "Provisional tree {frame}: {percent:.0}% of quartets, in {seconds:.2} s."
        ))?;
        if let Some(line) = note {
            self.log(&line)?;
        }
        self.emit(Event::Provisional(ProvisionalTree {
            frame,
            percent,
            newick: result.newick,
            halo,
            svg: provisional::latest_file(&self.dir, "svg")
                .display()
                .to_string(),
        }));
        Ok(())
    }

    /// `report/convergence.csv` from the saved provisional trees, if any.
    fn write_convergence(&self) -> Result<(), EngineError> {
        let Some(state) = provisional::load(&self.dir) else {
            return Ok(());
        };
        if state.frames.is_empty() || !self.weighted() {
            return Ok(());
        }
        let tree = String::from_utf8_lossy(&self.read("report/tree.nwk")?).into_owned();
        let csv = provisional::convergence_csv(&state.frames, &tree)?;
        let p = self.dir.report().join("convergence.csv");
        atomic::write_atomic(&p, csv.as_bytes()).map_err(io_err(&p))
    }

    /// Runs a stage over all q processing positions in chunks: the chunk
    /// plan is fixed by a calibration on the first run and stored; chunks
    /// with a valid file are skipped. Positions are processed in parallel
    /// blocks joined in order, so the bytes do not depend on the number of
    /// threads.
    fn run_chunks(
        &mut self,
        spec: ChunkedStage,
        q: u64,
        range: &(dyn Fn(u64, u64) -> Vec<u8> + Sync),
        counts: Option<&dyn Fn([usize; 4]) -> quartet::PatternCounts>,
    ) -> Result<Option<Outcome>, EngineError> {
        let stage = spec.stage;
        let block = spec.block;
        let run_range = |s: u64, e: u64| -> Vec<u8> { run_blocks(block, s, e, range) };
        let plan = match self.state.chunk_plans.get(stage) {
            Some(p) => *p,
            None => {
                // Calibration: time a sample of quartets the same way chunks
                // are processed, then fix the plan.
                let threads = rayon::current_num_threads() as u64;
                let sample = q.min(block * threads);
                let t0 = Instant::now();
                std::hint::black_box(run_range(0, sample));
                let rate = sample as f64 / t0.elapsed().as_secs_f64().max(1e-9);
                self.estimator.set_rate(stage, rate);
                // Target duration, but never more than MAX_CHUNK_BYTES of
                // records in memory at once.
                let max_per = (MAX_CHUNK_BYTES / spec.record) as u64;
                let per = self.options.chunk_quartets.unwrap_or(
                    ((rate * self.options.chunk_seconds).round() as u64).clamp(1, max_per),
                );
                let plan = ChunkPlan::new(q, per);
                self.state.chunk_plans.insert(stage.to_string(), plan);
                self.save()?;
                self.log(&format!(
                    "{}: {q} quartets, {} chunks of up to {} (calibration {:.0} quartets per second{}).",
                    spec.label,
                    plan.chunk_count(),
                    plan.units_per_chunk,
                    rate,
                    spec.note
                ))?;
                plan
            }
        };
        let chunks = plan.chunk_count();
        let file = |i: u64| self.dir.chunk_file(stage, i);
        let valid: Vec<bool> = (0..chunks).map(|i| atomic::is_valid(&file(i))).collect();
        let done_units: u64 = (0..chunks)
            .filter(|&i| valid[i as usize])
            .map(|i| plan.range(i).1 - plan.range(i).0)
            .sum();
        self.estimator.set_done(stage, done_units);
        if valid.iter().all(|&v| v) {
            if self.status(stage) != StageStatus::Done {
                self.set_done(stage, BTreeMap::new())?;
            }
            return Ok(None);
        }
        self.reset_from(stage);
        self.set_running(stage)?;
        let mut done = done_units;
        for i in 0..chunks {
            if valid[i as usize] {
                continue;
            }
            if self.should_stop()? {
                return self.stop().map(Some);
            }
            let (s, e) = plan.range(i);
            self.progress(
                stage,
                done,
                q,
                format!("chunk {} of {chunks} (positions {s} to {})", i + 1, e - 1),
            );
            let t0 = Instant::now();
            let out = run_range(s, e);
            let p = self.dir.chunk_file(stage, i);
            atomic::write_verified(&p, &out).map_err(io_err(&p))?;
            self.estimator
                .record(stage, e - s, t0.elapsed().as_secs_f64());
            done += e - s;
            self.save()?;
            if let Some(counts) = counts {
                self.live_quartet(&out, counts, done, q)?;
                self.provisional_update(done, q)?;
            }
        }
        self.progress(stage, q, q, format!("all {chunks} chunks done"));
        self.set_done(stage, BTreeMap::new())?;
        Ok(None)
    }

    fn names(&self) -> Result<Vec<String>, EngineError> {
        Ok(self.inputs()?.taxa.into_iter().map(|t| t.name).collect())
    }

    /// The weights of every quartet in rank order as used for the tree:
    /// W2c, or W2b when no resamples were made; `None` for quartets without
    /// a fit. Also returns the name of the weights.
    fn tree_weights(&self, m: usize) -> Result<(TreeWeights, &'static str), EngineError> {
        let q = quartet::quartet_count(m);
        let plan = self.state.chunk_plans[QUARTET_WEIGHT];
        let mut by_rank: Vec<Option<[f64; 3]>> = vec![None; q as usize];
        let mut source = "w2c";
        for i in 0..plan.chunk_count() {
            let p = self.dir.chunk_file(QUARTET_WEIGHT, i);
            let bytes = atomic::read_verified(&p).ok_or_else(|| {
                EngineError::Invalid(format!("{} is missing or damaged", p.display()))
            })?;
            for r in
                store::weight_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?
            {
                if let Some(w) = r.tree_weights() {
                    if !r.resampled() {
                        source = "w2b";
                    }
                    by_rank[r.rank as usize] = Some(w);
                }
            }
        }
        Ok((by_rank, source))
    }

    /// S1 of every internal edge and the halo value of every taxon, on the
    /// weights used for the tree.
    fn support(&mut self, m: usize) -> Result<(), EngineError> {
        self.set_running(SUPPORT)?;
        if !self.weighted() {
            return self.set_done(SUPPORT, BTreeMap::new());
        }
        let started = Instant::now();
        let (weights, _) = self.tree_weights(m)?;
        let names = self.names()?;
        let tree = String::from_utf8_lossy(&self.read("report/tree.nwk")?)
            .trim()
            .to_string();
        let mut acc = support::Accumulator::new(&names, &tree).map_err(EngineError::Invalid)?;
        for (rank, w) in weights.iter().enumerate() {
            if let Some(w) = w {
                acc.add(quartet::unrank(rank as u64), *w);
            }
        }
        drop(weights);
        let s = acc.finish();
        let na = |v: Option<f64>| v.map_or("NA".to_string(), |x| format!("{x:.6}"));
        let mut edges =
            String::from("edge\tsize\ts1\tconsistent_weight\ttotal_weight\tquartets\tclade\n");
        for (i, e) in s.edges.iter().enumerate() {
            edges.push_str(&format!(
                "{}\t{}\t{}\t{:.6}\t{:.6}\t{}\t{}\n",
                i + 1,
                e.clade.len(),
                na(e.s1),
                e.consistent,
                e.total,
                e.quartets,
                e.clade.join(",")
            ));
        }
        let mut halo = String::from("taxon\thalo\tconsistent_weight\ttotal_weight\n");
        for h in &s.halo {
            halo.push_str(&format!(
                "{}\t{}\t{:.6}\t{:.6}\n",
                h.taxon,
                na(h.value),
                h.consistent,
                h.total
            ));
        }
        let mut outputs = BTreeMap::new();
        for (rel, text) in [
            ("trees/tree_s1.nwk", format!("{}\n", s.newick)),
            ("report/support.tsv", edges),
            ("report/halo.tsv", halo),
        ] {
            outputs.insert(rel.to_string(), self.write(rel, text.as_bytes())?);
        }
        let s1: Vec<f64> = s.edges.iter().filter_map(|e| e.s1).collect();
        let low = s
            .halo
            .iter()
            .filter(|h| h.value.is_some_and(|v| v < LOW_HALO))
            .count();
        self.log(&format!(
            "Support: {} internal edges, S1 from {} to {}; {low} taxa with a halo value below {LOW_HALO}; in {:.1} s.",
            s.edges.len(),
            na(s1.iter().copied().reduce(f64::min)),
            na(s1.iter().copied().reduce(f64::max)),
            started.elapsed().as_secs_f64()
        ))?;
        self.set_done(SUPPORT, outputs)
    }

    /// S2 (decision D7): `bootstrap` column-bootstrap replicates with W2b
    /// weights inside, one replicate per unit (`work/bootstrap/`), then the
    /// split frequency of every internal edge of the run's tree. Returns
    /// `Some(Stopped)` when stopped on request.
    fn bootstrap(&mut self) -> Result<Option<Outcome>, EngineError> {
        if !self.bootstrapped() {
            self.set_done(BOOTSTRAP, BTreeMap::new())?;
            return Ok(None);
        }
        let b_total = self.options.config.bootstrap as u64;
        let pending: Vec<u64> = (0..b_total)
            .filter(|&b| !atomic::is_valid(&replicate_file(&self.dir, b)))
            .collect();
        self.estimator
            .set_done(BOOTSTRAP, b_total - pending.len() as u64);
        if !pending.is_empty() {
            self.set_running(BOOTSTRAP)?;
            let names = self.names()?;
            let full = self.load_full()?;
            let model = weight_model(&self.options.config, &full)?;
            if pending.len() as u64 == b_total {
                self.log(&format!(
                    "Bootstrap (S2): {b_total} replicates, Poisson(1) column weights, W2b weights inside."
                ))?;
            }
            let before = b_total - pending.len() as u64;
            for (i, &b) in pending.iter().enumerate() {
                if self.should_stop()? {
                    return self.stop().map(Some);
                }
                self.progress(
                    BOOTSTRAP,
                    before + i as u64,
                    b_total,
                    format!("replicate {} of {b_total}", b + 1),
                );
                let t0 = Instant::now();
                let r = crate::bootstrap::replicate_tree(
                    &names,
                    &full,
                    &model,
                    self.options.config.seed,
                    b,
                    crate::bootstrap::Inner::W2b,
                );
                let p = replicate_file(&self.dir, b);
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).map_err(io_err(parent))?;
                }
                atomic::write_verified(&p, format!("{}\n", r.newick).as_bytes())
                    .map_err(io_err(&p))?;
                self.estimator
                    .record(BOOTSTRAP, 1, t0.elapsed().as_secs_f64());
                self.save()?;
            }
            self.progress(BOOTSTRAP, b_total, b_total, "all replicates done".into());
        }
        let started = Instant::now();
        let mut trees = Vec::with_capacity(b_total as usize);
        for b in 0..b_total {
            let text = String::from_utf8_lossy(
                &atomic::read_verified(&replicate_file(&self.dir, b)).ok_or_else(|| {
                    EngineError::Invalid(format!("bootstrap replicate {b} is missing or damaged"))
                })?,
            )
            .trim()
            .to_string();
            trees.push(text);
        }
        let tree = String::from_utf8_lossy(&self.read("report/tree.nwk")?)
            .trim()
            .to_string();
        let (edges, labelled) =
            support::split_frequencies(&tree, &trees).map_err(EngineError::Invalid)?;
        let mut tsv = String::from("edge\tsize\ts2\treplicates\tclade\n");
        for (i, e) in edges.iter().enumerate() {
            tsv.push_str(&format!(
                "{}\t{}\t{:.6}\t{}\t{}\n",
                i + 1,
                e.clade.len(),
                e.s2,
                e.replicates,
                e.clade.join(",")
            ));
        }
        let mut outputs = BTreeMap::new();
        for (rel, text) in [
            ("trees/bootstrap_trees.nwk", trees.join("\n") + "\n"),
            ("trees/tree_s2.nwk", format!("{labelled}\n")),
            ("report/bootstrap.tsv", tsv),
        ] {
            outputs.insert(rel.to_string(), self.write(rel, text.as_bytes())?);
        }
        let s2: Vec<f64> = edges.iter().map(|e| e.s2).collect();
        let fmt = |v: Option<f64>| v.map_or("NA".to_string(), |x| format!("{x:.3}"));
        self.log(&format!(
            "Bootstrap (S2): {b_total} replicates; {} internal edges, S2 from {} to {}; split frequencies in {:.1} s.",
            edges.len(),
            fmt(s2.iter().copied().reduce(f64::min)),
            fmt(s2.iter().copied().reduce(f64::max)),
            started.elapsed().as_secs_f64()
        ))?;
        self.set_done(BOOTSTRAP, outputs)?;
        Ok(None)
    }

    /// Amalgamation with `wQFM-rs`: the weighted quartets of every quartet
    /// (W2c weights, or W2b when no resamples were made), in rank order so
    /// the tree does not depend on the processing order.
    fn amalgamate(&mut self, m: usize) -> Result<(), EngineError> {
        self.set_running(AMALGAMATE)?;
        if !self.weighted() {
            return self.set_done(AMALGAMATE, BTreeMap::new());
        }
        let started = Instant::now();
        let (by_rank, source) = self.tree_weights(m)?;
        let mut quartets = Vec::new();
        for (rank, w) in by_rank.iter().enumerate() {
            if let Some(w) = w {
                quartets.extend(amalgamate::quartets_of(quartet::unrank(rank as u64), *w));
            }
        }
        drop(by_rank);
        let names = self.names()?;
        let settings = amalgamate::Settings::default();
        let result = amalgamate::wqfm(&names, &quartets, &settings);
        let summary = serde_json::json!({
            "method": "wQFM-rs",
            "weights": source,
            "beta": settings.beta,
            "weighted_quartets": quartets.len(),
            "consistency_score": format!("{:.8e}", result.score),
            "total_weight": format!("{:.8e}", result.total_weight),
        });
        let h_tree = self.write("report/tree.nwk", format!("{}\n", result.newick).as_bytes())?;
        let h_summary = self.write(
            "work/amalgamation.json",
            (serde_json::to_string_pretty(&summary).expect("json") + "\n").as_bytes(),
        )?;
        self.log(&format!(
            "Amalgamation (wQFM-rs, {source} weights): {} weighted quartets, consistency score {:.4} of {:.4}, in {:.1} s.",
            quartets.len(),
            result.score,
            result.total_weight,
            started.elapsed().as_secs_f64()
        ))?;
        self.set_done(
            AMALGAMATE,
            BTreeMap::from([
                ("report/tree.nwk".to_string(), h_tree),
                ("work/amalgamation.json".to_string(), h_summary),
            ]),
        )
    }

    /// Summary numbers of the whole stages for `audit/stages.json` (plan
    /// 4.6.2), read from their outputs. The chunked stages are summarised
    /// in `finalize` while their chunks are read.
    fn stage_summaries(
        &self,
        m: usize,
    ) -> Result<BTreeMap<&'static str, serde_json::Value>, EngineError> {
        let inputs = self.inputs()?;
        let mut out = BTreeMap::new();
        out.insert(
            INGEST,
            serde_json::json!({
                "taxa": inputs.taxa.len(),
                "average_length": inputs.average_length,
                "maw_lengths": [inputs.lmin, inputs.lmax],
            }),
        );
        let sets = self.load_maws(m)?;
        let per_taxon: serde_json::Map<String, serde_json::Value> = inputs
            .taxa
            .iter()
            .zip(&sets)
            .map(|(t, s)| (t.name.clone(), serde_json::json!(s.count())))
            .collect();
        drop(sets);
        out.insert(
            MAW_EXTRACT,
            serde_json::json!({ "maws_per_taxon": per_taxon }),
        );
        let sel: Selection = serde_json::from_slice(&self.read("work/matrix/selection.json")?)
            .map_err(|e| EngineError::Invalid(format!("selection.json: {e}")))?;
        out.insert(
            LENGTH_SELECT,
            serde_json::json!({
                "entropy_per_length": sel.entropies.iter().map(|e| serde_json::json!({
                    "length": e.length, "variable_columns": e.characters, "entropy": sig9(e.entropy),
                })).collect::<Vec<_>>(),
                "selected_lengths": sel.selected,
            }),
        );
        let full_columns = self.load_full()?.columns_len();
        let phy = self.read("report/m_ml.phy")?;
        let ml_columns = String::from_utf8_lossy(&phy).lines().next().and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|c| c.parse::<u64>().ok())
        });
        out.insert(
            MATRIX_BUILD,
            serde_json::json!({
                "taxa": m,
                "m_full_columns": full_columns,
                "m_ml_columns": ml_columns,
            }),
        );
        if !self.weighted() {
            return Ok(out);
        }
        let amalgamation: serde_json::Value =
            serde_json::from_slice(&self.read("work/amalgamation.json")?)
                .map_err(|e| EngineError::Invalid(format!("amalgamation.json: {e}")))?;
        out.insert(AMALGAMATE, amalgamation);
        let column = |rel: &str, i: usize| -> Result<Vec<f64>, EngineError> {
            Ok(String::from_utf8_lossy(&self.read(rel)?)
                .lines()
                .skip(1)
                .filter_map(|l| l.split('\t').nth(i).and_then(|v| v.parse::<f64>().ok()))
                .collect())
        };
        let stats = |v: &[f64]| {
            let mean = (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
            serde_json::json!({
                "min": v.iter().copied().reduce(f64::min).map(sig9),
                "mean": mean.map(sig9),
                "max": v.iter().copied().reduce(f64::max).map(sig9),
            })
        };
        let s1 = column("report/support.tsv", 2)?;
        let halo = column("report/halo.tsv", 1)?;
        out.insert(
            SUPPORT,
            serde_json::json!({
                "internal_edges": s1.len(),
                "s1": stats(&s1),
                "halo": stats(&halo),
                "taxa_with_halo_below_0_6": halo.iter().filter(|&&h| h < LOW_HALO).count(),
            }),
        );
        if self.bootstrapped() {
            let s2 = column("report/bootstrap.tsv", 2)?;
            out.insert(
                BOOTSTRAP,
                serde_json::json!({
                    "replicates": self.options.config.bootstrap,
                    "weights": "w2b",
                    "s2": stats(&s2),
                }),
            );
        }
        Ok(out)
    }

    /// `audit/results.json`, `audit/quartet_decisions.bin.zst` and
    /// `audit/sample_worksheets.txt`.
    fn audit_files(&self, m: usize) -> Result<Vec<(&'static str, Vec<u8>)>, EngineError> {
        let names = self.names()?;
        let (weights, source) = self.tree_weights(m)?;
        // Decisions.
        let raw = store::decisions_to_bytes(&weights);
        let decisions = zstd::bulk::compress(&raw, DECISIONS_LEVEL)
            .map_err(|e| EngineError::Invalid(format!("compressing the decisions: {e}")))?;
        // Sample worksheets, each checked against the stored weights.
        let full = self.load_full()?;
        let cc = CoCounts::new(&full.rows, full.columns_len() as u64);
        let q = quartet::quartet_count(m);
        let perm = Permutation::new(q, self.options.config.seed);
        let weigher = Weigher {
            full: &full,
            cc: &cc,
            path: PopcountPath::detect(),
            perm: &perm,
            model: weight_model(&self.options.config, &full)?,
            seed: self.options.config.seed,
            replicates: self.options.config.replicates,
        };
        let ranks = sample_ranks(self.options.config.seed, q, SAMPLE_WORKSHEETS);
        let config = &self.options.config;
        let sheets: Vec<(String, weight::QuartetWeights)> = ranks
            .par_iter()
            .map(|&r| worksheet_of(config, &names, &weigher, quartet::unrank(r)))
            .collect();
        let mut text = format!(
            "Sample worksheets: {} quartets drawn with seed quartet_seed({}, 2^64 - 1), in rank order.\n\
             Each worksheet recomputes the quartet from M_full and is compared with the stored {source} weights.\n",
            ranks.len(),
            self.options.config.seed
        );
        for (&r, (sheet, w)) in ranks.iter().zip(&sheets) {
            let recomputed = if source == "w2c" { w.w2c } else { w.w2b };
            let stored = weights[r as usize];
            if recomputed != stored {
                return Err(EngineError::Invalid(format!(
                    "the worksheet of quartet {r} differs from the stored weights"
                )));
            }
            text.push_str("\n----------------------------------------------------------------\n");
            text.push_str(sheet);
            text.push_str(&format!(
                "   Stored {source} weights: identical; decision stored as {:?}\n",
                store::decision(stored)
            ));
        }
        // Results.
        let read_text = |rel: &str| -> Result<String, EngineError> {
            Ok(String::from_utf8_lossy(&self.read(rel)?).trim().to_string())
        };
        let amalgamation: serde_json::Value =
            serde_json::from_slice(&self.read("work/amalgamation.json")?)
                .map_err(|e| EngineError::Invalid(format!("amalgamation.json: {e}")))?;
        let tsv_rows = |rel: &str| -> Result<Vec<Vec<String>>, EngineError> {
            Ok(read_text(rel)?
                .lines()
                .skip(1)
                .map(|l| l.split('\t').map(str::to_string).collect())
                .collect())
        };
        let num = |s: &str| -> serde_json::Value {
            s.parse::<f64>().map_or(serde_json::Value::Null, sig9)
        };
        let support: Vec<serde_json::Value> = tsv_rows("report/support.tsv")?
            .iter()
            .map(|r| {
                serde_json::json!({
                    "clade": r[6].split(',').collect::<Vec<_>>(),
                    "s1": num(&r[2]),
                    "quartets": r[5].parse::<u64>().unwrap_or(0),
                })
            })
            .collect();
        let halo: Vec<serde_json::Value> = tsv_rows("report/halo.tsv")?
            .iter()
            .map(|r| serde_json::json!({ "taxon": r[0], "halo": num(&r[1]) }))
            .collect();
        let mut results = serde_json::json!({
            "tree": read_text("report/tree.nwk")?,
            "tree_with_s1": read_text("trees/tree_s1.nwk")?,
            "weights": source,
            "amalgamation": amalgamation,
            "support_s1": support,
            "halo": halo,
            "metrics": {},
        });
        if self.bootstrapped() {
            let s2: Vec<serde_json::Value> = tsv_rows("report/bootstrap.tsv")?
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "clade": r[4].split(',').collect::<Vec<_>>(),
                        "s2": num(&r[2]),
                        "replicates": r[3].parse::<u64>().unwrap_or(0),
                    })
                })
                .collect();
            results["tree_with_s2"] = read_text("trees/tree_s2.nwk")?.into();
            results["bootstrap"] = serde_json::json!({
                "replicates": self.options.config.bootstrap,
                "weights": "w2b",
                "column_weights": "Poisson(1), seed replicate_seed(global, b)",
            });
            results["support_s2"] = s2.into();
        }
        Ok(vec![
            (
                "audit/results.json",
                (serde_json::to_string_pretty(&results).unwrap() + "\n").into_bytes(),
            ),
            ("audit/quartet_decisions.bin.zst", decisions),
            ("audit/sample_worksheets.txt", text.into_bytes()),
        ])
    }

    fn finalize(&mut self, m: usize) -> Result<(), EngineError> {
        self.set_running(FINALIZE)?;
        // Content hash of each stage.
        let file_hash =
            |rel: &str| -> Result<String, EngineError> { Ok(sha256_hex(&self.read(rel)?)) };
        let mut maws = StreamHasher::new();
        for t in 0..m {
            let p = maw_file(&self.dir, t);
            maws.update(&atomic::read_verified(&p).ok_or_else(|| {
                EngineError::Invalid(format!("{} is missing or damaged", p.display()))
            })?);
        }
        let read_chunk = |p: &Path| {
            atomic::read_verified(p).ok_or_else(|| {
                EngineError::Invalid(format!("{} is missing or damaged", p.display()))
            })
        };
        let mut summaries = self.stage_summaries(m)?;
        let plan = self.state.chunk_plans[QUARTET_COUNT];
        let mut counts = StreamHasher::new();
        let mut chunk_list = Vec::new();
        // Quartets with no split pattern (uninformative for W1) and with no
        // counted column (no W2 fit).
        let (mut no_split, mut no_column) = (0u64, 0u64);
        for i in 0..plan.chunk_count() {
            let bytes = read_chunk(&self.dir.chunk_file(QUARTET_COUNT, i))?;
            for (_, c) in
                store::count_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?
            {
                if weight::SUPPORTING.iter().flatten().all(|&x| c[x] == 0) {
                    no_split += 1;
                }
                if c[1..].iter().all(|&x| x == 0) {
                    no_column += 1;
                }
            }
            counts.update(&bytes);
            let (s, e) = plan.range(i);
            chunk_list.push(serde_json::json!({
                "index": i, "start": s, "end": e, "sha256": sha256_hex(&bytes)
            }));
        }
        let mut stage_hashes = vec![
            (INGEST, ingest_content_hash(&self.inputs()?)),
            (MAW_EXTRACT, maws.finish_hex()),
            (LENGTH_SELECT, file_hash("work/matrix/selection.json")?),
            (MATRIX_BUILD, file_hash("work/matrix/m_full.bin")?),
            (QUARTET_COUNT, counts.finish_hex()),
        ];
        let mut chunk_stages = vec![serde_json::json!({
            "stage": QUARTET_COUNT,
            "units_per_chunk": plan.units_per_chunk,
            "chunks": chunk_list,
        })];
        if self.weighted() {
            // Floating-point content is hashed as text rounded to 9
            // significant digits (determinism contract).
            let plan = self.state.chunk_plans[QUARTET_WEIGHT];
            let mut weights = StreamHasher::new();
            let mut chunk_list = Vec::new();
            let (mut fitted, mut resampled) = (0u64, 0u64);
            for i in 0..plan.chunk_count() {
                let bytes = read_chunk(&self.dir.chunk_file(QUARTET_WEIGHT, i))?;
                let recs = store::weight_records(&bytes)
                    .map_err(|e| EngineError::Invalid(e.to_string()))?;
                let mut text = String::with_capacity(recs.len() * 120);
                for r in &recs {
                    text.push_str(&r.canonical());
                    fitted += r.fitted() as u64;
                    resampled += r.resampled() as u64;
                }
                weights.update(text.as_bytes());
                let (s, e) = plan.range(i);
                chunk_list.push(serde_json::json!({
                    "index": i, "start": s, "end": e, "sha256": sha256_hex(&bytes),
                    "rounded_sha256": sha256_hex(text.as_bytes())
                }));
            }
            stage_hashes.push((QUARTET_WEIGHT, weights.finish_hex()));
            chunk_stages.push(serde_json::json!({
                "stage": QUARTET_WEIGHT,
                "units_per_chunk": plan.units_per_chunk,
                "chunks": chunk_list,
            }));
            let mut tree = StreamHasher::new();
            tree.update(&self.read("report/tree.nwk")?);
            tree.update(&self.read("work/amalgamation.json")?);
            stage_hashes.push((AMALGAMATE, tree.finish_hex()));
            let mut support = StreamHasher::new();
            for rel in ["trees/tree_s1.nwk", "report/support.tsv", "report/halo.tsv"] {
                support.update(&self.read(rel)?);
            }
            stage_hashes.push((SUPPORT, support.finish_hex()));
            summaries.insert(
                QUARTET_WEIGHT,
                serde_json::json!({
                    "fitted_quartets": fitted,
                    "resampled_quartets": resampled,
                    "quartets_without_weight": quartet::quartet_count(m) - fitted,
                }),
            );
        }
        if self.bootstrapped() {
            // Each replicate tree is a unit; its hash is that of its line
            // in trees/bootstrap_trees.nwk (with the newline), so quick
            // verification needs no work folder.
            let trees = self.read("trees/bootstrap_trees.nwk")?;
            let text = String::from_utf8_lossy(&trees);
            let chunk_list: Vec<serde_json::Value> = text
                .lines()
                .enumerate()
                .map(|(b, line)| {
                    serde_json::json!({
                        "index": b, "start": b, "end": b + 1,
                        "sha256": sha256_hex(format!("{line}\n").as_bytes()),
                        "seed": qmaws_core::bootstrap::replicate_seed(self.options.config.seed, b as u64),
                    })
                })
                .collect();
            chunk_stages.push(serde_json::json!({
                "stage": BOOTSTRAP,
                "units_per_chunk": 1,
                "chunks": chunk_list,
            }));
            let mut boot = StreamHasher::new();
            boot.update(&trees);
            for rel in ["trees/tree_s2.nwk", "report/bootstrap.tsv"] {
                boot.update(&self.read(rel)?);
            }
            stage_hashes.push((BOOTSTRAP, boot.finish_hex()));
        }
        summaries.insert(
            QUARTET_COUNT,
            serde_json::json!({
                "quartets": quartet::quartet_count(m),
                "quartets_without_split_pattern": no_split,
                "quartets_without_counted_column": no_column,
            }),
        );
        let mut outputs = BTreeMap::new();
        if self.weighted() {
            for (rel, bytes) in self.audit_files(m)? {
                outputs.insert(rel.to_string(), self.write(rel, &bytes)?);
            }
        }
        let env = serde_json::to_string_pretty(&environment(&self.options.config)).unwrap() + "\n";
        outputs.insert(
            "audit/environment.json".to_string(),
            self.write("audit/environment.json", env.as_bytes())?,
        );
        let mut root_text = format!(
            "qmaws-root-v2\nkind {KIND}\nconfig {}\n",
            root_config_hash(&self.options.config)
        );
        for (stage, h) in &stage_hashes {
            root_text.push_str(&format!("stage {stage} {h}\n"));
        }
        let root = sha256_hex(root_text.as_bytes());
        // Times and summary numbers are not part of the root: the times
        // describe this device, the numbers repeat hashed content.
        let stage_rows: Vec<serde_json::Value> = stage_hashes
            .iter()
            .map(|(s, h)| {
                let st = self.state.stage(s);
                serde_json::json!({
                    "stage": s,
                    "content_sha256": h,
                    "started_utc": st.and_then(|x| x.started_utc.clone()),
                    "finished_utc": st.and_then(|x| x.finished_utc.clone()),
                    "summary": summaries.get(s).cloned().unwrap_or_else(|| serde_json::json!({})),
                })
            })
            .collect();
        let stages_json = serde_json::json!({
            "stages": stage_rows,
            "quartets": quartet::quartet_count(m),
            "seed": self.options.config.seed,
        });
        let chunks_json = serde_json::json!({ "stages": chunk_stages });
        for (rel, text) in [
            (
                "audit/stages.json",
                serde_json::to_string_pretty(&stages_json).unwrap() + "\n",
            ),
            (
                "audit/chunks.json",
                serde_json::to_string_pretty(&chunks_json).unwrap() + "\n",
            ),
            ("audit/root.txt", format!("{root}\n")),
        ] {
            outputs.insert(rel.to_string(), self.write(rel, text.as_bytes())?);
        }
        self.set_done(FINALIZE, outputs)
    }
}

/// Weights of every quartet in rank order (`None`: no weight).
type TreeWeights = Vec<Option<[f64; 3]>>;

/// Halo values below this are counted in the log (the threshold of the
/// Halo Tree figure).
const LOW_HALO: f64 = 0.6;

/// zstd level of `audit/quartet_decisions.bin.zst`.
const DECISIONS_LEVEL: i32 = 19;

/// Number of quartets in `audit/sample_worksheets.txt`.
pub const SAMPLE_WORKSHEETS: usize = 50;

/// Count records of processing positions `s..e`.
pub(crate) fn count_positions(
    full: &Matrix,
    cc: &CoCounts,
    path: PopcountPath,
    perm: &Permutation,
    s: u64,
    e: u64,
) -> Vec<u8> {
    let mut part = Vec::with_capacity(((e - s) as usize) * store::COUNT_RECORD);
    for pos in s..e {
        let r = perm.apply(pos);
        let quad = quartet::unrank(r);
        let n4 = path.and4(
            &full.rows[quad[0]],
            &full.rows[quad[1]],
            &full.rows[quad[2]],
            &full.rows[quad[3]],
        );
        store::push_count_record(&mut part, r, &quartet::pattern_counts(cc, quad, n4));
    }
    part
}

/// Everything needed to weigh quartets of a run.
pub(crate) struct Weigher<'a> {
    pub full: &'a Matrix,
    pub cc: &'a CoCounts,
    pub path: PopcountPath,
    pub perm: &'a Permutation,
    pub model: Model,
    pub seed: u64,
    pub replicates: u32,
}

impl Weigher<'_> {
    pub fn counts(&self, quad: [usize; 4]) -> quartet::PatternCounts {
        let r = &self.full.rows;
        let n4 = self
            .path
            .and4(&r[quad[0]], &r[quad[1]], &r[quad[2]], &r[quad[3]]);
        quartet::pattern_counts(self.cc, quad, n4)
    }

    /// The weight record of the quartet with rank `r`.
    pub fn record(&self, r: u64) -> store::WeightRecord {
        let counts = self.counts(quartet::unrank(r));
        let cond = Conditioning::NotAllZero;
        let mut rec = store::WeightRecord {
            rank: r,
            flags: 0,
            log_likelihoods: [0.0; 3],
            w2c: [0.0; 3],
        };
        if let Some(fits) = weight::fit_all(&self.model, cond, &counts) {
            rec.flags |= store::WEIGHT_FITTED;
            rec.log_likelihoods = fits.map(|f| f.log_likelihood);
            let qs = weight::quartet_seed(self.seed, r);
            if let Some(w) = weight::w2c(&self.model, cond, &counts, qs, self.replicates) {
                rec.flags |= store::WEIGHT_RESAMPLED;
                rec.w2c = w;
            }
        }
        rec
    }

    /// Weight records of processing positions `s..e`.
    pub fn positions(&self, s: u64, e: u64) -> Vec<u8> {
        let mut part = Vec::with_capacity(((e - s) as usize) * store::WEIGHT_RECORD);
        for pos in s..e {
            store::push_weight_record(&mut part, &self.record(self.perm.apply(pos)));
        }
        part
    }
}

/// Records of positions `s..e`, computed in parallel blocks of `block`
/// positions and joined in order, so the bytes do not depend on threads.
pub(crate) fn run_blocks(
    block: u64,
    s: u64,
    e: u64,
    range: &(dyn Fn(u64, u64) -> Vec<u8> + Sync),
) -> Vec<u8> {
    let blocks: Vec<(u64, u64)> = (s..e)
        .step_by(block as usize)
        .map(|b| (b, (b + block).min(e)))
        .collect();
    let parts: Vec<Vec<u8>> = blocks.par_iter().map(|&(b0, b1)| range(b0, b1)).collect();
    parts.concat()
}

/// The substitution model of a run's weighting.
pub(crate) fn weight_model(config: &AnalysisConfig, full: &Matrix) -> Result<Model, EngineError> {
    if config.weighting != WEIGHTING_EMP {
        return Ok(Model::symmetric());
    }
    let ones: u64 = full.ones.iter().map(|&o| o as u64).sum();
    let cells = full.taxa as f64 * full.columns_len() as f64;
    let pi1 = ones as f64 / cells;
    if !(pi1 > 0.0 && pi1 < 1.0) {
        return Err(EngineError::Invalid(
            "W2-emp needs both 0 and 1 in the full matrix".into(),
        ));
    }
    Ok(Model::with_frequency_of_one(pi1))
}

/// Name of a run's weighting model for worksheets.
pub(crate) fn model_name(config: &AnalysisConfig, model: &Model) -> String {
    if config.weighting == WEIGHTING_EMP {
        format!("W2-emp, frequency of 1 = {:.6}", model.pi[1])
    } else {
        "W2-sym".into()
    }
}

/// Ranks of the sample worksheets: a seeded draw of distinct ranks, sorted.
pub fn sample_ranks(seed: u64, q: u64, count: usize) -> Vec<u64> {
    if q <= count as u64 {
        return (0..q).collect();
    }
    // A stream of its own: rank u64::MAX never occurs as a quartet.
    let mut rng = weight::SplitMix64::new(weight::quartet_seed(seed, u64::MAX));
    let mut set = std::collections::BTreeSet::new();
    while set.len() < count {
        set.insert(rng.next_u64() % q);
    }
    set.into_iter().collect()
}

/// The worksheet text of quartet `quad` of a run, with its weights.
pub(crate) fn worksheet_of(
    config: &AnalysisConfig,
    names: &[String],
    weigher: &Weigher,
    quad: [usize; 4],
) -> (String, weight::QuartetWeights) {
    let r = &weigher.full.rows;
    let n4 = weigher
        .path
        .and4(&r[quad[0]], &r[quad[1]], &r[quad[2]], &r[quad[3]]);
    let n = worksheet::subset_counts(weigher.cc, quad, n4);
    let name = model_name(config, &weigher.model);
    let settings = worksheet::WeightSettings {
        model: &weigher.model,
        model_name: &name,
        conditioning: Conditioning::NotAllZero,
        seed: weigher.seed,
        replicates: weigher.replicates,
    };
    let (text, _, weights) = worksheet::quartet_worksheet(
        [
            &names[quad[0]],
            &names[quad[1]],
            &names[quad[2]],
            &names[quad[3]],
        ],
        quad,
        &n,
        &settings,
    );
    (text, weights)
}

/// Commit of the source the program was built from.
pub const GIT_COMMIT: &str = env!("QMAWS_GIT_COMMIT");

/// Program, device and settings of a run (`audit/environment.json`).
fn environment(config: &AnalysisConfig) -> serde_json::Value {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let cpu = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_default();
    serde_json::json!({
        "program": "qmaws",
        "version": env!("CARGO_PKG_VERSION"),
        "git_commit": GIT_COMMIT,
        "os": std::env::consts::OS,
        "os_version": sysinfo::System::long_os_version().unwrap_or_default(),
        "architecture": std::env::consts::ARCH,
        "cpu": cpu,
        "logical_cores": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "ram_bytes": sys.total_memory(),
        "popcount": format!("{:?}", PopcountPath::detect()),
        "settings": config,
        "seeds": {
            "global": config.seed,
            "processing_order": config.seed,
            "w2c": "per quartet: quartet_seed(global, rank)",
            "sample_worksheets": "quartet_seed(global, 2^64 - 1)",
            "bootstrap_replicate": "replicate_seed(global, b) = quartet_seed(global, 2^63 + b)",
        },
    })
}

/// SHA-256 of the configuration without the input location, for the root
/// fingerprint: the inputs enter the root through their content.
pub(crate) fn root_config_hash(config: &AnalysisConfig) -> String {
    let mut c = config.clone();
    c.input = String::new();
    let value = serde_json::to_value(&c).expect("configuration serialises");
    sha256_hex(value.to_string().as_bytes())
}

/// Content hash of the ingest stage for the root fingerprint:
/// `audit/inputs.json` with each file path reduced to its file name, so the
/// root does not depend on where the data are kept.
pub(crate) fn ingest_content_hash(inputs: &InputsRecord) -> String {
    let mut r = inputs.clone();
    for f in &mut r.files {
        f.path = Path::new(&f.path.replace('\\', "/"))
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    let text = serde_json::to_string_pretty(&r).expect("inputs serialise") + "\n";
    sha256_hex(text.as_bytes())
}

/// A number rounded to 9 significant digits (determinism contract).
fn sig9(v: f64) -> serde_json::Value {
    let rounded: f64 = format!("{v:.8e}").parse().unwrap_or(v);
    serde_json::json!(rounded)
}

/// Reads every quartet record of a finished (or partly finished) run, in
/// processing order. For tests and inspection.
pub fn read_counts(run_dir: &Path) -> Result<Vec<(u64, [u32; 16])>, EngineError> {
    let dir = RunDir::new(run_dir);
    let state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    let Some(plan) = state.chunk_plans.get(QUARTET_COUNT) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for i in 0..plan.chunk_count() {
        let p = dir.chunk_file(QUARTET_COUNT, i);
        if let Some(bytes) = atomic::read_verified(&p) {
            out.extend(
                store::count_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?,
            );
        }
    }
    Ok(out)
}

/// Taxon names and the full matrix of a run whose matrix stage is done.
pub fn load_matrix(run_dir: &Path) -> Result<(Vec<String>, Matrix), EngineError> {
    let dir = RunDir::new(run_dir);
    let read = |p: PathBuf| {
        atomic::read_verified(&p)
            .ok_or_else(|| EngineError::Invalid(format!("{} is missing or damaged", p.display())))
    };
    let inputs: InputsRecord = serde_json::from_slice(&read(dir.audit().join("inputs.json"))?)
        .map_err(|e| EngineError::Invalid(format!("audit/inputs.json: {e}")))?;
    let full = store::matrix_from_bytes(&read(dir.work().join("matrix").join("m_full.bin"))?)
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    Ok((inputs.taxa.into_iter().map(|t| t.name).collect(), full))
}

/// Reads every weight record of a finished (or partly finished) run, in
/// processing order. For tests and inspection.
pub fn read_weights(run_dir: &Path) -> Result<Vec<store::WeightRecord>, EngineError> {
    let dir = RunDir::new(run_dir);
    let state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    let Some(plan) = state.chunk_plans.get(QUARTET_WEIGHT) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for i in 0..plan.chunk_count() {
        let p = dir.chunk_file(QUARTET_WEIGHT, i);
        if let Some(bytes) = atomic::read_verified(&p) {
            out.extend(
                store::weight_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?,
            );
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::NullSink;
    use crate::testutil::TempDir;
    use std::cell::Cell;

    fn write_inputs(dir: &Path, m: usize, len: usize, seed: u64) {
        std::fs::create_dir_all(dir).unwrap();
        let mut x = seed;
        let mut next = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 33) as usize
        };
        // A common ancestor with independent mutations, so taxa are related.
        let base: Vec<u8> = (0..len).map(|_| b"ACGT"[next() % 4]).collect();
        for t in 0..m {
            let mut s = base.clone();
            for _ in 0..len / 20 {
                let i = next() % len;
                s[i] = b"ACGT"[next() % 4];
            }
            std::fs::write(
                dir.join(format!("T{t}.fasta")),
                format!(">T{t}\n{}\n", String::from_utf8(s).unwrap()),
            )
            .unwrap();
        }
    }

    fn options(input: &Path, chunk: u64) -> AnalysisOptions {
        AnalysisOptions {
            config: AnalysisConfig {
                input: input.display().to_string(),
                records: "per_file".into(),
                strand: true,
                lengths: None,
                seed: 7,
                ml_max_columns: matrix::MAX_ML_COLUMNS,
                weighting: WEIGHTING_SYM.into(),
                replicates: 3,
                bootstrap: 0,
            },
            chunk_seconds: 3.0,
            chunk_quartets: Some(chunk),
            memory_limit: Some(1 << 30),
            live_tree: false,
        }
    }

    fn finish(dir: &Path, opts: &AnalysisOptions) -> String {
        let cancel = AtomicBool::new(false);
        match start(dir, opts, "terminal", &NullSink, &cancel).unwrap() {
            Outcome::Finished { root } => root,
            Outcome::Stopped => panic!("stopped"),
        }
    }

    struct StopAfter<'a> {
        left: Cell<u32>,
        cancel: &'a AtomicBool,
    }
    impl ProgressSink for StopAfter<'_> {
        fn event(&self, e: &Event) {
            if let Event::Progress(_) = e {
                match self.left.get() {
                    0 => self.cancel.store(true, Ordering::SeqCst),
                    n => self.left.set(n - 1),
                }
            }
        }
    }

    #[test]
    fn analysis_counts_every_quartet_once_and_matches_the_kernel() {
        let tmp = TempDir::new("analysis_counts");
        let input = tmp.path().join("in");
        write_inputs(&input, 7, 800, 3);
        let run = tmp.path().join("run");
        finish(&run, &options(&input, 4));
        let recs = read_counts(&run).unwrap();
        let q = quartet::quartet_count(7);
        assert_eq!(recs.len() as u64, q);
        let mut ranks: Vec<u64> = recs.iter().map(|r| r.0).collect();
        ranks.sort_unstable();
        assert_eq!(ranks, (0..q).collect::<Vec<_>>());
        // Processing order is a permutation, not rank order.
        assert_ne!(
            recs.iter().map(|r| r.0).collect::<Vec<_>>(),
            (0..q).collect::<Vec<_>>()
        );
        // Counts equal a direct scan of the stored matrix.
        let dir = RunDir::new(&run);
        let full = store::matrix_from_bytes(
            &atomic::read_verified(&dir.work().join("matrix/m_full.bin")).unwrap(),
        )
        .unwrap();
        for (r, c) in &recs {
            let quad = quartet::unrank(*r);
            let brute = quartet::pattern_counts_brute(&full.rows, full.columns_len(), quad);
            let got: Vec<u64> = c.iter().map(|&x| x as u64).collect();
            assert_eq!(got, brute.to_vec());
        }
    }

    #[test]
    fn root_does_not_depend_on_chunks_and_stop_resume_gives_the_same_root() {
        let tmp = TempDir::new("analysis_resume");
        let input = tmp.path().join("in");
        write_inputs(&input, 8, 600, 11);
        let a = finish(&tmp.path().join("a"), &options(&input, 3));
        let b = finish(&tmp.path().join("b"), &options(&input, 70));
        assert_eq!(a, b);

        for stop_after in [0u32, 2, 5, 12] {
            let dir = tmp.path().join(format!("stopped_{stop_after}"));
            let cancel = AtomicBool::new(false);
            let sink = StopAfter {
                left: Cell::new(stop_after),
                cancel: &cancel,
            };
            let first = start(&dir, &options(&input, 3), "terminal", &sink, &cancel).unwrap();
            if first == Outcome::Stopped {
                let cancel = AtomicBool::new(false);
                let again =
                    crate::runner::resume_run(&dir, "terminal", &NullSink, &cancel).unwrap();
                assert_eq!(again, Outcome::Finished { root: a.clone() });
            } else {
                assert_eq!(first, Outcome::Finished { root: a.clone() });
            }
        }
    }

    /// Collects the events of a run.
    #[derive(Default)]
    struct Collect(std::cell::RefCell<Vec<Event>>);
    impl ProgressSink for Collect {
        fn event(&self, e: &Event) {
            self.0.borrow_mut().push(e.clone());
        }
    }

    #[test]
    fn live_tree_writes_frames_figures_and_convergence_without_changing_the_root() {
        let tmp = TempDir::new("analysis_live");
        let input = tmp.path().join("in");
        write_inputs(&input, 8, 600, 11);
        let plain = finish(&tmp.path().join("plain"), &options(&input, 3));
        let run = tmp.path().join("live");
        let live = AnalysisOptions {
            live_tree: true,
            ..options(&input, 3)
        };
        let sink = Collect::default();
        let cancel = AtomicBool::new(false);
        let outcome = start(&run, &live, "gui", &sink, &cancel).unwrap();
        assert_eq!(outcome, Outcome::Finished { root: plain });
        let events = sink.0.into_inner();
        // One live quartet per weighing chunk: 70 quartets in chunks of 3.
        let quartets: Vec<&LiveQuartet> = events
            .iter()
            .filter_map(|e| match e {
                Event::Quartet(q) => Some(q.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(quartets.len(), 24);
        let last = quartets.last().unwrap();
        assert_eq!((last.quartets_done, last.quartets_total), (70, 70));
        assert!(last.counts.iter().sum::<u64>() > 0);
        assert!(last.w1.is_some() && last.weights.is_some());
        let frames: Vec<&ProvisionalTree> = events
            .iter()
            .filter_map(|e| match e {
                Event::Provisional(p) => Some(p),
                _ => None,
            })
            .collect();
        let dir = RunDir::new(&run);
        let state = provisional::load(&dir).unwrap();
        // Chunks of 3 of 70 quartets take milliseconds, so updates exceed
        // the overhead budget and the interval doubles from 5% each time.
        assert!(frames.len() >= 3, "{} frames", frames.len());
        assert_eq!(state.frames.len(), frames.len());
        let doublings = (state.interval_percent / provisional::INTERVAL_PERCENT).log2() as usize;
        let log = std::fs::read_to_string(run.join("run.log")).unwrap();
        assert_eq!(log.matches("updating every").count(), doublings);
        for (i, f) in frames.iter().enumerate() {
            assert_eq!(f.frame as usize, i + 1);
            assert!(f.percent < 100.0);
            assert_eq!(f.halo.len(), 8);
            assert!(provisional::frame_file(&dir, f.frame).exists());
        }
        for ext in ["svg", "png", "pdf"] {
            assert!(provisional::latest_file(&dir, ext).exists(), "{ext}");
        }
        let svg = std::fs::read_to_string(provisional::latest_file(&dir, "svg")).unwrap();
        assert!(svg.contains("PROVISIONAL"));
        let csv = std::fs::read_to_string(dir.report().join("convergence.csv")).unwrap();
        assert_eq!(csv.lines().count(), frames.len() + 1);
        // The run without a live tree has none of these files.
        let plain = RunDir::new(tmp.path().join("plain"));
        assert!(!provisional::latest_file(&plain, "svg").exists());
        assert!(!plain.report().join("convergence.csv").exists());
    }

    #[test]
    fn live_tree_continues_after_a_resume() {
        let tmp = TempDir::new("analysis_live_resume");
        let input = tmp.path().join("in");
        write_inputs(&input, 8, 600, 11);
        let run = tmp.path().join("run");
        let live = AnalysisOptions {
            live_tree: true,
            ..options(&input, 3)
        };
        let cancel = AtomicBool::new(false);
        // Stop inside quartet_weight: after the counting chunks and a few
        // weighing chunks.
        let sink = StopAfter {
            left: Cell::new(40),
            cancel: &cancel,
        };
        let first = start(&run, &live, "terminal", &sink, &cancel).unwrap();
        assert_eq!(first, Outcome::Stopped);
        let dir = RunDir::new(&run);
        let before = provisional::load(&dir).unwrap().frames.len();
        assert!(before > 0);
        let cancel = AtomicBool::new(false);
        let again = crate::runner::resume_run(&run, "gui", &NullSink, &cancel).unwrap();
        assert!(matches!(again, Outcome::Finished { .. }));
        let state = provisional::load(&dir).unwrap();
        assert!(state.frames.len() > before);
        let numbers: Vec<u32> = state.frames.iter().map(|f| f.frame).collect();
        assert_eq!(numbers, (1..=state.frames.len() as u32).collect::<Vec<_>>());
        let percents: Vec<f64> = state.frames.iter().map(|f| f.percent).collect();
        assert!(percents.windows(2).all(|w| w[0] < w[1]));
    }

    /// Asks for a pause on the first `polls` checks.
    struct PauseFor {
        polls: Cell<u32>,
    }
    impl ProgressSink for PauseFor {
        fn event(&self, _e: &Event) {}
        fn pause_requested(&self) -> bool {
            let n = self.polls.get();
            self.polls.set(n.saturating_sub(1));
            n > 0
        }
    }

    #[test]
    fn pause_waits_and_continues_with_the_same_root() {
        let tmp = TempDir::new("analysis_pause");
        let input = tmp.path().join("in");
        write_inputs(&input, 7, 500, 2);
        let plain = finish(&tmp.path().join("plain"), &options(&input, 4));
        let run = tmp.path().join("paused");
        let sink = PauseFor {
            polls: Cell::new(4),
        };
        let cancel = AtomicBool::new(false);
        let outcome = start(&run, &options(&input, 4), "gui", &sink, &cancel).unwrap();
        assert_eq!(outcome, Outcome::Finished { root: plain });
        let log = std::fs::read_to_string(run.join("run.log")).unwrap();
        assert!(log.contains("Paused.") && log.contains("Continuing."));
    }

    /// Always asks for a pause; records when it was first asked.
    #[derive(Default)]
    struct PauseForever {
        first: std::sync::OnceLock<std::time::Instant>,
    }
    impl ProgressSink for PauseForever {
        fn event(&self, _e: &Event) {}
        fn pause_requested(&self) -> bool {
            self.first.get_or_init(std::time::Instant::now);
            true
        }
    }

    #[test]
    fn stop_during_a_pause_stops_the_run() {
        let tmp = TempDir::new("analysis_pause_stop");
        let input = tmp.path().join("in");
        write_inputs(&input, 6, 500, 2);
        let run = tmp.path().join("run");
        let cancel = AtomicBool::new(false);
        let sink = PauseForever::default();
        let pause = std::time::Duration::from_millis(500);
        let started = std::time::Instant::now();
        let outcome = std::thread::scope(|scope| {
            scope.spawn(|| {
                // Stop half a second after the pause began.
                while sink.first.get().is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                std::thread::sleep(pause);
                cancel.store(true, Ordering::SeqCst);
            });
            start(&run, &options(&input, 4), "gui", &sink, &cancel).unwrap()
        });
        let wall = started.elapsed().as_secs_f64();
        assert_eq!(outcome, Outcome::Stopped);
        let state = RunState::load(&RunDir::new(&run).run_json()).unwrap();
        // Working time excludes the pause: at most the wall time minus the
        // half second paused (with 50 ms for the polling interval).
        assert!(
            state.elapsed_seconds <= wall - pause.as_secs_f64() + 0.05,
            "working time {} s of {wall} s",
            state.elapsed_seconds
        );
        let log = std::fs::read_to_string(run.join("run.log")).unwrap();
        assert!(log.contains("Paused.") && log.contains("Stopped on request"));
    }

    #[test]
    fn damaged_files_are_recomputed_on_resume() {
        let tmp = TempDir::new("analysis_damage");
        let input = tmp.path().join("in");
        write_inputs(&input, 6, 500, 5);
        let dir = tmp.path().join("r");
        let root = finish(&dir, &options(&input, 4));
        let run = RunDir::new(&dir);
        std::fs::write(maw_file(&run, 2), b"damaged").unwrap();
        std::fs::remove_file(run.chunk_file(QUARTET_COUNT, 1)).unwrap();
        std::fs::write(run.chunk_file(QUARTET_WEIGHT, 0), b"damaged").unwrap();
        std::fs::write(run.root().join("work/matrix/m_full.bin"), b"x").unwrap();
        let cancel = AtomicBool::new(false);
        let again = crate::runner::resume_run(&dir, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(again, Outcome::Finished { root });
    }

    #[test]
    fn bootstrap_stage_gives_s2_and_enters_the_root() {
        let tmp = TempDir::new("analysis_bootstrap");
        let input = tmp.path().join("in");
        write_inputs(&input, 7, 600, 13);
        let plain = finish(&tmp.path().join("plain"), &options(&input, 6));
        let mut opts = options(&input, 6);
        opts.config.bootstrap = 4;
        let dir = tmp.path().join("boot");
        let root = finish(&dir, &opts);
        assert_ne!(root, plain, "the bootstrap stage is part of the root");
        let run = RunDir::new(&dir);
        let text = |rel: &str| {
            String::from_utf8(atomic::read_verified(&run.root().join(rel)).unwrap()).unwrap()
        };
        // Replicate trees: one per line, each a W2b replicate of the run.
        let trees: Vec<String> = text("trees/bootstrap_trees.nwk")
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(trees.len(), 4);
        for (b, t) in trees.iter().enumerate() {
            let again = crate::bootstrap::replicate(&dir, b as u64, false).unwrap();
            assert_eq!(&again.newick, t);
        }
        // S2 per edge of the run's tree, in the order of support.tsv.
        let tree = text("report/tree.nwk");
        let (edges, labelled) = support::split_frequencies(tree.trim(), &trees).unwrap();
        let s2_rows: Vec<String> = text("report/bootstrap.tsv")
            .lines()
            .skip(1)
            .map(str::to_string)
            .collect();
        let s1_rows: Vec<String> = text("report/support.tsv")
            .lines()
            .skip(1)
            .map(str::to_string)
            .collect();
        assert_eq!(s2_rows.len(), edges.len());
        assert_eq!(s2_rows.len(), s1_rows.len());
        for (row, (e, s1)) in s2_rows.iter().zip(edges.iter().zip(&s1_rows)) {
            let cols: Vec<&str> = row.split('\t').collect();
            assert_eq!(cols[2], format!("{:.6}", e.s2));
            assert_eq!(cols[4], s1.split('\t').nth(6).unwrap());
        }
        assert_eq!(text("trees/tree_s2.nwk").trim(), labelled);
        let results: serde_json::Value = serde_json::from_str(&text("audit/results.json")).unwrap();
        assert_eq!(results["bootstrap"]["replicates"], 4);
        assert_eq!(results["support_s2"].as_array().unwrap().len(), edges.len());
        // chunks.json lists each replicate with its seed and tree hash.
        let chunks: serde_json::Value = serde_json::from_str(&text("audit/chunks.json")).unwrap();
        let boot = chunks["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["stage"] == BOOTSTRAP)
            .unwrap();
        assert_eq!(boot["chunks"].as_array().unwrap().len(), 4);
        assert_eq!(
            boot["chunks"][1]["sha256"].as_str().unwrap(),
            sha256_hex(format!("{}\n", trees[1]).as_bytes())
        );
        // A lost replicate and damaged S2 files are recomputed on resume.
        std::fs::remove_file(replicate_file(&run, 2)).unwrap();
        std::fs::write(run.root().join("report/bootstrap.tsv"), b"damaged").unwrap();
        let cancel = AtomicBool::new(false);
        let again = crate::runner::resume_run(&dir, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(again, Outcome::Finished { root });
        // Without S2, run.json has no bootstrap setting at all.
        let plain_state = std::fs::read_to_string(tmp.path().join("plain/run.json")).unwrap();
        let plain_state: serde_json::Value = serde_json::from_str(&plain_state).unwrap();
        assert!(plain_state["config"].get("bootstrap").is_none());
        let boot_state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(boot_state["config"]["bootstrap"], 4);
    }

    #[test]
    fn stages_json_has_times_and_summaries() {
        let tmp = TempDir::new("analysis_stages");
        let input = tmp.path().join("in");
        write_inputs(&input, 6, 500, 17);
        let dir = tmp.path().join("r");
        finish(&dir, &options(&input, 4));
        let stages: serde_json::Value = serde_json::from_slice(
            &atomic::read_verified(&RunDir::new(&dir).audit().join("stages.json")).unwrap(),
        )
        .unwrap();
        let rows = stages["stages"].as_array().unwrap();
        assert_eq!(rows.len(), 8);
        for r in rows {
            assert!(
                r["started_utc"].as_str().is_some_and(|t| t.ends_with('Z')),
                "{r}"
            );
            assert!(
                r["finished_utc"].as_str().is_some_and(|t| t.ends_with('Z')),
                "{r}"
            );
            assert!(r["content_sha256"].as_str().unwrap().len() == 64);
        }
        let summary = |stage: &str| &rows.iter().find(|r| r["stage"] == stage).unwrap()["summary"];
        assert_eq!(summary(INGEST)["taxa"], 6);
        assert_eq!(
            summary(MAW_EXTRACT)["maws_per_taxon"]
                .as_object()
                .unwrap()
                .len(),
            6
        );
        assert!(!summary(LENGTH_SELECT)["selected_lengths"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(summary(MATRIX_BUILD)["m_full_columns"].as_u64().unwrap() > 0);
        let q = quartet::quartet_count(6);
        assert_eq!(summary(QUARTET_COUNT)["quartets"], q);
        let w = summary(QUARTET_WEIGHT);
        assert_eq!(
            w["fitted_quartets"].as_u64().unwrap() + w["quartets_without_weight"].as_u64().unwrap(),
            q
        );
        assert!(summary(SUPPORT)["internal_edges"].as_u64().unwrap() > 0);
    }

    #[test]
    fn a_changed_input_stops_the_resume() {
        let tmp = TempDir::new("analysis_changed");
        let input = tmp.path().join("in");
        write_inputs(&input, 5, 400, 9);
        let dir = tmp.path().join("r");
        let cancel = AtomicBool::new(false);
        let sink = StopAfter {
            left: Cell::new(0),
            cancel: &cancel,
        };
        let _ = start(&dir, &options(&input, 2), "terminal", &sink, &cancel).unwrap();
        std::fs::write(input.join("T0.fasta"), ">T0\nACGTACGT\n").unwrap();
        let cancel = AtomicBool::new(false);
        let err = crate::runner::resume_run(&dir, "terminal", &NullSink, &cancel).unwrap_err();
        assert!(matches!(err, EngineError::InputChanged(_)), "{err}");
    }

    #[test]
    fn stored_weights_equal_a_direct_computation_with_per_quartet_seeds() {
        let tmp = TempDir::new("analysis_weights");
        let input = tmp.path().join("in");
        write_inputs(&input, 7, 800, 3);
        for weighting in [WEIGHTING_SYM, WEIGHTING_EMP] {
            let run = tmp.path().join(weighting);
            let mut opts = options(&input, 6);
            opts.config.weighting = weighting.into();
            finish(&run, &opts);
            let counts = read_counts(&run).unwrap();
            let weights = read_weights(&run).unwrap();
            assert_eq!(weights.len(), counts.len());
            let full = store::matrix_from_bytes(
                &atomic::read_verified(&RunDir::new(&run).work().join("matrix/m_full.bin"))
                    .unwrap(),
            )
            .unwrap();
            let model = if weighting == WEIGHTING_EMP {
                let ones: u64 = full.ones.iter().map(|&o| o as u64).sum();
                Model::with_frequency_of_one(
                    ones as f64 / (full.taxa as f64 * full.columns_len() as f64),
                )
            } else {
                Model::symmetric()
            };
            for ((rank, c), w) in counts.iter().zip(&weights) {
                assert_eq!(*rank, w.rank);
                let c: [u64; 16] = c.map(|x| x as u64);
                let cond = Conditioning::NotAllZero;
                let fits = weight::fit_all(&model, cond, &c);
                assert_eq!(w.fitted(), fits.is_some());
                if let Some(fits) = fits {
                    assert_eq!(w.log_likelihoods, fits.map(|f| f.log_likelihood));
                    let seed = weight::quartet_seed(7, *rank);
                    assert_eq!(Some(w.w2c), weight::w2c(&model, cond, &c, seed, 3));
                    assert!(w.resampled());
                }
            }
        }
    }

    #[test]
    fn weighting_none_has_no_weight_stage() {
        let tmp = TempDir::new("analysis_none");
        let input = tmp.path().join("in");
        write_inputs(&input, 6, 500, 4);
        let mut opts = options(&input, 5);
        let with = finish(&tmp.path().join("with"), &opts);
        opts.config.weighting = WEIGHTING_NONE.into();
        let run = tmp.path().join("none");
        let without = finish(&run, &opts);
        assert_ne!(with, without);
        assert!(read_weights(&run).unwrap().is_empty());
        let stages = String::from_utf8(
            atomic::read_verified(&RunDir::new(&run).audit().join("stages.json")).unwrap(),
        )
        .unwrap();
        assert!(!stages.contains(QUARTET_WEIGHT));
        opts.config.weighting = "w9".into();
        let err = start(
            &tmp.path().join("bad"),
            &opts,
            "terminal",
            &NullSink,
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown weighting"), "{err}");
    }

    #[test]
    fn support_and_audit_files_follow_the_stored_weights() {
        let tmp = TempDir::new("analysis_audit");
        let input = tmp.path().join("in");
        write_inputs(&input, 8, 700, 21);
        let run = tmp.path().join("run");
        let root = finish(&run, &options(&input, 9));
        let dir = RunDir::new(&run);
        let read = |rel: &str| atomic::read_verified(&dir.root().join(rel)).unwrap();
        let (names, _) = load_matrix(&run).unwrap();
        let mut weights = read_weights(&run).unwrap();
        weights.sort_by_key(|w| w.rank);
        // S1 and halo values equal a direct computation.
        let tree = String::from_utf8(read("report/tree.nwk")).unwrap();
        let mut acc = support::Accumulator::new(&names, tree.trim()).unwrap();
        for w in &weights {
            if let Some(t) = w.tree_weights() {
                acc.add(quartet::unrank(w.rank), t);
            }
        }
        let direct = acc.finish();
        assert_eq!(
            String::from_utf8(read("trees/tree_s1.nwk")).unwrap().trim(),
            direct.newick
        );
        let halo = String::from_utf8(read("report/halo.tsv")).unwrap();
        assert_eq!(halo.lines().count(), 9);
        // Decisions: one per quartet in rank order.
        let decisions = store::decisions_from_bytes(
            &zstd::decode_all(&read("audit/quartet_decisions.bin.zst")[..]).unwrap(),
        )
        .unwrap();
        assert_eq!(decisions.len(), weights.len());
        for (w, d) in weights.iter().zip(&decisions) {
            assert_eq!(*d, store::decision(w.tree_weights()));
        }
        // Worksheets and results.
        let sheets = String::from_utf8(read("audit/sample_worksheets.txt")).unwrap();
        assert_eq!(
            sheets.matches("Stored w2c weights: identical").count(),
            SAMPLE_WORKSHEETS
        );
        let results: serde_json::Value =
            serde_json::from_slice(&read("audit/results.json")).unwrap();
        assert_eq!(results["tree_with_s1"], direct.newick);
        assert_eq!(results["halo"].as_array().unwrap().len(), 8);
        let env: serde_json::Value =
            serde_json::from_slice(&read("audit/environment.json")).unwrap();
        assert_eq!(env["settings"]["seed"], 7);
        // Damaged support outputs are recomputed with the same root.
        std::fs::write(dir.report().join("halo.tsv"), b"x").unwrap();
        std::fs::remove_file(dir.audit().join("sample_worksheets.txt")).unwrap();
        let cancel = AtomicBool::new(false);
        let again = crate::runner::resume_run(&run, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(again, Outcome::Finished { root });
        assert_eq!(String::from_utf8(read("report/halo.tsv")).unwrap(), halo);
    }

    #[test]
    fn the_root_does_not_depend_on_where_the_data_are() {
        let tmp = TempDir::new("analysis_location");
        let a = tmp.path().join("one").join("in");
        write_inputs(&a, 6, 500, 13);
        let b = tmp.path().join("two").join("deeper").join("data");
        std::fs::create_dir_all(&b).unwrap();
        for e in std::fs::read_dir(&a).unwrap() {
            let e = e.unwrap();
            std::fs::copy(e.path(), b.join(e.file_name())).unwrap();
        }
        let ra = finish(&tmp.path().join("ra"), &options(&a, 4));
        let rb = finish(&tmp.path().join("rb"), &options(&b, 7));
        assert_eq!(ra, rb);
        // Another setting changes it.
        let mut o = options(&b, 7);
        o.config.seed = 8;
        assert_ne!(finish(&tmp.path().join("rc"), &o), ra);
    }

    #[test]
    fn sample_ranks_are_distinct_sorted_and_seeded() {
        let a = sample_ranks(7, 10_000, 50);
        assert_eq!(a.len(), 50);
        assert!(a.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(a, sample_ranks(7, 10_000, 50));
        assert_ne!(a, sample_ranks(8, 10_000, 50));
        assert_eq!(sample_ranks(7, 30, 50), (0..30).collect::<Vec<_>>());
    }

    #[test]
    fn the_tree_is_wqfm_on_the_stored_weights_and_part_of_the_root() {
        let tmp = TempDir::new("analysis_tree");
        let input = tmp.path().join("in");
        write_inputs(&input, 7, 800, 3);
        let run = tmp.path().join("run");
        let root = finish(&run, &options(&input, 6));
        let dir = RunDir::new(&run);
        let tree =
            String::from_utf8(atomic::read_verified(&dir.report().join("tree.nwk")).unwrap())
                .unwrap();
        let (names, _) = load_matrix(&run).unwrap();
        let mut weights = read_weights(&run).unwrap();
        weights.sort_by_key(|w| w.rank);
        let quartets: Vec<_> = weights
            .iter()
            .filter(|w| w.fitted())
            .flat_map(|w| amalgamate::quartets_of(quartet::unrank(w.rank), w.w2c))
            .collect();
        let direct = amalgamate::wqfm(&names, &quartets, &Default::default());
        assert_eq!(tree.trim(), direct.newick);
        let parsed = qmaws_core::newick::Tree::parse(tree.trim()).unwrap();
        assert_eq!(parsed.leaf_names().len(), 7);
        // A damaged tree is recomputed on resume and gives the same root.
        std::fs::write(dir.report().join("tree.nwk"), b"(x);").unwrap();
        let cancel = AtomicBool::new(false);
        let again = crate::runner::resume_run(&run, "terminal", &NullSink, &cancel).unwrap();
        assert_eq!(again, Outcome::Finished { root });
    }
}
