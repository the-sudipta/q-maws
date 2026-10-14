//! Calibration of support values (plan 2.10.5, hypothesis H4): every
//! internal edge of an estimated tree gives a pair (support s, y), where y
//! is 1 if its split is in the true tree. The pairs are put into equal-width
//! bins of s; ECE = sum over bins of (n_bin / n) × |mean s − mean y|.
//!
//! Splits are read as in `Tree::splits` (unrooted; each split named by the
//! side without the alphabetically first leaf), so "the split is in the true
//! tree" means the same as for nRF. Support labels are read from the
//! internal nodes and divided by `scale` (1 for Q-MAWS S1 and S2, 100 for
//! IQ-TREE UFBoot); edges without a numeric label are left out.

use crate::newick::Tree;
use std::collections::BTreeMap;

/// The support of each non-trivial split of `tree`, read from the label of
/// the node below the edge. A split that appears twice (the two edges at
/// a two-child root) keeps the first label found.
pub fn split_supports(tree: &Tree, scale: f64) -> BTreeMap<Vec<String>, f64> {
    let mut all = tree.leaf_names();
    all.sort();
    let walker = Walker {
        tree,
        first: all.first().cloned().unwrap_or_default(),
        all,
        scale,
    };
    let mut out = BTreeMap::new();
    walker.walk(tree.root, true, &mut out);
    out
}

/// The fixed inputs of the walk over the tree in [`split_supports`].
struct Walker<'a> {
    tree: &'a Tree,
    /// Sorted leaf names.
    all: Vec<String>,
    first: String,
    scale: f64,
}

impl Walker<'_> {
    /// Leaves below node `n`; records the split above `n` if it is labelled.
    fn walk(&self, n: usize, is_root: bool, out: &mut BTreeMap<Vec<String>, f64>) -> Vec<String> {
        let node = &self.tree.nodes[n];
        if node.children.is_empty() {
            return vec![node.label.clone().unwrap_or_default()];
        }
        let mut below = Vec::new();
        for &c in &node.children {
            below.extend(self.walk(c, false, out));
        }
        let total = self.all.len();
        if !is_root && below.len() >= 2 && total - below.len() >= 2 {
            if let Some(v) = node.label.as_deref().and_then(|l| l.parse::<f64>().ok()) {
                let mut side = below.clone();
                side.sort();
                let side = if side.contains(&self.first) {
                    self.all
                        .iter()
                        .filter(|x| !side.contains(x))
                        .cloned()
                        .collect()
                } else {
                    side
                };
                out.entry(side).or_insert(v / self.scale);
            }
        }
        below
    }
}

/// The (support, in the true tree) pairs of one estimated tree. Fails if
/// the trees have different leaves.
pub fn pairs(estimate: &Tree, truth: &Tree, scale: f64) -> Result<Vec<(f64, bool)>, String> {
    let mut a = estimate.leaf_names();
    let mut b = truth.leaf_names();
    a.sort();
    b.sort();
    if a != b {
        return Err("the estimated tree and the true tree have different leaves".into());
    }
    let true_splits: std::collections::BTreeSet<Vec<String>> = truth.splits().into_iter().collect();
    Ok(split_supports(estimate, scale)
        .into_iter()
        .map(|(side, s)| (s, true_splits.contains(&side)))
        .collect())
}

/// One bin of the reliability table.
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    pub lower: f64,
    pub upper: f64,
    pub n: usize,
    /// Mean support in the bin (`None` when empty).
    pub mean_support: Option<f64>,
    /// Fraction of the bin's splits that are in the true tree.
    pub fraction_true: Option<f64>,
}

/// Reliability table and expected calibration error.
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    pub n: usize,
    pub ece: f64,
    pub bins: Vec<Bin>,
}

/// ECE with `bins` equal-width bins on [0, 1]; a support of exactly 1 goes
/// into the last bin. Supports outside [0, 1] are an error.
pub fn calibration(points: &[(f64, bool)], bins: usize) -> Result<Calibration, String> {
    assert!(bins > 0);
    let mut sum_s = vec![0.0; bins];
    let mut sum_y = vec![0.0; bins];
    let mut count = vec![0usize; bins];
    for &(s, y) in points {
        if !(0.0..=1.0).contains(&s) {
            return Err(format!("support {s} is outside 0 to 1 (check the scale)"));
        }
        let b = ((s * bins as f64) as usize).min(bins - 1);
        sum_s[b] += s;
        sum_y[b] += if y { 1.0 } else { 0.0 };
        count[b] += 1;
    }
    let n: usize = count.iter().sum();
    let mut ece = 0.0;
    let mut out = Vec::with_capacity(bins);
    for b in 0..bins {
        let (ms, my) = if count[b] > 0 {
            let c = count[b] as f64;
            (Some(sum_s[b] / c), Some(sum_y[b] / c))
        } else {
            (None, None)
        };
        if let (Some(ms), Some(my)) = (ms, my) {
            ece += count[b] as f64 / n as f64 * (ms - my).abs();
        }
        out.push(Bin {
            lower: b as f64 / bins as f64,
            upper: (b + 1) as f64 / bins as f64,
            n: count[b],
            mean_support: ms,
            fraction_true: my,
        });
    }
    Ok(Calibration { n, ece, bins: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Tree {
        Tree::parse(s).unwrap()
    }

    #[test]
    fn supports_are_read_per_split_with_their_scale() {
        // IQ-TREE style: lengths and UFBoot labels.
        let s = split_supports(&t("(A:1,(B:1,C:1)86:1,((D:1,E:1)55:1,F:1)90:1);"), 100.0);
        assert_eq!(s.len(), 3);
        assert_eq!(s[&vec!["B".to_string(), "C".to_string()]], 0.86);
        assert_eq!(s[&vec!["D".to_string(), "E".to_string()]], 0.55);
        assert_eq!(
            s[&vec!["D".to_string(), "E".to_string(), "F".to_string()]],
            0.90
        );
        // A two-child root: both edges are one split; unlabelled edges are left out.
        let r = split_supports(&t("(((A,B)0.9,C)0.5,(D,E));"), 1.0);
        assert_eq!(r.len(), 2);
        assert_eq!(r[&vec!["D".to_string(), "E".to_string()]], 0.5);
    }

    #[test]
    fn pairs_mark_the_true_splits() {
        let est = t("((A,B)0.9,C,(D,E)0.4);");
        let truth = t("((A,B),D,(C,E));");
        let mut p = pairs(&est, &truth, 1.0).unwrap();
        p.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(p, vec![(0.4, false), (0.9, true)]);
        assert!(pairs(&est, &t("((A,B),C,(D,X));"), 1.0).is_err());
    }

    #[test]
    fn ece_of_a_worked_example() {
        // Bin 9 [0.9, 1]: supports 0.95 and 1.0, both true: |0.975 - 1| = 0.025.
        // Bin 2 [0.2, 0.3): 0.2 false and 0.25 true: |0.225 - 0.5| = 0.275.
        let c = calibration(&[(0.95, true), (1.0, true), (0.2, false), (0.25, true)], 10).unwrap();
        assert_eq!(c.n, 4);
        assert!((c.ece - (0.5 * 0.025 + 0.5 * 0.275)).abs() < 1e-12);
        assert_eq!(c.bins[9].n, 2);
        assert_eq!(c.bins[2].fraction_true, Some(0.5));
        assert_eq!(c.bins[5].mean_support, None);
        // Perfect calibration in one bin: ECE 0.
        let p = calibration(&[(0.5, true), (0.5, false)], 10).unwrap();
        assert!(p.ece.abs() < 1e-12);
        assert!(calibration(&[(86.0, true)], 10).is_err());
    }
}
