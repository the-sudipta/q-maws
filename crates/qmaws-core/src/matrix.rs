//! MAW length selection and binary character matrices, as in ML-MAWS
//! (`EntropySelector.cpp`, `MatrixBuilder.cpp`, `main.cpp` at commit
//! `0c38db1`), with a fixed tie rule where ML-MAWS has none.
//!
//! - Length range: by the integer average sequence length and the number of
//!   taxa ([`adaptive_range`]).
//! - Entropy of a length l: for the matrix of the MAWs of length exactly l,
//!   the sum over variable columns of the binary Shannon entropy
//!   -(p0 log2 p0 + p1 log2 p1), with p1 = (taxa with the MAW) / m.
//! - Selection: the 3 lengths with the highest entropy among lengths with at
//!   least 5 variable columns ([`select_lengths`]).
//! - `M_full`: every MAW of the selected lengths that occurs in at least one
//!   taxon, in lexicographic order; nothing removed.
//! - `M_ml`: `M_full` without constant columns; if more than 50,000 remain,
//!   the 50,000 with the highest min(n_j, m - n_j), ties by column order.

use crate::maw::{compare_words, decode, Codes, MawSet};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// Default number of selected lengths (`topK` in ML-MAWS).
pub const TOP_K: usize = 3;
/// Minimum variable columns for a length to be preferred (`minChars`).
pub const MIN_CHARS: usize = 5;
/// Column limit of `M_ml` (`maxChars`).
pub const MAX_ML_COLUMNS: usize = 50_000;

/// Candidate MAW length range `[lmin, lmax]` (ML-MAWS `computeAdaptiveRange`).
/// `avg_len` is the integer average cleaned length (total ÷ number of taxa).
pub fn adaptive_range(avg_len: u64, taxa: usize) -> (usize, usize) {
    let (lmin, mut lmax) = match avg_len {
        0..=499 => (2, 6),
        500..=1_999 => (2, 8),
        2_000..=49_999 => (3, 10),
        50_000..=499_999 => (4, 12),
        _ => (5, 14),
    };
    if taxa > 50 {
        lmax = (lmax + 2).min(16);
    }
    if taxa > 100 {
        lmax = (lmax + 2).min(18);
    }
    (lmin, lmax)
}

/// Iterates over the distinct words of one length across taxa, in order,
/// giving each word with the (ascending) indices of the taxa that have it.
pub struct LengthMerge<'a> {
    lists: Vec<&'a Codes>,
    pos: Vec<usize>,
    heap: BinaryHeap<Reverse<(u128, u32)>>,
}

impl<'a> LengthMerge<'a> {
    pub fn new(sets: &'a [MawSet], len: usize) -> Self {
        let lists: Vec<&Codes> = sets
            .iter()
            .map(|s| s.of_length(len).expect("length within the extracted range"))
            .collect();
        let mut heap = BinaryHeap::with_capacity(lists.len());
        for (t, l) in lists.iter().enumerate() {
            if !l.is_empty() {
                heap.push(Reverse((l.get(0), t as u32)));
            }
        }
        LengthMerge {
            pos: vec![0; lists.len()],
            lists,
            heap,
        }
    }

    /// The next word and its taxa (written into `taxa`, which is cleared).
    pub fn next_into(&mut self, taxa: &mut Vec<u32>) -> Option<u128> {
        taxa.clear();
        let Reverse((code, _)) = *self.heap.peek()?;
        while let Some(&Reverse((c, t))) = self.heap.peek() {
            if c != code {
                break;
            }
            self.heap.pop();
            taxa.push(t);
            let ti = t as usize;
            self.pos[ti] += 1;
            if self.pos[ti] < self.lists[ti].len() {
                self.heap
                    .push(Reverse((self.lists[ti].get(self.pos[ti]), t)));
            }
        }
        taxa.sort_unstable();
        Some(code)
    }
}

/// Binary entropy of a column with `ones` of `m` taxa (0 for constant columns).
pub fn column_entropy(ones: usize, m: usize) -> f64 {
    if ones == 0 || ones == m {
        return 0.0;
    }
    let p1 = ones as f64 / m as f64;
    let p0 = 1.0 - p1;
    -(p0 * libm::log2(p0) + p1 * libm::log2(p1))
}

/// Entropy of one candidate length.
#[derive(Debug, Clone, PartialEq)]
pub struct LengthEntropy {
    pub length: usize,
    /// Variable columns of the length's matrix.
    pub characters: usize,
    pub entropy: f64,
}

/// Entropy of each length in `[lmin, lmax]`; every set must cover the range.
pub fn length_entropies(sets: &[MawSet], lmin: usize, lmax: usize) -> Vec<LengthEntropy> {
    let m = sets.len();
    let mut taxa = Vec::new();
    (lmin..=lmax)
        .map(|len| {
            let mut merge = LengthMerge::new(sets, len);
            let mut characters = 0;
            let mut entropy = 0.0;
            while merge.next_into(&mut taxa).is_some() {
                if taxa.len() < m {
                    characters += 1;
                    entropy += column_entropy(taxa.len(), m);
                }
            }
            LengthEntropy {
                length: len,
                characters,
                entropy,
            }
        })
        .collect()
}

/// The selected lengths, ascending (ML-MAWS `selectTopLengths` with
/// `topK` = 3 and `minChars` = 5, falling back to the single best length).
/// Ties in entropy are broken by the shorter length.
pub fn select_lengths(results: &[LengthEntropy], top_k: usize, min_chars: usize) -> Vec<usize> {
    let mut by_length: Vec<&LengthEntropy> = results.iter().collect();
    by_length.sort_by_key(|r| r.length);
    let mut candidates: Vec<&LengthEntropy> = by_length
        .iter()
        .copied()
        .filter(|r| r.characters >= min_chars && r.entropy > 0.0)
        .collect();
    if candidates.len() < top_k {
        candidates = by_length
            .iter()
            .copied()
            .filter(|r| r.characters > 0 && r.entropy > 0.0)
            .collect();
    }
    // Stable sort: equal entropies keep ascending length order.
    candidates.sort_by(|a, b| b.entropy.partial_cmp(&a.entropy).unwrap_or(Ordering::Equal));
    let mut selected: Vec<usize> = candidates.iter().take(top_k).map(|r| r.length).collect();
    selected.sort_unstable();
    if selected.is_empty() {
        // selectBestLength: the first length with the highest entropy.
        let mut best = by_length.first().map(|r| (r.length, r.entropy));
        for r in &by_length {
            if best.is_some_and(|(_, e)| r.entropy > e) {
                best = Some((r.length, r.entropy));
            }
        }
        if let Some((l, _)) = best {
            selected.push(l);
        }
    }
    selected
}

/// A binary character matrix: one column per MAW, one bitset row per taxon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    pub taxa: usize,
    /// Length of each column's word.
    pub lens: Vec<u8>,
    /// Code of each column's word (`u64` when every word has at most 32
    /// letters, `u128` otherwise).
    pub codes: Codes,
    /// Number of taxa with value 1, per column.
    pub ones: Vec<u32>,
    /// `rows[t]` is the bitset of taxon t: bit j of word j/64 is column j.
    /// Padded to a multiple of 4 words.
    pub rows: Vec<Vec<u64>>,
}

impl Matrix {
    pub fn columns_len(&self) -> usize {
        self.lens.len()
    }

    pub fn get(&self, taxon: usize, column: usize) -> bool {
        (self.rows[taxon][column / 64] >> (column % 64)) & 1 == 1
    }

    /// The MAW of a column as letters.
    pub fn word(&self, column: usize) -> String {
        decode(self.codes.get(column), self.lens[column] as usize)
    }

    /// Appends the MAW of a column and a line break to `out`.
    pub fn push_word(&self, column: usize, out: &mut String) {
        let len = self.lens[column] as usize;
        let code = self.codes.get(column);
        for i in 0..len {
            out.push(b"ACGT"[((code >> (2 * (len - 1 - i))) & 3) as usize] as char);
        }
        out.push('\n');
    }

    /// An empty matrix with room for `columns` columns; `wide` when some
    /// word is longer than 32 letters.
    fn with_capacity(taxa: usize, columns: usize, wide: bool) -> Self {
        let words = columns.div_ceil(64).div_ceil(4) * 4;
        Matrix {
            taxa,
            lens: Vec::with_capacity(columns),
            codes: if wide {
                Codes::Long(Vec::with_capacity(columns))
            } else {
                Codes::Short(Vec::with_capacity(columns))
            },
            ones: Vec::with_capacity(columns),
            rows: vec![vec![0u64; words]; taxa],
        }
    }

    fn push(&mut self, len: u8, code: u128, present: &[u32]) {
        let j = self.lens.len();
        for &t in present {
            self.rows[t as usize][j / 64] |= 1 << (j % 64);
        }
        self.lens.push(len);
        match &mut self.codes {
            Codes::Short(v) => v.push(code as u64),
            Codes::Long(v) => v.push(code),
        }
        self.ones.push(present.len() as u32);
    }

    /// The matrix restricted to `keep` (column indices in ascending order).
    pub fn select(&self, keep: &[usize]) -> Matrix {
        let wide = matches!(self.codes, Codes::Long(_));
        let mut out = Matrix::with_capacity(self.taxa, keep.len(), wide);
        let mut present = Vec::with_capacity(self.taxa);
        for &j in keep {
            present.clear();
            present.extend((0..self.taxa).filter(|&t| self.get(t, j)).map(|t| t as u32));
            out.push(self.lens[j], self.codes.get(j), &present);
        }
        out
    }

    /// Estimated memory of a matrix with these dimensions, in bytes
    /// (rows, word lengths and codes, column counts).
    pub fn estimate_bytes(taxa: usize, columns: usize, wide: bool) -> u64 {
        let row_words = columns.div_ceil(64).div_ceil(4) * 4;
        let per_column = 1 + if wide { 16 } else { 8 } + 4;
        (taxa as u64) * (row_words as u64) * 8 + (columns as u64) * per_column
    }
}

/// Merges the word streams of several lengths in lexicographic order.
struct MultiMerge<'a> {
    merges: Vec<(usize, LengthMerge<'a>, Option<u128>, Vec<u32>)>,
}

impl<'a> MultiMerge<'a> {
    fn new(sets: &'a [MawSet], lengths: &[usize]) -> Self {
        let merges = lengths
            .iter()
            .map(|&len| {
                let mut m = LengthMerge::new(sets, len);
                let mut taxa = Vec::new();
                let head = m.next_into(&mut taxa);
                (len, m, head, taxa)
            })
            .collect();
        MultiMerge { merges }
    }

    /// Calls `f(len, code, taxa)` for every word, in lexicographic order.
    fn for_each(mut self, mut f: impl FnMut(usize, u128, &[u32])) {
        loop {
            let mut best: Option<usize> = None;
            for (i, (len, _, head, _)) in self.merges.iter().enumerate() {
                let Some(code) = head else { continue };
                best = match best {
                    None => Some(i),
                    Some(b) => {
                        let (bl, _, bh, _) = &self.merges[b];
                        if compare_words(*code, *len, bh.expect("head"), *bl) == Ordering::Less {
                            Some(i)
                        } else {
                            Some(b)
                        }
                    }
                };
            }
            let Some(i) = best else { break };
            let (len, merge, head, taxa) = &mut self.merges[i];
            f(*len, head.expect("head"), taxa);
            *head = merge.next_into(taxa);
        }
    }
}

/// Number of columns `M_full` will have.
pub fn count_columns(sets: &[MawSet], lengths: &[usize]) -> usize {
    let mut n = 0;
    MultiMerge::new(sets, lengths).for_each(|_, _, _| n += 1);
    n
}

/// `M_full`: all MAWs of the selected lengths, in lexicographic order.
pub fn build_full(sets: &[MawSet], lengths: &[usize]) -> Matrix {
    let columns = count_columns(sets, lengths);
    let wide = lengths.iter().any(|&l| l > 32);
    let mut m = Matrix::with_capacity(sets.len(), columns, wide);
    MultiMerge::new(sets, lengths).for_each(|len, code, taxa| m.push(len as u8, code, taxa));
    m
}

/// Column indices of `M_ml` within `M_full`: variable columns, capped to
/// `max_columns` by min(n_j, m - n_j) (descending), ties by index.
pub fn ml_columns(full: &Matrix, max_columns: usize) -> Vec<usize> {
    let m = full.taxa as u32;
    let variable: Vec<usize> = (0..full.columns_len())
        .filter(|&j| full.ones[j] > 0 && full.ones[j] < m)
        .collect();
    if max_columns == 0 || variable.len() <= max_columns {
        return variable;
    }
    let mut scored: Vec<(u32, usize)> = variable
        .iter()
        .map(|&j| (full.ones[j].min(m - full.ones[j]), j))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.truncate(max_columns);
    let mut keep: Vec<usize> = scored.into_iter().map(|(_, j)| j).collect();
    keep.sort_unstable();
    keep
}

/// Taxon name as ML-MAWS writes it to PHYLIP: space, tab, ( ) [ ] : ; ,
/// replaced by `_`, truncated to 50 characters.
pub fn phylip_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            ' ' | '\t' | '(' | ')' | '[' | ']' | ':' | ';' | ',' => '_',
            other => other,
        })
        .take(50)
        .collect()
}

/// Relaxed PHYLIP text in ML-MAWS's layout: `m n`, then per taxon its name,
/// two spaces and the 0/1 row.
pub fn to_phylip(matrix: &Matrix, names: &[String]) -> String {
    let n = matrix.columns_len();
    let mut out = String::with_capacity(names.len() * (n + 60) + 32);
    out.push_str(&format!("{} {}\n", matrix.taxa, n));
    for (t, name) in names.iter().enumerate() {
        out.push_str(&phylip_name(name));
        out.push_str("  ");
        for j in 0..n {
            out.push(if matrix.get(t, j) { '1' } else { '0' });
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maw::{extract, extract_strand_aware};

    fn sets(seqs: &[&str], lmin: usize, lmax: usize) -> Vec<MawSet> {
        seqs.iter()
            .map(|s| extract(s.as_bytes(), lmin, lmax).unwrap())
            .collect()
    }

    const WORKSHEET: [&str; 5] = ["AACGTA", "ACGTAG", "ACGTTC", "CATTGC", "CATGGC"];

    #[test]
    fn ranges_follow_the_ml_maws_table() {
        assert_eq!(adaptive_range(499, 4), (2, 6));
        assert_eq!(adaptive_range(500, 4), (2, 8));
        assert_eq!(adaptive_range(16_800, 25), (3, 10));
        assert_eq!(adaptive_range(49_999, 25), (3, 10));
        assert_eq!(adaptive_range(50_000, 25), (4, 12));
        assert_eq!(adaptive_range(4_800_000, 29), (5, 14));
        assert_eq!(adaptive_range(4_800_000, 51), (5, 16));
        assert_eq!(adaptive_range(4_800_000, 101), (5, 18));
        assert_eq!(adaptive_range(7_000, 116), (3, 14));
    }

    #[test]
    fn worksheet_matrix_after_constant_removal() {
        // Appendix A: 16 two-letter words, CC, CT and GA are absent from every
        // sequence (all-1 columns, removed), 13 columns remain.
        let s = sets(&WORKSHEET, 2, 2);
        let full = build_full(&s, &[2]);
        assert_eq!(full.columns_len(), 16);
        let keep = ml_columns(&full, MAX_ML_COLUMNS);
        let ml = full.select(&keep);
        let words: Vec<String> = (0..ml.columns_len()).map(|j| ml.word(j)).collect();
        assert_eq!(
            words,
            ["AA", "AC", "AG", "AT", "CA", "CG", "GC", "GG", "GT", "TA", "TC", "TG", "TT"]
        );
        // Columns AA..TT; the appendix lists them as rows, so transpose.
        let table = [
            "01111", "00011", "10111", "11100", "11100", "00011", "11100", "11110", "00011",
            "00111", "11011", "11100", "11001",
        ];
        for t in 0..5 {
            let row: String = (0..13)
                .map(|j| if ml.get(t, j) { '1' } else { '0' })
                .collect();
            let from_table: String = table.iter().map(|c| c.as_bytes()[t] as char).collect();
            assert_eq!(row, from_table, "taxon {t}");
        }
        let names: Vec<String> = ["K", "L", "M", "N", "P"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let phy = to_phylip(&ml, &names);
        assert!(
            phy.starts_with("5 13\nK  0011101100111\nL  1001101100111\n"),
            "{phy}"
        );
    }

    #[test]
    fn entropy_and_selection() {
        assert_eq!(column_entropy(0, 5), 0.0);
        assert_eq!(column_entropy(5, 5), 0.0);
        assert!((column_entropy(2, 4) - 1.0).abs() < 1e-15);
        let r = |length, characters, entropy| LengthEntropy {
            length,
            characters,
            entropy,
        };
        // Top 3 by entropy among lengths with at least 5 characters.
        let res = vec![
            r(3, 100, 5.0),
            r(4, 100, 9.0),
            r(5, 100, 7.0),
            r(6, 4, 99.0),
            r(7, 100, 8.0),
        ];
        assert_eq!(select_lengths(&res, 3, 5), vec![4, 5, 7]);
        // Fewer than 3 candidates: the 5-character rule is relaxed.
        let res = vec![r(3, 2, 1.0), r(4, 9, 2.0), r(5, 0, 0.0)];
        assert_eq!(select_lengths(&res, 3, 5), vec![3, 4]);
        // Ties: the shorter length wins.
        let res = vec![r(3, 10, 2.0), r(4, 10, 2.0), r(5, 10, 2.0), r(6, 10, 2.0)];
        assert_eq!(select_lengths(&res, 3, 5), vec![3, 4, 5]);
        // Nothing variable: the first length (selectBestLength).
        let res = vec![r(3, 0, 0.0), r(4, 0, 0.0)];
        assert_eq!(select_lengths(&res, 3, 5), vec![3]);
    }

    #[test]
    fn length_entropy_counts_variable_columns() {
        let s = sets(&WORKSHEET, 2, 3);
        let e = length_entropies(&s, 2, 3);
        assert_eq!(e[0].length, 2);
        assert_eq!(e[0].characters, 13);
        // Column counts of A.4: ones per column.
        let ones = [4usize, 2, 4, 3, 3, 2, 3, 4, 2, 3, 4, 3, 3];
        let want: f64 = ones.iter().map(|&k| column_entropy(k, 5)).sum();
        assert!((e[0].entropy - want).abs() < 1e-12);
    }

    #[test]
    fn full_matrix_mixes_lengths_in_lexicographic_order() {
        let seqs = ["ACGTACGGTCA", "AACCGGTTAC", "ACGTTGCAAG", "CATTGCAGGT"];
        let s: Vec<MawSet> = seqs
            .iter()
            .map(|x| extract_strand_aware(x.as_bytes(), 2, 4).unwrap())
            .collect();
        let full = build_full(&s, &[2, 3, 4]);
        let words: Vec<String> = (0..full.columns_len()).map(|j| full.word(j)).collect();
        let mut sorted = words.clone();
        sorted.sort();
        assert_eq!(words, sorted);
        let mut union: Vec<String> = s.iter().flat_map(|m| m.to_strings()).collect();
        union.sort();
        union.dedup();
        assert_eq!(words, union);
        for (j, w) in words.iter().enumerate() {
            for (t, m) in s.iter().enumerate() {
                assert_eq!(full.get(t, j), m.to_strings().contains(w), "{w} taxon {t}");
            }
            assert_eq!(
                full.ones[j] as usize,
                (0..4).filter(|&t| full.get(t, j)).count()
            );
        }
        assert_eq!(count_columns(&s, &[2, 3, 4]), full.columns_len());
        assert_eq!(full.rows[0].len() % 4, 0);
    }

    #[test]
    fn ml_cap_keeps_the_most_informative_columns_in_order() {
        // 6 taxa; columns with ones = 1, 3, 2, 6 (constant), 3, 5, 0 (constant).
        let ones = [1u32, 3, 2, 6, 3, 5, 0];
        let mut full = Matrix::with_capacity(6, ones.len(), false);
        for (j, &k) in ones.iter().enumerate() {
            let present: Vec<u32> = (0..k).collect();
            full.push(2, j as u128, &present);
        }
        assert_eq!(ml_columns(&full, 0), vec![0, 1, 2, 4, 5]);
        // Scores min(n, 6-n): 1, 3, 2, -, 3, 1 -> top 3: columns 1, 4, 2.
        assert_eq!(ml_columns(&full, 3), vec![1, 2, 4]);
        // Ties at score 1 (columns 0 and 5): the lower index wins.
        assert_eq!(ml_columns(&full, 4), vec![0, 1, 2, 4]);
    }

    #[test]
    fn phylip_names_follow_ml_maws() {
        assert_eq!(phylip_name("A b(c)[d]:e;f,g\th"), "A_b_c__d__e_f_g_h");
        assert_eq!(phylip_name(&"x".repeat(60)).len(), 50);
    }
}
