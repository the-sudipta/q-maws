//! Newick tree parsing and writing.
//!
//! Supported: nested groups, unquoted and single-quoted labels (`''` inside
//! quotes is a quote), branch lengths after `:`, labels on internal nodes
//! (for example support values), comments in square brackets, whitespace and
//! line breaks anywhere between tokens. Unquoted labels may contain `_` and
//! any character except `( ) [ ] ' : ; ,` and whitespace; as in the Newick
//! standard, `_` in an unquoted label is kept as `_` (not turned into a
//! space), so labels match taxon names exactly.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Leaf name or internal label, if any.
    pub label: Option<String>,
    /// Branch length to the parent, if given.
    pub length: Option<f64>,
    pub children: Vec<usize>,
}

/// A rooted representation of a Newick tree (the root may be a
/// multifurcation, as in unrooted trees written with a basal trichotomy).
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewickError {
    /// Byte position in the input.
    pub position: usize,
    pub message: String,
}

impl fmt::Display for NewickError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid Newick at character {}: {}",
            self.position + 1,
            self.message
        )
    }
}

impl std::error::Error for NewickError {}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    nodes: Vec<Node>,
}

impl<'a> Parser<'a> {
    fn err<T>(&self, message: impl Into<String>) -> Result<T, NewickError> {
        Err(NewickError {
            position: self.pos,
            message: message.into(),
        })
    }

    fn skip(&mut self) -> Result<(), NewickError> {
        loop {
            match self.s.get(self.pos) {
                Some(b) if b.is_ascii_whitespace() => self.pos += 1,
                Some(b'[') => {
                    let start = self.pos;
                    while self.s.get(self.pos).is_some_and(|&b| b != b']') {
                        self.pos += 1;
                    }
                    if self.pos >= self.s.len() {
                        self.pos = start;
                        return self.err("comment '[' is not closed");
                    }
                    self.pos += 1;
                }
                _ => return Ok(()),
            }
        }
    }

    fn peek(&mut self) -> Result<Option<u8>, NewickError> {
        self.skip()?;
        Ok(self.s.get(self.pos).copied())
    }

    fn label(&mut self) -> Result<Option<String>, NewickError> {
        self.skip()?;
        match self.s.get(self.pos) {
            Some(b'\'') => {
                self.pos += 1;
                let mut out = Vec::new();
                loop {
                    match self.s.get(self.pos) {
                        None => return self.err("quoted label is not closed"),
                        Some(b'\'') if self.s.get(self.pos + 1) == Some(&b'\'') => {
                            out.push(b'\'');
                            self.pos += 2;
                        }
                        Some(b'\'') => {
                            self.pos += 1;
                            break;
                        }
                        Some(&b) => {
                            out.push(b);
                            self.pos += 1;
                        }
                    }
                }
                Ok(Some(String::from_utf8_lossy(&out).into_owned()))
            }
            _ => {
                let start = self.pos;
                while let Some(&b) = self.s.get(self.pos) {
                    if b"()[]':;,".contains(&b) || b.is_ascii_whitespace() {
                        break;
                    }
                    self.pos += 1;
                }
                if self.pos == start {
                    Ok(None)
                } else {
                    Ok(Some(
                        String::from_utf8_lossy(&self.s[start..self.pos]).into_owned(),
                    ))
                }
            }
        }
    }

    fn length(&mut self) -> Result<Option<f64>, NewickError> {
        if self.peek()? != Some(b':') {
            return Ok(None);
        }
        self.pos += 1;
        self.skip()?;
        let start = self.pos;
        while self
            .s
            .get(self.pos)
            .is_some_and(|b| b.is_ascii_digit() || b"+-.eE".contains(b))
        {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or_default();
        match text.parse::<f64>() {
            Ok(v) if v.is_finite() => Ok(Some(v)),
            _ => {
                self.pos = start;
                self.err("branch length is not a number")
            }
        }
    }

    fn subtree(&mut self, depth: usize) -> Result<usize, NewickError> {
        if depth > 100_000 {
            return self.err("tree is nested too deeply");
        }
        let mut children = Vec::new();
        if self.peek()? == Some(b'(') {
            self.pos += 1;
            loop {
                children.push(self.subtree(depth + 1)?);
                match self.peek()? {
                    Some(b',') => self.pos += 1,
                    Some(b')') => {
                        self.pos += 1;
                        break;
                    }
                    _ => return self.err("expected ',' or ')'"),
                }
            }
        }
        let label = self.label()?;
        let length = self.length()?;
        if children.is_empty() && label.is_none() {
            return self.err("a leaf has no name");
        }
        self.nodes.push(Node {
            label,
            length,
            children,
        });
        Ok(self.nodes.len() - 1)
    }
}

impl Tree {
    /// Parses one tree. The final `;` is required.
    pub fn parse(text: &str) -> Result<Tree, NewickError> {
        let mut p = Parser {
            s: text.as_bytes(),
            pos: 0,
            nodes: Vec::new(),
        };
        if p.peek()?.is_none() {
            return p.err("the text is empty");
        }
        let root = p.subtree(0)?;
        if p.peek()? != Some(b';') {
            return p.err("expected ';' at the end of the tree");
        }
        p.pos += 1;
        if p.peek()?.is_some() {
            return p.err("unexpected text after ';'");
        }
        Ok(Tree {
            nodes: p.nodes,
            root,
        })
    }

    pub fn is_leaf(&self, node: usize) -> bool {
        self.nodes[node].children.is_empty()
    }

    /// Leaf names in left-to-right order.
    pub fn leaf_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![self.root];
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n];
            if node.children.is_empty() {
                out.push(node.label.clone().unwrap_or_default());
            } else {
                stack.extend(node.children.iter().rev());
            }
        }
        out
    }

    /// Renames leaves using `map`. Every leaf must be in the map; returns
    /// the leaf names that are not.
    pub fn rename_leaves(&mut self, map: &BTreeMap<String, String>) -> Result<(), Vec<String>> {
        let missing: Vec<String> = self
            .leaf_names()
            .into_iter()
            .filter(|n| !map.contains_key(n))
            .collect();
        if !missing.is_empty() {
            return Err(missing);
        }
        for node in &mut self.nodes {
            if node.children.is_empty() {
                if let Some(label) = &node.label {
                    node.label = Some(map[label].clone());
                }
            }
        }
        Ok(())
    }

    /// Writes the tree in Newick. Labels that need it are quoted.
    pub fn to_newick(&self, with_lengths: bool) -> String {
        let mut out = String::new();
        self.write(self.root, with_lengths, &mut out);
        out.push(';');
        out
    }

    fn write(&self, n: usize, with_lengths: bool, out: &mut String) {
        let node = &self.nodes[n];
        if !node.children.is_empty() {
            out.push('(');
            for (i, &c) in node.children.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                self.write(c, with_lengths, out);
            }
            out.push(')');
        }
        if let Some(label) = &node.label {
            out.push_str(&quote_if_needed(label));
        }
        if with_lengths {
            if let Some(len) = node.length {
                out.push(':');
                out.push_str(&len.to_string());
            }
        }
    }
}

fn quote_if_needed(label: &str) -> String {
    let needs = label.is_empty()
        || label
            .bytes()
            .any(|b| b"()[]':;,".contains(&b) || b.is_ascii_whitespace());
    if needs {
        format!("'{}'", label.replace('\'', "''"))
    } else {
        label.to_string()
    }
}

/// Differences between the leaf names of a tree and a set of taxon names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameMatch {
    /// Taxa without a leaf in the tree.
    pub missing_in_tree: Vec<String>,
    /// Leaves without a taxon.
    pub extra_in_tree: Vec<String>,
    /// Leaf names that occur more than once.
    pub duplicated_in_tree: Vec<String>,
}

impl NameMatch {
    pub fn is_exact(&self) -> bool {
        self.missing_in_tree.is_empty()
            && self.extra_in_tree.is_empty()
            && self.duplicated_in_tree.is_empty()
    }
}

/// Compares the tree's leaf names with `taxa` exactly (no case folding).
pub fn match_names(tree: &Tree, taxa: &[String]) -> NameMatch {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in tree.leaf_names() {
        *counts.entry(name).or_default() += 1;
    }
    let taxa_set: std::collections::BTreeSet<&String> = taxa.iter().collect();
    NameMatch {
        missing_in_tree: taxa_set
            .iter()
            .filter(|t| !counts.contains_key(**t))
            .map(|t| (*t).clone())
            .collect(),
        extra_in_tree: counts
            .keys()
            .filter(|n| !taxa_set.contains(n))
            .cloned()
            .collect(),
        duplicated_in_tree: counts
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(n, _)| n.clone())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_worksheet_tree() {
        let t = Tree::parse("((K,L),M,(N,P));").unwrap();
        assert_eq!(t.leaf_names(), vec!["K", "L", "M", "N", "P"]);
        assert_eq!(t.nodes[t.root].children.len(), 3);
        assert_eq!(t.to_newick(false), "((K,L),M,(N,P));");
    }

    #[test]
    fn lengths_labels_comments_quotes_and_whitespace() {
        let text = "(\n 'A b':0.5 , (B:1e-3,C:2)95:0.25 [comment], 'it''s':3\n);";
        let t = Tree::parse(text).unwrap();
        assert_eq!(t.leaf_names(), vec!["A b", "B", "C", "it's"]);
        assert_eq!(
            t.to_newick(true),
            "('A b':0.5,(B:0.001,C:2)95:0.25,'it''s':3);"
        );
        let inner = t.nodes[t.root].children[1];
        assert_eq!(t.nodes[inner].label.as_deref(), Some("95"));
    }

    #[test]
    fn underscores_and_symbols_are_kept() {
        let t = Tree::parse("(E._coli_K_12,Oreochromis_sp-KM2006,HEV_cva-13*);").unwrap();
        assert_eq!(
            t.leaf_names(),
            vec!["E._coli_K_12", "Oreochromis_sp-KM2006", "HEV_cva-13*"]
        );
    }

    #[test]
    fn errors_report_a_position() {
        for bad in [
            "",
            "(A,B)",
            "(A,B;",
            "(A,,B);",
            "(A:x,B);",
            "(A,B);extra",
            "(A,B)[open;",
        ] {
            assert!(Tree::parse(bad).is_err(), "{bad:?} should fail");
        }
        let e = Tree::parse("(A,B").unwrap_err();
        assert_eq!(e.position, 4);
    }

    #[test]
    fn renaming_and_name_matching() {
        let mut t = Tree::parse("((a,b),c,d);").unwrap();
        let map: BTreeMap<String, String> = [("a", "A"), ("b", "B"), ("c", "C")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(t.clone().rename_leaves(&map), Err(vec!["d".to_string()]));
        let mut full = map.clone();
        full.insert("d".into(), "D".into());
        t.rename_leaves(&full).unwrap();
        assert_eq!(t.to_newick(false), "((A,B),C,D);");

        let taxa: Vec<String> = ["A", "B", "C", "E"].iter().map(|s| s.to_string()).collect();
        let m = match_names(&t, &taxa);
        assert_eq!(m.missing_in_tree, vec!["E"]);
        assert_eq!(m.extra_in_tree, vec!["D"]);
        assert!(!m.is_exact());
        let dup = Tree::parse("((A,A),B,C);").unwrap();
        assert_eq!(match_names(&dup, &taxa[..3]).duplicated_in_tree, vec!["A"]);
    }
}
