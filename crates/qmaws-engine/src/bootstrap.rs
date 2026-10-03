//! S2 column-bootstrap replicates. A replicate draws Poisson(1) column
//! weights, counts every quartet with those weights, weighs the quartets
//! (W2b, or W2c with per-quartet seeds of the replicate) and runs wQFM-rs.
//! The analysis stage `bootstrap` runs B replicates with W2b (decision D7);
//! the hidden command `s2-cost` times single replicates of a finished run.

use crate::analysis::{self, weight_model, AnalysisConfig};
use crate::atomic;
use crate::rundir::RunDir;
use crate::runner::EngineError;
use crate::state::RunState;
use qmaws_core::amalgamate;
use qmaws_core::bootstrap::{self, WeightedCounts};
use qmaws_core::matrix::Matrix;
use qmaws_core::quartet::{self, PopcountPath};
use qmaws_core::weight::{self, Conditioning, Model};
use rayon::prelude::*;
use std::path::Path;
use std::time::Instant;

/// Result and timing of one replicate.
#[derive(Debug, Clone)]
pub struct Replicate {
    pub index: u64,
    pub seed: u64,
    /// Seconds for column weights and weighted co-occurrence tables.
    pub tables_seconds: f64,
    /// Seconds for counting and weighting every quartet.
    pub weigh_seconds: f64,
    pub amalgamate_seconds: f64,
    pub newick: String,
    /// Columns with a positive weight.
    pub columns_used: usize,
    pub table_bytes: usize,
}

/// Weights inside a replicate: W2b, or W2c with this many resamples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inner {
    W2b,
    W2c(u32),
}

/// Replicate `b` on the full matrix `full` with taxa `names`: seed
/// `bootstrap::replicate_seed(global_seed, b)`. Quartets are weighed in
/// parallel and collected in rank order, so the tree does not depend on
/// threads.
pub(crate) fn replicate_tree(
    names: &[String],
    full: &Matrix,
    model: &Model,
    global_seed: u64,
    b: u64,
    inner: Inner,
) -> Replicate {
    let m = names.len();
    let seed = bootstrap::replicate_seed(global_seed, b);
    let t0 = Instant::now();
    let weights = bootstrap::column_weights(full.columns_len(), seed);
    let columns_used = weights.iter().filter(|&&w| w > 0).count();
    let wc = WeightedCounts::new(&full.rows, full.columns_len(), &weights);
    let tables_seconds = t0.elapsed().as_secs_f64();
    let path = PopcountPath::detect();
    let cond = Conditioning::NotAllZero;
    let t1 = Instant::now();
    let q = quartet::quartet_count(m);
    let per_rank: Vec<Option<[f64; 3]>> = (0..q)
        .into_par_iter()
        .map(|r| {
            let counts = wc.pattern_counts(quartet::unrank(r), path);
            match inner {
                Inner::W2c(resamples) => weight::w2c(
                    model,
                    cond,
                    &counts,
                    weight::quartet_seed(seed, r),
                    resamples,
                ),
                Inner::W2b => weight::fit_all(model, cond, &counts)
                    .map(|f| weight::w2b(&f.map(|x| x.log_likelihood))),
            }
        })
        .collect();
    let weigh_seconds = t1.elapsed().as_secs_f64();
    let t2 = Instant::now();
    let mut quartets = Vec::new();
    for (r, w) in per_rank.iter().enumerate() {
        if let Some(w) = w {
            quartets.extend(amalgamate::quartets_of(quartet::unrank(r as u64), *w));
        }
    }
    let result = amalgamate::wqfm(names, &quartets, &Default::default());
    Replicate {
        index: b,
        seed,
        tables_seconds,
        weigh_seconds,
        amalgamate_seconds: t2.elapsed().as_secs_f64(),
        newick: result.newick,
        columns_used,
        table_bytes: wc.bytes(),
    }
}

/// Runs replicate `b` of the finished run in `run_dir`; `w2c` chooses W2c
/// (with the run's number of resamples) or W2b weights.
pub fn replicate(run_dir: &Path, b: u64, w2c: bool) -> Result<Replicate, EngineError> {
    let dir = RunDir::new(run_dir);
    let state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    let config: AnalysisConfig = serde_json::from_value(state.config.clone())
        .map_err(|e| EngineError::Invalid(format!("run.json configuration: {e}")))?;
    let (names, full) = analysis::load_matrix(run_dir)?;
    let model = weight_model(&config, &full)?;
    let inner = if w2c {
        Inner::W2c(config.replicates)
    } else {
        Inner::W2b
    };
    Ok(replicate_tree(&names, &full, &model, config.seed, b, inner))
}

/// The run's tree (`report/tree.nwk`).
pub fn run_tree(run_dir: &Path) -> Result<String, EngineError> {
    let p = RunDir::new(run_dir).report().join("tree.nwk");
    atomic::read_verified(&p)
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .ok_or_else(|| EngineError::Invalid(format!("{} is missing or damaged", p.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{start, AnalysisOptions, WEIGHTING_SYM};
    use crate::progress::NullSink;
    use crate::testutil::TempDir;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn replicates_are_seeded_and_use_weighted_counts() {
        let tmp = TempDir::new("s2");
        let input = tmp.path().join("in");
        std::fs::create_dir_all(&input).unwrap();
        let mut x: u64 = 77;
        let mut next = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 33) as usize
        };
        let base: Vec<u8> = (0..800).map(|_| b"ACGT"[next() % 4]).collect();
        for t in 0..6 {
            let mut s = base.clone();
            for _ in 0..50 {
                let i = next() % 800;
                s[i] = b"ACGT"[next() % 4];
            }
            std::fs::write(
                input.join(format!("T{t}.fasta")),
                format!(">T{t}\n{}\n", String::from_utf8(s).unwrap()),
            )
            .unwrap();
        }
        let run = tmp.path().join("run");
        let options = AnalysisOptions {
            config: AnalysisConfig {
                input: input.display().to_string(),
                records: "per_file".into(),
                strand: true,
                lengths: None,
                seed: 2,
                ml_max_columns: qmaws_core::matrix::MAX_ML_COLUMNS,
                weighting: WEIGHTING_SYM.into(),
                replicates: 5,
                bootstrap: 0,
            },
            chunk_seconds: 3.0,
            chunk_quartets: Some(4),
            memory_limit: Some(1 << 30),
        };
        start(
            &run,
            &options,
            "terminal",
            &NullSink,
            &AtomicBool::new(false),
        )
        .unwrap();
        let a = replicate(&run, 0, true).unwrap();
        let again = replicate(&run, 0, true).unwrap();
        assert_eq!(a.newick, again.newick);
        assert_eq!(a.seed, bootstrap::replicate_seed(2, 0));
        let b = replicate(&run, 1, false).unwrap();
        assert_ne!(a.seed, b.seed);
        let (_, full) = analysis::load_matrix(&run).unwrap();
        assert!(a.columns_used < full.columns_len());
        let tree = qmaws_core::newick::Tree::parse(&b.newick).unwrap();
        assert_eq!(tree.leaf_names().len(), 6);
        assert!(!run_tree(&run).unwrap().is_empty());
    }
}
