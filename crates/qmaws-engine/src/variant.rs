//! Weight variants of a finished run (plan Part 8, item 6; M12 sensitivity
//! analysis, docs/PREREGISTRATION.md Amendment 3): the tree is made again
//! from the run's stored quartet results with W1, W2a, W2b or W2c weights,
//! with S1 and halo values, and compared with the run's reference tree.
//! Nothing is recomputed from the sequences, so a variant takes minutes.
//!
//! - W1: votes of the split patterns, from the stored pattern counts;
//! - W2a: the best topology only, weighted by its log-likelihood lead;
//! - W2b: likelihood weights exp(l - max), normalised;
//! - W2c: the resampled weights the run used.
//!
//! Results are exploratory. The run folder is not changed.

use crate::analysis::{InputsRecord, QUARTET_COUNT, QUARTET_WEIGHT};
use crate::atomic;
use crate::rundir::RunDir;
use crate::runner::{io_err, EngineError};
use crate::state::RunState;
use crate::store;
use qmaws_core::{amalgamate, quartet, support, weight};
use std::path::Path;

/// The weights a variant uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weights {
    W1,
    W2a,
    W2b,
    W2c,
}

impl Weights {
    pub fn name(self) -> &'static str {
        match self {
            Weights::W1 => "w1",
            Weights::W2a => "w2a",
            Weights::W2b => "w2b",
            Weights::W2c => "w2c",
        }
    }
}

/// Summary of a variant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VariantSummary {
    pub run: String,
    pub run_root: String,
    pub weights: String,
    pub weighted_quartets: usize,
    pub quartets_without_weight: u64,
    pub newick: String,
    pub evaluation: Option<crate::evaluation::Evaluation>,
}

fn read_verified(path: &Path) -> Result<Vec<u8>, EngineError> {
    atomic::read_verified(path)
        .ok_or_else(|| EngineError::Invalid(format!("{} is missing or damaged", path.display())))
}

/// The per-rank weights of a stored, finished run.
fn rank_weights(
    dir: &RunDir,
    state: &RunState,
    q: u64,
    weights: Weights,
) -> Result<Vec<Option<[f64; 3]>>, EngineError> {
    let mut by_rank: Vec<Option<[f64; 3]>> = vec![None; q as usize];
    let stage = if weights == Weights::W1 {
        QUARTET_COUNT
    } else {
        QUARTET_WEIGHT
    };
    let plan =
        state.chunk_plans.get(stage).copied().ok_or_else(|| {
            EngineError::Invalid(format!("the run has no chunk plan for {stage}"))
        })?;
    for i in 0..plan.chunk_count() {
        let bytes = read_verified(&dir.chunk_file(stage, i))?;
        if weights == Weights::W1 {
            for (rank, c) in
                store::count_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?
            {
                by_rank[rank as usize] = weight::w1(&c.map(u64::from));
            }
        } else {
            for r in
                store::weight_records(&bytes).map_err(|e| EngineError::Invalid(e.to_string()))?
            {
                if !r.fitted() {
                    continue;
                }
                by_rank[r.rank as usize] = Some(match weights {
                    Weights::W2a => {
                        let (best, gap) = weight::w2a(&r.log_likelihoods);
                        let mut w = [0.0; 3];
                        w[best] = gap;
                        w
                    }
                    Weights::W2b => weight::w2b(&r.log_likelihoods),
                    _ => match r.tree_weights() {
                        Some(w) if r.resampled() => w,
                        _ => {
                            return Err(EngineError::Invalid(
                                "the run made no W2c resamples".into(),
                            ))
                        }
                    },
                });
            }
        }
    }
    Ok(by_rank)
}

/// Makes the variant of the finished run `run_dir` and writes it to
/// `output`: `tree.nwk`, `tree_s1.nwk`, `support.tsv`, `halo.tsv` and
/// `variant.json` (with the comparison against the run's reference tree,
/// when it has one), each with its SHA-256.
pub fn make(
    run_dir: &Path,
    weights: Weights,
    output: &Path,
) -> Result<VariantSummary, EngineError> {
    let dir = RunDir::new(run_dir);
    let state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    let root = atomic::read_verified(&dir.audit().join("root.txt"))
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .ok_or_else(|| EngineError::Invalid("the run has not finished".into()))?;
    let inputs: InputsRecord =
        serde_json::from_slice(&read_verified(&dir.audit().join("inputs.json"))?)
            .map_err(|e| EngineError::Invalid(format!("inputs.json: {e}")))?;
    let names: Vec<String> = inputs.taxa.iter().map(|t| t.name.clone()).collect();
    let q = quartet::quartet_count(names.len());
    let by_rank = rank_weights(&dir, &state, q, weights)?;
    let mut quartets = Vec::new();
    let mut without = 0u64;
    for (rank, w) in by_rank.iter().enumerate() {
        match w {
            Some(w) => quartets.extend(amalgamate::quartets_of(quartet::unrank(rank as u64), *w)),
            None => without += 1,
        }
    }
    let result = amalgamate::wqfm(&names, &quartets, &amalgamate::Settings::default());
    let mut acc =
        support::Accumulator::new(&names, &result.newick).map_err(EngineError::Invalid)?;
    for (rank, w) in by_rank.iter().enumerate() {
        if let Some(w) = w {
            acc.add(quartet::unrank(rank as u64), *w);
        }
    }
    drop(by_rank);
    let s = acc.finish();
    std::fs::create_dir_all(output).map_err(io_err(output))?;
    let write = |name: &str, text: String| -> Result<(), EngineError> {
        let p = output.join(name);
        atomic::write_verified(&p, text.as_bytes()).map_err(io_err(&p))?;
        Ok(())
    };
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
    write("tree.nwk", format!("{}\n", result.newick))?;
    write("tree_s1.nwk", format!("{}\n", s.newick))?;
    write("support.tsv", edges)?;
    write("halo.tsv", halo)?;
    // The comparison uses the run's stored reference, the same as the run's
    // own evaluation (OI-19).
    let evaluation = match crate::evaluation::stored_reference(run_dir)? {
        Some(r) => {
            let tree = qmaws_core::newick::Tree::parse(&result.newick)
                .map_err(|e| EngineError::Invalid(e.to_string()))?;
            let c = crate::figures::comparison(&tree, &r.tree).map_err(EngineError::Invalid)?;
            let m = names.len();
            Some(crate::evaluation::Evaluation {
                reference_label: r.info.label,
                reference_sha256: r.info.sha256,
                tree_file: "tree.nwk".into(),
                tree_sha256: crate::hash::sha256_hex(format!("{}\n", result.newick).as_bytes()),
                taxa: m,
                rf_splits_differ: c.rf,
                rf_max: 2 * m.saturating_sub(3),
                nrf: c.nrf,
                nqd_differ: c.nqd.differ,
                nqd_compared: c.nqd.compared,
                nqd_unresolved_in_reference: c.nqd.unresolved_in_reference,
                nqd: c.nqd.value,
                msd: c.msd,
            })
        }
        None => None,
    };
    let summary = VariantSummary {
        run: run_dir.display().to_string(),
        run_root: root,
        weights: weights.name().into(),
        weighted_quartets: quartets.len(),
        quartets_without_weight: without,
        newick: result.newick,
        evaluation,
    };
    write(
        "variant.json",
        serde_json::to_string_pretty(&summary).expect("serialises") + "\n",
    )?;
    Ok(summary)
}
