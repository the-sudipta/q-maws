//! S2 column bootstrap: each replicate gives every matrix column a
//! Poisson(1) weight from a seeded generator, and the quartet pattern
//! counts become weighted counts. Weighting and amalgamation are then
//! repeated on those counts.
//!
//! Weighted counts are computed per weight class: the columns with weight
//! k are counted with the inclusion–exclusion kernel on the rows masked to
//! those columns, and the class counts are added k times.

use crate::quartet::{self, CoCounts, PatternCounts, PopcountPath};
use crate::weight::{self, SplitMix64};

/// Largest Poisson(1) draw (P(X > 20) is below 10⁻¹⁹).
const MAX_DRAW: u32 = 20;

/// Seed of bootstrap replicate `b` (a stream of its own: quartet ranks are
/// far below 2⁶³).
pub fn replicate_seed(global_seed: u64, b: u64) -> u64 {
    weight::quartet_seed(global_seed, (1u64 << 63) + b)
}

/// One Poisson(1) draw by inversion.
pub fn poisson1(rng: &mut SplitMix64) -> u32 {
    let u = rng.next_f64();
    let mut p = libm::exp(-1.0);
    let mut cum = p;
    let mut k = 0;
    while u > cum && k < MAX_DRAW {
        k += 1;
        p /= k as f64;
        cum += p;
    }
    k
}

/// Poisson(1) weights of `columns` columns, in column order.
pub fn column_weights(columns: usize, seed: u64) -> Vec<u32> {
    let mut rng = SplitMix64::new(seed);
    (0..columns).map(|_| poisson1(&mut rng)).collect()
}

/// Co-occurrence tables of each weight class of a weighted matrix.
pub struct WeightedCounts {
    /// (weight k, rows masked to the columns of weight k, their tables).
    classes: Vec<(u64, Vec<Vec<u64>>, CoCounts)>,
}

impl WeightedCounts {
    /// Tables for bitset `rows` with `columns` columns and their weights.
    pub fn new(rows: &[Vec<u64>], columns: usize, weights: &[u32]) -> Self {
        assert_eq!(weights.len(), columns);
        let words = rows.first().map_or(0, |r| r.len());
        let top = weights.iter().copied().max().unwrap_or(0);
        let mut classes = Vec::new();
        for k in 1..=top {
            let mut mask = vec![0u64; words];
            let mut n = 0u64;
            for (j, &w) in weights.iter().enumerate() {
                if w == k {
                    mask[j / 64] |= 1 << (j % 64);
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let masked: Vec<Vec<u64>> = rows
                .iter()
                .map(|r| r.iter().zip(&mask).map(|(a, b)| a & b).collect())
                .collect();
            let cc = CoCounts::new(&masked, n);
            classes.push((k as u64, masked, cc));
        }
        Self { classes }
    }

    /// Weighted pattern counts of quartet `q`.
    pub fn pattern_counts(&self, q: [usize; 4], path: PopcountPath) -> PatternCounts {
        let mut out = [0u64; 16];
        for (k, rows, cc) in &self.classes {
            let n4 = path.and4(&rows[q[0]], &rows[q[1]], &rows[q[2]], &rows[q[3]]);
            let c = quartet::pattern_counts(cc, q, n4);
            for x in 0..16 {
                out[x] += k * c[x];
            }
        }
        out
    }

    /// Memory of the tables and masked rows in bytes.
    pub fn bytes(&self) -> usize {
        self.classes
            .iter()
            .map(|(_, rows, cc)| cc.bytes() + rows.iter().map(|r| r.len() * 8).sum::<usize>())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_draws_have_mean_and_variance_one() {
        let w = column_weights(200_000, 42);
        let n = w.len() as f64;
        let mean = w.iter().map(|&x| x as f64).sum::<f64>() / n;
        let var = w.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / n;
        // Standard errors: 1/sqrt(n) for the mean, sqrt(2/n) roughly for the variance.
        assert!((mean - 1.0).abs() < 5.0 / n.sqrt(), "mean {mean}");
        assert!((var - 1.0).abs() < 5.0 * (3.0 / n).sqrt(), "variance {var}");
        let zeros = w.iter().filter(|&&x| x == 0).count() as f64 / n;
        assert!((zeros - libm::exp(-1.0)).abs() < 0.005, "P(0) {zeros}");
        assert_eq!(w, column_weights(200_000, 42));
        assert_ne!(w[..100], column_weights(100, 43)[..]);
    }

    #[test]
    fn weighted_counts_equal_a_weighted_column_scan() {
        let mut x: u64 = 9;
        let mut next = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            x >> 3
        };
        let columns = 300;
        let rows: Vec<Vec<u64>> = (0..7)
            .map(|_| {
                let mut r: Vec<u64> = (0..5).map(|_| next()).collect();
                r[4] &= (1 << (columns - 256)) - 1;
                r
            })
            .collect();
        let weights = column_weights(columns, 3);
        let wc = WeightedCounts::new(&rows, columns, &weights);
        let path = PopcountPath::detect();
        let bit = |t: usize, j: usize| (rows[t][j / 64] >> (j % 64)) & 1;
        for r in 0..quartet::quartet_count(7) {
            let q = quartet::unrank(r);
            let mut scan = [0u64; 16];
            for (j, &w) in weights.iter().enumerate() {
                let p =
                    (bit(q[0], j) << 3) | (bit(q[1], j) << 2) | (bit(q[2], j) << 1) | bit(q[3], j);
                scan[p as usize] += w as u64;
            }
            assert_eq!(wc.pattern_counts(q, path), scan);
        }
        // All weights 1 give the ordinary counts.
        let ones = vec![1u32; columns];
        let wc = WeightedCounts::new(&rows, columns, &ones);
        let cc = CoCounts::new(&rows, columns as u64);
        let q = [0, 2, 3, 6];
        let n4 = path.and4(&rows[0], &rows[2], &rows[3], &rows[6]);
        assert_eq!(
            wc.pattern_counts(q, path),
            quartet::pattern_counts(&cc, q, n4)
        );
    }

    #[test]
    fn replicate_seeds_differ_from_quartet_seeds() {
        assert_ne!(replicate_seed(1, 0), weight::quartet_seed(1, 0));
        assert_ne!(replicate_seed(1, 0), replicate_seed(1, 1));
        assert_eq!(replicate_seed(1, 5), replicate_seed(1, 5));
    }
}
