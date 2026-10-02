//! The teaching worksheet (`qmaws teach`): a hand-calculable version of the
//! method, with every step and every arithmetic operation written out.
//!
//! Steps: two-letter MAWs ("xy is a MAW if x and y occur in s and xy never
//! occurs as neighbours"), no strand filter and no length selection; the
//! matrix without constant columns; the pattern table of every quartet;
//! informative-pattern voting (W1) with weights; classroom amalgamation (the
//! pair score is the sum of the weights of winning topologies that put the
//! pair together; the highest pair is merged, ties in alphabetical order of
//! the pair; the new group's score with X is the average of the two merged
//! scores with X; stop at 3 groups); nRF against a reference tree.

use crate::newick::{nrf, Tree};

/// Letters in the order used for words.
const LETTERS: [char; 4] = ['A', 'C', 'G', 'T'];

/// Largest worksheet input accepted.
pub const MAX_TAXA: usize = 8;
pub const MAX_LENGTH: usize = 200;

/// The worksheet example of the teaching material (taxa K, L, M, N, P).
pub const EXAMPLE: [(&str, &str); 5] = [
    ("K", "AACGTA"),
    ("L", "ACGTAG"),
    ("M", "ACGTTC"),
    ("N", "CATTGC"),
    ("P", "CATGGC"),
];
/// Reference tree of the example.
pub const EXAMPLE_REFERENCE: &str = "((K,L),M,(N,P));";
/// Control tree of the example.
pub const EXAMPLE_CONTROL: &str = "((K,M),L,(N,P));";

/// Formats a number with up to 10 decimals and no trailing zeros.
pub fn num(x: f64) -> String {
    let s = format!("{:.10}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeachError(pub String);

impl std::fmt::Display for TeachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TeachError {}

/// What the worksheet computed, for tests and for other uses.
#[derive(Debug, Clone, PartialEq)]
pub struct Worksheet {
    pub text: String,
    /// Kept words (matrix columns) in order.
    pub words: Vec<String>,
    /// matrix[word][taxon] = 0 or 1.
    pub matrix: Vec<Vec<u8>>,
    /// Quartets as taxon indices, in worksheet order.
    pub quartets: Vec<[usize; 4]>,
    /// patterns[word][quartet] as a 4-character string.
    pub patterns: Vec<Vec<String>>,
    pub tree: String,
    pub nrf: Option<f64>,
}

fn is_maw(seq: &str, w: &str) -> bool {
    let x = w.as_bytes()[0] as char;
    let y = w.as_bytes()[1] as char;
    seq.contains(x) && seq.contains(y) && !seq.contains(w)
}

fn pairs_of(seq: &str) -> Vec<String> {
    seq.as_bytes()
        .windows(2)
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

struct Labels {
    names: Vec<String>,
    single: bool,
}

impl Labels {
    fn join(&self, idx: &[usize]) -> String {
        let parts: Vec<&str> = idx.iter().map(|&i| self.names[i].as_str()).collect();
        if self.single {
            parts.concat()
        } else {
            parts.join(",")
        }
    }
    fn pair(&self, i: usize, j: usize) -> String {
        format!("{}-{}", self.names[i], self.names[j])
    }
}

/// A cluster of the classroom amalgamation.
#[derive(Clone)]
struct Group {
    members: Vec<usize>,
    newick: String,
}

fn table(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.get(c))
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = String::new();
    for r in rows {
        let line: Vec<String> = r
            .iter()
            .enumerate()
            .map(|(c, s)| format!("{:<w$}", s, w = widths[c]))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out
}

fn heading(out: &mut String, title: &str) {
    out.push('\n');
    out.push_str(title);
    out.push('\n');
    out.push_str(&"-".repeat(title.chars().count()));
    out.push('\n');
}

/// Checks a student's input against the worksheet limits.
pub fn check_input(taxa: &[(String, String)]) -> Result<(), TeachError> {
    if taxa.len() < 4 {
        return Err(TeachError(format!(
            "the worksheet needs at least 4 taxa; got {}",
            taxa.len()
        )));
    }
    if taxa.len() > MAX_TAXA {
        return Err(TeachError(format!(
            "the worksheet is meant to be worked by hand: at most {MAX_TAXA} taxa, got {}",
            taxa.len()
        )));
    }
    for (name, seq) in taxa {
        if seq.len() > MAX_LENGTH {
            return Err(TeachError(format!(
                "the worksheet is meant to be worked by hand: sequences of at most {MAX_LENGTH} letters, but {name} has {}",
                seq.len()
            )));
        }
        if seq.len() < 2 {
            return Err(TeachError(format!("{name} needs at least 2 letters")));
        }
    }
    Ok(())
}

/// Builds the worksheet. `taxa` are (name, cleaned sequence) in name order;
/// `control` is an extra tree compared with the reference (example only);
/// `model_example` adds the two-state model illustration.
pub fn worksheet(
    taxa: &[(String, String)],
    reference: Option<&Tree>,
    control: Option<&Tree>,
    model_example: bool,
) -> Result<Worksheet, TeachError> {
    check_input(taxa)?;
    let m = taxa.len();
    let labels = Labels {
        names: taxa.iter().map(|t| t.0.clone()).collect(),
        single: taxa.iter().all(|t| t.0.chars().count() == 1),
    };
    let mut out = String::new();
    out.push_str("Q-MAWS teaching worksheet\n");
    out.push_str("=========================\n");

    // Step 1. Input.
    heading(&mut out, "Step 1. Input");
    let mut rows = vec![vec!["Taxon".to_string(), "Sequence".to_string()]];
    for (n, s) in taxa {
        rows.push(vec![n.clone(), s.clone()]);
    }
    out.push_str(&table(&rows));

    // Step 2. Neighbouring pairs.
    heading(&mut out, "Step 2. Neighbouring pairs");
    out.push_str("A sliding window of width 2: a sequence of length z has z - 1 pairs.\n\n");
    let mut rows = vec![vec!["Taxon".to_string(), "Pairs".to_string()]];
    for (n, s) in taxa {
        rows.push(vec![n.clone(), pairs_of(s).join(", ")]);
    }
    out.push_str(&table(&rows));

    // Step 3. Two-letter MAWs.
    heading(&mut out, "Step 3. Two-letter minimal absent words");
    out.push_str(
        "xy is a MAW of s if x occurs in s, y occurs in s, and xy is not among the\nneighbouring pairs of s.\n",
    );
    let all_letters = taxa
        .iter()
        .all(|(_, s)| LETTERS.iter().all(|&c| s.contains(c)));
    if all_letters {
        out.push_str(&format!(
            "All {} sequences contain A, C, G and T, so here xy is a MAW exactly when xy is\nnot a neighbouring pair.",
            m
        ));
    } else {
        out.push_str("Letters that occur:");
        for (n, s) in taxa {
            let occ: Vec<String> = LETTERS
                .iter()
                .filter(|&&c| s.contains(c))
                .map(|c| c.to_string())
                .collect();
            out.push_str(&format!(" {n}: {};", occ.join(" ")));
        }
        out.pop();
        out.push('.');
    }
    out.push_str(" There are 4 x 4 = 16 candidate words.\n\n");
    let all_words: Vec<String> = LETTERS
        .iter()
        .flat_map(|&x| LETTERS.iter().map(move |&y| format!("{x}{y}")))
        .collect();
    let full: Vec<Vec<u8>> = all_words
        .iter()
        .map(|w| taxa.iter().map(|(_, s)| is_maw(s, w) as u8).collect())
        .collect();
    let all_one: Vec<&String> = all_words
        .iter()
        .zip(&full)
        .filter(|(_, r)| r.iter().all(|&v| v == 1))
        .map(|(w, _)| w)
        .collect();
    let all_zero: Vec<&String> = all_words
        .iter()
        .zip(&full)
        .filter(|(_, r)| r.iter().all(|&v| v == 0))
        .map(|(w, _)| w)
        .collect();
    let list = |v: &[&String]| {
        if v.is_empty() {
            "none".to_string()
        } else {
            v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        }
    };
    out.push_str(&format!(
        "MAW of every sequence (constant all-1 columns, removed): {}.\n",
        list(&all_one)
    ));
    out.push_str(&format!(
        "MAW of no sequence (constant all-0 columns, removed): {}.\n",
        list(&all_zero)
    ));

    // Step 4. Matrix.
    let mut words = Vec::new();
    let mut matrix = Vec::new();
    for (w, r) in all_words.iter().zip(&full) {
        if r.contains(&1) && r.contains(&0) {
            words.push(w.clone());
            matrix.push(r.clone());
        }
    }
    heading(&mut out, "Step 4. Matrix");
    out.push_str("1 = the word is a MAW of the sequence; 0 = it is not.\n\n");
    let mut rows = vec![std::iter::once("Word".to_string())
        .chain(labels.names.iter().cloned())
        .collect::<Vec<_>>()];
    for (w, r) in words.iter().zip(&matrix) {
        rows.push(
            std::iter::once(w.clone())
                .chain(r.iter().map(|v| v.to_string()))
                .collect(),
        );
    }
    out.push_str(&table(&rows));
    out.push_str(&format!("\n{} columns.\n", words.len()));

    // Step 5. Quartets.
    let mut quartets = Vec::new();
    for a in 0..m {
        for b in a + 1..m {
            for c in b + 1..m {
                for d in c + 1..m {
                    quartets.push([a, b, c, d]);
                }
            }
        }
    }
    heading(&mut out, "Step 5. Quartets");
    let numerator: usize = m * (m - 1) * (m - 2) * (m - 3);
    let qnames: Vec<String> = quartets.iter().map(|q| labels.join(q)).collect();
    out.push_str(&format!(
        "Q = {} x {} x {} x {} / 24 = {} / 24 = {}: {}",
        m,
        m - 1,
        m - 2,
        m - 3,
        numerator,
        quartets.len(),
        qnames.join(", ")
    ));
    if m == 5 {
        out.push_str(" (each leaves out exactly one taxon)");
    }
    out.push_str(".\n");

    // Step 6. Pattern table.
    heading(&mut out, "Step 6. Pattern table");
    out.push_str("Members of each quartet are read in name order.\n\n");
    let patterns: Vec<Vec<String>> = matrix
        .iter()
        .map(|r| {
            quartets
                .iter()
                .map(|q| q.iter().map(|&t| char::from(b'0' + r[t])).collect())
                .collect()
        })
        .collect();
    let mut rows = vec![std::iter::once("Word".to_string())
        .chain(qnames.iter().cloned())
        .collect::<Vec<_>>()];
    for (w, p) in words.iter().zip(&patterns) {
        rows.push(
            std::iter::once(w.clone())
                .chain(p.iter().cloned())
                .collect(),
        );
    }
    out.push_str(&table(&rows));
    let zeros = patterns
        .iter()
        .flatten()
        .filter(|p| p.as_str() == "0000")
        .count();
    if zeros == 0 {
        out.push_str("\nNo 0000 pattern occurs.\n");
    } else {
        out.push_str(&format!(
            "\nThe pattern 0000 occurs {zeros} times; it casts no vote.\n"
        ));
    }

    // Step 7. W1 votes.
    heading(
        &mut out,
        "Step 7. Informative-pattern votes (W1) and weights",
    );
    out.push_str(
        "For members (a, b, c, d): ab|cd is supported by 1100 and 0011, ac|bd by 1010 and\n0101, ad|bc by 1001 and 0110. Weight of the winner = its votes / all votes.\n\n",
    );
    let topo = |q: &[usize; 4], t: usize| -> (String, [usize; 2], [usize; 2]) {
        let (x, y) = match t {
            0 => ([q[0], q[1]], [q[2], q[3]]),
            1 => ([q[0], q[2]], [q[1], q[3]]),
            _ => ([q[0], q[3]], [q[1], q[2]]),
        };
        (format!("{}|{}", labels.join(&x), labels.join(&y)), x, y)
    };
    let support = [["1100", "0011"], ["1010", "0101"], ["1001", "0110"]];
    let mut rows = vec![vec![
        "Quartet".to_string(),
        "T1 (votes)".to_string(),
        "T2 (votes)".to_string(),
        "T3 (votes)".to_string(),
        "Winner".to_string(),
        "Weight".to_string(),
    ]];
    // Winning topologies: (pair 1, pair 2, weight, label).
    let mut winners: Vec<([usize; 2], [usize; 2], f64, String)> = Vec::new();
    let mut votes_per_quartet = Vec::new();
    for (qi, q) in quartets.iter().enumerate() {
        let mut cells = vec![qnames[qi].clone()];
        let mut scores = [0usize; 3];
        for t in 0..3 {
            let mut voters = Vec::new();
            for pat in support[t] {
                for (w, p) in words.iter().zip(&patterns) {
                    if p[qi] == pat {
                        voters.push(w.clone());
                    }
                }
            }
            scores[t] = voters.len();
            let (name, _, _) = topo(q, t);
            if voters.is_empty() {
                cells.push(format!("{name} (0)"));
            } else {
                cells.push(format!("{name} ({}: {})", voters.len(), voters.join(", ")));
            }
        }
        let total: usize = scores.iter().sum();
        votes_per_quartet.push(total);
        if total == 0 {
            cells.push("none".to_string());
            cells.push("no vote: skipped".to_string());
        } else {
            // Highest score; ties go to the first topology (T1, T2, T3).
            let best = (0..3)
                .max_by(|&i, &j| scores[i].cmp(&scores[j]).then(j.cmp(&i)))
                .unwrap();
            let (name, x, y) = topo(q, best);
            let w = scores[best] as f64 / total as f64;
            cells.push(name.clone());
            cells.push(format!("{} / {} = {}", scores[best], total, num(w)));
            winners.push((x, y, w, name));
        }
        rows.push(cells);
    }
    out.push_str(&table(&rows));
    let cells = words.len() * quartets.len();
    let used: usize = votes_per_quartet.iter().sum();
    out.push_str(&format!(
        "\nVotes used: {} = {} of {} pattern cells; {} cells cast no vote (one-taxon-differs\nor all-equal patterns).\n",
        votes_per_quartet
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" + "),
        used,
        cells,
        cells - used
    ));

    // Step 8. Classroom amalgamation.
    heading(&mut out, "Step 8. Classroom amalgamation");
    let pair_list: Vec<(usize, usize)> = (0..m)
        .flat_map(|i| (i + 1..m).map(move |j| (i, j)))
        .collect();
    out.push_str(&format!(
        "Number of pairs = {} x {} / 2 = {}.\n\n",
        m,
        m - 1,
        pair_list.len()
    ));
    out.push_str("Contribution of each winning topology to its two pairs:\n\n");
    let mut score = vec![vec![0.0f64; m]; m];
    let mut rows = vec![std::iter::once("Winner (weight)".to_string())
        .chain(pair_list.iter().map(|&(i, j)| labels.pair(i, j)))
        .collect::<Vec<_>>()];
    for (x, y, w, name) in &winners {
        let mut row = vec![format!("{name} ({})", num(*w))];
        for &(i, j) in &pair_list {
            let hit = (x.contains(&i) && x.contains(&j)) || (y.contains(&i) && y.contains(&j));
            row.push(if hit { num(*w) } else { String::new() });
        }
        score[x[0]][x[1]] += w;
        score[x[1]][x[0]] += w;
        score[y[0]][y[1]] += w;
        score[y[1]][y[0]] += w;
        rows.push(row);
    }
    rows.push(
        std::iter::once("Sum".to_string())
            .chain(pair_list.iter().map(|&(i, j)| num(score[i][j])))
            .collect(),
    );
    out.push_str(&table(&rows));

    let mut groups: Vec<Group> = (0..m)
        .map(|i| Group {
            members: vec![i],
            newick: labels.names[i].clone(),
        })
        .collect();
    let group_name = |g: &Group| -> String {
        if g.members.len() == 1 {
            labels.names[g.members[0]].clone()
        } else {
            let mut names: Vec<String> =
                g.members.iter().map(|&i| labels.names[i].clone()).collect();
            names.sort();
            format!("{{{}}}", names.join(","))
        }
    };
    let score_table = |groups: &[Group], score: &Vec<Vec<f64>>| -> String {
        let mut rows = vec![std::iter::once(String::new())
            .chain(groups.iter().map(group_name))
            .collect::<Vec<_>>()];
        for (a, g) in groups.iter().enumerate() {
            let mut r = vec![group_name(g)];
            for (b, s) in score[a].iter().enumerate().take(groups.len()) {
                r.push(if a == b { "-".to_string() } else { num(*s) });
            }
            rows.push(r);
        }
        table(&rows)
    };
    out.push_str("\nScore matrix:\n\n");
    out.push_str(&score_table(&groups, &score));

    let mut round = 0;
    while groups.len() > 3 {
        round += 1;
        // Highest score; ties in alphabetical order of the pair.
        let mut best: Option<(usize, usize)> = None;
        for a in 0..groups.len() {
            for b in a + 1..groups.len() {
                let key = |x: usize, y: usize| {
                    let (p, q) = (group_name(&groups[x]), group_name(&groups[y]));
                    if p <= q {
                        (p, q)
                    } else {
                        (q, p)
                    }
                };
                best = match best {
                    None => Some((a, b)),
                    Some((c, d)) => {
                        let s1 = score[a][b];
                        let s2 = score[c][d];
                        if s1 > s2 || (s1 == s2 && key(a, b) < key(c, d)) {
                            Some((a, b))
                        } else {
                            Some((c, d))
                        }
                    }
                };
            }
        }
        let (a, b) = best.expect("at least two groups");
        let (ga, gb) = (groups[a].clone(), groups[b].clone());
        let mut members = ga.members.clone();
        members.extend(&gb.members);
        members.sort_unstable();
        let (first, second) = if ga.members[0] < gb.members[0] {
            (&ga, &gb)
        } else {
            (&gb, &ga)
        };
        let merged = Group {
            members,
            newick: format!("({},{})", first.newick, second.newick),
        };
        out.push_str(&format!(
            "\nRound {round}: highest = {} ({}-{}) -> merge {}.\n",
            num(score[a][b]),
            group_name(first),
            group_name(second),
            group_name(&merged)
        ));
        // New scores with every other group.
        let others: Vec<usize> = (0..groups.len()).filter(|&x| x != a && x != b).collect();
        let mut new_scores = Vec::new();
        let mut parts = Vec::new();
        for &x in &others {
            let s = (score[a][x] + score[b][x]) / 2.0;
            parts.push(format!(
                "{}-{} = ({} + {}) / 2 = {}",
                group_name(&merged),
                group_name(&groups[x]),
                num(score[a][x]),
                num(score[b][x]),
                num(s)
            ));
            new_scores.push(s);
        }
        // Rebuild groups and scores: the merged group takes the place of the
        // first of the two, the second is removed.
        let keep_at = a.min(b);
        let mut new_groups = Vec::new();
        let mut index_map = Vec::new();
        for (x, g) in groups.iter().enumerate() {
            if x == keep_at {
                new_groups.push(merged.clone());
                index_map.push(None);
            } else if x != a && x != b {
                new_groups.push(g.clone());
                index_map.push(Some(x));
            }
        }
        let n = new_groups.len();
        let mut new_score = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                new_score[i][j] = match (index_map[i], index_map[j]) {
                    (Some(x), Some(y)) => score[x][y],
                    (None, Some(y)) | (Some(y), None) => {
                        new_scores[others.iter().position(|&o| o == y).unwrap()]
                    }
                    (None, None) => 0.0,
                };
            }
        }
        groups = new_groups;
        score = new_score;
        if groups.len() > 3 {
            out.push_str(&format!("New scores: {}.\n\n", parts.join("; ")));
            out.push_str(&score_table(&groups, &score));
        } else {
            out.push_str(&format!(
                "Three groups remain ({}); stop. Merges performed: n - 3 = {}.\n",
                groups.iter().map(group_name).collect::<Vec<_>>().join(", "),
                round
            ));
        }
    }
    let mut ordered = groups.clone();
    ordered.sort_by_key(|g| g.members[0]);
    let tree = format!(
        "({});",
        ordered
            .iter()
            .map(|g| g.newick.clone())
            .collect::<Vec<_>>()
            .join(",")
    );
    out.push_str(&format!("\nFinal tree: {tree}\n"));

    // Step 9. Evaluation.
    heading(&mut out, "Step 9. Evaluation");
    let final_tree = Tree::parse(&tree).expect("the worksheet tree parses");
    let side = |s: &Vec<String>| {
        if labels.single {
            s.concat()
        } else {
            s.join(",")
        }
    };
    let all: Vec<String> = {
        let mut v = labels.names.clone();
        v.sort();
        v
    };
    let split_text = |sp: &Vec<String>| {
        let rest: Vec<String> = all.iter().filter(|x| !sp.contains(x)).cloned().collect();
        format!("{} | {}", side(&rest), side(sp))
    };
    let splits = final_tree.splits();
    out.push_str(&format!(
        "Splits of the final tree: {} (n - 3 = {} splits).\n",
        splits
            .iter()
            .map(split_text)
            .collect::<Vec<_>>()
            .join(" and "),
        splits.len()
    ));
    let mut nrf_value = None;
    if let Some(r) = reference {
        let (diff, v) = nrf(&final_tree, r);
        let denom = 2 * (m - 3);
        out.push_str(&format!(
            "Reference tree {}\nnRF = {} / (2 x ({} - 3)) = {} / {} = {}.\n",
            r.to_newick(false),
            diff,
            m,
            diff,
            denom,
            num(v)
        ));
        nrf_value = Some(v);
        if let Some(c) = control {
            let (cd, cv) = nrf(c, r);
            out.push_str(&format!(
                "Control: tree {} against the same reference: unmatched splits = {}, nRF =\n{} / {} = {}.\n",
                c.to_newick(false),
                cd,
                cd,
                denom,
                num(cv)
            ));
        }
    } else {
        out.push_str("No reference tree was given.\n");
    }

    if model_example {
        heading(&mut out, "Model example (two-state symmetric model)");
        out.push_str(
            "Every branch changes state with probability 0.1. The root state has probability\n0.5. Sum over the two internal states u (joining the first pair) and v.\n",
        );
        let p = 0.1f64;
        let prob = |same: bool| if same { 1.0 - p } else { p };
        // Topology ab|cd: a, b hang from u; c, d from v. Pattern 1100.
        let term_ab =
            |u: u8, v: u8| prob(u == 1) * prob(u == 1) * prob(u == v) * prob(v == 0) * prob(v == 0);
        // Topology ac|bd: a, c hang from u; b, d from v. Pattern 1100.
        let term_ac =
            |u: u8, v: u8| prob(u == 1) * prob(u == 0) * prob(u == v) * prob(v == 1) * prob(v == 0);
        for (label, f) in [
            ("P(1100 | ab|cd)", &term_ab as &dyn Fn(u8, u8) -> f64),
            ("P(1100 | ac|bd)", &term_ac),
        ] {
            let terms: Vec<f64> = [(0, 0), (0, 1), (1, 0), (1, 1)]
                .iter()
                .map(|&(u, v)| f(u, v))
                .collect();
            let total = 0.5 * terms.iter().sum::<f64>();
            out.push_str(&format!(
                "{label} = 0.5 x ({}) = {}\n",
                terms
                    .iter()
                    .map(|&t| num(t))
                    .collect::<Vec<_>>()
                    .join(" + "),
                num(total)
            ));
        }
    }

    Ok(Worksheet {
        text: out,
        words,
        matrix,
        quartets,
        patterns,
        tree,
        nrf: nrf_value,
    })
}

/// The worksheet of the built-in example.
pub fn example() -> Worksheet {
    let taxa: Vec<(String, String)> = EXAMPLE
        .iter()
        .map(|(n, s)| (n.to_string(), s.to_string()))
        .collect();
    let reference = Tree::parse(EXAMPLE_REFERENCE).expect("example reference parses");
    let control = Tree::parse(EXAMPLE_CONTROL).expect("example control parses");
    worksheet(&taxa, Some(&reference), Some(&control), true).expect("the example is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_formatted_without_trailing_zeros() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(0.875), "0.875");
        assert_eq!(num(2.75), "2.75");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(0.5 * (0.06561 + 0.00729 + 0.00729 + 0.00001)), "0.0401");
    }

    #[test]
    fn example_reproduces_appendix_a() {
        let w = example();
        // A.4: 13 columns, words in order, and every row.
        assert_eq!(
            w.words,
            ["AA", "AC", "AG", "AT", "CA", "CG", "GC", "GG", "GT", "TA", "TC", "TG", "TT"]
        );
        let a4 = [
            "01111", "00011", "10111", "11100", "11100", "00011", "11100", "11110", "00011",
            "00111", "11011", "11100", "11001",
        ];
        for (row, want) in w.matrix.iter().zip(a4) {
            let got: String = row.iter().map(|v| char::from(b'0' + v)).collect();
            assert_eq!(got, want);
        }
        // A.5: five quartets.
        assert_eq!(w.quartets.len(), 5);
        // A.6: the pattern table.
        let a6 = [
            ["0111", "0111", "0111", "0111", "1111"],
            ["0001", "0001", "0011", "0011", "0011"],
            ["1011", "1011", "1011", "1111", "0111"],
            ["1110", "1110", "1100", "1100", "1100"],
            ["1110", "1110", "1100", "1100", "1100"],
            ["0001", "0001", "0011", "0011", "0011"],
            ["1110", "1110", "1100", "1100", "1100"],
            ["1111", "1110", "1110", "1110", "1110"],
            ["0001", "0001", "0011", "0011", "0011"],
            ["0011", "0011", "0011", "0111", "0111"],
            ["1101", "1101", "1111", "1011", "1011"],
            ["1110", "1110", "1100", "1100", "1100"],
            ["1100", "1101", "1101", "1001", "1001"],
        ];
        for (row, want) in w.patterns.iter().zip(a6) {
            assert_eq!(row, &want);
        }
        let t = &w.text;
        // A.3.
        assert!(t.contains("MAW of every sequence (constant all-1 columns, removed): CC, CT, GA."));
        assert!(t.contains("MAW of no sequence (constant all-0 columns, removed): none."));
        // A.5.
        assert!(t.contains("Q = 5 x 4 x 3 x 2 / 24 = 120 / 24 = 5: KLMN, KLMP, KLNP, KMNP, LMNP"));
        // A.7.
        assert!(t.contains("KL|MN (2: TT, TA)"));
        assert!(t.contains("KL|MP (1: TA)"));
        assert!(t.contains("KL|NP (8: AT, CA, GC, TG, AC, CG, GT, TA)"));
        assert!(t.contains("KM|NP (7: AT, CA, GC, TG, AC, CG, GT)"));
        assert!(t.contains("KP|MN (1: TT)"));
        assert!(t.contains("LP|MN (1: TT)"));
        assert!(t.contains("7 / 8 = 0.875"));
        assert!(t.contains(
            "Votes used: 2 + 1 + 8 + 8 + 8 = 27 of 65 pattern cells; 38 cells cast no vote"
        ));
        // A.8.
        assert!(t.contains("Number of pairs = 5 x 4 / 2 = 10."));
        assert!(
            t.contains("Sum              3    0.875  0    0    0.875  0    0    1    1    2.75")
        );
        assert!(t.contains("Round 1: highest = 3 (K-L) -> merge {K,L}."));
        assert!(t.contains("{K,L}-M = (0.875 + 0.875) / 2 = 0.875"));
        assert!(t.contains("Round 2: highest = 2.75 (N-P) -> merge {N,P}."));
        assert!(
            t.contains("Three groups remain ({K,L}, M, {N,P}); stop. Merges performed: n - 3 = 2.")
        );
        assert_eq!(w.tree, "((K,L),M,(N,P));");
        // A.9.
        assert!(t.contains("Splits of the final tree: KL | MNP and KLM | NP (n - 3 = 2 splits)."));
        assert!(t.contains("nRF = 0 / (2 x (5 - 3)) = 0 / 4 = 0."));
        assert!(t.contains("unmatched splits = 2, nRF =\n2 / 4 = 0.5."));
        assert!(
            t.contains("P(1100 | ab|cd) = 0.5 x (0.00729 + 0.00001 + 0.06561 + 0.00729) = 0.0401")
        );
        assert!(
            t.contains("P(1100 | ac|bd) = 0.5 x (0.00729 + 0.00081 + 0.00081 + 0.00729) = 0.0081")
        );
        assert_eq!(w.nrf, Some(0.0));
    }

    #[test]
    fn g1_output_matches_the_golden_file() {
        let golden = include_str!("../../../tests/golden/worksheet_example.txt");
        assert_eq!(example().text, golden.replace("\r\n", "\n"));
    }

    #[test]
    fn g2_production_pipeline_gives_the_worksheet_matrix_pattern_counts_and_tree() {
        use crate::matrix::{build_full, ml_columns, MAX_ML_COLUMNS};
        use crate::maw::extract;
        use crate::quartet::{pattern_counts, quartet_count, rank, CoCounts, PopcountPath};

        let w = example();
        // Production MAW extraction with teaching settings: length 2, no
        // strand filter, no length selection.
        let sets: Vec<_> = EXAMPLE
            .iter()
            .map(|(_, s)| extract(s.as_bytes(), 2, 2).unwrap())
            .collect();
        let full = build_full(&sets, &[2]);
        let keep = ml_columns(&full, MAX_ML_COLUMNS);
        let ml = full.select(&keep);
        let words: Vec<String> = (0..ml.columns_len()).map(|j| ml.word(j)).collect();
        assert_eq!(words, w.words, "same columns");
        for (j, row) in w.matrix.iter().enumerate() {
            for (t, &v) in row.iter().enumerate() {
                assert_eq!(ml.get(t, j), v == 1, "word {} taxon {t}", words[j]);
            }
        }
        // Pattern counts of the production kernel equal the worksheet's
        // pattern table, quartet by quartet.
        let cc = CoCounts::new(&ml.rows, ml.columns_len() as u64);
        let path = PopcountPath::detect();
        assert_eq!(quartet_count(5), w.quartets.len() as u64);
        for (qi, q) in w.quartets.iter().enumerate() {
            let n4 = path.and4(
                &ml.rows[q[0]],
                &ml.rows[q[1]],
                &ml.rows[q[2]],
                &ml.rows[q[3]],
            );
            let counts = pattern_counts(&cc, *q, n4);
            let mut from_sheet = [0u64; 16];
            for row in &w.patterns {
                from_sheet[usize::from_str_radix(&row[qi], 2).unwrap()] += 1;
            }
            assert_eq!(counts, from_sheet, "quartet {q:?} (rank {})", rank(*q));
        }
        // The rest of G2: production W1 weights and wQFM-rs give the
        // worksheet's final tree ((K,L),M,(N,P)); with nRF 0.
        let names: Vec<String> = EXAMPLE.iter().map(|(n, _)| n.to_string()).collect();
        let mut weighted = Vec::new();
        for q in &w.quartets {
            let n4 = path.and4(
                &ml.rows[q[0]],
                &ml.rows[q[1]],
                &ml.rows[q[2]],
                &ml.rows[q[3]],
            );
            let counts = pattern_counts(&cc, *q, n4);
            if let Some(w1) = crate::weight::w1(&counts) {
                weighted.extend(crate::amalgamate::quartets_of(*q, w1));
            }
        }
        let result = crate::amalgamate::wqfm(&names, &weighted, &Default::default());
        let got = crate::newick::Tree::parse(&result.newick).unwrap();
        let reference = crate::newick::Tree::parse("((K,L),M,(N,P));").unwrap();
        assert_eq!(
            crate::newick::nrf(&got, &reference).1,
            0.0,
            "tree {}",
            result.newick
        );
    }

    #[test]
    fn limits_are_enforced() {
        let t = |n: usize, len: usize| -> Vec<(String, String)> {
            (0..n)
                .map(|i| {
                    (
                        format!("T{i}"),
                        "ACGT".repeat(len / 4 + 1)[..len].to_string(),
                    )
                })
                .collect()
        };
        assert!(check_input(&t(4, 10)).is_ok());
        assert!(check_input(&t(3, 10)).is_err());
        assert!(check_input(&t(9, 10)).is_err());
        assert!(check_input(&t(5, 201)).is_err());
        assert!(check_input(&t(5, 200)).is_ok());
    }
}
