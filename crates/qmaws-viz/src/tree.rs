//! Circular cladogram layout and the provisional Quartet Halo Tree (plan
//! 4.7, 4.8.1).
//!
//! Layout: the unrooted tree is drawn rooted at the midpoint of its longest
//! leaf-to-leaf path in edge count; leaves are equally spaced by angle, each
//! internal node's angle is the mean of its children's angles and its radius
//! is proportional to its depth. The same layout is used by the SVG figure
//! and by the GUI's live panel.
//!
//! The figure of this module is the basic provisional Halo Tree of M9: the
//! cladogram, the label ring, the halo ring with a legend and an optional
//! watermark. Group bands, support colours and the final figures follow in
//! M10.

use qmaws_core::newick::Tree;
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::fmt::Write as _;

/// One node of a [`TreeLayout`].
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutNode {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Taxon name of a leaf.
    pub name: Option<String>,
    /// Angle in radians from the positive x axis, in screen coordinates
    /// (y down), so increasing angles run clockwise.
    pub angle: f64,
    /// Number of edges from the display root.
    pub depth: usize,
}

/// A circular cladogram layout; node 0 is the display root.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeLayout {
    pub nodes: Vec<LayoutNode>,
    pub max_depth: usize,
}

impl TreeLayout {
    /// Lays out a Newick tree (read as unrooted).
    pub fn from_newick(text: &str) -> Result<Self, String> {
        let tree = Tree::parse(text.trim()).map_err(|e| e.to_string())?;
        Ok(Self::from_tree(&tree))
    }

    pub fn from_tree(tree: &Tree) -> Self {
        // Undirected adjacency in the order of the Newick text.
        let n = tree.nodes.len();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, node) in tree.nodes.iter().enumerate() {
            for &c in &node.children {
                adj[i].push(c);
                adj[c].push(i);
            }
        }
        if n == 1 {
            return Self {
                nodes: vec![LayoutNode {
                    parent: None,
                    children: Vec::new(),
                    name: tree.nodes[0].label.clone(),
                    angle: 0.0,
                    depth: 0,
                }],
                max_depth: 0,
            };
        }
        // Longest leaf-to-leaf path by two breadth-first searches.
        let start = (0..n).find(|&v| adj[v].len() == 1).unwrap_or(0);
        let (u, _) = farthest(&adj, start);
        let (v, parent) = farthest(&adj, u);
        let mut path = vec![v];
        while let Some(p) = parent[*path.last().unwrap()] {
            path.push(p);
        }
        // Display root: the middle node, or a new node on the middle edge.
        let edges = path.len() - 1;
        let root = if edges % 2 == 0 {
            path[edges / 2]
        } else {
            let (a, b) = (path[edges / 2], path[edges / 2 + 1]);
            let r = adj.len();
            for (x, y) in [(a, b), (b, a)] {
                let k = adj[x].iter().position(|&z| z == y).expect("edge");
                adj[x][k] = r;
            }
            adj.push(vec![a, b]);
            r
        };
        let label = |v: usize| (v < n).then(|| tree.nodes[v].label.clone()).flatten();
        // Rooted copy; nodes of degree 2 other than the root are passed through.
        let mut nodes = vec![LayoutNode {
            parent: None,
            children: Vec::new(),
            name: None,
            angle: 0.0,
            depth: 0,
        }];
        let mut stack: Vec<(usize, usize, usize)> =
            adj[root].iter().rev().map(|&c| (c, root, 0usize)).collect();
        while let Some((v, from, parent)) = stack.pop() {
            let next: Vec<usize> = adj[v].iter().copied().filter(|&w| w != from).collect();
            if next.len() == 1 {
                stack.push((next[0], v, parent));
                continue;
            }
            let id = nodes.len();
            let depth = nodes[parent].depth + 1;
            nodes.push(LayoutNode {
                parent: Some(parent),
                children: Vec::new(),
                name: if next.is_empty() { label(v) } else { None },
                angle: 0.0,
                depth,
            });
            nodes[parent].children.push(id);
            for &w in next.iter().rev() {
                stack.push((w, v, id));
            }
        }
        let mut layout = Self {
            max_depth: nodes.iter().map(|n| n.depth).max().unwrap_or(0),
            nodes,
        };
        layout.assign_angles();
        layout
    }

    fn assign_angles(&mut self) {
        let leaves = self.leaves();
        let count = leaves.len().max(1) as f64;
        for (i, &leaf) in leaves.iter().enumerate() {
            self.nodes[leaf].angle = 2.0 * PI * i as f64 / count;
        }
        for v in self.postorder() {
            let c = &self.nodes[v].children;
            if !c.is_empty() {
                let mean = c.iter().map(|&x| self.nodes[x].angle).sum::<f64>() / c.len() as f64;
                self.nodes[v].angle = mean;
            }
        }
    }

    /// Leaves in drawing order.
    pub fn leaves(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![0];
        while let Some(v) = stack.pop() {
            if self.nodes[v].children.is_empty() {
                out.push(v);
            } else {
                stack.extend(self.nodes[v].children.iter().rev());
            }
        }
        out
    }

    fn postorder(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![(0usize, false)];
        while let Some((v, expanded)) = stack.pop() {
            if expanded || self.nodes[v].children.is_empty() {
                out.push(v);
            } else {
                stack.push((v, true));
                for &c in self.nodes[v].children.iter().rev() {
                    stack.push((c, false));
                }
            }
        }
        out
    }

    /// Radius of `node` when the leaves are at `outer`: every leaf on the
    /// outer circle, internal nodes by depth.
    pub fn radius(&self, node: usize, outer: f64) -> f64 {
        if self.max_depth == 0 {
            0.0
        } else if self.nodes[node].children.is_empty() {
            outer
        } else {
            outer * self.nodes[node].depth as f64 / self.max_depth as f64
        }
    }

    /// The taxa below `node`, sorted: the split that the edge above `node`
    /// defines, used to find edges that changed between two trees.
    pub fn clade(&self, node: usize) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![node];
        while let Some(v) = stack.pop() {
            match &self.nodes[v].name {
                Some(name) if self.nodes[v].children.is_empty() => out.push(name.clone()),
                _ => stack.extend(&self.nodes[v].children),
            }
        }
        out.sort();
        out
    }

    /// Canonical form of the split above `node` among `all` taxa: the side
    /// that does not contain the first taxon in sorted order.
    pub fn split(&self, node: usize, all: &[String]) -> Vec<String> {
        let clade = self.clade(node);
        let first = all.iter().min();
        if first.is_some_and(|f| clade.contains(f)) {
            let mut other: Vec<String> =
                all.iter().filter(|t| !clade.contains(t)).cloned().collect();
            other.sort();
            other
        } else {
            clade
        }
    }

    /// For every node other than the root, whether the split above it is
    /// absent from `previous` (a newer tree's changed edges). Leaf edges
    /// never change.
    pub fn changed_edges(&self, previous: &TreeLayout) -> Vec<bool> {
        let all = self.taxa();
        let before: std::collections::BTreeSet<Vec<String>> = (1..previous.nodes.len())
            .filter(|&v| !previous.nodes[v].children.is_empty())
            .map(|v| previous.split(v, &all))
            .collect();
        (0..self.nodes.len())
            .map(|v| {
                v != 0
                    && !self.nodes[v].children.is_empty()
                    && !before.contains(&self.split(v, &all))
            })
            .collect()
    }

    /// All taxon names, sorted.
    pub fn taxa(&self) -> Vec<String> {
        let mut t = self.clade(0);
        t.sort();
        t
    }
}

fn farthest(adj: &[Vec<usize>], start: usize) -> (usize, Vec<Option<usize>>) {
    let mut parent = vec![None; adj.len()];
    let mut seen = vec![false; adj.len()];
    let mut queue = std::collections::VecDeque::from([start]);
    seen[start] = true;
    let mut last = start;
    while let Some(v) = queue.pop_front() {
        last = v;
        for &w in &adj[v] {
            if !seen[w] {
                seen[w] = true;
                parent[w] = Some(v);
                queue.push_back(w);
            }
        }
    }
    (last, parent)
}

/// Colour of a halo value on a diverging, colour-blind safe scale (ColorBrewer
/// PuOr: orange for low, light grey in the middle, purple for high); grey
/// when there is no value.
pub fn halo_colour(value: Option<f64>) -> [u8; 3] {
    const STOPS: [(f64, [u8; 3]); 5] = [
        (0.0, [179, 88, 6]),
        (0.25, [241, 163, 64]),
        (0.5, [247, 247, 247]),
        (0.75, [153, 142, 195]),
        (1.0, [84, 39, 136]),
    ];
    let Some(v) = value.filter(|v| v.is_finite()) else {
        return [189, 189, 189];
    };
    let v = v.clamp(0.0, 1.0);
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if v <= b {
            let t = (v - a) / (b - a);
            return std::array::from_fn(|i| {
                (ca[i] as f64 + t * (cb[i] as f64 - ca[i] as f64)).round() as u8
            });
        }
    }
    STOPS[4].1
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Halo values below this are marked with a symbol (plan 4.8.1).
pub const LOW_HALO: f64 = 0.6;

/// Support of the internal edges, keyed by split in the canonical form of
/// [`TreeLayout::split`] (see [`canonical_split`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgeSupport {
    /// `S1` or `S2`.
    pub label: String,
    pub values: BTreeMap<Vec<String>, f64>,
}

/// The canonical form of the split that `clade` defines among `all` taxa:
/// the side without the alphabetically first taxon, sorted.
pub fn canonical_split(clade: &[String], all: &[String]) -> Vec<String> {
    let first = all.iter().min();
    let mut side: Vec<String> = if first.is_some_and(|f| clade.contains(f)) {
        all.iter().filter(|t| !clade.contains(t)).cloned().collect()
    } else {
        clade.to_vec()
    };
    side.sort();
    side
}

/// Groups of taxa for the group bands, with the source named in the
/// legend.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Groups {
    /// For example `groups.tsv`, `Open Tree of Life taxonomy (family)` or
    /// `automatic: clades of the tree`.
    pub source: String,
    /// Taxon to group name; taxa without a group get no band.
    pub of: BTreeMap<String, String>,
}

impl Groups {
    /// Group names, sorted; a group's colour is [`crate::colour::group_colour`]
    /// of its index here.
    pub fn names(&self) -> Vec<String> {
        let mut n: Vec<String> = self.of.values().cloned().collect();
        n.sort();
        n.dedup();
        n
    }

    pub fn colour_of(&self, group: &str) -> [u8; 3] {
        let i = self.names().iter().position(|g| g == group).unwrap_or(0);
        crate::colour::group_colour(i)
    }
}

/// Text, size and optional layers of a Halo Tree figure.
#[derive(Debug, Clone, PartialEq)]
pub struct HaloTreeStyle {
    /// Title lines (dataset, taxa, quartets, configuration, date).
    pub title: Vec<String>,
    /// Watermark across the figure, for example `PROVISIONAL — 35% of quartets`.
    pub watermark: Option<String>,
    /// Width of the tree area in pixels (square).
    pub size: f64,
    /// Branch colour and width by support.
    pub support: Option<EdgeSupport>,
    /// Group bands outside the halo ring.
    pub groups: Option<Groups>,
}

impl Default for HaloTreeStyle {
    fn default() -> Self {
        Self {
            title: Vec::new(),
            watermark: None,
            size: 800.0,
            support: None,
            groups: None,
        }
    }
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Runs of consecutive leaves (in drawing order) that share a group:
/// (group, first index, last index).
pub(crate) fn group_runs(leaf_names: &[String], groups: &Groups) -> Vec<(String, usize, usize)> {
    let mut runs: Vec<(String, usize, usize)> = Vec::new();
    for (i, name) in leaf_names.iter().enumerate() {
        let Some(g) = groups.of.get(name) else {
            continue;
        };
        match runs.last_mut() {
            Some((last, _, end)) if last == g && *end + 1 == i => *end = i,
            _ => runs.push((g.clone(), i, i)),
        }
    }
    runs
}

/// Support of the edge above `node`, if it is internal and known.
pub(crate) fn node_support(
    layout: &TreeLayout,
    node: usize,
    all: &[String],
    support: &EdgeSupport,
) -> Option<f64> {
    if node == 0 || layout.nodes[node].children.is_empty() {
        return None;
    }
    support.values.get(&layout.split(node, all)).copied()
}

/// The Halo Tree as SVG. `halo` maps taxon names to halo values.
pub fn halo_tree_svg(
    layout: &TreeLayout,
    halo: &BTreeMap<String, Option<f64>>,
    style: &HaloTreeStyle,
) -> String {
    use crate::colour::{support_colour, support_width};
    let w = style.size;
    let title_h = 18.0 * style.title.len() as f64 + 10.0;
    let all = layout.taxa();
    let leaves = layout.leaves();
    let leaf_names: Vec<String> = leaves
        .iter()
        .map(|&l| layout.nodes[l].name.clone().unwrap_or_default())
        .collect();
    let group_names = style.groups.as_ref().map(Groups::names).unwrap_or_default();
    let legend_h = 8.0
        + 32.0
        + if style.support.is_some() { 30.0 } else { 0.0 }
        + if style.groups.is_some() {
            16.0 + 16.0 * group_names.len().div_ceil(3) as f64
        } else {
            0.0
        }
        + 18.0;
    let h = w + title_h + legend_h;
    let (cx, cy) = (w / 2.0, title_h + w / 2.0);
    let n = leaves.len().max(1);
    // Rings from the outside in: group labels, group band, halo ring, labels.
    let group_label = match &style.groups {
        Some(g) => {
            let longest = g
                .names()
                .iter()
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0) as f64;
            (12.0 + longest * 11.0 * 0.62).clamp(30.0, w * 0.16)
        }
        None => 0.0,
    };
    let band_outer = w / 2.0 - 10.0 - group_label;
    let band_inner = band_outer - if style.groups.is_some() { 10.0 } else { 0.0 };
    let halo_outer = band_inner - if style.groups.is_some() { 4.0 } else { 6.0 };
    let halo_inner = halo_outer - 12.0;
    let longest = leaf_names
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1) as f64;
    // Font size: at most 11 px, smaller when the label ring is crowded.
    let mut font = 11.0f64;
    let mut tree_r = halo_inner - 12.0 - longest * font * 0.62;
    for _ in 0..4 {
        let fit = 2.0 * PI * tree_r.max(1.0) / n as f64 * 0.85;
        font = font.min(fit).max(4.0);
        tree_r = (halo_inner - 12.0 - longest * font * 0.62).max(w * 0.12);
    }
    let polar = |a: f64, r: f64| (cx + r * a.cos(), cy + r * a.sin());
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif">"#
    );
    let _ = write!(s, r##"<rect width="{w}" height="{h}" fill="#ffffff"/>"##);
    for (i, line) in style.title.iter().enumerate() {
        let (size, weight) = if i == 0 { (15, "bold") } else { (12, "normal") };
        let _ = write!(
            s,
            r##"<text x="12" y="{:.1}" font-size="{size}" font-weight="{weight}" fill="#222222">{}</text>"##,
            20.0 + 18.0 * i as f64,
            escape(line)
        );
    }
    // Branches: for each child, an arc at the parent's radius from the
    // parent's angle to the child's, then a radial line; both coloured by
    // the support of the child's edge.
    let edge_style = |child: usize| -> (String, f64) {
        match &style.support {
            Some(sup) => {
                let v = node_support(layout, child, &all, sup);
                if layout.nodes[child].children.is_empty() {
                    ("#555555".to_string(), 1.2)
                } else {
                    (hex(support_colour(v)), support_width(v))
                }
            }
            None => ("#333333".to_string(), 1.4),
        }
    };
    let _ = write!(s, r#"<g fill="none" stroke-linecap="round">"#);
    for (v, node) in layout.nodes.iter().enumerate() {
        if node.children.is_empty() {
            continue;
        }
        let r = layout.radius(v, tree_r);
        let pa = node.angle;
        for &c in &node.children {
            let a = layout.nodes[c].angle;
            let (colour, width) = edge_style(c);
            if r > 0.0 && (a - pa).abs() > 1e-9 {
                let (x0, y0) = polar(pa, r);
                let (x1, y1) = polar(a, r);
                let sweep = if a > pa { 1 } else { 0 };
                let large = if (a - pa).abs() > PI { 1 } else { 0 };
                let _ = write!(
                    s,
                    r#"<path d="M{x0:.2},{y0:.2} A{r:.2},{r:.2} 0 {large} {sweep} {x1:.2},{y1:.2}" stroke="{colour}" stroke-width="{width:.2}"/>"#
                );
            }
            let (x0, y0) = polar(a, r);
            let (x1, y1) = polar(a, layout.radius(c, tree_r));
            let _ = write!(
                s,
                r#"<line x1="{x0:.2}" y1="{y0:.2}" x2="{x1:.2}" y2="{y1:.2}" stroke="{colour}" stroke-width="{width:.2}"/>"#
            );
        }
    }
    s.push_str("</g>");
    // Labels and halo ring.
    let half = PI / n as f64;
    let gap = half * 0.12;
    let arc_path = |a0: f64, a1: f64, outer: f64, inner: f64| -> String {
        let (p0, p1) = (polar(a0, outer), polar(a1, outer));
        let (p2, p3) = (polar(a1, inner), polar(a0, inner));
        let large = if a1 - a0 > PI { 1 } else { 0 };
        format!(
            "M{:.2},{:.2} A{outer:.2},{outer:.2} 0 {large} 1 {:.2},{:.2} L{:.2},{:.2} A{inner:.2},{inner:.2} 0 {large} 0 {:.2},{:.2} Z",
            p0.0, p0.1, p1.0, p1.1, p2.0, p2.1, p3.0, p3.1
        )
    };
    for (&l, name) in leaves.iter().zip(&leaf_names) {
        let a = layout.nodes[l].angle;
        let deg = a.to_degrees();
        let (x, y) = polar(a, tree_r + 6.0);
        // Readable orientation: flip text on the left half.
        let left = a.cos() < 0.0;
        let (rot, anchor) = if left {
            (deg + 180.0, "end")
        } else {
            (deg, "start")
        };
        let _ = write!(
            s,
            r##"<text x="{x:.2}" y="{y:.2}" font-size="{font:.1}" fill="#222222" text-anchor="{anchor}" dominant-baseline="central" transform="rotate({rot:.2} {x:.2} {y:.2})">{}</text>"##,
            escape(name)
        );
        let value = halo.get(name).copied().flatten();
        let _ = write!(
            s,
            r##"<path d="{}" fill="{}" stroke="#666666" stroke-width="0.5"/>"##,
            arc_path(a - half + gap, a + half - gap, halo_outer, halo_inner),
            hex(halo_colour(value))
        );
        if value.is_some_and(|v| v < LOW_HALO) {
            // The mark sits on the halo segment (low values are light).
            let (mx, my) = polar(a, (halo_inner + halo_outer) / 2.0);
            let _ = write!(
                s,
                r##"<circle cx="{mx:.2}" cy="{my:.2}" r="2.2" fill="#222222"/>"##
            );
        }
    }
    // Group bands: one arc per run of neighbouring taxa in a group; the name
    // is written at the longest run of each group.
    if let Some(groups) = &style.groups {
        let runs = group_runs(&leaf_names, groups);
        let mut longest: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, (g, a, b)) in runs.iter().enumerate() {
            let len = b - a;
            match longest.get(g.as_str()) {
                Some(&j) if runs[j].2 - runs[j].1 >= len => {}
                _ => {
                    longest.insert(g, i);
                }
            }
        }
        for (i, (g, first, last)) in runs.iter().enumerate() {
            let a0 = layout.nodes[leaves[*first]].angle - half + gap;
            let a1 = layout.nodes[leaves[*last]].angle + half - gap;
            let _ = write!(
                s,
                r##"<path d="{}" fill="{}" stroke="#444444" stroke-width="0.4"/>"##,
                arc_path(a0, a1, band_outer, band_inner),
                hex(groups.colour_of(g))
            );
            if longest.get(g.as_str()) == Some(&i) {
                let mid = (a0 + a1) / 2.0;
                let (x, y) = polar(mid, band_outer + 4.0);
                let left = mid.cos() < 0.0;
                let (rot, anchor) = if left {
                    (mid.to_degrees() + 180.0, "end")
                } else {
                    (mid.to_degrees(), "start")
                };
                let size = font.clamp(7.0, 11.0);
                let shown: String = g
                    .chars()
                    .take((group_label / (size * 0.6)) as usize)
                    .collect();
                let _ = write!(
                    s,
                    r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.1}" font-style="italic" fill="#222222" text-anchor="{anchor}" dominant-baseline="central" transform="rotate({rot:.2} {x:.2} {y:.2})">{}</text>"##,
                    escape(&shown)
                );
            }
        }
    }
    // Legend.
    let mut ly = title_h + w + 8.0;
    let gradient = |s: &mut String, id: &str, f: &dyn Fn(f64) -> [u8; 3]| {
        let _ = write!(
            s,
            r#"<defs><linearGradient id="{id}" x1="0" x2="1" y1="0" y2="0">"#
        );
        for i in 0..=10 {
            let t = i as f64 / 10.0;
            let _ = write!(s, r#"<stop offset="{t:.3}" stop-color="{}"/>"#, hex(f(t)));
        }
        s.push_str("</linearGradient></defs>");
    };
    gradient(&mut s, "halo-scale", &|t| halo_colour(Some(t)));
    let _ = write!(
        s,
        r##"<text x="12" y="{:.1}" font-size="11" fill="#222222">Halo value</text><rect x="90" y="{:.1}" width="120" height="10" fill="url(#halo-scale)" stroke="#666666" stroke-width="0.5"/><text x="90" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">0</text><text x="150" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">0.5</text><text x="210" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">1</text><circle cx="238" cy="{:.1}" r="2.6" fill="#222222"/><text x="246" y="{:.1}" font-size="11" fill="#222222">halo value below {LOW_HALO}</text>"##,
        ly + 9.0,
        ly,
        ly + 22.0,
        ly + 22.0,
        ly + 22.0,
        ly + 5.0,
        ly + 9.0
    );
    ly += 32.0;
    if let Some(sup) = &style.support {
        gradient(&mut s, "support-scale", &|t| support_colour(Some(t)));
        let _ = write!(
            s,
            r##"<text x="12" y="{:.1}" font-size="11" fill="#222222">{} support</text><rect x="90" y="{:.1}" width="120" height="10" fill="url(#support-scale)" stroke="#666666" stroke-width="0.5"/><text x="90" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">0</text><text x="210" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">1</text><text x="236" y="{:.1}" font-size="11" fill="#222222">(branch colour; width grows with support; grey: no value)</text>"##,
            ly + 9.0,
            escape(&sup.label),
            ly,
            ly + 22.0,
            ly + 22.0,
            ly + 9.0
        );
        ly += 30.0;
    }
    if let Some(groups) = &style.groups {
        let _ = write!(
            s,
            r##"<text x="12" y="{:.1}" font-size="11" fill="#222222">Groups ({}):</text>"##,
            ly + 9.0,
            escape(&groups.source)
        );
        ly += 16.0;
        let col_w = (w - 24.0) / 3.0;
        for (i, g) in group_names.iter().enumerate() {
            let x = 12.0 + col_w * (i % 3) as f64;
            let y = ly + 16.0 * (i / 3) as f64;
            let _ = write!(
                s,
                r##"<rect x="{x:.1}" y="{y:.1}" width="14" height="10" fill="{}" stroke="#444444" stroke-width="0.4"/><text x="{:.1}" y="{:.1}" font-size="10" fill="#222222">{}</text>"##,
                hex(groups.colour_of(g)),
                x + 20.0,
                y + 9.0,
                escape(g)
            );
        }
        ly += 16.0 * group_names.len().div_ceil(3) as f64;
    }
    let _ = write!(
        s,
        r##"<text x="12" y="{:.1}" font-size="10" fill="#555555">Unrooted tree, drawn rooted at the midpoint of its longest path. Branch lengths are not drawn.</text>"##,
        ly + 10.0
    );
    if let Some(mark) = &style.watermark {
        let _ = write!(
            s,
            r##"<text x="{cx:.1}" y="{cy:.1}" font-size="{:.1}" font-weight="bold" fill="#c0392b" fill-opacity="0.22" text-anchor="middle" dominant-baseline="central" transform="rotate(-30 {cx:.1} {cy:.1})">{}</text>"##,
            (w / mark.chars().count().max(1) as f64 * 1.3).min(48.0),
            escape(mark)
        );
        let _ = write!(
            s,
            r##"<text x="{:.1}" y="20" font-size="12" font-weight="bold" fill="#c0392b" text-anchor="end">{}</text>"##,
            w - 12.0,
            escape(mark)
        );
    }
    s.push_str("</svg>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(nwk: &str) -> TreeLayout {
        TreeLayout::from_newick(nwk).unwrap()
    }

    #[test]
    fn midpoint_root_of_a_caterpillar() {
        // Longest path A..F has 5 edges: the root is on its middle edge,
        // between the parents of C and D.
        let l = layout("(A,B,(C,(D,(E,F))));");
        let root_clades: Vec<Vec<String>> =
            l.nodes[0].children.iter().map(|&c| l.clade(c)).collect();
        assert_eq!(root_clades.len(), 2);
        let mut sizes: Vec<usize> = root_clades.iter().map(|c| c.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![3, 3]);
        assert_eq!(l.taxa(), ["A", "B", "C", "D", "E", "F"]);
    }

    #[test]
    fn midpoint_root_on_a_node() {
        // ((A,B),(C,D),E): longest path A..C has 4 edges; the middle node is
        // the basal one.
        let l = layout("((A,B),(C,D),E);");
        assert_eq!(l.nodes[0].children.len(), 3);
        assert_eq!(l.max_depth, 2);
    }

    #[test]
    fn leaves_are_equally_spaced_and_internal_angles_are_means() {
        let l = layout("((A,B),(C,D),E);");
        let leaves = l.leaves();
        assert_eq!(leaves.len(), 5);
        for (i, &v) in leaves.iter().enumerate() {
            assert!((l.nodes[v].angle - 2.0 * PI * i as f64 / 5.0).abs() < 1e-12);
        }
        for (v, node) in l.nodes.iter().enumerate() {
            if !node.children.is_empty() {
                let mean = node.children.iter().map(|&c| l.nodes[c].angle).sum::<f64>()
                    / node.children.len() as f64;
                assert!((l.nodes[v].angle - mean).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn rooted_input_with_a_degree_two_root_is_read_unrooted() {
        let a = layout("((A,B),(C,D));");
        let b = layout("(A,B,(C,D));");
        assert_eq!(a.changed_edges(&b), vec![false; a.nodes.len()]);
        assert_eq!(a.leaves().len(), 4);
    }

    #[test]
    fn changed_edges_mark_new_splits_only() {
        let before = layout("((A,B),(C,D),(E,F));");
        let after = layout("((A,B),(C,E),(D,F));");
        let changed = after.changed_edges(&before);
        let all = after.taxa();
        for (v, &c) in changed.iter().enumerate() {
            if c {
                let split = after.split(v, &all);
                assert!(split != vec!["A".to_string(), "B".to_string()]);
            }
        }
        assert_eq!(changed.iter().filter(|&&c| c).count(), 2);
        assert_eq!(after.changed_edges(&after), vec![false; after.nodes.len()]);
    }

    #[test]
    fn halo_colours_run_from_orange_to_purple() {
        assert_eq!(halo_colour(Some(0.0)), [179, 88, 6]);
        assert_eq!(halo_colour(Some(0.5)), [247, 247, 247]);
        assert_eq!(halo_colour(Some(1.0)), [84, 39, 136]);
        assert_eq!(halo_colour(Some(2.0)), [84, 39, 136]);
        assert_eq!(halo_colour(None), [189, 189, 189]);
    }

    #[test]
    fn svg_has_every_label_halo_segment_marker_and_the_watermark() {
        let l = layout("((A,B),(C,D),(E,F));");
        let halo: BTreeMap<String, Option<f64>> = [
            ("A", Some(0.9)),
            ("B", Some(0.4)),
            ("C", None),
            ("D", Some(0.7)),
            ("E", Some(0.59)),
            ("F", Some(1.0)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let style = HaloTreeStyle {
            title: vec!["Test <tree>".into(), "6 taxa".into()],
            watermark: Some("PROVISIONAL — 35% of quartets".into()),
            size: 480.0,
            ..Default::default()
        };
        let svg = halo_tree_svg(&l, &halo, &style);
        for t in ["A", "B", "C", "D", "E", "F"] {
            assert!(svg.contains(&format!(">{t}</text>")), "label {t}");
        }
        assert!(svg.contains("Test &lt;tree&gt;"));
        assert_eq!(svg.matches("PROVISIONAL — 35% of quartets").count(), 2);
        // Two taxa below 0.6 plus the legend symbol.
        assert_eq!(svg.matches("<circle").count(), 3);
        assert!(svg.contains("#bdbdbd"), "no value is grey");
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>\n"));
        let no_mark = halo_tree_svg(
            &l,
            &halo,
            &HaloTreeStyle {
                watermark: None,
                ..style
            },
        );
        assert!(!no_mark.contains("PROVISIONAL"));
    }
}
