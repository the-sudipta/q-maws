//! Quartet amalgamation: `wQFM-rs`, a Rust implementation of wQFM (Mahbub,
//! Wahab, Reaz, Rahman and Bayzid, Bioinformatics 37:3734–3743, 2021;
//! official code github.com/Mahim1997/wQFM-2020, v1.4, Apache License 2.0),
//! with the exhaustive tree oracle and the weighted quartet consistency score.
//!
//! The algorithm (paper sections 2.5 and 2.6; details as in the v1.4 code,
//! see `docs/DESIGN.md`):
//!
//! 1. A level has a taxon set and weighted quartets ab|cd. With at most three
//!    taxa, or no quartets, it returns a star.
//! 2. Initial bipartition: quartets in descending weight (input order on
//!    ties) assign their unassigned taxa greedily, keeping sisters together
//!    and the two sister pairs apart; taxa left over are spread to balance
//!    the sides.
//! 3. Partition score: weight of satisfied quartets (the bipartition
//!    separates the two sister pairs) minus β times the weight of violated
//!    quartets (two taxa on each side, sisters split); β = 1 by default.
//!    Deferred (three on one side) and blank (all four on one side) quartets
//!    do not count.
//! 4. FM refinement: in each pass every free taxon is moved hypothetically
//!    (not if a side would keep fewer than two taxa); the one with the
//!    highest gain moves and is locked (ties: most satisfied quartets after
//!    the move, then the last such taxon). After all taxa are locked, the
//!    prefix of moves with the highest cumulative gain is kept. Iterations
//!    repeat while that gain is positive and the bipartition changes.
//! 5. Division: each side gets a new dummy taxon for the other side. Blank
//!    quartets go to their side; deferred quartets go to the side of their
//!    three taxa with the fourth replaced by the dummy, and equal new
//!    quartets are merged with the mean weight. Satisfied and violated
//!    quartets are dropped.
//! 6. The two subtrees are joined by removing both dummies and connecting
//!    their neighbours.

use std::collections::HashMap;
use std::fmt::Write as _;

/// A weighted quartet ab|cd on taxon ids, normalised so that `l[0] < l[1]`,
/// `r[0] < r[1]` and `l[0] < r[0]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quartet {
    pub l: [u32; 2],
    pub r: [u32; 2],
    pub weight: f64,
}

impl Quartet {
    /// The quartet a,b | c,d with the given weight.
    pub fn new(a: u32, b: u32, c: u32, d: u32, weight: f64) -> Self {
        let mut l = [a.min(b), a.max(b)];
        let mut r = [c.min(d), c.max(d)];
        if l[0] > r[0] {
            std::mem::swap(&mut l, &mut r);
        }
        Self { l, r, weight }
    }

    fn key(&self) -> [u32; 4] {
        [self.l[0], self.l[1], self.r[0], self.r[1]]
    }
}

/// Settings of `wQFM-rs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Weight of violated quartets in the partition score (wQFM default 1).
    pub beta: f64,
    /// Limit of FM iterations per level (wQFM: 1,000,000).
    pub max_iterations: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            beta: 1.0,
            max_iterations: 1_000_000,
        }
    }
}

const LEFT: i8 = -1;
const RIGHT: i8 = 1;
const UNASSIGNED: i8 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Satisfied,
    Violated,
    Deferred,
    Blank,
}

fn status(s: [i8; 4]) -> Status {
    let sum: i8 = s.iter().sum();
    if sum.abs() == 4 {
        Status::Blank
    } else if sum.abs() == 2 {
        Status::Deferred
    } else if s[0] == s[1] && s[2] == s[3] && s[0] != s[2] {
        Status::Satisfied
    } else {
        Status::Violated
    }
}

/// Satisfied and violated weight and the number of satisfied quartets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tally {
    n_satisfied: i64,
    w_satisfied: f64,
    w_violated: f64,
}

impl Tally {
    fn add(&mut self, w: f64, s: Status) {
        match s {
            Status::Satisfied => {
                self.n_satisfied += 1;
                self.w_satisfied += w;
            }
            Status::Violated => self.w_violated += w,
            _ => {}
        }
    }

    fn score(&self, beta: f64) -> f64 {
        self.w_satisfied - beta * self.w_violated
    }
}

/// A rooted tree built during the recursion.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Leaf(u32),
    Inner(Vec<Node>),
}

struct Level {
    taxa: Vec<u32>,
    quartets: Vec<Quartet>,
}

/// The tree found by `wQFM-rs` as Newick (unrooted, with a basal
/// multifurcation), and its weighted quartet consistency score.
#[derive(Clone, Debug, PartialEq)]
pub struct Amalgamation {
    pub newick: String,
    pub score: f64,
    pub total_weight: f64,
}

/// Runs `wQFM-rs` on weighted quartets over taxa `0..names.len()`.
pub fn wqfm(names: &[String], quartets: &[Quartet], settings: &Settings) -> Amalgamation {
    let n = names.len() as u32;
    let mut used = vec![false; names.len()];
    for q in quartets {
        for t in q.key() {
            used[t as usize] = true;
        }
    }
    // Taxa that appear in no quartet are attached at the root (wQFM only
    // knows the taxa of its input).
    let mut taxa: Vec<u32> = (0..n).filter(|&t| used[t as usize]).collect();
    let missing: Vec<u32> = (0..n).filter(|&t| !used[t as usize]).collect();
    let top = Level {
        taxa: std::mem::take(&mut taxa),
        quartets: quartets
            .iter()
            .filter(|q| q.weight.is_finite())
            .copied()
            .collect(),
    };
    let mut next_dummy = n;
    let mut tree = solve(top, settings, &mut next_dummy);
    if !missing.is_empty() {
        let mut children = match tree {
            Node::Inner(c) => c,
            leaf => vec![leaf],
        };
        children.extend(missing.into_iter().map(Node::Leaf));
        tree = Node::Inner(children);
    }
    let newick = to_newick(&unroot(tree), names);
    let parsed = Topology::from_newick(&newick, names).expect("own Newick parses");
    let (score, total_weight) = parsed.score(quartets);
    Amalgamation {
        newick,
        score,
        total_weight,
    }
}

fn solve(level: Level, settings: &Settings, next_dummy: &mut u32) -> Node {
    if level.taxa.len() <= 3 || level.quartets.is_empty() {
        return Node::Inner(level.taxa.iter().map(|&t| Node::Leaf(t)).collect());
    }
    let max_id = *level.taxa.iter().max().expect("taxa") as usize;
    let mut side = initial_bipartition(&level, max_id);
    fm(&level, &mut side, settings);
    let dummy = *next_dummy;
    *next_dummy += 1;
    let (left, right) = divide(level, &side, dummy);
    let left_tree = solve(left, settings, next_dummy);
    let right_tree = solve(right, settings, next_dummy);
    merge(left_tree, right_tree, dummy)
}

fn initial_bipartition(level: &Level, max_id: usize) -> Vec<i8> {
    let mut side = vec![UNASSIGNED; max_id + 1];
    let mut order: Vec<usize> = (0..level.quartets.len()).collect();
    // Descending weight; equal weights keep their order (stable sort).
    order.sort_by(|&a, &b| {
        level.quartets[b]
            .weight
            .total_cmp(&level.quartets[a].weight)
    });
    let (mut n_left, mut n_right) = (0i64, 0i64);
    let put = |side: &mut Vec<i8>, t: u32, v: i8, n_left: &mut i64, n_right: &mut i64| {
        side[t as usize] = v;
        if v == LEFT {
            *n_left += 1;
        } else {
            *n_right += 1;
        }
    };
    for i in order {
        let q = &level.quartets[i];
        let [q1, q2, q3, q4] = q.key();
        let mut s = [q1, q2, q3, q4].map(|t| side[t as usize]);
        if s.iter().all(|&v| v == UNASSIGNED) {
            put(&mut side, q1, LEFT, &mut n_left, &mut n_right);
            put(&mut side, q2, LEFT, &mut n_left, &mut n_right);
            put(&mut side, q3, RIGHT, &mut n_left, &mut n_right);
            put(&mut side, q4, RIGHT, &mut n_left, &mut n_right);
            continue;
        }
        if s[0] == UNASSIGNED {
            let v = if s[1] != UNASSIGNED {
                s[1]
            } else if s[2] != UNASSIGNED {
                -s[2]
            } else if s[3] != UNASSIGNED {
                -s[3]
            } else {
                UNASSIGNED
            };
            if v != UNASSIGNED {
                put(&mut side, q1, v, &mut n_left, &mut n_right);
                s[0] = v;
            }
        }
        if s[1] == UNASSIGNED {
            let v = if s[0] == LEFT { LEFT } else { RIGHT };
            put(&mut side, q2, v, &mut n_left, &mut n_right);
            s[1] = v;
        }
        if s[2] == UNASSIGNED {
            let v = if s[3] != UNASSIGNED {
                s[3]
            } else if s[0] == RIGHT {
                LEFT
            } else {
                RIGHT
            };
            put(&mut side, q3, v, &mut n_left, &mut n_right);
            s[2] = v;
        }
        if s[3] == UNASSIGNED {
            let v = if s[2] == LEFT { LEFT } else { RIGHT };
            put(&mut side, q4, v, &mut n_left, &mut n_right);
        }
    }
    // Leftover taxa, balancing the sides.
    let mut flag = 0i64;
    for &t in &level.taxa {
        if side[t as usize] != UNASSIGNED {
            continue;
        }
        if n_left < n_right {
            flag = 2;
        } else if n_left > n_right {
            flag = 1;
        } else {
            flag += 1;
        }
        if flag % 2 == 0 {
            put(&mut side, t, LEFT, &mut n_left, &mut n_right);
        } else {
            put(&mut side, t, RIGHT, &mut n_left, &mut n_right);
        }
    }
    side
}

fn sides_of(side: &[i8], q: &Quartet) -> [i8; 4] {
    q.key().map(|t| side[t as usize])
}

fn whole_tally(level: &Level, side: &[i8]) -> Tally {
    let mut t = Tally::default();
    for q in &level.quartets {
        t.add(q.weight, status(sides_of(side, q)));
    }
    t
}

/// True if a side has fewer than two taxa.
fn is_trivial(level: &Level, side: &[i8]) -> bool {
    let left = level
        .taxa
        .iter()
        .filter(|&&t| side[t as usize] == LEFT)
        .count();
    let right = level.taxa.len() - left;
    left < 2 || right < 2
}

fn fm(level: &Level, side: &mut Vec<i8>, settings: &Settings) {
    let beta = settings.beta;
    let max_id = side.len() - 1;
    // Relevant quartets per taxon, in quartet order.
    let mut relevant: Vec<Vec<u32>> = vec![Vec::new(); max_id + 1];
    for (i, q) in level.quartets.iter().enumerate() {
        for t in q.key() {
            relevant[t as usize].push(i as u32);
        }
    }
    let mut tally = whole_tally(level, side);
    let mut previous_best_gain = 0.0;
    let mut previous_side: Option<Vec<i8>> = None;
    let mut iterations = 0u32;
    loop {
        if iterations > settings.max_iterations {
            break;
        }
        iterations += 1;
        let start_side = side.clone();
        let start_tally = tally;
        // One iteration: passes until every taxon is locked.
        let mut locked = vec![false; max_id + 1];
        let mut current = side.clone();
        let mut current_tally = tally;
        // (gain, side after the move, tally after the move)
        let mut passes: Vec<(f64, Vec<i8>, Tally)> = Vec::new();
        loop {
            let mut best: Option<(f64, i64, u32, Tally)> = None;
            let mut any_candidate = false;
            for &t in &level.taxa {
                if locked[t as usize] {
                    continue;
                }
                let before_side = current[t as usize];
                current[t as usize] = -before_side;
                let trivial = is_trivial(level, &current);
                current[t as usize] = before_side;
                if trivial {
                    continue;
                }
                if relevant[t as usize].is_empty() {
                    locked[t as usize] = true;
                    continue;
                }
                let mut before = Tally::default();
                let mut after = Tally::default();
                for &qi in &relevant[t as usize] {
                    let q = &level.quartets[qi as usize];
                    let s0 = sides_of(&current, q);
                    let st0 = status(s0);
                    before.add(q.weight, st0);
                    if st0 == Status::Deferred {
                        // Moving one taxon of a deferred quartet can give
                        // any status; others become deferred.
                        let s1 = q.key().map(|x| {
                            if x == t {
                                -current[x as usize]
                            } else {
                                current[x as usize]
                            }
                        });
                        after.add(q.weight, status(s1));
                    }
                }
                let gain = after.score(beta) - before.score(beta);
                let whole = Tally {
                    n_satisfied: current_tally.n_satisfied + after.n_satisfied - before.n_satisfied,
                    w_satisfied: current_tally.w_satisfied + after.w_satisfied - before.w_satisfied,
                    w_violated: current_tally.w_violated + after.w_violated - before.w_violated,
                };
                any_candidate = true;
                // Highest gain (Double order of Java's TreeMap, so −0 < +0);
                // ties: most satisfied quartets; then the last taxon.
                let better = match &best {
                    None => true,
                    Some((g, ns, _, _)) => match gain.total_cmp(g) {
                        std::cmp::Ordering::Greater => true,
                        std::cmp::Ordering::Less => false,
                        std::cmp::Ordering::Equal => whole.n_satisfied >= *ns,
                    },
                };
                if better {
                    best = Some((gain, whole.n_satisfied, t, whole));
                }
            }
            if !any_candidate || best.is_none() {
                // Every remaining move would make a side trivial: lock all.
                break;
            }
            let (gain, _, t, whole) = best.expect("checked");
            locked[t as usize] = true;
            current[t as usize] = -current[t as usize];
            current_tally = whole;
            passes.push((gain, current.clone(), whole));
            if level.taxa.iter().all(|&x| locked[x as usize]) {
                break;
            }
        }
        if passes.is_empty() {
            *side = start_side;
            tally = start_tally;
            break;
        }
        let mut best_cumulative = f64::from(i32::MIN);
        let mut cumulative = 0.0;
        let mut best_index = 0;
        for (i, p) in passes.iter().enumerate() {
            cumulative += p.0;
            if cumulative > best_cumulative {
                best_cumulative = cumulative;
                best_index = i;
            }
        }
        let chosen = &passes[best_index];
        if best_cumulative > 0.0 {
            if best_cumulative == previous_best_gain {
                if let Some(prev) = &previous_side {
                    if same_bipartition(level, &chosen.1, prev) {
                        *side = start_side;
                        tally = start_tally;
                        break;
                    }
                }
            }
            *side = chosen.1.clone();
            tally = chosen.2;
            previous_best_gain = best_cumulative;
            previous_side = Some(side.clone());
        } else {
            *side = start_side;
            tally = start_tally;
            break;
        }
    }
    let _ = tally;
}

fn same_bipartition(level: &Level, a: &[i8], b: &[i8]) -> bool {
    let same = level.taxa.iter().all(|&t| a[t as usize] == b[t as usize]);
    let flipped = level.taxa.iter().all(|&t| a[t as usize] == -b[t as usize]);
    same || flipped
}

fn divide(level: Level, side: &[i8], dummy: u32) -> (Level, Level) {
    let mut left = Level {
        taxa: Vec::new(),
        quartets: Vec::new(),
    };
    let mut right = Level {
        taxa: Vec::new(),
        quartets: Vec::new(),
    };
    for &t in &level.taxa {
        if side[t as usize] == LEFT {
            left.taxa.push(t);
        } else {
            right.taxa.push(t);
        }
    }
    left.taxa.push(dummy);
    right.taxa.push(dummy);
    // New dummy quartets in order of first appearance: key -> (index, count).
    let mut seen: HashMap<[u32; 4], (bool, usize, u32)> = HashMap::new();
    let mut dummy_quartets: Vec<(bool, Quartet)> = Vec::new();
    for q in &level.quartets {
        let s = sides_of(side, q);
        match status(s) {
            Status::Blank => {
                if s[0] == LEFT {
                    left.quartets.push(*q);
                } else {
                    right.quartets.push(*q);
                }
            }
            Status::Deferred => {
                let sum: i8 = s.iter().sum();
                let common = if sum < 0 { LEFT } else { RIGHT };
                let mut k = q.key();
                // The (last) taxon on the other side becomes the dummy.
                let odd = (0..4).rev().find(|&i| s[i] != common).expect("deferred");
                k[odd] = dummy;
                let nq = Quartet::new(k[0], k[1], k[2], k[3], q.weight);
                let to_left = common == LEFT;
                match seen.get_mut(&nq.key()) {
                    None => {
                        seen.insert(nq.key(), (to_left, dummy_quartets.len(), 1));
                        dummy_quartets.push((to_left, nq));
                    }
                    Some((_, idx, count)) => {
                        let old = dummy_quartets[*idx].1.weight;
                        let c = *count as f64;
                        dummy_quartets[*idx].1.weight = (c * old + q.weight) / (c + 1.0);
                        *count += 1;
                    }
                }
            }
            _ => {}
        }
    }
    for (to_left, q) in dummy_quartets {
        if to_left {
            left.quartets.push(q);
        } else {
            right.quartets.push(q);
        }
    }
    (left, right)
}

/// Undirected tree as adjacency lists; leaves carry taxon ids.
struct Graph {
    adj: Vec<Vec<usize>>,
    leaf: Vec<Option<u32>>,
}

impl Graph {
    fn from_rooted(root: &Node) -> Self {
        let mut g = Graph {
            adj: Vec::new(),
            leaf: Vec::new(),
        };
        g.add(root, None);
        g.suppress_degree_two();
        g
    }

    fn add(&mut self, n: &Node, parent: Option<usize>) {
        let id = self.adj.len();
        self.adj.push(Vec::new());
        self.leaf.push(match n {
            Node::Leaf(t) => Some(*t),
            Node::Inner(_) => None,
        });
        if let Some(p) = parent {
            self.adj[p].push(id);
            self.adj[id].push(p);
        }
        if let Node::Inner(children) = n {
            for c in children {
                self.add(c, Some(id));
            }
        }
    }

    /// Removes inner nodes with exactly two neighbours by joining them.
    fn suppress_degree_two(&mut self) {
        while let Some(v) =
            (0..self.adj.len()).find(|&v| self.leaf[v].is_none() && self.adj[v].len() == 2)
        {
            let (a, b) = (self.adj[v][0], self.adj[v][1]);
            for (x, y) in [(a, b), (b, a)] {
                let pos = self.adj[x].iter().position(|&z| z == v).expect("edge");
                self.adj[x][pos] = y;
            }
            self.adj[v].clear();
        }
    }

    /// The tree rooted at `root`, not going towards `from`.
    fn rooted(&self, root: usize, from: Option<usize>) -> Node {
        if let Some(t) = self.leaf[root] {
            return Node::Leaf(t);
        }
        Node::Inner(
            self.adj[root]
                .iter()
                .filter(|&&c| Some(c) != from)
                .map(|&c| self.rooted(c, Some(root)))
                .collect(),
        )
    }

    fn leaf_node(&self, t: u32) -> Option<usize> {
        (0..self.leaf.len()).find(|&v| self.leaf[v] == Some(t) && !self.adj[v].is_empty())
    }
}

/// The part of `tree` seen from the neighbour of leaf `dummy`.
fn side_without(tree: Node, dummy: u32) -> Node {
    let g = Graph::from_rooted(&tree);
    let d = g.leaf_node(dummy).expect("dummy leaf in subtree");
    let p = g.adj[d][0];
    g.rooted(p, Some(d))
}

fn merge(left: Node, right: Node, dummy: u32) -> Node {
    Node::Inner(vec![side_without(left, dummy), side_without(right, dummy)])
}

/// Unrooted form: a root with two children is replaced by the multifurcation
/// of one inner child with the other.
fn unroot(tree: Node) -> Node {
    let g = Graph::from_rooted(&tree);
    // Root at the first inner node with at least three neighbours.
    match (0..g.adj.len()).find(|&v| g.leaf[v].is_none() && g.adj[v].len() >= 3) {
        Some(v) => g.rooted(v, None),
        None => tree,
    }
}

fn to_newick(tree: &Node, names: &[String]) -> String {
    fn write(n: &Node, names: &[String], out: &mut String) {
        match n {
            Node::Leaf(t) => out.push_str(&names[*t as usize]),
            Node::Inner(children) => {
                out.push('(');
                for (i, c) in children.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(c, names, out);
                }
                out.push(')');
            }
        }
    }
    let mut out = String::new();
    write(tree, names, &mut out);
    out.push(';');
    out
}

/// A tree on taxa `0..m` with leaf-to-leaf path lengths, for quartet tests.
pub struct Topology {
    m: usize,
    dist: Vec<u32>,
}

impl Topology {
    /// Reads a Newick tree whose leaves are exactly `names`.
    pub fn from_newick(newick: &str, names: &[String]) -> Result<Self, String> {
        let tree = crate::newick::Tree::parse(newick).map_err(|e| e.to_string())?;
        let index: HashMap<&str, u32> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i as u32))
            .collect();
        fn convert(
            tree: &crate::newick::Tree,
            n: usize,
            index: &HashMap<&str, u32>,
        ) -> Result<Node, String> {
            if tree.is_leaf(n) {
                let name = tree.nodes[n].label.as_deref().unwrap_or("");
                return index
                    .get(name)
                    .map(|&t| Node::Leaf(t))
                    .ok_or_else(|| format!("unknown leaf {name}"));
            }
            tree.nodes[n]
                .children
                .iter()
                .map(|&c| convert(tree, c, index))
                .collect::<Result<Vec<_>, _>>()
                .map(Node::Inner)
        }
        let root = convert(&tree, tree.root, &index)?;
        Ok(Self::from_node(&root, names.len()))
    }

    fn from_node(root: &Node, m: usize) -> Self {
        let g = Graph::from_rooted(root);
        let mut dist = vec![u32::MAX; m * m];
        for t in 0..m as u32 {
            let Some(start) = g.leaf_node(t) else {
                continue;
            };
            // Breadth-first search over the tree.
            let mut d = vec![u32::MAX; g.adj.len()];
            d[start] = 0;
            let mut queue = std::collections::VecDeque::from([start]);
            while let Some(v) = queue.pop_front() {
                for &w in &g.adj[v] {
                    if d[w] == u32::MAX {
                        d[w] = d[v] + 1;
                        queue.push_back(w);
                    }
                }
            }
            for (v, &dv) in d.iter().enumerate() {
                if let Some(u) = g.leaf[v] {
                    if !g.adj[v].is_empty() || g.adj.len() == 1 {
                        dist[t as usize * m + u as usize] = dv;
                    }
                }
            }
        }
        Self { m, dist }
    }

    fn d(&self, a: u32, b: u32) -> u32 {
        self.dist[a as usize * self.m + b as usize]
    }

    /// True if the tree induces ab|cd (four-point condition on path lengths;
    /// false for a star on the four taxa).
    pub fn displays(&self, q: &Quartet) -> bool {
        let [a, b] = q.l;
        let [c, d] = q.r;
        let s1 = self.d(a, b) as u64 + self.d(c, d) as u64;
        let s2 = self.d(a, c) as u64 + self.d(b, d) as u64;
        let s3 = self.d(a, d) as u64 + self.d(b, c) as u64;
        s1 < s2 && s1 < s3
    }

    /// Weighted quartet consistency score and total weight.
    pub fn score(&self, quartets: &[Quartet]) -> (f64, f64) {
        let mut score = 0.0;
        let mut total = 0.0;
        for q in quartets {
            total += q.weight;
            if self.displays(q) {
                score += q.weight;
            }
        }
        (score, total)
    }
}

/// Every unrooted binary tree on `m` taxa (3 ≤ m ≤ 10), by stepwise
/// addition of taxa to every edge, as Newick strings with `names`.
pub fn all_binary_trees(names: &[String]) -> Vec<String> {
    let m = names.len();
    assert!(
        (3..=10).contains(&m),
        "exhaustive enumeration needs 3 to 10 taxa"
    );
    // Trees as edge lists over nodes; leaves are 0..m, inner nodes m...
    let mut trees: Vec<Vec<(usize, usize)>> = vec![vec![(0, m), (1, m), (2, m)]];
    for t in 3..m {
        let mut next = Vec::with_capacity(trees.len() * (2 * t - 3));
        for edges in &trees {
            let inner = m + (t - 2);
            for (i, &(u, v)) in edges.iter().enumerate() {
                let mut e = edges.clone();
                e[i] = (u, inner);
                e.push((inner, v));
                e.push((t, inner));
                next.push(e);
            }
        }
        trees = next;
    }
    trees
        .iter()
        .map(|edges| {
            let n_nodes = m + (m - 2);
            let mut adj = vec![Vec::new(); n_nodes];
            for &(u, v) in edges {
                adj[u].push(v);
                adj[v].push(u);
            }
            fn write(
                v: usize,
                from: usize,
                adj: &[Vec<usize>],
                m: usize,
                names: &[String],
                out: &mut String,
            ) {
                if v < m {
                    out.push_str(&names[v]);
                    return;
                }
                out.push('(');
                let mut first = true;
                for &w in &adj[v] {
                    if w == from {
                        continue;
                    }
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    write(w, v, adj, m, names, out);
                }
                out.push(')');
            }
            let mut out = String::new();
            write(m, usize::MAX, &adj, m, names, &mut out);
            out.push(';');
            out
        })
        .collect()
}

/// The best tree of the exhaustive search (the first on equal scores) and
/// its score.
pub fn exhaustive_best(names: &[String], quartets: &[Quartet]) -> (String, f64) {
    let mut best: Option<(String, f64)> = None;
    for nwk in all_binary_trees(names) {
        let t = Topology::from_newick(&nwk, names).expect("own Newick");
        let (s, _) = t.score(quartets);
        if best.as_ref().is_none_or(|b| s > b.1) {
            best = Some((nwk, s));
        }
    }
    best.expect("at least one tree")
}

/// Weighted quartets in wQFM's input format: one line `((a,b),(c,d)); w`.
pub fn to_wqfm_input(names: &[String], quartets: &[Quartet]) -> String {
    let mut out = String::new();
    for q in quartets {
        let n = |t: u32| names[t as usize].as_str();
        let _ = writeln!(
            out,
            "(({},{}),({},{})); {}",
            n(q.l[0]),
            n(q.l[1]),
            n(q.r[0]),
            n(q.r[1]),
            q.weight
        );
    }
    out
}

/// The weighted quartets of a quartet a < b < c < d with weights for
/// ab|cd, ac|bd and ad|bc; weights that are not positive are left out.
pub fn quartets_of(q: [usize; 4], weights: [f64; 3]) -> impl Iterator<Item = Quartet> {
    let [a, b, c, d] = q.map(|x| x as u32);
    let tops = [(a, b, c, d), (a, c, b, d), (a, d, b, c)];
    tops.into_iter()
        .zip(weights)
        .filter(|(_, w)| *w > 0.0)
        .map(|((w1, x1, y1, z1), w)| Quartet::new(w1, x1, y1, z1, w))
}

/// A random unrooted binary tree on `names` by random stepwise addition
/// (each new taxon on a uniformly chosen edge), as Newick.
pub fn random_binary_tree(names: &[String], rng: &mut crate::weight::SplitMix64) -> String {
    let m = names.len();
    assert!(m >= 3, "a tree needs at least 3 taxa");
    let mut edges: Vec<(usize, usize)> = vec![(0, m), (1, m), (2, m)];
    for t in 3..m {
        let inner = m + (t - 2);
        let i = (rng.next_u64() % edges.len() as u64) as usize;
        let (u, v) = edges[i];
        edges[i] = (u, inner);
        edges.push((inner, v));
        edges.push((t, inner));
    }
    let mut adj = vec![Vec::new(); 2 * m - 2];
    for &(u, v) in &edges {
        adj[u].push(v);
        adj[v].push(u);
    }
    fn write(
        v: usize,
        from: usize,
        adj: &[Vec<usize>],
        m: usize,
        names: &[String],
        out: &mut String,
    ) {
        if v < m {
            out.push_str(&names[v]);
            return;
        }
        out.push('(');
        let mut first = true;
        for &w in &adj[v] {
            if w != from {
                if !first {
                    out.push(',');
                }
                first = false;
                write(w, v, adj, m, names, out);
            }
        }
        out.push(')');
    }
    let mut out = String::new();
    write(m, usize::MAX, &adj, m, names, &mut out);
    out.push(';');
    out
}

/// The topology that `tree` induces on every 4-set of taxa, each with
/// weight 1, in rank order.
pub fn induced_quartets(tree: &Topology) -> Vec<Quartet> {
    let m = tree.m as u32;
    let mut out = Vec::new();
    for d in 3..m {
        for c in 2..d {
            for b in 1..c {
                for a in 0..b {
                    for q in [
                        Quartet::new(a, b, c, d, 1.0),
                        Quartet::new(a, c, b, d, 1.0),
                        Quartet::new(a, d, b, c, 1.0),
                    ] {
                        if tree.displays(&q) {
                            out.push(q);
                        }
                    }
                }
            }
        }
    }
    out
}

/// Reads wQFM's input format, one `((a,b),(c,d)); w` per line, mapping
/// names to the indices of `names` (new names are appended).
pub fn parse_wqfm_input(text: &str, names: &mut Vec<String>) -> Result<Vec<Quartet>, String> {
    let mut index: HashMap<String, u32> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i as u32))
        .collect();
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cleaned: String = line
            .chars()
            .map(|c| {
                if c == '(' || c == ')' || c == ';' {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        let parts: Vec<&str> = cleaned
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() != 5 {
            return Err(format!("line {}: expected ((a,b),(c,d)); w", n + 1));
        }
        let mut id = |s: &str| -> u32 {
            if let Some(&i) = index.get(s) {
                return i;
            }
            let i = names.len() as u32;
            names.push(s.to_string());
            index.insert(s.to_string(), i);
            i
        };
        let t: Vec<u32> = parts[..4].iter().map(|s| id(s)).collect();
        let w: f64 = parts[4]
            .parse()
            .map_err(|_| format!("line {}: weight {} is not a number", n + 1, parts[4]))?;
        out.push(Quartet::new(t[0], t[1], t[2], t[3], w));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(m: usize) -> Vec<String> {
        (0..m).map(|i| format!("t{i}")).collect()
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
        fn unit(&mut self) -> f64 {
            self.next() as f64 / (1u64 << 31) as f64
        }
    }

    /// A random unrooted binary tree as Newick (random stepwise addition).
    fn random_tree(names: &[String], rng: &mut Lcg) -> String {
        all_binary_trees_small(names, rng)
    }

    fn all_binary_trees_small(names: &[String], rng: &mut Lcg) -> String {
        // Random stepwise addition without enumerating everything.
        let m = names.len();
        let mut edges: Vec<(usize, usize)> = vec![(0, m), (1, m), (2, m)];
        for t in 3..m {
            let inner = m + (t - 2);
            let i = (rng.next() as usize) % edges.len();
            let (u, v) = edges[i];
            edges[i] = (u, inner);
            edges.push((inner, v));
            edges.push((t, inner));
        }
        let mut adj = vec![Vec::new(); 2 * m - 2];
        for &(u, v) in &edges {
            adj[u].push(v);
            adj[v].push(u);
        }
        fn write(
            v: usize,
            from: usize,
            adj: &[Vec<usize>],
            m: usize,
            names: &[String],
            out: &mut String,
        ) {
            if v < m {
                out.push_str(&names[v]);
                return;
            }
            out.push('(');
            let mut first = true;
            for &w in &adj[v] {
                if w != from {
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    write(w, v, adj, m, names, out);
                }
            }
            out.push(')');
        }
        let mut out = String::new();
        write(m, usize::MAX, &adj, m, names, &mut out);
        out.push(';');
        out
    }

    /// Every quartet induced by a tree, with random positive weights.
    fn induced(tree: &str, names: &[String], rng: &mut Lcg) -> Vec<Quartet> {
        let topo = Topology::from_newick(tree, names).unwrap();
        let m = names.len() as u32;
        let mut out = Vec::new();
        for a in 0..m {
            for b in a + 1..m {
                for c in b + 1..m {
                    for d in c + 1..m {
                        for q in [
                            Quartet::new(a, b, c, d, 1.0),
                            Quartet::new(a, c, b, d, 1.0),
                            Quartet::new(a, d, b, c, 1.0),
                        ] {
                            if topo.displays(&q) {
                                out.push(Quartet {
                                    weight: 1.0 + 9.0 * rng.unit(),
                                    ..q
                                });
                            }
                        }
                    }
                }
            }
        }
        out
    }

    fn same_topology(a: &str, b: &str, names: &[String]) -> bool {
        let ta = crate::newick::Tree::parse(a).unwrap();
        let tb = crate::newick::Tree::parse(b).unwrap();
        crate::newick::nrf(&ta, &tb).0 == 0 && names.len() >= 3
    }

    #[test]
    fn quartet_normalisation_and_status() {
        let q = Quartet::new(5, 2, 1, 4, 3.0);
        assert_eq!((q.l, q.r), ([1, 4], [2, 5]));
        assert_eq!(status([LEFT, LEFT, RIGHT, RIGHT]), Status::Satisfied);
        assert_eq!(status([LEFT, RIGHT, LEFT, RIGHT]), Status::Violated);
        assert_eq!(status([LEFT, LEFT, LEFT, RIGHT]), Status::Deferred);
        assert_eq!(status([RIGHT; 4]), Status::Blank);
    }

    #[test]
    fn enumeration_counts_trees() {
        // (2m − 5)!! unrooted binary trees.
        for (m, count) in [(3, 1), (4, 3), (5, 15), (6, 105), (7, 945), (8, 10_395)] {
            let all = all_binary_trees(&names(m));
            assert_eq!(all.len(), count);
            let mut distinct = std::collections::BTreeSet::new();
            let nm = names(m);
            for t in &all {
                let tree = crate::newick::Tree::parse(t).unwrap();
                let mut splits = tree.splits();
                splits.sort();
                distinct.insert(format!("{splits:?}"));
                assert_eq!(Topology::from_newick(t, &nm).unwrap().m, m);
            }
            assert_eq!(distinct.len(), count, "trees for m = {m} are distinct");
        }
    }

    #[test]
    fn displays_follows_the_tree() {
        let nm = names(5);
        let t = Topology::from_newick("((t0,t1),t2,(t3,t4));", &nm).unwrap();
        assert!(t.displays(&Quartet::new(0, 1, 3, 4, 1.0)));
        assert!(t.displays(&Quartet::new(0, 1, 2, 3, 1.0)));
        assert!(!t.displays(&Quartet::new(0, 2, 1, 3, 1.0)));
        let star = Topology::from_newick("(t0,t1,t2,t3,t4);", &nm).unwrap();
        assert!(!star.displays(&Quartet::new(0, 1, 2, 3, 1.0)));
    }

    #[test]
    fn g8_noise_free_inputs_reach_the_optimum() {
        // Golden test G8: on quartets induced by a tree, wQFM-rs returns a
        // tree with the exhaustive optimum score (all the weight), i.e. the
        // tree itself.
        let mut rng = Lcg(8);
        for m in 4..=8 {
            for _ in 0..12 {
                let nm = names(m);
                let truth = random_tree(&nm, &mut rng);
                let qs = induced(&truth, &nm, &mut rng);
                let result = wqfm(&nm, &qs, &Settings::default());
                let (best, best_score) = exhaustive_best(&nm, &qs);
                assert!(
                    (result.score - best_score).abs() < 1e-9,
                    "m {m}: wQFM-rs {} ({}) vs optimum {best_score} ({best})",
                    result.score,
                    result.newick
                );
                assert!(
                    same_topology(&result.newick, &truth, &nm),
                    "{} vs {truth}",
                    result.newick
                );
            }
        }
    }

    #[test]
    fn noisy_inputs_stay_close_to_the_optimum() {
        let mut rng = Lcg(81);
        let mut worst: f64 = 1.0;
        for m in 5..=8 {
            for _ in 0..10 {
                let nm = names(m);
                let truth = random_tree(&nm, &mut rng);
                let mut qs = induced(&truth, &nm, &mut rng);
                // Noise: every 4-set also gets its two other topologies with
                // smaller random weights.
                let extra: Vec<Quartet> = qs
                    .iter()
                    .flat_map(|q| {
                        let [a, b] = q.l;
                        let [c, d] = q.r;
                        [Quartet::new(a, c, b, d, 0.0), Quartet::new(a, d, b, c, 0.0)]
                    })
                    .collect();
                for mut q in extra {
                    q.weight = 6.0 * rng.unit();
                    qs.push(q);
                }
                let result = wqfm(&nm, &qs, &Settings::default());
                let (_, best_score) = exhaustive_best(&nm, &qs);
                assert!(result.score <= best_score + 1e-9);
                worst = worst.min(result.score / best_score);
            }
        }
        // The gap is reported (cargo test -- --nocapture) for the M7 report.
        println!("G8 noisy inputs: worst wQFM-rs score / optimum = {worst:.6}");
        assert!(worst > 0.9, "worst ratio {worst}");
    }

    #[test]
    fn merging_joins_the_subtrees_at_the_dummy() {
        // Left (a, b, X), right (c, d, X) give ((a,b),(c,d)).
        let left = Node::Inner(vec![Node::Leaf(0), Node::Leaf(1), Node::Leaf(9)]);
        let right = Node::Inner(vec![Node::Leaf(2), Node::Leaf(3), Node::Leaf(9)]);
        let nm: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
        let merged = merge(left, right, 9);
        assert_eq!(to_newick(&unroot(merged), &nm), "((t2,t3),t0,t1);");
    }

    #[test]
    fn taxa_without_quartets_are_kept() {
        let nm = names(6);
        let qs = vec![Quartet::new(0, 1, 2, 3, 1.0), Quartet::new(0, 1, 2, 4, 1.0)];
        let r = wqfm(&nm, &qs, &Settings::default());
        let tree = crate::newick::Tree::parse(&r.newick).unwrap();
        assert_eq!(tree.leaf_names().len(), 6);
        assert_eq!(r.score, 2.0);
    }

    #[test]
    fn input_format_matches_wqfm() {
        let nm = names(4);
        let text = to_wqfm_input(&nm, &[Quartet::new(0, 1, 2, 3, 34.0)]);
        assert_eq!(text, "((t0,t1),(t2,t3)); 34\n");
        let qs: Vec<Quartet> = quartets_of([0, 1, 2, 3], [0.5, 0.0, 0.25]).collect();
        assert_eq!(qs.len(), 2);
        assert_eq!((qs[1].l, qs[1].r), ([0, 3], [1, 2]));
    }

    /// Run time on quartets induced from a random tree, all three
    /// topologies weighted (the true one highest), as from W2c (run with
    /// `--ignored --nocapture` in a release build).
    #[test]
    #[ignore]
    fn benchmark_wqfm() {
        let mut rng = Lcg(2026);
        let sizes: Vec<usize> = std::env::var("QMAWS_BENCH_TAXA")
            .map(|v| v.split(',').map(|x| x.parse().unwrap()).collect())
            .unwrap_or_else(|_| vec![25, 50, 80]);
        for m in sizes {
            let nm = names(m);
            let truth = random_tree(&nm, &mut rng);
            let topo = Topology::from_newick(&truth, &nm).unwrap();
            let mut qs = Vec::new();
            for a in 0..m as u32 {
                for b in a + 1..m as u32 {
                    for c in b + 1..m as u32 {
                        for d in c + 1..m as u32 {
                            for q in [
                                Quartet::new(a, b, c, d, 1.0),
                                Quartet::new(a, c, b, d, 1.0),
                                Quartet::new(a, d, b, c, 1.0),
                            ] {
                                let w = if topo.displays(&q) {
                                    0.5 + 0.5 * rng.unit()
                                } else {
                                    0.3 * rng.unit()
                                };
                                qs.push(Quartet { weight: w, ..q });
                            }
                        }
                    }
                }
            }
            let t0 = std::time::Instant::now();
            let r = wqfm(&nm, &qs, &Settings::default());
            let secs = t0.elapsed().as_secs_f64();
            let tree = crate::newick::Tree::parse(&r.newick).unwrap();
            let truth_tree = crate::newick::Tree::parse(&truth).unwrap();
            eprintln!(
                "m {m}: {} weighted quartets, {secs:.2} s, nRF to the true tree {:.3}, score {:.4} of total",
                qs.len(),
                crate::newick::nrf(&tree, &truth_tree).1,
                r.score / r.total_weight
            );
        }
    }

    #[test]
    fn input_round_trip_and_induced_quartets() {
        let nm = names(6);
        let mut rng = crate::weight::SplitMix64::new(4);
        let tree = random_binary_tree(&nm, &mut rng);
        let topo = Topology::from_newick(&tree, &nm).unwrap();
        let qs = induced_quartets(&topo);
        assert_eq!(qs.len(), 15, "one topology per 4-set");
        assert!(qs.iter().all(|q| topo.displays(q)));
        let text = to_wqfm_input(&nm, &qs);
        let mut names_back = Vec::new();
        let back = parse_wqfm_input(&text, &mut names_back).unwrap();
        assert_eq!(back.len(), qs.len());
        let to_names = |q: &Quartet, n: &[String]| {
            let s = |t: u32| n[t as usize].clone();
            let mut l = [s(q.l[0]), s(q.l[1])];
            let mut r = [s(q.r[0]), s(q.r[1])];
            l.sort();
            r.sort();
            let mut pair = [l, r];
            pair.sort();
            (pair, q.weight)
        };
        for (a, b) in qs.iter().zip(&back) {
            assert_eq!(to_names(a, &nm), to_names(b, &names_back));
        }
        assert!(parse_wqfm_input("((a,b),(c)); 1", &mut Vec::new()).is_err());
    }
}
