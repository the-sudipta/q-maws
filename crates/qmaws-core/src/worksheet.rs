//! Worksheet of one quartet: every number from the co-occurrence counts to
//! the final weights, written out so that it can be checked by hand. Used
//! for the audit samples and for the verification of a single quartet.

use crate::quartet::{self, CoCounts, PatternCounts};
use crate::weight::{self, Conditioning, Model, QuartetWeights, TOPOLOGY_NAMES};
use std::fmt::Write as _;

/// n(S) for every subset S of quartet a < b < c < d, S as a 4-bit mask in
/// pattern order (bit 3 = a, …, bit 0 = d): the number of columns in which
/// every taxon of S has 1. n(∅) is the number of columns.
pub fn subset_counts(cc: &CoCounts, q: [usize; 4], n4: u64) -> [u64; 16] {
    let mut n = [0u64; 16];
    for (s, v) in n.iter_mut().enumerate() {
        let members: Vec<usize> = (0..4)
            .filter(|&p| s >> (3 - p) & 1 == 1)
            .map(|p| q[p])
            .collect();
        *v = match members.as_slice() {
            [] => cc.columns,
            [i] => cc.single(*i),
            [i, j] => cc.pair(*i, *j),
            [i, j, k] => cc.triple(*i, *j, *k),
            _ => n4,
        };
    }
    n
}

/// Pattern counts from n(S) by inclusion–exclusion.
pub fn counts_from_subsets(n: &[u64; 16]) -> PatternCounts {
    let mut out = [0u64; 16];
    for (p, o) in out.iter_mut().enumerate() {
        let mut v: i64 = 0;
        for (s, &ns) in n.iter().enumerate() {
            if s & p == p {
                let sign = if (s ^ p).count_ones() % 2 == 0 { 1 } else { -1 };
                v += sign * ns as i64;
            }
        }
        *o = v as u64;
    }
    out
}

fn subset_name(s: usize) -> String {
    let letters: String = (0..4)
        .filter(|&p| s >> (3 - p) & 1 == 1)
        .map(|p| ['a', 'b', 'c', 'd'][p])
        .collect();
    if letters.is_empty() {
        "N".into()
    } else {
        format!("n({letters})")
    }
}

/// Settings of the weighting in a worksheet.
pub struct WeightSettings<'a> {
    pub model: &'a Model,
    /// For example "W2-sym".
    pub model_name: &'a str,
    pub conditioning: Conditioning,
    /// Global seed of the run.
    pub seed: u64,
    pub replicates: u32,
}

/// The worksheet of quartet `q` with taxon names `names`, and its weights.
pub fn quartet_worksheet(
    names: [&str; 4],
    q: [usize; 4],
    n: &[u64; 16],
    settings: &WeightSettings,
) -> (String, PatternCounts, QuartetWeights) {
    let rank = quartet::rank(q);
    let counts = counts_from_subsets(n);
    let qseed = weight::quartet_seed(settings.seed, rank);
    let weights = weight::weigh(
        settings.model,
        settings.conditioning,
        &counts,
        qseed,
        settings.replicates,
    );
    let mut w = String::new();
    let _ = writeln!(
        w,
        "Quartet rank {rank}: a = {}, b = {}, c = {}, d = {}",
        names[0], names[1], names[2], names[3]
    );
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "1. Co-occurrence counts n(S): columns where every taxon of S has 1"
    );
    for size in 0..=4 {
        let line: Vec<String> = (0..16)
            .filter(|s: &usize| s.count_ones() == size)
            .rev()
            .map(|s| format!("{} = {}", subset_name(s), n[s]))
            .collect();
        let _ = writeln!(w, "   {}", line.join(", "));
    }
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "2. Pattern counts c(abcd) by inclusion-exclusion: c(P) = sum over S containing P of (-1)^(|S|-|P|) n(S)"
    );
    for p in (0..16).rev() {
        // n(P) first, then its supersets by size.
        let mut terms: Vec<usize> = (0..16).rev().filter(|&s| s & p == p).collect();
        terms.sort_by_key(|s| s.count_ones());
        let mut symbolic = String::new();
        let mut numeric = String::new();
        for (i, &s) in terms.iter().enumerate() {
            let minus = (s ^ p).count_ones() % 2 == 1;
            let op = match (i, minus) {
                (0, _) => "",
                (_, true) => " - ",
                (_, false) => " + ",
            };
            symbolic.push_str(&format!("{op}{}", subset_name(s)));
            numeric.push_str(&format!("{op}{}", n[s]));
        }
        let _ = writeln!(
            w,
            "   c({}) = {symbolic} = {numeric} = {}",
            quartet::pattern_name(p),
            counts[p]
        );
    }
    let _ = writeln!(w, "   Sum of the 16 counts: {}", counts.iter().sum::<u64>());
    let _ = writeln!(w);
    let _ = writeln!(w, "3. W1 votes (split patterns supporting each topology)");
    for (t, [x, y]) in weight::SUPPORTING.iter().enumerate() {
        let _ = writeln!(
            w,
            "   {}: c({}) + c({}) = {} + {} = {}",
            TOPOLOGY_NAMES[t],
            quartet::pattern_name(*x),
            quartet::pattern_name(*y),
            counts[*x],
            counts[*y],
            counts[*x] + counts[*y]
        );
    }
    match weights.w1 {
        Some(v) => {
            let _ = writeln!(w, "   W1 weights: {}", fmt3(&v));
        }
        None => {
            let _ = writeln!(w, "   W1: no split pattern, no vote");
        }
    }
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "4. W2 ({}, conditioned on {:?}): maximised log-likelihood and branch lengths (a, b, c, d, internal)",
        settings.model_name, settings.conditioning
    );
    match &weights.fits {
        Some(fits) => {
            for (t, f) in fits.iter().enumerate() {
                let _ = writeln!(
                    w,
                    "   {}: log-likelihood {:.6}; lengths {:.6}, {:.6}, {:.6}, {:.6}, {:.6}",
                    TOPOLOGY_NAMES[t],
                    f.log_likelihood,
                    f.lengths[0],
                    f.lengths[1],
                    f.lengths[2],
                    f.lengths[3],
                    f.lengths[4]
                );
            }
        }
        None => {
            let _ = writeln!(
                w,
                "   No counted column: no fit, the quartet carries no weight"
            );
        }
    }
    if let Some((t, gap)) = weights.w2a {
        let _ = writeln!(w, "   W2a: best {} with gap {gap:.6}", TOPOLOGY_NAMES[t]);
    }
    if let Some(v) = weights.w2b {
        let _ = writeln!(w, "   W2b weights: {}", fmt3(&v));
    }
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "5. W2c: {} multinomial resamples with quartet seed {qseed}",
        settings.replicates
    );
    match weights.w2c {
        Some(v) => {
            let _ = writeln!(w, "   W2c weights (final): {}", fmt3(&v));
        }
        None => {
            let _ = writeln!(w, "   No W2c weights");
        }
    }
    (w, counts, weights)
}

fn fmt3(v: &[f64; 3]) -> String {
    (0..3)
        .map(|t| format!("{} {:.6}", TOPOLOGY_NAMES[t], v[t]))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusion_exclusion_matches_the_kernel_and_the_column_scan() {
        let mut x: u64 = 5;
        let mut next = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            x >> 7
        };
        let rows: Vec<Vec<u64>> = (0..6).map(|_| (0..3).map(|_| next()).collect()).collect();
        let cc = CoCounts::new(&rows, 192);
        for r in 0..quartet::quartet_count(6) {
            let q = quartet::unrank(r);
            let n4 = (0..3)
                .map(|i| {
                    (rows[q[0]][i] & rows[q[1]][i] & rows[q[2]][i] & rows[q[3]][i]).count_ones()
                        as u64
                })
                .sum();
            let n = subset_counts(&cc, q, n4);
            let c = counts_from_subsets(&n);
            assert_eq!(c, quartet::pattern_counts(&cc, q, n4));
            assert_eq!(c, quartet::pattern_counts_brute(&rows, 192, q));
        }
    }

    #[test]
    fn worksheet_shows_each_step_and_the_stored_weights() {
        // Columns: 1100 x5, 0011 x3, 1010 x1, 1111 x2, 1000 x4.
        let mut rows = vec![vec![0u64]; 4];
        let mut col = 0;
        for (pattern, k) in [
            (0b1100, 5),
            (0b0011, 3),
            (0b1010, 1),
            (0b1111, 2),
            (0b1000, 4),
        ] {
            for _ in 0..k {
                for (t, row) in rows.iter_mut().enumerate() {
                    if pattern >> (3 - t) & 1 == 1 {
                        row[0] |= 1 << col;
                    }
                }
                col += 1;
            }
        }
        let cc = CoCounts::new(&rows, col);
        let q = [0, 1, 2, 3];
        let n4 = 2;
        let n = subset_counts(&cc, q, n4);
        let model = Model::symmetric();
        let settings = WeightSettings {
            model: &model,
            model_name: "W2-sym",
            conditioning: Conditioning::NotAllZero,
            seed: 7,
            replicates: 10,
        };
        let (text, counts, weights) = quartet_worksheet(["A", "B", "C", "D"], q, &n, &settings);
        assert_eq!(counts[0b1100], 5);
        assert_eq!(counts[0b1000], 4);
        assert!(text.contains("n(ab) = 7"), "{text}");
        assert!(
            text.contains("c(1100) = n(ab) - n(abc) - n(abd) + n(abcd) = 7 - 2 - 2 + 2 = 5"),
            "{text}"
        );
        assert!(
            text.contains("ab|cd: c(1100) + c(0011) = 5 + 3 = 8"),
            "{text}"
        );
        let direct = weight::weigh(
            &model,
            Conditioning::NotAllZero,
            &counts,
            weight::quartet_seed(7, 0),
            10,
        );
        assert_eq!(weights, direct);
        assert!(text.contains("W2c weights (final)"), "{text}");
    }
}
