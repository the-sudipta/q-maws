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
use crate::progress::{Estimator, Event, ProgressSink, Snapshot};
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

pub const STAGES: [&str; 9] = [
    INGEST,
    MAW_EXTRACT,
    LENGTH_SELECT,
    MATRIX_BUILD,
    QUARTET_COUNT,
    QUARTET_WEIGHT,
    AMALGAMATE,
    SUPPORT,
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
}

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
    estimator: Estimator,
    /// Stop after `matrix_build` (verification).
    until_matrix: bool,
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
            estimator: Estimator::new(&[]),
            until_matrix: false,
        }
    }

    fn elapsed(&self) -> f64 {
        self.elapsed_before + self.started.elapsed().as_secs_f64()
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
        if let Some(s) = self.state.stage_mut(stage) {
            s.status = StageStatus::Done;
            s.outputs = outputs;
        }
        self.save()
    }

    fn set_running(&mut self, stage: &str) -> Result<(), EngineError> {
        if let Some(s) = self.state.stage_mut(stage) {
            s.status = StageStatus::Running;
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
        self.estimator = Estimator::new(&units);
        for (stage, rate) in self.state.throughput.clone() {
            if units.iter().any(|(s, _)| *s == stage) {
                self.estimator.set_rate(&stage, rate);
            }
        }
        if let Some(outcome) = self.maw_extract(&inputs)? {
            return Ok(outcome);
        }
        if self.cancelled() {
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
        }
        if self.status(SUPPORT) != StageStatus::Done {
            self.support(m)?;
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
            if self.cancelled() {
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
        self.run_chunks(
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
        )
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
            if self.cancelled() {
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
        let results = serde_json::json!({
            "tree": read_text("report/tree.nwk")?,
            "tree_with_s1": read_text("trees/tree_s1.nwk")?,
            "weights": source,
            "amalgamation": amalgamation,
            "support_s1": support,
            "halo": halo,
            "metrics": {},
        });
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
        let plan = self.state.chunk_plans[QUARTET_COUNT];
        let mut counts = StreamHasher::new();
        let mut chunk_list = Vec::new();
        for i in 0..plan.chunk_count() {
            let bytes = read_chunk(&self.dir.chunk_file(QUARTET_COUNT, i))?;
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
            for i in 0..plan.chunk_count() {
                let bytes = read_chunk(&self.dir.chunk_file(QUARTET_WEIGHT, i))?;
                let recs = store::weight_records(&bytes)
                    .map_err(|e| EngineError::Invalid(e.to_string()))?;
                let mut text = String::with_capacity(recs.len() * 120);
                for r in &recs {
                    text.push_str(&r.canonical());
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
        }
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
        let stages_json = serde_json::json!({
            "stages": stage_hashes.iter().map(|(s, h)| serde_json::json!({"stage": s, "content_sha256": h})).collect::<Vec<_>>(),
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
            },
            chunk_seconds: 3.0,
            chunk_quartets: Some(chunk),
            memory_limit: Some(1 << 30),
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
