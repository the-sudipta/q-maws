//! Statistics of the pre-registered hypothesis test H1
//! (docs/PREREGISTRATION.md): the exact one-sided Wilcoxon signed-rank
//! test on paired differences, the Holm correction across metrics, the
//! median and the Hodges–Lehmann estimate.
//!
//! - **Exact test:** the absolute differences are ranked (tied values get
//!   the mean of their ranks) and `W+`, the sum of the ranks of the
//!   positive differences, is computed. Under the null hypothesis every
//!   sign assignment of the ranks is equally likely; the p-value of the
//!   alternative "differences tend to be negative" is the fraction of the
//!   `2^n` assignments whose `W+` is at most the observed one. Enumerating
//!   the assignments gives the exact distribution also with tied ranks.
//! - **Zero differences:** handled as chosen by [`Zeros`]: dropped before
//!   ranking (Wilcoxon) or ranked with the others and then left out of
//!   both `W+` and the enumeration (Pratt).
//! - **Hodges–Lehmann:** the median of the Walsh averages
//!   `(d_i + d_j) / 2`, `i ≤ j`, over all differences.

/// Absolute differences closer than this are treated as tied, and
/// differences smaller than this in magnitude as zero. Inputs are rounded
/// published values (3 decimals) and exact fractions, so real differences
/// are never this small.
pub const TIE_TOLERANCE: f64 = 1e-12;

/// Largest number of non-zero differences the exact enumeration accepts.
pub const MAX_EXACT: usize = 24;

/// How zero differences enter the signed-rank test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zeros {
    /// Dropped before ranking (Wilcoxon's original procedure).
    Wilcoxon,
    /// Ranked together with the non-zero differences, then left out.
    Pratt,
}

/// Result of the exact one-sided signed-rank test.
#[derive(Debug, Clone, PartialEq)]
pub struct SignedRank {
    /// Number of differences given.
    pub n: usize,
    /// Number of zero differences.
    pub zeros: usize,
    /// Sum of the ranks of the positive differences.
    pub w_plus: f64,
    /// Sum of the ranks of the negative differences.
    pub w_minus: f64,
    /// `P(W+ <= observed)` under the null hypothesis; 1 when every
    /// difference is zero.
    pub p_less: f64,
}

/// Mean ranks (1-based) of `values`, ties within [`TIE_TOLERANCE`]
/// getting the mean of their ranks.
pub fn mid_ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len() && (values[order[j]] - values[order[i]]).abs() <= TIE_TOLERANCE {
            j += 1;
        }
        // Positions i..j (0-based) share ranks i+1..=j.
        let mean = (i + 1 + j) as f64 / 2.0;
        for &k in &order[i..j] {
            ranks[k] = mean;
        }
        i = j;
    }
    ranks
}

/// Exact one-sided Wilcoxon signed-rank test of "the differences tend to
/// be negative".
pub fn signed_rank_less(diffs: &[f64], zeros: Zeros) -> Result<SignedRank, String> {
    if diffs.iter().any(|d| !d.is_finite()) {
        return Err("a paired difference is not a finite number".to_string());
    }
    let is_zero = |d: f64| d.abs() <= TIE_TOLERANCE;
    let n_zero = diffs.iter().filter(|&&d| is_zero(d)).count();
    let ranked: Vec<f64> = match zeros {
        Zeros::Wilcoxon => diffs.iter().copied().filter(|&d| !is_zero(d)).collect(),
        Zeros::Pratt => diffs.to_vec(),
    };
    let abs: Vec<f64> = ranked
        .iter()
        .map(|d| if is_zero(*d) { 0.0 } else { d.abs() })
        .collect();
    let all_ranks = mid_ranks(&abs);
    // Ranks of the non-zero differences, doubled to stay integral.
    let mut ranks2 = Vec::new();
    let mut w_plus2 = 0u64;
    let mut w_minus2 = 0u64;
    for (d, r) in ranked.iter().zip(&all_ranks) {
        if is_zero(*d) {
            continue;
        }
        let r2 = (r * 2.0).round() as u64;
        ranks2.push(r2);
        if *d > 0.0 {
            w_plus2 += r2;
        } else {
            w_minus2 += r2;
        }
    }
    let m = ranks2.len();
    if m > MAX_EXACT {
        return Err(format!(
            "{m} non-zero differences: more than the {MAX_EXACT} the exact test enumerates"
        ));
    }
    let p_less = if m == 0 {
        1.0
    } else {
        let total = 1u64 << m;
        let mut at_most = 0u64;
        for mask in 0..total {
            let mut s = 0u64;
            for (i, r) in ranks2.iter().enumerate() {
                if mask >> i & 1 == 1 {
                    s += r;
                }
            }
            if s <= w_plus2 {
                at_most += 1;
            }
        }
        at_most as f64 / total as f64
    };
    Ok(SignedRank {
        n: diffs.len(),
        zeros: n_zero,
        w_plus: w_plus2 as f64 / 2.0,
        w_minus: w_minus2 as f64 / 2.0,
        p_less,
    })
}

/// Holm step-down adjusted p-values, in the order of `p`.
pub fn holm(p: &[f64]) -> Vec<f64> {
    let k = p.len();
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| p[a].total_cmp(&p[b]));
    let mut adjusted = vec![0.0; k];
    let mut running = 0.0f64;
    for (step, &i) in order.iter().enumerate() {
        let v = ((k - step) as f64 * p[i]).min(1.0);
        running = running.max(v);
        adjusted[i] = running;
    }
    adjusted
}

/// Median of `values` (mean of the two middle values for an even count);
/// `None` when empty.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// Hodges–Lehmann estimate: the median of the Walsh averages.
pub fn hodges_lehmann(diffs: &[f64]) -> Option<f64> {
    let mut walsh = Vec::with_capacity(diffs.len() * (diffs.len() + 1) / 2);
    for i in 0..diffs.len() {
        for j in i..diffs.len() {
            walsh.push((diffs[i] + diffs[j]) / 2.0);
        }
    }
    median(&walsh)
}

/// Mean and sample standard deviation (`n - 1`; 0 for one value).
pub fn mean_sd(values: &[f64]) -> Option<(f64, f64)> {
    if values.is_empty() {
        return None;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    if values.len() == 1 {
        return Some((mean, 0.0));
    }
    let ss: f64 = values.iter().map(|v| (v - mean) * (v - mean)).sum();
    Some((mean, (ss / (n - 1.0)).sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_negative_gives_the_smallest_p() {
        let d = [-0.1, -0.2, -0.3, -0.4, -0.5, -0.6, -0.7];
        let r = signed_rank_less(&d, Zeros::Wilcoxon).unwrap();
        assert_eq!(r.w_plus, 0.0);
        assert_eq!(r.w_minus, 28.0);
        assert_eq!(r.p_less, 1.0 / 128.0);
    }

    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is a data value, not pi
    fn matches_the_textbook_example() {
        // Hollander and Wolfe, depression scale (x - y); one-sided exact
        // p-value 0.01953 (R: wilcox.test(x, y, paired = TRUE,
        // alternative = "greater"), V = 40). Negated for "less".
        let x = [1.83, 0.50, 1.62, 2.48, 1.68, 1.88, 1.55, 3.06, 1.30];
        let y = [0.878, 0.647, 0.598, 2.05, 1.06, 1.29, 1.06, 3.14, 1.29];
        let d: Vec<f64> = x.iter().zip(&y).map(|(a, b)| b - a).collect();
        let r = signed_rank_less(&d, Zeros::Wilcoxon).unwrap();
        assert_eq!(r.w_minus, 40.0);
        assert_eq!(r.w_plus, 5.0);
        assert_eq!(r.p_less, 10.0 / 512.0);
    }

    #[test]
    fn tied_ranks_use_the_exact_conditional_distribution() {
        // Ranks 1.5, 1.5, 3, 4; W+ = 7; 13 of the 16 sign assignments
        // have W+ <= 7 (worked by hand).
        let r = signed_rank_less(&[1.0, 1.0, -2.0, 3.0], Zeros::Wilcoxon).unwrap();
        assert_eq!(r.w_plus, 7.0);
        assert_eq!(r.p_less, 13.0 / 16.0);
    }

    #[test]
    fn zeros_are_dropped_or_ranked() {
        let d = [0.0, -1.0, -2.0, 3.0];
        let w = signed_rank_less(&d, Zeros::Wilcoxon).unwrap();
        // Ranks 1, 2, 3 for |-1|, |-2|, |3|: W+ = 3; sums <= 3 of subsets
        // of {1, 2, 3}: 0, 1, 2, 3, 1+2 -> 5 of 8.
        assert_eq!((w.zeros, w.w_plus, w.p_less), (1, 3.0, 5.0 / 8.0));
        let p = signed_rank_less(&d, Zeros::Pratt).unwrap();
        // Ranks 2, 3, 4 (the zero takes rank 1): W+ = 4; subsets of
        // {2, 3, 4} with sum <= 4: 0, 2, 3, 4 -> 4 of 8.
        assert_eq!((p.zeros, p.w_plus, p.p_less), (1, 4.0, 4.0 / 8.0));
    }

    #[test]
    fn all_zero_differences_give_p_one() {
        let r = signed_rank_less(&[0.0, 0.0], Zeros::Wilcoxon).unwrap();
        assert_eq!(r.p_less, 1.0);
    }

    #[test]
    fn holm_is_monotone_and_capped() {
        assert_eq!(holm(&[0.01, 0.04]), vec![0.02, 0.04]);
        assert_eq!(holm(&[0.04, 0.01]), vec![0.04, 0.02]);
        assert_eq!(holm(&[0.03, 0.02]), vec![0.04, 0.04]);
        assert_eq!(holm(&[0.6, 0.9]), vec![1.0, 1.0]);
    }

    #[test]
    fn hodges_lehmann_and_median() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        // Walsh averages of 1, 2, 9: 1, 1.5, 5, 2, 5.5, 9 -> median 3.5.
        assert_eq!(hodges_lehmann(&[1.0, 2.0, 9.0]), Some(3.5));
        assert_eq!(hodges_lehmann(&[]), None);
    }

    #[test]
    fn mean_and_sample_sd() {
        let (m, s) = mean_sd(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert_eq!(m, 5.0);
        assert!((s - (32.0f64 / 7.0).sqrt()).abs() < 1e-15);
        assert_eq!(mean_sd(&[0.5]), Some((0.5, 0.0)));
    }

    #[test]
    fn non_finite_input_is_an_error() {
        assert!(signed_rank_less(&[f64::NAN], Zeros::Wilcoxon).is_err());
    }
}
