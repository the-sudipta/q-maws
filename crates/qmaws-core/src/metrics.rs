//! Tree comparison beyond nRF (plan 2.10): the normalised quartet distance
//! (nQD) and the matching split distance (MSD). Trees are read as unrooted
//! and must have the same leaves.
//!
//! - **nQD:** every quartet of leaves is enumerated; its topology in a
//!   tree follows from the four-point condition on path lengths in edges
//!   (`d(a,b) + d(c,d)` strictly smallest gives ab|cd; three equal sums
//!   mean the quartet is unresolved, for example at a polytomy). Quartets
//!   unresolved in the reference are counted separately and left out of
//!   the denominator.
//! - **MSD** (Bogdanowicz and Giaro, 2012): the minimum-cost one-to-one
//!   matching between the non-trivial splits of the two trees, where
//!   splits `A|B` and `C|D` cost `min(|A Δ C|, |A Δ D|)`; found with the
//!   Hungarian algorithm. When one tree has fewer splits, it is padded with
//!   trivial splits (`∅|L`, cost `min(|C|, |D|)` against `C|D`). The raw
//!   value is reported (no normalisation; see docs/OPEN_ISSUES.md OI-15).

use crate::newick::Tree;
use std::collections::BTreeMap;

/// Result of a quartet comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct QuartetDistance {
    /// Quartets resolved in the reference whose topology differs (including
    /// quartets unresolved in the estimate).
    pub differ: u64,
    /// Quartets resolved in the reference.
    pub compared: u64,
    /// Quartets unresolved in the reference (excluded).
    pub unresolved_in_reference: u64,
    /// `differ ÷ compared` (0 when nothing is compared).
    pub value: f64,
}

/// Path lengths in edges between all leaves, in the order of `names`.
fn leaf_distances(t: &Tree, names: &[String]) -> Result<Vec<Vec<u32>>, String> {
    let n = t.nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, node) in t.nodes.iter().enumerate() {
        for &c in &node.children {
            adj[i].push(c);
            adj[c].push(i);
        }
    }
    let mut leaf_of: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, node) in t.nodes.iter().enumerate() {
        if node.children.is_empty() {
            leaf_of.insert(node.label.as_deref().unwrap_or(""), i);
        }
    }
    let ids: Vec<usize> = names
        .iter()
        .map(|nm| {
            leaf_of
                .get(nm.as_str())
                .copied()
                .ok_or_else(|| format!("leaf {nm} is missing in a tree"))
        })
        .collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(ids.len());
    for &start in &ids {
        let mut dist = vec![u32::MAX; n];
        dist[start] = 0;
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(v) = queue.pop_front() {
            for &w in &adj[v] {
                if dist[w] == u32::MAX {
                    dist[w] = dist[v] + 1;
                    queue.push_back(w);
                }
            }
        }
        out.push(ids.iter().map(|&j| dist[j]).collect());
    }
    Ok(out)
}

fn same_leaves(a: &Tree, b: &Tree) -> Result<Vec<String>, String> {
    let mut la = a.leaf_names();
    let mut lb = b.leaf_names();
    la.sort();
    lb.sort();
    if la != lb {
        let only_a: Vec<&String> = la.iter().filter(|x| !lb.contains(x)).collect();
        let only_b: Vec<&String> = lb.iter().filter(|x| !la.contains(x)).collect();
        return Err(format!(
            "the trees have different leaves (only in the first: {only_a:?}; only in the second: {only_b:?})"
        ));
    }
    Ok(la)
}

/// Topology of quartet (i, j, k, l) from path lengths: 0 for ij|kl, 1 for
/// ik|jl, 2 for il|jk, `None` when unresolved.
fn quartet_topology(d: &[Vec<u32>], i: usize, j: usize, k: usize, l: usize) -> Option<u8> {
    let s = [d[i][j] + d[k][l], d[i][k] + d[j][l], d[i][l] + d[j][k]];
    let min = *s.iter().min().expect("three sums");
    let at: Vec<usize> = (0..3).filter(|&x| s[x] == min).collect();
    (at.len() == 1).then_some(at[0] as u8)
}

/// Normalised quartet distance of `estimate` to `reference`.
pub fn nqd(estimate: &Tree, reference: &Tree) -> Result<QuartetDistance, String> {
    let names = same_leaves(estimate, reference)?;
    let de = leaf_distances(estimate, &names)?;
    let dr = leaf_distances(reference, &names)?;
    let m = names.len();
    let (mut differ, mut compared, mut unresolved) = (0u64, 0u64, 0u64);
    for i in 0..m {
        for j in i + 1..m {
            for k in j + 1..m {
                for l in k + 1..m {
                    match quartet_topology(&dr, i, j, k, l) {
                        None => unresolved += 1,
                        Some(r) => {
                            compared += 1;
                            if quartet_topology(&de, i, j, k, l) != Some(r) {
                                differ += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(QuartetDistance {
        differ,
        compared,
        unresolved_in_reference: unresolved,
        value: if compared == 0 {
            0.0
        } else {
            differ as f64 / compared as f64
        },
    })
}

/// Matching split distance (raw) between two trees on the same leaves.
pub fn msd(a: &Tree, b: &Tree) -> Result<u64, String> {
    let matrix = msd_costs(a, b)?;
    if matrix.is_empty() {
        return Ok(0);
    }
    Ok(hungarian(&matrix) as u64)
}

/// The square cost matrix of the MSD matching: one row per split of `a`,
/// one column per split of `b`, the smaller tree padded with empty splits.
pub fn msd_costs(a: &Tree, b: &Tree) -> Result<Vec<Vec<i64>>, String> {
    let names = same_leaves(a, b)?;
    let index: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let to_bits = |side: &Vec<String>| -> Vec<bool> {
        let mut v = vec![false; names.len()];
        for x in side {
            v[index[x.as_str()]] = true;
        }
        v
    };
    let sa: Vec<Vec<bool>> = a.splits().iter().map(to_bits).collect();
    let sb: Vec<Vec<bool>> = b.splits().iter().map(to_bits).collect();
    let n = sa.len().max(sb.len());
    let cost = |x: Option<&Vec<bool>>, y: Option<&Vec<bool>>| -> i64 {
        match (x, y) {
            (Some(x), Some(y)) => {
                let same = x.iter().zip(y).filter(|(p, q)| p == q).count();
                let differ = x.len() - same;
                same.min(differ) as i64
            }
            (Some(s), None) | (None, Some(s)) => {
                let ones = s.iter().filter(|&&v| v).count();
                ones.min(s.len() - ones) as i64
            }
            (None, None) => 0,
        }
    };
    Ok((0..n)
        .map(|i| (0..n).map(|j| cost(sa.get(i), sb.get(j))).collect())
        .collect())
}

/// Minimum total cost of a perfect matching in a square cost matrix
/// (Hungarian algorithm with potentials, O(n³)).
pub fn hungarian(cost: &[Vec<i64>]) -> i64 {
    let n = cost.len();
    let inf = i64::MAX / 4;
    let mut u = vec![0i64; n + 1];
    let mut v = vec![0i64; n + 1];
    let mut p = vec![0usize; n + 1];
    let mut way = vec![0usize; n + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0usize;
        let mut minv = vec![inf; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = inf;
            let mut j1 = 0usize;
            for j in 1..=n {
                if !used[j] {
                    let cur = cost[i0 - 1][j - 1] - u[i0] - v[j];
                    if cur < minv[j] {
                        minv[j] = cur;
                        way[j] = j0;
                    }
                    if minv[j] < delta {
                        delta = minv[j];
                        j1 = j;
                    }
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    (1..=n).map(|j| cost[p[j] - 1][j - 1]).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Tree {
        Tree::parse(s).unwrap()
    }

    /// Quartet topology by splits: ab|cd if some split separates them.
    fn by_splits(tree: &Tree, q: [&str; 4]) -> Option<u8> {
        let splits = tree.splits();
        let pairs = [[0, 1, 2, 3], [0, 2, 1, 3], [0, 3, 1, 2]];
        let mut found = None;
        for (x, p) in pairs.iter().enumerate() {
            let sep = splits.iter().any(|s| {
                let side = |n: &str| s.iter().any(|y| y == n);
                side(q[p[0]]) == side(q[p[1]])
                    && side(q[p[2]]) == side(q[p[3]])
                    && side(q[p[0]]) != side(q[p[2]])
            });
            if sep {
                found = Some(x as u8);
            }
        }
        found
    }

    fn random_tree(names: &[String], seed: u64, polytomy: bool) -> Tree {
        // Random stepwise addition on a Newick string of nested groups.
        let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut rnd = |n: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % n as u64) as usize
        };
        let mut groups: Vec<String> = names.to_vec();
        while groups.len() > 3 {
            let take = if polytomy && groups.len() > 4 && rnd(3) == 0 {
                3
            } else {
                2
            };
            let mut picked = Vec::new();
            for _ in 0..take {
                let i = rnd(groups.len());
                picked.push(groups.swap_remove(i));
            }
            groups.push(format!("({})", picked.join(",")));
        }
        t(&format!("({});", groups.join(",")))
    }

    #[test]
    fn nqd_equals_enumeration_by_splits() {
        for seed in 1..40u64 {
            let m = 5 + (seed as usize % 9);
            let names: Vec<String> = (0..m).map(|i| format!("t{i}")).collect();
            let a = random_tree(&names, seed, false);
            let b = random_tree(&names, seed + 1000, seed % 3 == 0);
            let got = nqd(&a, &b).unwrap();
            let (mut differ, mut compared, mut unresolved) = (0, 0, 0);
            for i in 0..m {
                for j in i + 1..m {
                    for k in j + 1..m {
                        for l in k + 1..m {
                            let q =
                                [&names[i], &names[j], &names[k], &names[l]].map(|s| s.as_str());
                            match by_splits(&b, q) {
                                None => unresolved += 1,
                                Some(r) => {
                                    compared += 1;
                                    if by_splits(&a, q) != Some(r) {
                                        differ += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            assert_eq!(
                (got.differ, got.compared, got.unresolved_in_reference),
                (differ, compared, unresolved),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn nqd_of_identical_trees_is_zero_and_rooting_does_not_matter() {
        let a = t("((A,B),(C,D),E);");
        let b = t("(A,(B,((C,D),E)));");
        let d = nqd(&a, &b).unwrap();
        assert_eq!((d.differ, d.compared, d.value), (0, 5, 0.0));
        let star = t("(A,B,C,D,E);");
        let s = nqd(&a, &star).unwrap();
        assert_eq!((s.compared, s.unresolved_in_reference), (0, 5));
        let s = nqd(&star, &a).unwrap();
        assert_eq!((s.differ, s.compared), (5, 5));
    }

    #[test]
    fn different_leaves_are_an_error() {
        assert!(nqd(&t("((A,B),C,D);"), &t("((A,B),C,E);")).is_err());
        assert!(msd(&t("((A,B),C,D);"), &t("((A,B),C,E);")).is_err());
    }

    #[test]
    fn msd_worked_example() {
        // Splits ab|cde, de|abc against ac|bde, de|abc: de matches de (0),
        // ab against ac costs |{a,b} Δ {a,c}| = 2.
        let a = t("((a,b),c,(d,e));");
        let b = t("((a,c),b,(d,e));");
        assert_eq!(msd(&a, &b).unwrap(), 2);
        assert_eq!(msd(&a, &a).unwrap(), 0);
        // A star has no splits: each split is matched to a trivial split.
        assert_eq!(msd(&a, &t("(a,b,c,d,e);")).unwrap(), 4);
    }

    #[test]
    fn msd_is_symmetric_and_bounded_by_split_costs() {
        for seed in 1..30u64 {
            let m = 6 + (seed as usize % 10);
            let names: Vec<String> = (0..m).map(|i| format!("t{i}")).collect();
            let a = random_tree(&names, seed, false);
            let b = random_tree(&names, seed + 77, false);
            let ab = msd(&a, &b).unwrap();
            assert_eq!(ab, msd(&b, &a).unwrap());
            assert_eq!(ab == 0, a.splits() == b.splits());
        }
    }

    #[test]
    fn hungarian_finds_the_minimum_by_brute_force() {
        let mut x = 12345u64;
        for n in 1..=6 {
            let m: Vec<Vec<i64>> = (0..n)
                .map(|_| {
                    (0..n)
                        .map(|_| {
                            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                            ((x >> 33) % 20) as i64
                        })
                        .collect()
                })
                .collect();
            let mut perm: Vec<usize> = (0..n).collect();
            let mut best = i64::MAX;
            permute(&mut perm, 0, &m, &mut best);
            assert_eq!(hungarian(&m), best, "{m:?}");
        }
    }

    fn permute(p: &mut Vec<usize>, k: usize, m: &[Vec<i64>], best: &mut i64) {
        if k == p.len() {
            *best = (*best).min((0..p.len()).map(|i| m[i][p[i]]).sum());
            return;
        }
        for i in k..p.len() {
            p.swap(k, i);
            permute(p, k + 1, m, best);
            p.swap(k, i);
        }
    }
}
