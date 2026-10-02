//! The analysis run: from sequence files to quartet pattern counts, as a
//! resumable sequence of stages. (Weighting, amalgamation and the later
//! stages are added in later milestones.)
//!
//! | Stage | Unit of work | Output |
//! |---|---|---|
//! | `ingest` | whole stage | `audit/inputs.json` |
//! | `maw_extract` | one taxon (strand filter included) | `work/maws/<taxon>.bin` |
//! | `length_select` | whole stage | `work/matrix/selection.json` |
//! | `matrix_build` | whole stage | `work/matrix/m_full.bin`, `report/m_ml.phy` |
//! | `quartet_count` | one chunk of quartets in Feistel order | `work/chunks/quartet_count_<n>.bin` |
//! | `finalize` | whole stage | `audit/stages.json`, `audit/chunks.json`, `audit/root.txt` |
//!
//! Every file is written atomically with a hash file; a unit is done exactly
//! when its file is valid. The root fingerprint covers each stage's content
//! hash; for chunked stages the content hash is taken over all outputs in
//! order, so it does not depend on chunk boundaries.

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
use qmaws_core::input::RecordMode;
use qmaws_core::matrix::{self, Matrix};
use qmaws_core::maw::{self, MawSet};
use qmaws_core::quartet::{self, CoCounts, Permutation, PopcountPath};
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
pub const FINALIZE: &str = "finalize";
/// Quartets per parallel block when counting.
const COUNT_BLOCK: u64 = 4096;

/// Largest chunk of count records held in memory (bytes).
const MAX_CHUNK_BYTES: usize = 64 << 20;

pub const STAGES: [&str; 6] = [
    INGEST,
    MAW_EXTRACT,
    LENGTH_SELECT,
    MATRIX_BUILD,
    QUARTET_COUNT,
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
    /// Seed of the quartet processing order.
    pub seed: u64,
    pub ml_max_columns: usize,
}

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
    fn mode(&self) -> RecordMode {
        if self.records == "per_record" {
            RecordMode::OneTaxonPerRecord
        } else {
            RecordMode::ConcatenatePerFile
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaxonRecord {
    name: String,
    original_name: String,
    source_file: String,
    original_length: u64,
    cleaned_length: u64,
    removed: BTreeMap<char, u64>,
    cleaned_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InputsRecord {
    taxa: Vec<TaxonRecord>,
    files: Vec<InputFingerprint>,
    lmin: usize,
    lmax: usize,
    average_length: u64,
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
    let dir = RunDir::new(run_dir);
    if dir.run_json().exists() {
        return Err(EngineError::RunExists(run_dir.to_path_buf()));
    }
    if !(options.chunk_seconds.is_finite() && options.chunk_seconds > 0.0) {
        return Err(EngineError::Invalid(
            "the chunk duration must be a positive number of seconds".into(),
        ));
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
        for stage in [INGEST, LENGTH_SELECT, MATRIX_BUILD, FINALIZE] {
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
        self.estimator = Estimator::new(&[(MAW_EXTRACT, m as u64), (QUARTET_COUNT, q)]);
        for (stage, rate) in self.state.throughput.clone() {
            if stage == MAW_EXTRACT || stage == QUARTET_COUNT {
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
        if let Some(outcome) = self.quartet_count(m)? {
            return Ok(outcome);
        }
        if self.status(FINALIZE) != StageStatus::Done {
            self.finalize(m)?;
        }
        let root = String::from_utf8_lossy(&self.read("audit/root.txt")?)
            .trim()
            .to_string();
        self.log(&format!("Run finished. Root fingerprint: {root}"))?;
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

    fn chunk_file(&self, index: u64) -> PathBuf {
        self.dir.chunk_file(QUARTET_COUNT, index)
    }

    fn quartet_count(&mut self, m: usize) -> Result<Option<Outcome>, EngineError> {
        let q = quartet::quartet_count(m);
        let full = store::matrix_from_bytes(&self.read("work/matrix/m_full.bin")?)
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        let cc = CoCounts::new(&full.rows, full.columns_len() as u64);
        let path = PopcountPath::detect();
        let perm = Permutation::new(q, self.options.config.seed);
        let count_one = |pos: u64, out: &mut Vec<u8>| {
            let r = perm.apply(pos);
            let quad = quartet::unrank(r);
            let n4 = path.and4(
                &full.rows[quad[0]],
                &full.rows[quad[1]],
                &full.rows[quad[2]],
                &full.rows[quad[3]],
            );
            store::push_count_record(out, r, &quartet::pattern_counts(&cc, quad, n4));
        };
        // Positions s..e, counted in parallel blocks and joined in order, so
        // the bytes do not depend on the number of threads.
        let count_range = |s: u64, e: u64| -> Vec<u8> {
            let blocks: Vec<(u64, u64)> = (s..e)
                .step_by(COUNT_BLOCK as usize)
                .map(|b| (b, (b + COUNT_BLOCK).min(e)))
                .collect();
            let parts: Vec<Vec<u8>> = blocks
                .par_iter()
                .map(|&(b0, b1)| {
                    let mut part = Vec::with_capacity(((b1 - b0) as usize) * store::COUNT_RECORD);
                    for pos in b0..b1 {
                        count_one(pos, &mut part);
                    }
                    part
                })
                .collect();
            parts.concat()
        };
        let plan = match self.state.chunk_plans.get(QUARTET_COUNT) {
            Some(p) => *p,
            None => {
                // Calibration: time a sample of quartets the same way chunks
                // are counted, then fix the plan.
                let threads = rayon::current_num_threads() as u64;
                let sample = q.min(COUNT_BLOCK * threads);
                let t0 = Instant::now();
                std::hint::black_box(count_range(0, sample));
                let rate = sample as f64 / t0.elapsed().as_secs_f64().max(1e-9);
                self.estimator.set_rate(QUARTET_COUNT, rate);
                // Target duration, but never more than MAX_CHUNK_BYTES of
                // records in memory at once.
                let max_per = (MAX_CHUNK_BYTES / store::COUNT_RECORD) as u64;
                let per = self
                    .options
                    .chunk_quartets
                    .unwrap_or(((rate * self.options.chunk_seconds).round() as u64).min(max_per));
                let plan = ChunkPlan::new(q, per);
                self.state
                    .chunk_plans
                    .insert(QUARTET_COUNT.to_string(), plan);
                self.save()?;
                self.log(&format!(
                    "Quartet counting: {q} quartets, {} chunks of up to {} (calibration {:.0} quartets per second; popcount {:?}).",
                    plan.chunk_count(),
                    plan.units_per_chunk,
                    rate,
                    path
                ))?;
                plan
            }
        };
        let chunks = plan.chunk_count();
        let valid: Vec<bool> = (0..chunks)
            .map(|i| atomic::is_valid(&self.chunk_file(i)))
            .collect();
        let done_units: u64 = (0..chunks)
            .filter(|&i| valid[i as usize])
            .map(|i| plan.range(i).1 - plan.range(i).0)
            .sum();
        self.estimator.set_done(QUARTET_COUNT, done_units);
        if valid.iter().all(|&v| v) {
            if self.status(QUARTET_COUNT) != StageStatus::Done {
                self.set_done(QUARTET_COUNT, BTreeMap::new())?;
            }
            return Ok(None);
        }
        self.reset_from(QUARTET_COUNT);
        self.set_running(QUARTET_COUNT)?;
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
                QUARTET_COUNT,
                done,
                q,
                format!("chunk {} of {chunks} (positions {s} to {})", i + 1, e - 1),
            );
            let t0 = Instant::now();
            let out = count_range(s, e);
            let p = self.chunk_file(i);
            atomic::write_verified(&p, &out).map_err(io_err(&p))?;
            self.estimator
                .record(QUARTET_COUNT, e - s, t0.elapsed().as_secs_f64());
            done += e - s;
            self.save()?;
        }
        self.progress(QUARTET_COUNT, q, q, format!("all {chunks} chunks done"));
        self.set_done(QUARTET_COUNT, BTreeMap::new())?;
        Ok(None)
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
        let plan = self.state.chunk_plans[QUARTET_COUNT];
        let mut counts = StreamHasher::new();
        let mut chunk_list = Vec::new();
        for i in 0..plan.chunk_count() {
            let p = self.chunk_file(i);
            let bytes = atomic::read_verified(&p).ok_or_else(|| {
                EngineError::Invalid(format!("{} is missing or damaged", p.display()))
            })?;
            counts.update(&bytes);
            let (s, e) = plan.range(i);
            chunk_list.push(serde_json::json!({
                "index": i, "start": s, "end": e, "sha256": sha256_hex(&bytes)
            }));
        }
        let stage_hashes = vec![
            (INGEST, file_hash("audit/inputs.json")?),
            (MAW_EXTRACT, maws.finish_hex()),
            (LENGTH_SELECT, file_hash("work/matrix/selection.json")?),
            (MATRIX_BUILD, file_hash("work/matrix/m_full.bin")?),
            (QUARTET_COUNT, counts.finish_hex()),
        ];
        let mut root_text = format!(
            "qmaws-root-v1\nkind {KIND}\nconfig {}\n",
            self.state.config_sha256
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
        let chunks_json = serde_json::json!({
            "stage": QUARTET_COUNT,
            "units_per_chunk": plan.units_per_chunk,
            "chunks": chunk_list,
        });
        let mut outputs = BTreeMap::new();
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
}
