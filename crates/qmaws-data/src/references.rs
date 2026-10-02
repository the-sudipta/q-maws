//! Reference trees for the benchmark datasets (`data/references/`),
//! compiled into the program.
//!
//! For each reference `<id>`:
//! - `<id>.afproject.nwk`: the tree as published on the AFproject results
//!   page, with AFproject's leaf names;
//! - `<id>.names.tsv` (when names differ): AFproject leaf name to sequence
//!   identifier, as used in the dataset's FASTA files;
//! - `<id>.nwk`: the tree with sequence identifiers, used by the program.
//!
//! The tests check that translating the AFproject tree with the name table
//! gives exactly the committed `<id>.nwk`.

use qmaws_core::newick::{NewickError, Tree};
use std::collections::BTreeMap;

pub struct Reference {
    pub id: &'static str,
    /// Tree with sequence identifiers.
    pub newick: &'static str,
    /// Tree as published by AFproject.
    pub afproject_newick: &'static str,
    /// AFproject leaf name to sequence identifier, if names differ.
    pub names_tsv: Option<&'static str>,
}

macro_rules! reference {
    ($id:literal, names) => {
        Reference {
            id: $id,
            newick: include_str!(concat!("../../../data/references/", $id, ".nwk")),
            afproject_newick: include_str!(concat!(
                "../../../data/references/",
                $id,
                ".afproject.nwk"
            )),
            names_tsv: Some(include_str!(concat!(
                "../../../data/references/",
                $id,
                ".names.tsv"
            ))),
        }
    };
    ($id:literal) => {
        Reference {
            id: $id,
            newick: include_str!(concat!("../../../data/references/", $id, ".nwk")),
            afproject_newick: include_str!(concat!(
                "../../../data/references/",
                $id,
                ".afproject.nwk"
            )),
            names_tsv: None,
        }
    };
}

pub const REFERENCES: [Reference; 5] = [
    reference!("fish_mito", names),
    reference!("ecoli"),
    reference!("ecoli_shigella_hgt", names),
    reference!("yersinia_hgt", names),
    reference!("sim_hgt"),
];

pub fn reference(id: &str) -> Option<&'static Reference> {
    REFERENCES.iter().find(|r| r.id == id)
}

impl Reference {
    pub fn tree(&self) -> Result<Tree, NewickError> {
        Tree::parse(self.newick)
    }
}

/// Parses a two-column, tab-separated name table with a header line.
pub fn parse_names(tsv: &str) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for (i, line) in tsv.lines().enumerate().skip(1) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let (from, to) = line
            .split_once('\t')
            .ok_or_else(|| format!("line {} has no tab", i + 1))?;
        if map.insert(from.to_string(), to.to_string()).is_some() {
            return Err(format!("name {from} appears twice"));
        }
    }
    Ok(map)
}

/// Finds the reference tree embedded in an AFproject results page: the
/// download link `data:text/plain;charset=utf-8,<tree>` of the reference
/// tree panel. Percent-encoded characters are decoded.
pub fn tree_from_results_page(html: &str) -> Option<String> {
    const PREFIX: &str = "data:text/plain;charset=utf-8,";
    let start = html.find(PREFIX)? + PREFIX.len();
    let end = html[start..].find(['"', '\''])? + start;
    Some(percent_decode(&html[start..end]))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Translates an AFproject tree to sequence identifiers. Fails if a leaf
/// has no entry in the table.
pub fn translate(afproject_newick: &str, names: &BTreeMap<String, String>) -> Result<Tree, String> {
    let mut tree = Tree::parse(afproject_newick.trim()).map_err(|e| e.to_string())?;
    tree.rename_leaves(names)
        .map_err(|missing| format!("no translation for: {}", missing.join(", ")))?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(text: &str) -> String {
        Tree::parse(text.trim()).unwrap().to_newick(true)
    }

    #[test]
    fn every_reference_parses() {
        for r in &REFERENCES {
            let t = r.tree().unwrap_or_else(|e| panic!("{}: {e}", r.id));
            assert!(t.leaf_names().len() >= 8, "{}", r.id);
        }
        assert_eq!(
            reference("fish_mito")
                .unwrap()
                .tree()
                .unwrap()
                .leaf_names()
                .len(),
            25
        );
        assert_eq!(
            reference("ecoli")
                .unwrap()
                .tree()
                .unwrap()
                .leaf_names()
                .len(),
            29
        );
        assert_eq!(
            reference("ecoli_shigella_hgt")
                .unwrap()
                .tree()
                .unwrap()
                .leaf_names()
                .len(),
            27
        );
        assert_eq!(
            reference("yersinia_hgt")
                .unwrap()
                .tree()
                .unwrap()
                .leaf_names()
                .len(),
            8
        );
        assert_eq!(
            reference("sim_hgt")
                .unwrap()
                .tree()
                .unwrap()
                .leaf_names()
                .len(),
            33
        );
    }

    #[test]
    fn translated_afproject_trees_equal_the_committed_trees() {
        for r in &REFERENCES {
            let committed = canonical(r.newick);
            let translated = match r.names_tsv {
                Some(tsv) => {
                    let names = parse_names(tsv).unwrap();
                    assert_eq!(
                        names.len(),
                        Tree::parse(r.afproject_newick.trim())
                            .unwrap()
                            .leaf_names()
                            .len(),
                        "{}: one table entry per leaf",
                        r.id
                    );
                    translate(r.afproject_newick, &names)
                        .unwrap()
                        .to_newick(true)
                }
                None => canonical(r.afproject_newick),
            };
            assert_eq!(translated, committed, "{}", r.id);
        }
    }

    #[test]
    fn the_tree_is_found_on_a_results_page() {
        let html = r#"<a id="download_usr_tree" href="data:text/plain;charset=utf-8,((A,B),(C,%44));" download>"#;
        assert_eq!(
            tree_from_results_page(html).as_deref(),
            Some("((A,B),(C,D));")
        );
        assert_eq!(tree_from_results_page("<html></html>"), None);
    }

    #[test]
    fn name_tables_reject_bad_lines() {
        assert!(parse_names("a\tb\nx\ty\n").is_ok());
        assert!(parse_names("a\tb\nxy\n").is_err());
        assert!(parse_names("a\tb\nx\ty\nx\tz\n").is_err());
    }
}
