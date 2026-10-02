//! The four-taxon long-branch simulation of hypothesis H3 (plan 2.7.6).
//!
//! True tree ab|cd under the two-state symmetric model. Each character is
//! simulated along the tree: the state of the internal node u (joining a
//! and b) is 0 or 1 with probability 0.5; a, b and the other internal node v
//! (joining c and d) change from u, and c and d from v, each with
//! probability (1 − exp(−2t)) ÷ 2 for a branch of length t. The simulator
//! uses only this rule, not the likelihood code it tests. Columns 0000 are
//! removed, as MAW columns never have that pattern.
//!
//! Recovery of the true topology ab|cd (index 0) by a method is 1 if it is
//! the only best topology, 1 ÷ k if it is one of k tied best topologies
//! (the expected value of a random choice among them), and 0 otherwise. W1
//! with no split pattern at all counts as a tie of all three topologies.

use crate::quartet::PatternCounts;
use crate::weight::{self, Conditioning, Model, SplitMix64};
use sha2::{Digest, Sha256};

/// Leaf branch lengths of a and c (pre-registered).
pub const LONG_BRANCHES: [f64; 3] = [0.5, 1.0, 1.5];

/// Leaf branches of b and d and the internal branch (pre-registered).
pub const SHORT_BRANCH: f64 = 0.05;

/// Numbers of simulated characters before 0000 columns are removed.
pub const CHARACTERS: [u64; 4] = [100, 1_000, 10_000, 100_000];

/// Replicates per setting (pre-registered).
pub const REPLICATES: u32 = 200;

/// Branch lengths (leaf of a, b, c, d; internal) of a setting.
pub fn true_lengths(t_long: f64) -> [f64; 5] {
    [t_long, SHORT_BRANCH, t_long, SHORT_BRANCH, SHORT_BRANCH]
}

/// Seed of one replicate: the first 8 bytes (little-endian) of SHA-256 of
/// the global seed, the index of t_long, the index of N and the replicate
/// number, each as 8 bytes little-endian.
pub fn replicate_seed(global: u64, t_index: u64, n_index: u64, replicate: u64) -> u64 {
    let mut h = Sha256::new();
    for v in [global, t_index, n_index, replicate] {
        h.update(v.to_le_bytes());
    }
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
}

/// Simulates `n` characters on ab|cd with the given branch lengths and
/// returns the pattern counts (0000 included).
pub fn simulate(rng: &mut SplitMix64, lengths: &[f64; 5], n: u64) -> PatternCounts {
    let p = lengths.map(|t| (1.0 - libm::exp(-2.0 * t)) / 2.0);
    let flip = |rng: &mut SplitMix64, state: u8, b: usize| state ^ u8::from(rng.next_f64() < p[b]);
    let mut counts = [0u64; 16];
    for _ in 0..n {
        let u = u8::from(rng.next_f64() < 0.5);
        let a = flip(rng, u, 0);
        let b = flip(rng, u, 1);
        let v = flip(rng, u, 4);
        let c = flip(rng, v, 2);
        let d = flip(rng, v, 3);
        let x =
            (usize::from(a) << 3) | (usize::from(b) << 2) | (usize::from(c) << 1) | usize::from(d);
        counts[x] += 1;
    }
    counts
}

/// Share of recovery of topology 0 among the best topologies `best`.
fn share(best: [bool; 3]) -> f64 {
    let k = best.iter().filter(|&&b| b).count();
    if best[0] {
        1.0 / k as f64
    } else {
        0.0
    }
}

/// One replicate of the experiment.
#[derive(Clone, Debug, PartialEq)]
pub struct Replicate {
    pub t_long: f64,
    pub characters: u64,
    pub replicate: u32,
    pub seed: u64,
    /// Pattern counts after removing 0000 (index 0 is 0).
    pub counts: PatternCounts,
    /// W1 scores of ab|cd, ac|bd, ad|bc.
    pub w1_scores: [u64; 3],
    /// W1 recovery (see the module documentation).
    pub w1_recovery: f64,
    /// W2-sym log-likelihoods; `None` if no column is left.
    pub log_likelihoods: Option<[f64; 3]>,
    /// W2 recovery (ties within `weight::TIE_TOLERANCE`).
    pub w2_recovery: f64,
    /// W2c weights (supplementary); `None` if no column is left.
    pub w2c: Option<[f64; 3]>,
}

/// Runs one replicate. W2c uses the seed `weight::quartet_seed(seed, 1)`.
pub fn run_replicate(
    t_long: f64,
    characters: u64,
    replicate: u32,
    seed: u64,
    w2c_replicates: u32,
) -> Replicate {
    let mut rng = SplitMix64::new(seed);
    let mut counts = simulate(&mut rng, &true_lengths(t_long), characters);
    counts[0] = 0;
    let w1_scores = weight::SUPPORTING.map(|[x, y]| counts[x] + counts[y]);
    let top = *w1_scores.iter().max().expect("three scores");
    let w1_recovery = share(w1_scores.map(|s| s == top));
    let model = Model::symmetric();
    let cond = Conditioning::NotAllZero;
    let log_likelihoods =
        weight::fit_all(&model, cond, &counts).map(|f| f.map(|x| x.log_likelihood));
    let w2_recovery = match &log_likelihoods {
        Some(ll) => share(weight::winners(ll)),
        None => 1.0 / 3.0,
    };
    let w2c = weight::w2c(
        &model,
        cond,
        &counts,
        weight::quartet_seed(seed, 1),
        w2c_replicates,
    );
    Replicate {
        t_long,
        characters,
        replicate,
        seed,
        counts,
        w1_scores,
        w1_recovery,
        log_likelihoods,
        w2_recovery,
        w2c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_frequencies_match_the_model() {
        // The simulator and the likelihood code are independent; their
        // pattern frequencies must agree.
        let model = Model::symmetric();
        for t_long in LONG_BRANCHES {
            let lengths = true_lengths(t_long);
            let n = 400_000u64;
            let counts = simulate(&mut SplitMix64::new(77), &lengths, n);
            let p = weight::pattern_probabilities(&model, 0, &lengths);
            for x in 0..16 {
                let f = counts[x] as f64 / n as f64;
                let sd = (p[x] * (1.0 - p[x]) / n as f64).sqrt();
                assert!(
                    (f - p[x]).abs() < 5.0 * sd,
                    "t {t_long} x {x}: {f} vs {}",
                    p[x]
                );
            }
        }
    }

    #[test]
    fn replicate_seeds_are_fixed_and_distinct() {
        let s = replicate_seed(1, 0, 0, 0);
        let mut bytes = Vec::new();
        for v in [1u64, 0, 0, 0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let d = Sha256::digest(&bytes);
        assert_eq!(s, u64::from_le_bytes(d[..8].try_into().unwrap()));
        assert_ne!(replicate_seed(1, 0, 0, 1), s);
        assert_ne!(replicate_seed(1, 1, 0, 0), s);
        assert_ne!(replicate_seed(1, 0, 1, 0), s);
    }

    #[test]
    fn a_replicate_is_reproducible_and_removes_0000() {
        let a = run_replicate(1.0, 1_000, 3, 99, 5);
        let b = run_replicate(1.0, 1_000, 3, 99, 5);
        assert_eq!(a, b);
        assert_eq!(a.counts[0], 0);
        assert!(a.counts.iter().sum::<u64>() < 1_000);
    }

    #[test]
    fn recovery_shares_ties() {
        assert_eq!(share([true, false, false]), 1.0);
        assert_eq!(share([true, true, false]), 0.5);
        assert_eq!(share([false, true, true]), 0.0);
        assert!((share([true, true, true]) - 1.0 / 3.0).abs() < 1e-15);
    }

    #[test]
    fn a_committed_replicate_is_reproduced_exactly() {
        // Row t_long 1.5, N 10,000, replicate 0 of results/h3/replicates.csv
        // (global seed 1). Run on every CI platform, this checks that the
        // committed results do not depend on the operating system.
        let seed = replicate_seed(1, 2, 2, 0);
        assert_eq!(seed, 10_762_077_317_441_206_578);
        let r = run_replicate(1.5, 10_000, 0, seed, crate::weight::REPLICATES);
        let expected: [u64; 15] = [
            140, 1071, 163, 176, 978, 176, 1010, 1120, 164, 987, 171, 157, 1125, 184, 1195,
        ];
        assert_eq!(&r.counts[1..], &expected);
        assert_eq!(r.w1_scores, [320, 1965, 340]);
        let ll = r.log_likelihoods.unwrap();
        for (got, want) in ll.iter().zip([-21060.811135, -21060.798584, -21060.625896]) {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
        assert_eq!(r.w2_recovery, 0.0);
        assert_eq!(r.w2c, Some([0.16, 0.22, 0.62]));
    }
}
