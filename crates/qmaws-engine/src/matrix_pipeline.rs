//! From cleaned sequences to the MAW matrices: MAW extraction (in parallel,
//! within a memory limit), length selection by entropy, `M_full` and `M_ml`.

use qmaws_core::matrix::{self, LengthEntropy, Matrix};
use qmaws_core::maw::{self, MawSet};
use rayon::prelude::*;
use serde::Serialize;
use std::time::Instant;

/// Default share of the available memory the pipeline may use.
pub const DEFAULT_MEMORY_SHARE: f64 = 0.70;

/// MAW lists kept per letter of input while all taxa are extracted (bytes);
/// measured on E. coli with the strand filter: about 2.3 bytes per letter.
const HELD_BYTES_PER_LETTER: u64 = 3;

/// Settings of a matrix build.
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixOptions {
    /// Strand filter: keep MAWs whose reverse complement is also a MAW.
    pub strand: bool,
    /// Fixed lengths instead of the entropy selection.
    pub lengths: Option<Vec<usize>>,
    /// Memory limit in bytes; `None` measures 70% of the available memory.
    pub memory_limit: Option<u64>,
    /// Column limit of `M_ml` (0 = no limit).
    pub ml_max_columns: usize,
}

impl Default for MatrixOptions {
    fn default() -> Self {
        Self {
            strand: true,
            lengths: None,
            memory_limit: None,
            ml_max_columns: matrix::MAX_ML_COLUMNS,
        }
    }
}

/// Everything the pipeline computed, with measurements.
#[derive(Debug, Clone, Serialize)]
pub struct MatrixSummary {
    pub taxa: usize,
    pub average_length: u64,
    pub strand: bool,
    pub lmin: usize,
    pub lmax: usize,
    pub memory_limit_bytes: u64,
    pub workers: usize,
    /// MAWs per taxon over `[lmin, lmax]` (after the strand filter).
    pub maws_per_taxon: Vec<usize>,
    pub entropies: Vec<EntropyRow>,
    pub selected_lengths: Vec<usize>,
    pub full_columns: usize,
    pub full_estimated_bytes: u64,
    pub ml_columns: usize,
    pub extraction_seconds: f64,
    pub selection_seconds: f64,
    pub matrix_seconds: f64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct EntropyRow {
    pub length: usize,
    pub characters: usize,
    pub entropy: f64,
}

impl From<&LengthEntropy> for EntropyRow {
    fn from(e: &LengthEntropy) -> Self {
        Self {
            length: e.length,
            characters: e.characters,
            entropy: e.entropy,
        }
    }
}

/// Result of [`build_matrices`].
pub struct Matrices {
    pub full: Matrix,
    /// Column indices of `M_ml` within `M_full`.
    pub ml_columns: Vec<usize>,
    pub summary: MatrixSummary,
}

#[derive(Debug)]
pub enum PipelineError {
    TooFewTaxa(usize),
    Maw(maw::MawError),
    /// The matrix would not fit in the memory limit (decision D3).
    MatrixTooLarge {
        columns: usize,
        estimated_bytes: u64,
        limit_bytes: u64,
    },
    NoVariableCharacters,
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::TooFewTaxa(n) => write!(f, "at least 4 taxa are needed; got {n}"),
            PipelineError::Maw(e) => write!(f, "{e}"),
            PipelineError::MatrixTooLarge {
                columns,
                estimated_bytes,
                limit_bytes,
            } => write!(
                f,
                "the full matrix would have {columns} columns and need about {} MB, more than the memory limit of {} MB",
                estimated_bytes / 1_000_000,
                limit_bytes / 1_000_000
            ),
            PipelineError::NoVariableCharacters => {
                write!(f, "no variable characters; try another length range")
            }
        }
    }
}

impl std::error::Error for PipelineError {}

/// The memory limit: 70% of the memory available now.
pub fn measured_memory_limit() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    (sys.available_memory() as f64 * DEFAULT_MEMORY_SHARE) as u64
}

/// Peak memory of extracting one sequence of `n` letters (suffix automaton
/// of at most 2n states of 24 bytes, built twice in turn with the strand
/// filter, plus MAW lists); a deliberately generous bound.
pub fn extraction_bytes(n: usize) -> u64 {
    (n as u64) * 64 + (1 << 20)
}

/// Builds `M_full` and the `M_ml` column selection. The sequences are
/// released as soon as their MAWs are extracted. `log` receives progress
/// lines.
pub fn build_matrices(
    names: &[String],
    sequences: Vec<Vec<u8>>,
    options: &MatrixOptions,
    log: &(dyn Fn(String) + Sync),
) -> Result<Matrices, PipelineError> {
    let m = sequences.len();
    if m < 4 {
        return Err(PipelineError::TooFewTaxa(m));
    }
    let total: u64 = sequences.iter().map(|s| s.len() as u64).sum();
    let average_length = total / m as u64;
    let (lmin, lmax) = match &options.lengths {
        Some(l) if !l.is_empty() => (*l.iter().min().unwrap(), *l.iter().max().unwrap()),
        _ => matrix::adaptive_range(average_length, m),
    };
    let limit = options.memory_limit.unwrap_or_else(measured_memory_limit);
    let longest = sequences.iter().map(|s| s.len()).max().unwrap_or(0);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    // Memory already committed: the sequences and the MAW lists of all taxa.
    let held = total + total * HELD_BYTES_PER_LETTER;
    let per_worker = extraction_bytes(longest).max(1);
    let room = limit.saturating_sub(held);
    let workers = ((room / per_worker) as usize).clamp(1, cores);
    log(format!(
        "{m} taxa, average length {average_length}; MAW lengths {lmin} to {lmax}; strand filter {}; {workers} parallel extraction(s) within a memory limit of {} MB",
        if options.strand { "on" } else { "off" },
        limit / 1_000_000
    ));
    if held + per_worker > limit {
        log(format!(
            "Warning: even one extraction at a time may need about {} MB, more than the memory limit",
            (held + per_worker).div_ceil(1_000_000)
        ));
    }

    let t = Instant::now();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .expect("thread pool");
    let sets: Vec<Result<MawSet, maw::MawError>> = pool.install(|| {
        sequences
            .into_par_iter()
            .enumerate()
            .map(|(i, seq)| {
                let r = if options.strand {
                    maw::extract_strand_aware(&seq, lmin, lmax)
                } else {
                    maw::extract(&seq, lmin, lmax)
                };
                drop(seq);
                if let Ok(set) = &r {
                    log(format!("  {}: {} MAWs", names[i], set.count()));
                }
                r
            })
            .collect()
    });
    let mut sets: Vec<MawSet> = sets
        .into_iter()
        .collect::<Result<_, _>>()
        .map_err(PipelineError::Maw)?;
    let extraction_seconds = t.elapsed().as_secs_f64();
    let maws_per_taxon: Vec<usize> = sets.iter().map(MawSet::count).collect();

    let t = Instant::now();
    let entropies = matrix::length_entropies(&sets, lmin, lmax);
    let selected = match &options.lengths {
        Some(l) if !l.is_empty() => {
            let mut l = l.clone();
            l.sort_unstable();
            l.dedup();
            l
        }
        _ => matrix::select_lengths(&entropies, matrix::TOP_K, matrix::MIN_CHARS),
    };
    let selection_seconds = t.elapsed().as_secs_f64();
    log(format!("Selected lengths: {selected:?}"));
    for s in &mut sets {
        s.keep_lengths(&selected);
    }

    let t = Instant::now();
    let columns = matrix::count_columns(&sets, &selected);
    let wide = selected.iter().any(|&l| l > 32);
    let estimated = Matrix::estimate_bytes(m, columns, wide);
    log(format!(
        "Full matrix: {m} taxa x {columns} columns, estimated {} MB",
        estimated.div_ceil(1_000_000)
    ));
    if estimated > limit {
        return Err(PipelineError::MatrixTooLarge {
            columns,
            estimated_bytes: estimated,
            limit_bytes: limit,
        });
    }
    let full = matrix::build_full(&sets, &selected);
    drop(sets);
    let ml_columns = matrix::ml_columns(&full, options.ml_max_columns);
    if ml_columns.is_empty() {
        return Err(PipelineError::NoVariableCharacters);
    }
    let matrix_seconds = t.elapsed().as_secs_f64();
    log(format!("M_ml: {} columns", ml_columns.len()));

    let summary = MatrixSummary {
        taxa: m,
        average_length,
        strand: options.strand,
        lmin,
        lmax,
        memory_limit_bytes: limit,
        workers,
        maws_per_taxon,
        entropies: entropies.iter().map(EntropyRow::from).collect(),
        selected_lengths: selected,
        full_columns: full.columns_len(),
        full_estimated_bytes: estimated,
        ml_columns: ml_columns.len(),
        extraction_seconds,
        selection_seconds,
        matrix_seconds,
    };
    Ok(Matrices {
        full,
        ml_columns,
        summary,
    })
}

/// The entropy table in ML-MAWS's `entropy_results.tsv` layout.
pub fn entropy_tsv(rows: &[EntropyRow]) -> String {
    let mut out = String::from("Length\tCharacters\tEntropy\n");
    for r in rows {
        out.push_str(&format!("{}\t{}\t{}\n", r.length, r.characters, r.entropy));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(seqs: &[&str]) -> Vec<Vec<u8>> {
        seqs.iter().map(|s| s.as_bytes().to_vec()).collect()
    }

    #[test]
    fn worksheet_sequences_with_fixed_length_two() {
        let names: Vec<String> = ["K", "L", "M", "N", "P"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let seqs = owned(&["AACGTA", "ACGTAG", "ACGTTC", "CATTGC", "CATGGC"]);
        let options = MatrixOptions {
            strand: false,
            lengths: Some(vec![2]),
            memory_limit: Some(1 << 30),
            ml_max_columns: matrix::MAX_ML_COLUMNS,
        };
        let out = build_matrices(&names, seqs, &options, &|_| {}).unwrap();
        assert_eq!(out.full.columns_len(), 16);
        assert_eq!(out.ml_columns.len(), 13);
        assert_eq!(out.summary.selected_lengths, vec![2]);
        assert_eq!(out.summary.maws_per_taxon.len(), 5);
    }

    #[test]
    fn a_matrix_over_the_memory_limit_is_refused() {
        let names: Vec<String> = (0..4).map(|i| format!("t{i}")).collect();
        let seqs = owned(&["ACGTACGTTA", "AACCGGTTAC", "ACGTTGCAAG", "CATTGCAGGT"]);
        let options = MatrixOptions {
            strand: false,
            lengths: Some(vec![2, 3]),
            memory_limit: Some(10),
            ml_max_columns: 0,
        };
        assert!(matches!(
            build_matrices(&names, seqs, &options, &|_| {}),
            Err(PipelineError::MatrixTooLarge { .. })
        ));
    }

    #[test]
    fn too_few_taxa_are_refused() {
        let names: Vec<String> = (0..3).map(|i| format!("t{i}")).collect();
        let seqs = owned(&["ACGT", "AACC", "GGTT"]);
        assert!(matches!(
            build_matrices(&names, seqs, &MatrixOptions::default(), &|_| {}),
            Err(PipelineError::TooFewTaxa(3))
        ));
    }

    #[test]
    fn entropy_table_layout() {
        let rows = vec![EntropyRow {
            length: 7,
            characters: 120,
            entropy: 55.5,
        }];
        assert_eq!(
            entropy_tsv(&rows),
            "Length\tCharacters\tEntropy\n7\t120\t55.5\n"
        );
    }
}
