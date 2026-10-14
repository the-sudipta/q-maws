//! Gene trees with horizontal transfer for the confirmatory test H5
//! (docs/PREREGISTRATION.md, Amendment 4): a transferred gene's tree is the
//! species tree after one random subtree prune-and-regraft (SPR). A random
//! subtree (not the whole tree, and leaving at least 3 other leaves) is cut
//! from its parent; a parent left with one child is removed and its two
//! branches are joined. The subtree is attached at the midpoint of a random
//! edge outside it, keeping the length of its own stem. Moves that give back
//! the species tree's topology are drawn again. The total branch length is
//! kept.

use crate::newick::{Node, Tree};
use crate::weight::SplitMix64;

/// One transfer: the leaves moved and the leaves below the edge they were
/// attached to (both sorted).
#[derive(Debug, Clone, PartialEq)]
pub struct Transfer {
    pub moved: Vec<String>,
    pub target: Vec<String>,
}

fn parents(t: &Tree) -> Vec<Option<usize>> {
    let mut p = vec![None; t.nodes.len()];
    for (i, n) in t.nodes.iter().enumerate() {
        for &c in &n.children {
            p[c] = Some(i);
        }
    }
    p
}

fn leaves_below(t: &Tree, n: usize, out: &mut Vec<String>) {
    let node = &t.nodes[n];
    if node.children.is_empty() {
        out.push(node.label.clone().unwrap_or_default());
    }
    for &c in &node.children {
        leaves_below(t, c, out);
    }
}

fn reachable(t: &Tree) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![t.root];
    while let Some(n) = stack.pop() {
        out.push(n);
        stack.extend(t.nodes[n].children.iter().copied());
    }
    out.sort_unstable();
    out
}

fn pick(rng: &mut SplitMix64, n: usize) -> usize {
    (rng.next_u64() % n as u64) as usize
}

/// One SPR move; `None` if the tree is too small (fewer than 4 leaves).
fn try_spr(species: &Tree, rng: &mut SplitMix64) -> Option<(Tree, Transfer)> {
    let mut t = species.clone();
    let total = t.leaf_names().len();
    if total < 4 {
        return None;
    }
    let par = parents(&t);
    let nodes = reachable(&t);
    let sizes: Vec<usize> = (0..t.nodes.len())
        .map(|n| {
            let mut v = Vec::new();
            leaves_below(&t, n, &mut v);
            v.len()
        })
        .collect();
    // The subtree to move: any non-root node leaving at least 3 leaves.
    let movable: Vec<usize> = nodes
        .iter()
        .copied()
        .filter(|&n| n != t.root && sizes[n] + 3 <= total)
        .collect();
    let p = movable[pick(rng, movable.len())];
    let mut moved = Vec::new();
    leaves_below(&t, p, &mut moved);
    moved.sort();
    // Prune.
    let q = par[p].expect("not the root");
    t.nodes[q].children.retain(|&c| c != p);
    if t.nodes[q].children.len() == 1 {
        let c = t.nodes[q].children[0];
        match par[q] {
            Some(g) => {
                let joined = t.nodes[c].length.unwrap_or(0.0) + t.nodes[q].length.unwrap_or(0.0);
                t.nodes[c].length = Some(joined);
                for x in t.nodes[g].children.iter_mut() {
                    if *x == q {
                        *x = c;
                    }
                }
            }
            None => {
                t.root = c;
                t.nodes[c].length = None;
            }
        }
    }
    // Regraft on an edge outside the moved subtree.
    let par = parents(&t);
    let targets: Vec<usize> = reachable(&t).into_iter().filter(|&n| n != t.root).collect();
    let target = targets[pick(rng, targets.len())];
    let up = par[target].expect("not the root");
    let half = t.nodes[target].length.unwrap_or(0.0) / 2.0;
    t.nodes[target].length = Some(half);
    let new = t.nodes.len();
    t.nodes.push(Node {
        label: None,
        length: Some(half),
        children: vec![target, p],
    });
    for x in t.nodes[up].children.iter_mut() {
        if *x == target {
            *x = new;
        }
    }
    let mut below = Vec::new();
    leaves_below(&t, target, &mut below);
    below.sort();
    // A clean copy without the nodes that are no longer reachable.
    let clean = Tree::parse(&t.to_newick(true)).expect("written trees parse");
    Some((
        clean,
        Transfer {
            moved,
            target: below,
        },
    ))
}

/// The species tree after one SPR move that changes its topology.
/// Panics if the tree has fewer than 5 leaves (no such move may exist).
pub fn transferred(species: &Tree, rng: &mut SplitMix64) -> (Tree, Transfer) {
    assert!(
        species.leaf_names().len() >= 5,
        "a transfer needs at least 5 leaves"
    );
    let splits = species.splits();
    loop {
        if let Some((t, x)) = try_spr(species, rng) {
            if t.splits() != splits {
                return (t, x);
            }
        }
    }
}

/// Which of `genes` genes get a transfer: exactly round(genes × fraction)
/// distinct indices (0-based, sorted), drawn with `rng`.
pub fn chosen_genes(genes: usize, fraction: f64, rng: &mut SplitMix64) -> Vec<usize> {
    let k = ((genes as f64) * fraction).round() as usize;
    let mut all: Vec<usize> = (0..genes).collect();
    for i in 0..k.min(genes) {
        let j = i + pick(rng, genes - i);
        all.swap(i, j);
    }
    let mut out = all[..k.min(genes)].to_vec();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn length(t: &Tree) -> f64 {
        t.nodes.iter().filter_map(|n| n.length).sum()
    }

    const SPECIES: &str =
        "((A:0.1,B:0.2):0.05,(C:0.1,(D:0.3,E:0.1):0.02):0.04,(F:0.2,(G:0.1,H:0.1):0.03):0.01);";

    #[test]
    fn a_transfer_keeps_the_leaves_and_the_length_and_changes_the_topology() {
        let s = Tree::parse(SPECIES).unwrap();
        let mut rng = SplitMix64::new(3);
        for _ in 0..200 {
            let (t, x) = transferred(&s, &mut rng);
            let mut a = t.leaf_names();
            let mut b = s.leaf_names();
            a.sort();
            b.sort();
            assert_eq!(a, b);
            assert!(
                (length(&t) - length(&s)).abs() < 1e-9,
                "{}",
                t.to_newick(true)
            );
            assert_ne!(t.splits(), s.splits());
            assert!(!x.moved.is_empty() && x.moved.len() <= 5);
            assert!(x.target.iter().all(|l| !x.moved.contains(l)));
        }
    }

    #[test]
    fn transfers_are_seeded() {
        let s = Tree::parse(SPECIES).unwrap();
        let a = transferred(&s, &mut SplitMix64::new(9)).0.to_newick(true);
        let b = transferred(&s, &mut SplitMix64::new(9)).0.to_newick(true);
        assert_eq!(a, b);
    }

    #[test]
    fn the_chosen_genes_have_the_exact_count() {
        let mut rng = SplitMix64::new(1);
        for (f, k) in [(0.0, 0), (0.1, 100), (0.2, 200), (0.4, 400)] {
            let g = chosen_genes(1000, f, &mut rng);
            assert_eq!(g.len(), k);
            assert!(g.windows(2).all(|w| w[0] < w[1]));
        }
    }
}
