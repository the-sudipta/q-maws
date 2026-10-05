//! Golden test G9 (plan 2.10.4): random tree pairs and independent oracles
//! for the tree metrics.
//!
//! - **Pairs:** pair `i` has `m = 5 + (i mod 56)` leaves, so `m` runs from
//!   5 to 60. The first tree is binary; the second is binary too, except
//!   every fourth pair, whose second tree has polytomies (quartets
//!   unresolved in the reference). Trees are built by random grouping:
//!   two (or, for polytomies, sometimes three) random groups are joined
//!   until three remain.
//! - **nQD oracle:** every quartet's topology is read from the trees'
//!   splits (ab|cd when a split puts a, b on one side and c, d on the
//!   other), not from path lengths as in [`crate::metrics::nqd`].
//! - **MSD oracle:** the minimum of the cost matrix over every one-to-one
//!   assignment (all permutations), for matrices of at most
//!   [`MSD_BRUTE_FORCE_MAX`] rows; it checks the Hungarian matching.
//! - **nRF** is checked outside Q-MAWS against DendroPy
//!   (`scripts/metrics_check.py`).

use crate::metrics::{msd_costs, QuartetDistance};
use crate::newick::Tree;
use crate::weight::SplitMix64;

/// Largest MSD cost matrix checked by brute force (8! = 40,320 assignments).
pub const MSD_BRUTE_FORCE_MAX: usize = 8;

/// Smallest and largest number of leaves of a pair.
pub const MIN_LEAVES: usize = 5;
pub const MAX_LEAVES: usize = 60;

/// One random tree pair.
pub struct Pair {
    pub leaves: usize,
    /// Binary tree (the estimate).
    pub a: String,
    /// The reference: binary, or with polytomies.
    pub b: String,
}

fn random_tree(names: &[String], rng: &mut SplitMix64, polytomy: bool) -> String {
    let mut groups: Vec<String> = names.to_vec();
    let pick = |n: usize, rng: &mut SplitMix64| (rng.next_u64() % n as u64) as usize;
    while groups.len() > 3 {
        let take = if polytomy && groups.len() > 4 && pick(3, rng) == 0 {
            3
        } else {
            2
        };
        let mut joined = Vec::with_capacity(take);
        for _ in 0..take {
            let i = pick(groups.len(), rng);
            joined.push(groups.swap_remove(i));
        }
        groups.push(format!("({})", joined.join(",")));
    }
    format!("({});", groups.join(","))
}

/// `count` pairs drawn with `seed`, with at most `max_leaves` leaves.
pub fn pairs(count: usize, seed: u64, max_leaves: usize) -> Vec<Pair> {
    let span = max_leaves.clamp(MIN_LEAVES, MAX_LEAVES) - MIN_LEAVES + 1;
    let mut rng = SplitMix64::new(seed);
    (0..count)
        .map(|i| {
            let leaves = MIN_LEAVES + i % span;
            let names: Vec<String> = (0..leaves).map(|k| format!("t{k:02}")).collect();
            let a = random_tree(&names, &mut rng, false);
            let b = random_tree(&names, &mut rng, i % 4 == 3);
            Pair { leaves, a, b }
        })
        .collect()
}

/// Split bitmasks of a tree over `names` (at most 64 leaves).
fn split_masks(t: &Tree, names: &[String]) -> Vec<u64> {
    t.splits()
        .iter()
        .map(|side| {
            side.iter().fold(0u64, |mask, leaf| {
                let k = names.iter().position(|n| n == leaf).expect("same leaves");
                mask | (1 << k)
            })
        })
        .collect()
}

/// Topology of quartet (i, j, k, l) from splits: 0 for ij|kl, 1 for ik|jl,
/// 2 for il|jk, `None` when no split resolves it.
fn topology_by_splits(masks: &[u64], i: usize, j: usize, k: usize, l: usize) -> Option<u8> {
    for &s in masks {
        let side = |x: usize| s >> x & 1;
        let (a, b, c, d) = (side(i), side(j), side(k), side(l));
        if a == b && c == d && a != c {
            return Some(0);
        }
        if a == c && b == d && a != b {
            return Some(1);
        }
        if a == d && b == c && a != b {
            return Some(2);
        }
    }
    None
}

/// nQD of `estimate` to `reference` by enumeration over splits.
pub fn nqd_by_splits(estimate: &Tree, reference: &Tree) -> QuartetDistance {
    let mut names = reference.leaf_names();
    names.sort();
    assert!(names.len() <= 64, "the oracle handles at most 64 leaves");
    let me = split_masks(estimate, &names);
    let mr = split_masks(reference, &names);
    let m = names.len();
    let (mut differ, mut compared, mut unresolved) = (0u64, 0u64, 0u64);
    for i in 0..m {
        for j in i + 1..m {
            for k in j + 1..m {
                for l in k + 1..m {
                    match topology_by_splits(&mr, i, j, k, l) {
                        None => unresolved += 1,
                        Some(r) => {
                            compared += 1;
                            if topology_by_splits(&me, i, j, k, l) != Some(r) {
                                differ += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    QuartetDistance {
        differ,
        compared,
        unresolved_in_reference: unresolved,
        value: if compared == 0 {
            0.0
        } else {
            differ as f64 / compared as f64
        },
    }
}

/// Minimum-cost assignment by trying every permutation (Heap's algorithm).
fn min_assignment(cost: &[Vec<i64>]) -> i64 {
    let n = cost.len();
    let mut perm: Vec<usize> = (0..n).collect();
    let total = |p: &[usize]| (0..n).map(|i| cost[i][p[i]]).sum::<i64>();
    let mut best = total(&perm);
    let mut c = vec![0usize; n];
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                perm.swap(0, i);
            } else {
                perm.swap(c[i], i);
            }
            best = best.min(total(&perm));
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    best
}

/// MSD by brute force over all matchings; `None` when the cost matrix has
/// more than [`MSD_BRUTE_FORCE_MAX`] rows.
pub fn msd_by_permutations(a: &Tree, b: &Tree) -> Result<Option<u64>, String> {
    let cost = msd_costs(a, b)?;
    if cost.len() > MSD_BRUTE_FORCE_MAX {
        return Ok(None);
    }
    Ok(Some(if cost.is_empty() {
        0
    } else {
        min_assignment(&cost) as u64
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{msd, nqd};

    #[test]
    fn pairs_cover_the_leaf_range_and_are_seeded() {
        let p = pairs(112, 3, MAX_LEAVES);
        assert_eq!(p[0].leaves, 5);
        assert_eq!(p[55].leaves, 60);
        assert_eq!(p[56].leaves, 5);
        let q = pairs(112, 3, MAX_LEAVES);
        assert!(p.iter().zip(&q).all(|(x, y)| x.a == y.a && x.b == y.b));
        for pair in &p {
            let a = Tree::parse(&pair.a).unwrap();
            let b = Tree::parse(&pair.b).unwrap();
            assert_eq!(a.leaf_names().len(), pair.leaves);
            assert_eq!(
                a.splits().len(),
                pair.leaves - 3,
                "the first tree is binary"
            );
            assert!(b.splits().len() <= pair.leaves - 3);
        }
        assert!(p
            .iter()
            .any(|x| Tree::parse(&x.b).unwrap().splits().len() < x.leaves - 3));
    }

    #[test]
    fn brute_force_assignment_finds_the_minimum() {
        let cost = vec![vec![4, 1, 3], vec![2, 0, 5], vec![3, 2, 2]];
        assert_eq!(min_assignment(&cost), 5);
        assert_eq!(min_assignment(&[vec![7]]), 7);
    }

    /// G9 on a small sample (the 500-pair check runs in
    /// `qmaws metrics-check`, release build).
    #[test]
    fn metrics_agree_with_the_oracles() {
        for pair in pairs(60, 9, 24) {
            let a = Tree::parse(&pair.a).unwrap();
            let b = Tree::parse(&pair.b).unwrap();
            assert_eq!(nqd(&a, &b).unwrap(), nqd_by_splits(&a, &b), "{}", pair.a);
            if let Some(expected) = msd_by_permutations(&a, &b).unwrap() {
                assert_eq!(msd(&a, &b).unwrap(), expected, "{}", pair.a);
            }
        }
    }
}
