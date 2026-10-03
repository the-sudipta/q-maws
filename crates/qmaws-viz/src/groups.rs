//! Group bands that need no outside information: a group file and the
//! automatic grouping by cutting the tree into clades (plan 4.8.1, sources
//! 1 and 3). Source 2, the Open Tree of Life taxonomy, is in
//! `qmaws-data::otl`.

use crate::tree::{Groups, TreeLayout};
use std::collections::BTreeMap;

/// Largest number of automatic groups (the size of the palette).
pub const MAX_AUTO_GROUPS: usize = 7;

/// Reads a group file: one `taxon<TAB>group` per line; empty lines, lines
/// starting with `#`, and a header line `taxon<TAB>group` are skipped.
/// Returns the groups of the given taxa and the problems found (unknown
/// taxa, lines without a tab, taxa listed twice).
pub fn read_group_file(text: &str, taxa: &[String]) -> (BTreeMap<String, String>, Vec<String>) {
    let mut out = BTreeMap::new();
    let mut problems = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((taxon, group)) = line.split_once('\t') else {
            problems.push(format!("line {}: no tab between taxon and group", i + 1));
            continue;
        };
        let (taxon, group) = (taxon.trim(), group.trim());
        if i == 0 && taxon.eq_ignore_ascii_case("taxon") && group.eq_ignore_ascii_case("group") {
            continue;
        }
        if group.is_empty() {
            problems.push(format!("line {}: no group for {taxon}", i + 1));
        } else if !taxa.iter().any(|t| t == taxon) {
            problems.push(format!(
                "line {}: {taxon} is not a taxon of this run",
                i + 1
            ));
        } else if out.contains_key(taxon) {
            problems.push(format!("line {}: {taxon} is listed again", i + 1));
        } else {
            out.insert(taxon.to_string(), group.to_string());
        }
    }
    (out, problems)
}

/// Cuts the tree (with its display root) into clades: starting from the
/// root's children, the largest clade is split into its children until
/// there are `target` clades of at least two taxa (target: one clade per
/// six taxa, 2 to 7). Single taxa get no band. Clades are named `Clade A`,
/// `Clade B`, ... in drawing order.
pub fn automatic_groups(layout: &TreeLayout) -> Groups {
    let m = layout.leaves().len();
    let target = m.div_ceil(6).clamp(2, MAX_AUTO_GROUPS);
    let mut clades: Vec<usize> = layout.nodes[0].children.clone();
    loop {
        let banded = clades
            .iter()
            .filter(|&&c| layout.clade(c).len() >= 2)
            .count();
        if banded >= target {
            break;
        }
        // Split the largest clade that can be split.
        let Some((pos, _)) = clades
            .iter()
            .enumerate()
            .filter(|(_, &c)| !layout.nodes[c].children.is_empty())
            .max_by_key(|(_, &c)| layout.clade(c).len())
        else {
            break;
        };
        let c = clades.remove(pos);
        for (k, &child) in layout.nodes[c].children.iter().enumerate() {
            clades.insert(pos + k, child);
        }
        if clades.len() > m {
            break;
        }
    }
    let mut of = BTreeMap::new();
    let mut letter = 0u8;
    for &c in &clades {
        let members = layout.clade(c);
        if members.len() < 2 {
            continue;
        }
        let name = format!("Clade {}", (b'A' + letter.min(25)) as char);
        letter += 1;
        for t in members {
            of.insert(t, name.clone());
        }
    }
    Groups {
        source: "automatic: clades of the tree".into(),
        of,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_files_are_read_with_their_problems() {
        let taxa: Vec<String> = ["A", "B", "C"].iter().map(|s| s.to_string()).collect();
        let text = "taxon\tgroup\nA\tFish\n# note\n\nB\tFish\nZ\tOther\nC Nope\nA\tAgain\n";
        let (groups, problems) = read_group_file(text, &taxa);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["A"], "Fish");
        assert_eq!(problems.len(), 3, "{problems:?}");
    }

    #[test]
    fn automatic_groups_are_clades_of_two_or_more() {
        let l = TreeLayout::from_newick(
            "((((A,B),(C,D)),((E,F),(G,H))),(((I,J),(K,L)),((M,N),(O,P))));",
        )
        .unwrap();
        let g = automatic_groups(&l);
        let names = g.names();
        // 16 taxa: target 3 clades.
        assert!(
            names.len() >= 3 && names.len() <= MAX_AUTO_GROUPS,
            "{names:?}"
        );
        for name in &names {
            let members: Vec<&String> =
                g.of.iter()
                    .filter(|(_, v)| *v == name)
                    .map(|(k, _)| k)
                    .collect();
            assert!(members.len() >= 2);
            // A group is a clade of the layout.
            let mut m: Vec<String> = members.into_iter().cloned().collect();
            m.sort();
            assert!((0..l.nodes.len()).any(|v| l.clade(v) == m), "{name}: {m:?}");
        }
    }
}
