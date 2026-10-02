//! Data for the positive and negative controls.
//!
//! - Positive: DNA sequences evolved along a random binary tree under the
//!   Jukes–Cantor model, so the true tree is known.
//! - Negative: each sequence shuffled on its own (Fisher–Yates), which keeps
//!   its letter composition and destroys shared history.
//! - Reference level: normalised Robinson–Foulds distances of random trees
//!   to a reference tree, to judge whether a tree is closer than chance.

use crate::amalgamate::random_binary_tree;
use crate::newick::{nrf, Tree};
use crate::weight::SplitMix64;

const BASES: &[u8; 4] = b"ACGT";

/// A simulated dataset with its true tree.
pub struct Simulated {
    /// The true tree with branch lengths (expected substitutions per site).
    pub newick: String,
    /// Sequences in the order of `names`.
    pub sequences: Vec<Vec<u8>>,
}

/// Sequences of `length` bases evolved along a random binary tree on
/// `names` (random stepwise addition) under Jukes–Cantor, with every branch
/// length drawn uniformly from `[min_length, max_length]`.
pub fn simulate_jc69(
    names: &[String],
    length: usize,
    min_length: f64,
    max_length: f64,
    seed: u64,
) -> Simulated {
    let mut rng = SplitMix64::new(seed);
    let mut tree = Tree::parse(&random_binary_tree(names, &mut rng)).expect("tree parses");
    for n in 0..tree.nodes.len() {
        if n != tree.root {
            tree.nodes[n].length = Some(min_length + (max_length - min_length) * rng.next_f64());
        }
    }
    let root: Vec<u8> = (0..length)
        .map(|_| BASES[(rng.next_u64() % 4) as usize])
        .collect();
    let mut seqs: Vec<Option<Vec<u8>>> = vec![None; tree.nodes.len()];
    seqs[tree.root] = Some(root);
    // Parents before children: a walk from the root.
    let mut stack = vec![tree.root];
    while let Some(n) = stack.pop() {
        let parent = seqs[n].clone().expect("parent done");
        for &c in &tree.nodes[n].children {
            let t = tree.nodes[c].length.unwrap_or(0.0);
            // P(the base differs) = 3/4 (1 - exp(-4t/3)); the new base is
            // one of the other three, uniformly.
            let p = 0.75 * (1.0 - libm::exp(-4.0 * t / 3.0));
            let child: Vec<u8> = parent
                .iter()
                .map(|&b| {
                    if rng.next_f64() < p {
                        let i = BASES.iter().position(|&x| x == b).unwrap();
                        BASES[(i + 1 + (rng.next_u64() % 3) as usize) % 4]
                    } else {
                        b
                    }
                })
                .collect();
            seqs[c] = Some(child);
            stack.push(c);
        }
    }
    let mut by_name = std::collections::HashMap::new();
    for (n, node) in tree.nodes.iter().enumerate() {
        if node.children.is_empty() {
            by_name.insert(node.label.clone().unwrap_or_default(), n);
        }
    }
    let sequences = names
        .iter()
        .map(|name| seqs[by_name[name]].clone().expect("leaf done"))
        .collect();
    Simulated {
        newick: tree.to_newick(true),
        sequences,
    }
}

/// `seq` shuffled by Fisher–Yates with `seed`.
pub fn shuffle(seq: &[u8], seed: u64) -> Vec<u8> {
    let mut out = seq.to_vec();
    let mut rng = SplitMix64::new(seed);
    for i in (1..out.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        out.swap(i, j);
    }
    out
}

/// nRF of `count` random binary trees (random stepwise addition) on the
/// reference's leaves to the reference, in draw order.
pub fn random_tree_nrf(reference: &Tree, count: usize, seed: u64) -> Vec<f64> {
    let mut names = reference.leaf_names();
    names.sort();
    let mut rng = SplitMix64::new(seed);
    (0..count)
        .map(|_| {
            let t = Tree::parse(&random_binary_tree(&names, &mut rng)).expect("tree parses");
            nrf(&t, reference).1
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(m: usize) -> Vec<String> {
        (0..m).map(|i| format!("S{i:02}")).collect()
    }

    #[test]
    fn simulation_is_seeded_and_divergence_follows_branch_lengths() {
        let nm = names(6);
        let a = simulate_jc69(&nm, 50_000, 0.1, 0.1, 4);
        let b = simulate_jc69(&nm, 50_000, 0.1, 0.1, 4);
        assert_eq!(a.newick, b.newick);
        assert_eq!(a.sequences, b.sequences);
        let tree = Tree::parse(&a.newick).unwrap();
        let mut leaves = tree.leaf_names();
        leaves.sort();
        assert_eq!(leaves, nm);
        // Two leaves joined by a cherry are 0.2 substitutions apart; under
        // JC69 they differ at 3/4 (1 - exp(-4 x 0.2 / 3)) of the sites.
        let cherry = tree
            .nodes
            .iter()
            .find(|n| n.children.len() == 2 && n.children.iter().all(|&c| tree.is_leaf(c)))
            .unwrap();
        let idx = |c: usize| {
            nm.iter()
                .position(|x| Some(x) == tree.nodes[c].label.as_ref())
                .unwrap()
        };
        let (x, y) = (idx(cherry.children[0]), idx(cherry.children[1]));
        let diff = a.sequences[x]
            .iter()
            .zip(&a.sequences[y])
            .filter(|(p, q)| p != q)
            .count() as f64
            / 50_000.0;
        let expected = 0.75 * (1.0 - (-4.0f64 * 0.2 / 3.0).exp());
        assert!((diff - expected).abs() < 0.01, "{diff} vs {expected}");
    }

    #[test]
    fn shuffling_keeps_the_composition() {
        let s = b"AAAACCCGGT".repeat(50);
        let t = shuffle(&s, 1);
        assert_ne!(s, t);
        let count = |v: &[u8], b: u8| v.iter().filter(|&&x| x == b).count();
        for b in BASES {
            assert_eq!(count(&s, *b), count(&t, *b));
        }
        assert_eq!(t, shuffle(&s, 1));
    }

    #[test]
    fn random_trees_are_far_from_a_reference() {
        let r = Tree::parse("((S00,S01),(S02,S03),((S04,S05),((S06,S07),(S08,S09))));").unwrap();
        let d = random_tree_nrf(&r, 200, 3);
        assert_eq!(d.len(), 200);
        let mean = d.iter().sum::<f64>() / 200.0;
        assert!(mean > 0.7, "mean {mean}");
        assert_eq!(d, random_tree_nrf(&r, 200, 3));
    }
}
