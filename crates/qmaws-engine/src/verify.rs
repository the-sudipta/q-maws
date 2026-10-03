//! Verification of a finished analysis run against its audit record.
//!
//! | Mode | What it does |
//! |---|---|
//! | input check | Recomputes the hash of every input file and of every cleaned sequence and compares them with `run.json` and `audit/inputs.json`. Always runs first |
//! | quick | Recomputes 20 chunks of each chunked stage, drawn with a printed seed, and compares their hashes with `audit/chunks.json`; also 3 bootstrap replicates, whose trees are compared the same way |
//! | full | Recomputes the whole run in a temporary folder and compares the root fingerprint |
//! | single quartet | Recomputes one quartet, writes its worksheet, and compares it with `audit/quartet_decisions.bin.zst` (and the stored weights, if present) |
//!
//! The full matrix is taken from the run folder when its hash matches
//! `audit/stages.json`; otherwise it is rebuilt from the inputs, so a
//! reviewer needs only the committed `audit/` folder, `run.json` and the
//! raw data. Every comparison is written to `report/verify_<time>.txt`.

use crate::analysis::{
    self, count_positions, run_blocks, weight_model, worksheet_of, AnalysisConfig, AnalysisOptions,
    InputsRecord, Weigher, BOOTSTRAP, COUNT_BLOCK, QUARTET_COUNT, QUARTET_WEIGHT, WEIGHT_BLOCK,
};
use crate::atomic;
use crate::clock::UtcDateTime;
use crate::hash::sha256_hex;
use crate::progress::NullSink;
use crate::rundir::RunDir;
use crate::runner::{io_err, EngineError};
use crate::state::RunState;
use crate::store;
use qmaws_core::matrix::Matrix;
use qmaws_core::quartet::{self, CoCounts, Permutation, PopcountPath};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Chunks recomputed per chunked stage in the quick mode.
pub const QUICK_CHUNKS: usize = 20;

/// Bootstrap replicates recomputed in the quick mode (each one weighs every
/// quartet once).
pub const QUICK_REPLICATES: usize = 3;

/// Floating-point tolerance against stored weights (determinism contract).
pub const TOLERANCE: f64 = 0.000_001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Only the input check.
    Inputs,
    /// Quick check with an optional seed (a fresh one if `None`).
    Quick(Option<u64>),
    Full,
    /// Single quartet by taxon names.
    Quartet([String; 4]),
}

/// Result of a verification.
#[derive(Debug, Clone)]
pub struct Report {
    pub lines: Vec<String>,
    pub failures: usize,
    /// The report file.
    pub path: PathBuf,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.failures == 0
    }
}

struct Writer {
    lines: Vec<String>,
    failures: usize,
}

impl Writer {
    fn line(&mut self, s: impl Into<String>) {
        self.lines.push(s.into());
    }
    fn check(&mut self, ok: bool, what: impl Into<String>) {
        if !ok {
            self.failures += 1;
        }
        self.lines.push(format!(
            "[{}] {}",
            if ok { "PASS" } else { "FAIL" },
            what.into()
        ));
    }
}

struct Run {
    dir: RunDir,
    state: RunState,
    config: AnalysisConfig,
    inputs: InputsRecord,
    stages: serde_json::Value,
    chunks: serde_json::Value,
}

fn read_json(dir: &RunDir, rel: &str) -> Result<serde_json::Value, EngineError> {
    let p = dir.root().join(rel);
    let bytes = atomic::read_verified(&p)
        .ok_or_else(|| EngineError::Invalid(format!("{} is missing or damaged", p.display())))?;
    serde_json::from_slice(&bytes).map_err(|e| EngineError::Invalid(format!("{rel}: {e}")))
}

fn stage_hash(stages: &serde_json::Value, stage: &str) -> Option<String> {
    stages["stages"]
        .as_array()?
        .iter()
        .find(|s| s["stage"] == stage)
        .and_then(|s| s["content_sha256"].as_str().map(str::to_string))
}

/// Verifies the finished run in `run_dir`. `input` replaces the run's input
/// path (for data kept elsewhere).
pub fn verify(
    run_dir: &Path,
    mode: &Mode,
    input: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<Report, EngineError> {
    let dir = RunDir::new(run_dir);
    let state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    if state.kind != analysis::KIND {
        return Err(EngineError::Invalid(
            "only analysis runs can be verified".into(),
        ));
    }
    let mut config: AnalysisConfig = serde_json::from_value(state.config.clone())
        .map_err(|e| EngineError::Invalid(format!("run.json configuration: {e}")))?;
    let original_input = PathBuf::from(&config.input);
    if let Some(p) = input {
        config.input = p.display().to_string();
    }
    let root_path = dir.audit().join("root.txt");
    let root = atomic::read_verified(&root_path)
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .ok_or_else(|| {
            EngineError::Invalid("the run has not finished (audit/root.txt is missing)".into())
        })?;
    let inputs: InputsRecord = serde_json::from_value(read_json(&dir, "audit/inputs.json")?)
        .map_err(|e| EngineError::Invalid(format!("audit/inputs.json: {e}")))?;
    let run = Run {
        stages: read_json(&dir, "audit/stages.json")?,
        chunks: read_json(&dir, "audit/chunks.json")?,
        dir,
        state,
        config,
        inputs,
    };
    let now = UtcDateTime::now();
    let mut w = Writer {
        lines: Vec::new(),
        failures: 0,
    };
    w.line(format!("Q-MAWS verification of {}", run_dir.display()));
    w.line(format!(
        "Date: {}; program {} (commit {})",
        now.iso8601(),
        env!("CARGO_PKG_VERSION"),
        analysis::GIT_COMMIT
    ));
    w.line(format!("Stored root fingerprint: {root}"));
    w.line(String::new());
    check_inputs(&run, &original_input, &mut w)?;
    if w.failures == 0 {
        match mode {
            Mode::Inputs => {}
            Mode::Quick(seed) => quick(&run, *seed, &mut w, cancel)?,
            Mode::Full => full(&run, &root, &mut w, cancel)?,
            Mode::Quartet(names) => single_quartet(&run, names, &mut w, cancel)?,
        }
    } else {
        w.line("The input check failed; the other checks need the same inputs and were not run.");
    }
    w.line(String::new());
    w.line(format!(
        "Verdict: {}",
        if w.failures == 0 {
            "PASS (every comparison agrees)".to_string()
        } else {
            format!("FAIL ({} comparisons differ)", w.failures)
        }
    ));
    let path = run
        .dir
        .report()
        .join(format!("verify_{}.txt", now.run_id_part()));
    std::fs::create_dir_all(run.dir.report()).map_err(io_err(&run.dir.report()))?;
    atomic::write_verified(&path, (w.lines.join("\n") + "\n").as_bytes()).map_err(io_err(&path))?;
    crate::readme::write_run_readmes(run.dir.root()).map_err(io_err(run.dir.root()))?;
    Ok(Report {
        lines: w.lines,
        failures: w.failures,
        path,
    })
}

/// Raw input files and cleaned sequences.
fn check_inputs(run: &Run, original: &Path, w: &mut Writer) -> Result<(), EngineError> {
    w.line("Input check");
    let new_root = Path::new(&run.config.input);
    for f in &run.state.inputs {
        let stored = Path::new(&f.path);
        let path = if new_root == original {
            stored.to_path_buf()
        } else if original.is_file() || stored == original {
            new_root.to_path_buf()
        } else {
            match stored.strip_prefix(original) {
                Ok(rel) => new_root.join(rel),
                Err(_) => stored.to_path_buf(),
            }
        };
        match std::fs::read(&path) {
            Ok(bytes) => w.check(
                sha256_hex(&bytes) == f.sha256 && bytes.len() as u64 == f.bytes,
                format!("file {} (SHA-256 {})", path.display(), &f.sha256[..16]),
            ),
            Err(e) => w.check(false, format!("file {}: {e}", path.display())),
        }
    }
    if w.failures > 0 {
        return Ok(());
    }
    let loaded = qmaws_data::loader::load(new_root, run.config.mode())
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    w.check(
        loaded.taxa.len() == run.inputs.taxa.len(),
        format!(
            "{} taxa read, {} recorded",
            loaded.taxa.len(),
            run.inputs.taxa.len()
        ),
    );
    let mut differ = Vec::new();
    for ((t, h), rec) in loaded
        .taxa
        .iter()
        .zip(&loaded.cleaned_sha256)
        .zip(&run.inputs.taxa)
    {
        if t.name != rec.name || *h != rec.cleaned_sha256 {
            differ.push(rec.name.clone());
        }
    }
    w.check(
        differ.is_empty(),
        if differ.is_empty() {
            "every taxon name and cleaned sequence hash agrees with audit/inputs.json".to_string()
        } else {
            format!("cleaned sequences differ: {}", differ.join(", "))
        },
    );
    Ok(())
}

/// The full matrix: from the run folder if its hash agrees, otherwise
/// rebuilt from the inputs in `work/verify/`.
fn matrix(run: &Run, w: &mut Writer, cancel: &AtomicBool) -> Result<Option<Matrix>, EngineError> {
    let expected = stage_hash(&run.stages, analysis::MATRIX_BUILD)
        .ok_or_else(|| EngineError::Invalid("audit/stages.json has no matrix hash".into()))?;
    let stored = run.dir.work().join("matrix").join("m_full.bin");
    if let Some(bytes) = atomic::read_verified(&stored) {
        if sha256_hex(&bytes) == expected {
            w.line(
                "Full matrix: taken from the run folder (its hash agrees with audit/stages.json).",
            );
            return store::matrix_from_bytes(&bytes)
                .map(Some)
                .map_err(|e| EngineError::Invalid(e.to_string()));
        }
    }
    let tmp = run
        .dir
        .work()
        .join("verify")
        .join(format!("matrix_{}", UtcDateTime::now().run_id_part()));
    let _ = std::fs::remove_dir_all(&tmp);
    let options = AnalysisOptions {
        config: run.config.clone(),
        chunk_seconds: 3.0,
        chunk_quartets: None,
        memory_limit: None,
        live_tree: false,
    };
    let Some(full) = analysis::build_matrix_only(&tmp, &options, &NullSink, cancel)? else {
        return Ok(None);
    };
    let bytes = store::matrix_to_bytes(&full);
    w.check(
        sha256_hex(&bytes) == expected,
        "full matrix rebuilt from the inputs; its hash agrees with audit/stages.json",
    );
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(Some(full))
}

/// Chunk list of a chunked stage in `audit/chunks.json`.
fn chunk_list<'a>(chunks: &'a serde_json::Value, stage: &str) -> Vec<&'a serde_json::Value> {
    chunks["stages"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["stage"] == stage))
        .and_then(|s| s["chunks"].as_array())
        .map(|c| c.iter().collect())
        .unwrap_or_default()
}

fn draw(seed: u64, n: usize, k: usize) -> Vec<usize> {
    if n <= k {
        return (0..n).collect();
    }
    let mut rng = qmaws_core::weight::SplitMix64::new(seed);
    let mut set = std::collections::BTreeSet::new();
    while set.len() < k {
        set.insert((rng.next_u64() % n as u64) as usize);
    }
    set.into_iter().collect()
}

fn quick(
    run: &Run,
    seed: Option<u64>,
    w: &mut Writer,
    cancel: &AtomicBool,
) -> Result<(), EngineError> {
    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
    });
    w.line(String::new());
    w.line(format!(
        "Quick check: {QUICK_CHUNKS} chunks per chunked stage and up to {QUICK_REPLICATES} bootstrap replicates, drawn with seed {seed} (repeat with --seed {seed})"
    ));
    let Some(full) = matrix(run, w, cancel)? else {
        w.check(false, "stopped before the matrix was rebuilt");
        return Ok(());
    };
    let m = run.inputs.taxa.len();
    let q = quartet::quartet_count(m);
    let cc = CoCounts::new(&full.rows, full.columns_len() as u64);
    let path = PopcountPath::detect();
    let perm = Permutation::new(q, run.config.seed);
    let counts = chunk_list(&run.chunks, QUARTET_COUNT);
    for i in draw(seed, counts.len(), QUICK_CHUNKS) {
        let c = counts[i];
        let (s, e) = (
            c["start"].as_u64().unwrap_or(0),
            c["end"].as_u64().unwrap_or(0),
        );
        let bytes = run_blocks(COUNT_BLOCK, s, e, &|a, b| {
            count_positions(&full, &cc, path, &perm, a, b)
        });
        w.check(
            Some(sha256_hex(&bytes).as_str()) == c["sha256"].as_str(),
            format!(
                "{} chunk {} (positions {s} to {}): SHA-256 of the counts",
                QUARTET_COUNT,
                i,
                e - 1
            ),
        );
    }
    let weights = chunk_list(&run.chunks, QUARTET_WEIGHT);
    if !weights.is_empty() {
        let weigher = Weigher {
            full: &full,
            cc: &cc,
            path,
            perm: &perm,
            model: weight_model(&run.config, &full)?,
            seed: run.config.seed,
            replicates: run.config.replicates,
        };
        for i in draw(seed.wrapping_add(1), weights.len(), QUICK_CHUNKS) {
            let c = weights[i];
            let (s, e) = (
                c["start"].as_u64().unwrap_or(0),
                c["end"].as_u64().unwrap_or(0),
            );
            let bytes = run_blocks(WEIGHT_BLOCK, s, e, &|a, b| weigher.positions(a, b));
            let recs =
                store::weight_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?;
            let text: String = recs.iter().map(|r| r.canonical()).collect();
            let raw = Some(sha256_hex(&bytes).as_str()) == c["sha256"].as_str();
            let rounded =
                Some(sha256_hex(text.as_bytes()).as_str()) == c["rounded_sha256"].as_str();
            w.check(
                rounded,
                format!(
                    "{} chunk {} (positions {s} to {}): SHA-256 of the weights rounded to 9 significant digits{}",
                    QUARTET_WEIGHT,
                    i,
                    e - 1,
                    if raw {
                        "; the raw bytes agree too"
                    } else {
                        "; the raw bytes differ in the last bits (allowed by the determinism contract)"
                    }
                ),
            );
        }
    }
    let replicates = chunk_list(&run.chunks, BOOTSTRAP);
    if !replicates.is_empty() {
        let names: Vec<String> = run.inputs.taxa.iter().map(|t| t.name.clone()).collect();
        let model = weight_model(&run.config, &full)?;
        for b in draw(seed.wrapping_add(2), replicates.len(), QUICK_REPLICATES) {
            let c = replicates[b];
            let r = crate::bootstrap::replicate_tree(
                &names,
                &full,
                &model,
                run.config.seed,
                b as u64,
                crate::bootstrap::Inner::W2b,
            );
            w.check(
                Some(sha256_hex(format!("{}\n", r.newick).as_bytes()).as_str())
                    == c["sha256"].as_str(),
                format!(
                    "{BOOTSTRAP} replicate {b} (seed {}): SHA-256 of its tree",
                    r.seed
                ),
            );
        }
    }
    Ok(())
}

fn full(run: &Run, root: &str, w: &mut Writer, cancel: &AtomicBool) -> Result<(), EngineError> {
    w.line(String::new());
    w.line("Full check: the whole run is recomputed in a temporary folder");
    let tmp = run
        .dir
        .work()
        .join("verify")
        .join(format!("full_{}", UtcDateTime::now().run_id_part()));
    let _ = std::fs::remove_dir_all(&tmp);
    let options = AnalysisOptions {
        config: run.config.clone(),
        chunk_seconds: 3.0,
        chunk_quartets: None,
        memory_limit: None,
        live_tree: false,
    };
    let outcome = analysis::start(&tmp, &options, "terminal", &NullSink, cancel)?;
    match outcome {
        crate::runner::Outcome::Finished { root: again } => {
            w.check(
                again == root,
                format!("recomputed root fingerprint {again}"),
            );
            let _ = std::fs::remove_dir_all(&tmp);
        }
        crate::runner::Outcome::Stopped => {
            w.check(
                false,
                format!("stopped; the partial recomputation is in {}", tmp.display()),
            );
        }
    }
    Ok(())
}

fn single_quartet(
    run: &Run,
    names: &[String; 4],
    w: &mut Writer,
    cancel: &AtomicBool,
) -> Result<(), EngineError> {
    w.line(String::new());
    let taxa: Vec<String> = run.inputs.taxa.iter().map(|t| t.name.clone()).collect();
    let mut idx = Vec::new();
    for n in names {
        match taxa.iter().position(|t| t == n) {
            Some(i) => idx.push(i),
            None => {
                return Err(EngineError::Invalid(format!(
                    "{n} is not a taxon of this run"
                )))
            }
        }
    }
    idx.sort_unstable();
    idx.dedup();
    if idx.len() != 4 {
        return Err(EngineError::Invalid("give four different taxa".into()));
    }
    let quad = [idx[0], idx[1], idx[2], idx[3]];
    let rank = quartet::rank(quad);
    w.line(format!(
        "Single quartet: {} (rank {rank})",
        quad.map(|i| taxa[i].as_str()).join(", ")
    ));
    let Some(full) = matrix(run, w, cancel)? else {
        w.check(false, "stopped before the matrix was rebuilt");
        return Ok(());
    };
    let q = quartet::quartet_count(taxa.len());
    let cc = CoCounts::new(&full.rows, full.columns_len() as u64);
    let perm = Permutation::new(q, run.config.seed);
    let weigher = Weigher {
        full: &full,
        cc: &cc,
        path: PopcountPath::detect(),
        perm: &perm,
        model: weight_model(&run.config, &full)?,
        seed: run.config.seed,
        replicates: run.config.replicates,
    };
    let (sheet, weights) = worksheet_of(&run.config, &taxa, &weigher, quad);
    w.line(String::new());
    for l in sheet.lines() {
        w.line(l);
    }
    w.line(String::new());
    let tree_weights = if run.config.replicates > 0 {
        weights.w2c
    } else {
        weights.w2b
    };
    let dec_path = run.dir.audit().join("quartet_decisions.bin.zst");
    let stored = atomic::read_verified(&dec_path)
        .ok_or_else(|| {
            EngineError::Invalid("audit/quartet_decisions.bin.zst is missing or damaged".into())
        })
        .and_then(|b| {
            zstd::decode_all(&b[..]).map_err(|e| EngineError::Invalid(format!("decisions: {e}")))
        })
        .and_then(|raw| {
            store::decisions_from_bytes(&raw).map_err(|e| EngineError::Invalid(e.to_string()))
        })?;
    let (t, v) = store::decision(tree_weights);
    let (st, sv) = stored
        .get(rank as usize)
        .copied()
        .unwrap_or((store::NO_DECISION, 0));
    w.check(
        (t, v) == (st, sv),
        format!(
            "decision: recomputed {} with quantised weight {v}, stored {} with {sv}",
            topology_name(t),
            topology_name(st)
        ),
    );
    // The stored weight record, if the work folder is present.
    if let Ok(records) = analysis::read_weights(run.dir.root()) {
        if let Some(r) = records.iter().find(|r| r.rank == rank) {
            let same = match (r.tree_weights(), tree_weights) {
                (Some(a), Some(b)) => (0..3).all(|i| (a[i] - b[i]).abs() <= TOLERANCE),
                (None, None) => true,
                _ => false,
            };
            w.check(
                same,
                format!("stored weights in the work folder agree within {TOLERANCE}"),
            );
        }
    }
    Ok(())
}

fn topology_name(t: u8) -> &'static str {
    match t {
        0..=2 => qmaws_core::weight::TOPOLOGY_NAMES[t as usize],
        _ => "no weight",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{start, WEIGHTING_SYM};
    use crate::testutil::TempDir;

    fn inputs(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        let mut x: u64 = 3;
        let mut next = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 33) as usize
        };
        let base: Vec<u8> = (0..700).map(|_| b"ACGT"[next() % 4]).collect();
        for t in 0..7 {
            let mut s = base.clone();
            for _ in 0..40 {
                let i = next() % 700;
                s[i] = b"ACGT"[next() % 4];
            }
            std::fs::write(
                dir.join(format!("T{t}.fasta")),
                format!(">T{t}\n{}\n", String::from_utf8(s).unwrap()),
            )
            .unwrap();
        }
    }

    fn finished_run(tmp: &TempDir) -> (PathBuf, PathBuf) {
        let input = tmp.path().join("in");
        inputs(&input);
        let run = tmp.path().join("run");
        let options = AnalysisOptions {
            config: AnalysisConfig {
                input: input.display().to_string(),
                records: "per_file".into(),
                strand: true,
                lengths: None,
                seed: 5,
                ml_max_columns: qmaws_core::matrix::MAX_ML_COLUMNS,
                weighting: WEIGHTING_SYM.into(),
                replicates: 4,
                bootstrap: 3,
            },
            chunk_seconds: 3.0,
            chunk_quartets: Some(3),
            memory_limit: Some(1 << 30),
            live_tree: false,
        };
        start(
            &run,
            &options,
            "terminal",
            &NullSink,
            &AtomicBool::new(false),
        )
        .unwrap();
        (input, run)
    }

    #[test]
    fn chunk_draws_are_distinct_sorted_and_seeded() {
        let a = draw(3, 100, QUICK_CHUNKS);
        assert_eq!(a.len(), QUICK_CHUNKS);
        assert!(a.windows(2).all(|w| w[0] < w[1]) && a[QUICK_CHUNKS - 1] < 100);
        assert_eq!(a, draw(3, 100, QUICK_CHUNKS));
        assert_ne!(a, draw(4, 100, QUICK_CHUNKS));
        assert_eq!(draw(3, 7, QUICK_CHUNKS), (0..7).collect::<Vec<_>>());
    }

    #[test]
    fn every_mode_passes_on_an_untouched_run() {
        let tmp = TempDir::new("verify_pass");
        let (_, run) = finished_run(&tmp);
        let cancel = AtomicBool::new(false);
        for mode in [
            Mode::Inputs,
            Mode::Quick(Some(9)),
            Mode::Full,
            Mode::Quartet(["T0", "T3", "T5", "T6"].map(String::from)),
        ] {
            let r = verify(&run, &mode, None, &cancel).unwrap();
            assert!(r.passed(), "{mode:?}: {}", r.lines.join("\n"));
            assert!(r.path.exists());
        }
        // 35 quartets in chunks of 3: all 12 count chunks are recomputed.
        let r = verify(&run, &Mode::Quick(Some(9)), None, &cancel).unwrap();
        let text = r.lines.join("\n");
        assert_eq!(text.matches("[PASS] quartet_count chunk").count(), 12);
        // 3 bootstrap replicates: all recomputed.
        assert_eq!(text.matches("[PASS] bootstrap replicate").count(), 3);
        assert!(text.contains("taken from the run folder"), "{text}");
        let q = verify(
            &run,
            &Mode::Quartet(["T0", "T3", "T5", "T6"].map(String::from)),
            None,
            &cancel,
        )
        .unwrap();
        let text = q.lines.join("\n");
        assert!(text.contains("c(1100) = n(ab)"), "{text}");
        assert!(
            text.contains("[PASS] stored weights in the work folder"),
            "{text}"
        );
    }

    #[test]
    fn the_matrix_is_rebuilt_without_the_work_folder_and_changes_are_found() {
        let tmp = TempDir::new("verify_rebuild");
        let (input, run) = finished_run(&tmp);
        // A reviewer has only audit/, run.json and the data.
        std::fs::remove_dir_all(run.join("work")).unwrap();
        let cancel = AtomicBool::new(false);
        let r = verify(&run, &Mode::Quick(Some(1)), None, &cancel).unwrap();
        assert!(r.passed(), "{}", r.lines.join("\n"));
        assert!(r
            .lines
            .iter()
            .any(|l| l.contains("rebuilt from the inputs")));
        // Data moved elsewhere: --input.
        let moved = tmp.path().join("moved");
        std::fs::rename(&input, &moved).unwrap();
        assert!(verify(&run, &Mode::Inputs, None, &cancel).unwrap().failures > 0);
        let r = verify(&run, &Mode::Inputs, Some(&moved), &cancel).unwrap();
        assert!(r.passed(), "{}", r.lines.join("\n"));
        // The root does not depend on where the data are.
        let r = verify(&run, &Mode::Full, Some(&moved), &cancel).unwrap();
        assert!(r.passed(), "{}", r.lines.join("\n"));
        // A changed stored chunk hash is reported.
        let chunks = run.join("audit").join("chunks.json");
        let text = std::fs::read_to_string(&chunks).unwrap();
        let first = text.find("\"sha256\": \"").unwrap() + 11;
        let mut changed = text.clone();
        changed.replace_range(first..first + 4, "0000");
        atomic::write_verified(&chunks, changed.as_bytes()).unwrap();
        let r = verify(&run, &Mode::Quick(Some(1)), Some(&moved), &cancel).unwrap();
        assert_eq!(r.failures, 1, "{}", r.lines.join("\n"));
        // So is a changed tree hash of a bootstrap replicate (the last chunk
        // listed); all 3 replicates are recomputed.
        let last = changed.rfind("\"sha256\": \"").unwrap() + 11;
        changed.replace_range(last..last + 4, "0000");
        atomic::write_verified(&chunks, changed.as_bytes()).unwrap();
        let r = verify(&run, &Mode::Quick(Some(1)), Some(&moved), &cancel).unwrap();
        assert_eq!(r.failures, 2, "{}", r.lines.join("\n"));
        assert!(r
            .lines
            .iter()
            .any(|l| l.starts_with("[FAIL] bootstrap replicate")));
        // A changed input file fails the input check.
        let f = moved.join("T2.fasta");
        let mut body = std::fs::read_to_string(&f).unwrap();
        body.push('A');
        std::fs::write(&f, body).unwrap();
        let r = verify(&run, &Mode::Full, Some(&moved), &cancel).unwrap();
        assert!(!r.passed());
        assert!(r.lines.iter().any(|l| l.contains("were not run")));
    }
}
