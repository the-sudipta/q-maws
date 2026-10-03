//! Support and concordance of a tree on weighted quartets.
//!
//! - **S1** of an internal edge with split A | B: over all quartets with two
//!   taxa in A and two in B, the weight of the topology consistent with the
//!   split divided by the total weight of those quartets.
//! - **Halo value** of taxon i: over all quartets containing i, the weight
//!   of the topology induced by the tree divided by the total weight.
//! - **S2** of an internal edge: the fraction of column-bootstrap replicate
//!   trees that contain its split (`split_frequencies`).
//!
//! Quartets are added one at a time (the caller streams them in rank
//! order), and every sum runs in the order of addition, so the results do
//! not depend on threads.

use crate::amalgamate::Topology;
use crate::newick::Tree;

/// Support of one internal edge.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSupport {
    /// Node of the parsed tree below the edge.
    pub node: usize,
    /// Taxa below the edge (sorted names).
    pub clade: Vec<String>,
    /// S1, `None` if no weighted quartet spans the edge.
    pub s1: Option<f64>,
    /// Weight of the consistent topologies.
    pub consistent: f64,
    /// Total weight of the quartets spanning the edge.
    pub total: f64,
    /// Number of weighted quartets spanning the edge.
    pub quartets: u64,
}

/// Halo value of one taxon.
#[derive(Clone, Debug, PartialEq)]
pub struct Halo {
    pub taxon: String,
    /// `None` if no weighted quartet contains the taxon.
    pub value: Option<f64>,
    pub consistent: f64,
    pub total: f64,
}

/// S1 of every internal edge and the halo value of every taxon.
#[derive(Clone, Debug, PartialEq)]
pub struct Support {
    pub edges: Vec<EdgeSupport>,
    pub halo: Vec<Halo>,
    /// The tree with S1 (3 decimals) as internal node labels.
    pub newick: String,
}

/// Collects S1 and halo sums over a stream of weighted quartets.
pub struct Accumulator {
    names: Vec<String>,
    tree: Tree,
    topology: Topology,
    /// Node of each internal edge.
    nodes: Vec<usize>,
    /// Membership below each edge: `inside[e * m + t]`.
    inside: Vec<bool>,
    edge_consistent: Vec<f64>,
    edge_total: Vec<f64>,
    edge_quartets: Vec<u64>,
    halo_consistent: Vec<f64>,
    halo_total: Vec<f64>,
}

impl Accumulator {
    /// Prepares the sums for `newick`, whose leaves must be exactly `names`.
    pub fn new(names: &[String], newick: &str) -> Result<Self, String> {
        let tree = Tree::parse(newick).map_err(|e| e.to_string())?;
        let mut leaves = tree.leaf_names();
        leaves.sort();
        let mut sorted = names.to_vec();
        sorted.sort();
        if leaves != sorted {
            return Err("the tree's leaves differ from the taxa".into());
        }
        let topology = Topology::from_newick(newick, names)?;
        let m = names.len();
        let index: std::collections::HashMap<&str, usize> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        // Leaves below every node, by a post-order walk.
        let mut below: Vec<Vec<usize>> = vec![Vec::new(); tree.nodes.len()];
        let mut order = Vec::new();
        let mut stack = vec![tree.root];
        while let Some(n) = stack.pop() {
            order.push(n);
            stack.extend(&tree.nodes[n].children);
        }
        for &n in order.iter().rev() {
            if tree.is_leaf(n) {
                let name = tree.nodes[n].label.as_deref().unwrap_or("");
                below[n] = vec![index[name]];
            } else {
                let mut v: Vec<usize> = tree.nodes[n]
                    .children
                    .iter()
                    .flat_map(|&c| below[c].iter().copied())
                    .collect();
                v.sort_unstable();
                below[n] = v;
            }
        }
        let mut nodes = Vec::new();
        let mut inside = Vec::new();
        for (n, leaves) in below.iter().enumerate() {
            let size = leaves.len();
            if n == tree.root || tree.is_leaf(n) || size < 2 || m - size < 2 {
                continue;
            }
            nodes.push(n);
            let mut row = vec![false; m];
            for &t in leaves {
                row[t] = true;
            }
            inside.extend(row);
        }
        let e = nodes.len();
        Ok(Self {
            names: names.to_vec(),
            tree,
            topology,
            nodes,
            inside,
            edge_consistent: vec![0.0; e],
            edge_total: vec![0.0; e],
            edge_quartets: vec![0; e],
            halo_consistent: vec![0.0; m],
            halo_total: vec![0.0; m],
        })
    }

    /// Adds quartet a < b < c < d with weights of ab|cd, ac|bd and ad|bc.
    pub fn add(&mut self, q: [usize; 4], w: [f64; 3]) {
        let total = w[0] + w[1] + w[2];
        if total.is_nan() || total <= 0.0 {
            return;
        }
        let induced = self.topology.induced(q);
        for &t in &q {
            self.halo_total[t] += total;
            if let Some(i) = induced {
                self.halo_consistent[t] += w[i];
            }
        }
        let m = self.names.len();
        for e in 0..self.nodes.len() {
            let row = &self.inside[e * m..(e + 1) * m];
            let s = q.map(|t| row[t]);
            if s.iter().filter(|&&x| x).count() != 2 {
                continue;
            }
            // The partner of a on its side gives the consistent topology.
            let topo = if s[1] == s[0] {
                0
            } else if s[2] == s[0] {
                1
            } else {
                2
            };
            self.edge_consistent[e] += w[topo];
            self.edge_total[e] += total;
            self.edge_quartets[e] += 1;
        }
    }

    pub fn finish(mut self) -> Support {
        let m = self.names.len();
        let ratio = |c: f64, t: f64| if t > 0.0 { Some(c / t) } else { None };
        let mut edges = Vec::new();
        for (e, &node) in self.nodes.iter().enumerate() {
            let clade: Vec<String> = {
                let mut v: Vec<String> = (0..m)
                    .filter(|&t| self.inside[e * m + t])
                    .map(|t| self.names[t].clone())
                    .collect();
                v.sort();
                v
            };
            let s1 = ratio(self.edge_consistent[e], self.edge_total[e]);
            self.tree.nodes[node].label = s1.map(|v| format!("{v:.3}"));
            edges.push(EdgeSupport {
                node,
                clade,
                s1,
                consistent: self.edge_consistent[e],
                total: self.edge_total[e],
                quartets: self.edge_quartets[e],
            });
        }
        let halo = (0..m)
            .map(|t| Halo {
                taxon: self.names[t].clone(),
                value: ratio(self.halo_consistent[t], self.halo_total[t]),
                consistent: self.halo_consistent[t],
                total: self.halo_total[t],
            })
            .collect();
        Support {
            edges,
            halo,
            newick: self.tree.to_newick(false),
        }
    }
}

/// S2 of one internal edge.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeFrequency {
    /// Taxa below the edge (sorted names).
    pub clade: Vec<String>,
    /// Fraction of the replicate trees that contain the edge's split.
    pub s2: f64,
    /// Number of replicate trees that contain it.
    pub replicates: usize,
}

/// S2: for every internal edge of `newick` (in the order of
/// [`Accumulator`], so of `report/support.tsv`), the fraction of
/// `replicates` that contain its split, read as unrooted. Also returns the
/// tree with S2 (3 decimals) as internal node labels.
pub fn split_frequencies(
    newick: &str,
    replicates: &[String],
) -> Result<(Vec<EdgeFrequency>, String), String> {
    let mut tree = Tree::parse(newick).map_err(|e| e.to_string())?;
    let mut all = tree.leaf_names();
    all.sort();
    let mut counts: std::collections::BTreeMap<Vec<String>, usize> = Default::default();
    for (i, r) in replicates.iter().enumerate() {
        let t = Tree::parse(r).map_err(|e| format!("replicate {i}: {e}"))?;
        let mut leaves = t.leaf_names();
        leaves.sort();
        if leaves != all {
            return Err(format!("replicate {i}: its leaves differ from the tree's"));
        }
        for s in t.splits() {
            *counts.entry(s).or_insert(0) += 1;
        }
    }
    let m = all.len();
    let first = all.first().cloned().unwrap_or_default();
    // Leaves below every node, by a post-order walk.
    let mut below: Vec<Vec<String>> = vec![Vec::new(); tree.nodes.len()];
    let mut order = Vec::new();
    let mut stack = vec![tree.root];
    while let Some(n) = stack.pop() {
        order.push(n);
        stack.extend(&tree.nodes[n].children);
    }
    for &n in order.iter().rev() {
        if tree.is_leaf(n) {
            below[n] = vec![tree.nodes[n].label.clone().unwrap_or_default()];
        } else {
            let mut v: Vec<String> = tree.nodes[n]
                .children
                .iter()
                .flat_map(|&c| below[c].iter().cloned())
                .collect();
            v.sort();
            below[n] = v;
        }
    }
    let mut edges = Vec::new();
    for (n, clade) in below.into_iter().enumerate() {
        let size = clade.len();
        if n == tree.root || tree.is_leaf(n) || size < 2 || m - size < 2 {
            continue;
        }
        let split: Vec<String> = if clade.contains(&first) {
            all.iter().filter(|x| !clade.contains(x)).cloned().collect()
        } else {
            clade.clone()
        };
        let found = counts.get(&split).copied().unwrap_or(0);
        let s2 = if replicates.is_empty() {
            0.0
        } else {
            found as f64 / replicates.len() as f64
        };
        tree.nodes[n].label = Some(format!("{s2:.3}"));
        edges.push(EdgeFrequency {
            clade,
            s2,
            replicates: found,
        });
    }
    Ok((edges, tree.to_newick(false)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quartet;

    #[test]
    fn split_frequencies_read_replicates_unrooted() {
        let tree = "((t0,t1),t2,(t3,t4));";
        let reps = [
            // The same tree, rooted elsewhere.
            "(t0,(t1,(t2,(t3,t4))));".to_string(),
            // Keeps t3t4, loses t0t1.
            "((t0,t2),t1,(t3,t4));".to_string(),
            // Loses both.
            "((t0,t3),t2,(t1,t4));".to_string(),
            "((t0,t1),t2,(t3,t4));".to_string(),
        ];
        let (edges, labelled) = split_frequencies(tree, &reps).unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].clade, ["t0", "t1"]);
        assert_eq!((edges[0].replicates, edges[0].s2), (2, 0.5));
        assert_eq!(edges[1].clade, ["t3", "t4"]);
        assert_eq!((edges[1].replicates, edges[1].s2), (3, 0.75));
        assert_eq!(labelled, "((t0,t1)0.500,t2,(t3,t4)0.750);");
        // Same edge order as the S1 accumulator.
        let nm = names(5);
        let s = Accumulator::new(&nm, tree).unwrap().finish();
        let clades: Vec<_> = s.edges.iter().map(|e| e.clade.clone()).collect();
        assert_eq!(
            clades,
            edges.iter().map(|e| e.clade.clone()).collect::<Vec<_>>()
        );
        assert!(split_frequencies(tree, &["((t0,t1),t2,(t3,x));".to_string()]).is_err());
    }

    fn names(m: usize) -> Vec<String> {
        (0..m).map(|i| format!("t{i}")).collect()
    }

    /// Direct definition, quartet by quartet over the edge's split.
    fn s1_direct(clade: &[usize], qs: &[([usize; 4], [f64; 3])]) -> Option<f64> {
        let (mut c, mut t) = (0.0, 0.0);
        for (q, w) in qs {
            let inside: Vec<usize> = q.iter().copied().filter(|x| clade.contains(x)).collect();
            if inside.len() != 2 {
                continue;
            }
            // The side holding q[0], and q[0]'s partner on it.
            let side: Vec<usize> = if inside.contains(&q[0]) {
                inside
            } else {
                q.iter().copied().filter(|x| !clade.contains(x)).collect()
            };
            let partner = side.into_iter().find(|&x| x != q[0]).unwrap();
            let topo = q.iter().position(|&x| x == partner).unwrap() - 1;
            c += w[topo];
            t += w[0] + w[1] + w[2];
        }
        (t > 0.0).then(|| c / t)
    }

    #[test]
    fn worked_example_on_five_taxa() {
        // Tree ((t0,t1),t2,(t3,t4)); one internal edge per cherry.
        let nm = names(5);
        let tree = "((t0,t1),t2,(t3,t4));";
        let mut acc = Accumulator::new(&nm, tree).unwrap();
        // 0123: t0 t1 | t2 t3 is consistent (index 0).
        acc.add([0, 1, 2, 3], [0.6, 0.3, 0.1]);
        // 0134: 01|34 consistent with both edges.
        acc.add([0, 1, 3, 4], [0.9, 0.05, 0.05]);
        // 0234: the tree induces 02|34 (ab|cd, index 0).
        acc.add([0, 2, 3, 4], [0.5, 0.25, 0.25]);
        let s = acc.finish();
        assert_eq!(s.edges.len(), 2);
        let e01 = s.edges.iter().find(|e| e.clade == ["t0", "t1"]).unwrap();
        let e34 = s.edges.iter().find(|e| e.clade == ["t3", "t4"]).unwrap();
        // Edge (t0,t1): quartets 0123 and 0134 span it.
        assert_eq!(e01.quartets, 2);
        assert!((e01.s1.unwrap() - (0.6 + 0.9) / 2.0).abs() < 1e-12);
        // Edge (t3,t4): quartets 0134 and 0234.
        assert_eq!(e34.quartets, 2);
        assert!((e34.s1.unwrap() - (0.9 + 0.5) / 2.0).abs() < 1e-12);
        // Halo of t2: quartets 0123 (induced 01|23, weight 0.6) and 0234
        // (induced 02|34, weight 0.5), total 2.
        assert!((s.halo[2].value.unwrap() - 1.1 / 2.0).abs() < 1e-12);
        // t1 lies in 0123 and 0134: (0.6 + 0.9) / 2.
        assert!((s.halo[1].value.unwrap() - 0.75).abs() < 1e-12);
        assert_eq!(s.newick, "((t0,t1)0.750,t2,(t3,t4)0.700);");
    }

    #[test]
    fn matches_the_direct_definition_on_random_weights() {
        let mut x: u64 = 17;
        let mut unit = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 11) as f64 / (1u64 << 53) as f64
        };
        let nm = names(9);
        let tree = "((t0,(t1,t2)),(t3,t4),((t5,t6),(t7,t8)));";
        let parsed = Tree::parse(tree).unwrap();
        let mut qs = Vec::new();
        for r in 0..quartet::quartet_count(9) {
            let q = quartet::unrank(r);
            qs.push((q, [unit(), unit(), unit()]));
        }
        let mut acc = Accumulator::new(&nm, tree).unwrap();
        for (q, w) in &qs {
            acc.add(*q, *w);
        }
        let s = acc.finish();
        assert_eq!(s.edges.len(), parsed.splits().len());
        for e in &s.edges {
            let clade: Vec<usize> = e.clade.iter().map(|n| n[1..].parse().unwrap()).collect();
            let direct = s1_direct(&clade, &qs).unwrap();
            assert!((e.s1.unwrap() - direct).abs() < 1e-12);
        }
        // Quartets induced by the tree give S1 = 1 and halo 1 everywhere.
        let topo = Topology::from_newick(tree, &nm).unwrap();
        let mut acc = Accumulator::new(&nm, tree).unwrap();
        for (q, _) in &qs {
            let mut w = [0.0; 3];
            w[topo.induced(*q).unwrap()] = 1.0;
            acc.add(*q, w);
        }
        let s = acc.finish();
        assert!(s.edges.iter().all(|e| e.s1 == Some(1.0)));
        assert!(s.halo.iter().all(|h| h.value == Some(1.0)));
    }

    #[test]
    fn missing_weights_and_wrong_leaves() {
        let nm = names(5);
        let acc = Accumulator::new(&nm, "((t0,t1),t2,(t3,t4));").unwrap();
        let s = acc.finish();
        assert!(s.edges.iter().all(|e| e.s1.is_none() && e.quartets == 0));
        assert!(s.halo.iter().all(|h| h.value.is_none()));
        assert_eq!(s.newick, "((t0,t1),t2,(t3,t4));");
        assert!(Accumulator::new(&nm, "((t0,t1),t2,(t3,x));").is_err());
    }
}
