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

/// Text and size of a Halo Tree figure.
#[derive(Debug, Clone, PartialEq)]
pub struct HaloTreeStyle {
    /// Title lines (dataset, taxa, quartets, configuration, date).
    pub title: Vec<String>,
    /// Watermark across the figure, for example `PROVISIONAL — 35% of quartets`.
    pub watermark: Option<String>,
    /// Width and height in pixels (square).
    pub size: f64,
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The Halo Tree as SVG. `halo` maps taxon names to halo values.
pub fn halo_tree_svg(
    layout: &TreeLayout,
    halo: &BTreeMap<String, Option<f64>>,
    style: &HaloTreeStyle,
) -> String {
    let w = style.size;
    let title_h = 18.0 * style.title.len() as f64 + 10.0;
    let legend_h = 46.0;
    let h = w + title_h + legend_h;
    let (cx, cy) = (w / 2.0, title_h + w / 2.0);
    let leaves = layout.leaves();
    let n = leaves.len().max(1);
    let halo_outer = w / 2.0 - 16.0;
    let halo_inner = halo_outer - 12.0;
    let longest = leaves
        .iter()
        .filter_map(|&l| layout.nodes[l].name.as_ref())
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1) as f64;
    // Font size: at most 11 px, smaller when the label ring is crowded.
    let mut font = 11.0f64;
    let mut tree_r = halo_inner - 10.0 - longest * font * 0.6;
    for _ in 0..4 {
        let fit = 2.0 * PI * tree_r.max(1.0) / n as f64 * 0.85;
        font = font.min(fit).max(4.0);
        tree_r = (halo_inner - 10.0 - longest * font * 0.6).max(w * 0.12);
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
    // Branches: an arc at the parent's radius, then a radial line.
    let _ = write!(
        s,
        r##"<g fill="none" stroke="#333333" stroke-width="1.4" stroke-linecap="round">"##
    );
    for (v, node) in layout.nodes.iter().enumerate() {
        if node.children.is_empty() {
            continue;
        }
        let r = layout.radius(v, tree_r);
        let first = node
            .children
            .iter()
            .map(|&c| layout.nodes[c].angle)
            .fold(f64::MAX, f64::min);
        let last = node
            .children
            .iter()
            .map(|&c| layout.nodes[c].angle)
            .fold(f64::MIN, f64::max);
        if r > 0.0 && last > first {
            let (x0, y0) = polar(first, r);
            let (x1, y1) = polar(last, r);
            let large = if last - first > PI { 1 } else { 0 };
            let _ = write!(
                s,
                r#"<path d="M{x0:.2},{y0:.2} A{r:.2},{r:.2} 0 {large} 1 {x1:.2},{y1:.2}"/>"#
            );
        }
        for &c in &node.children {
            let a = layout.nodes[c].angle;
            let (x0, y0) = polar(a, r);
            let (x1, y1) = polar(a, layout.radius(c, tree_r));
            let _ = write!(
                s,
                r#"<line x1="{x0:.2}" y1="{y0:.2}" x2="{x1:.2}" y2="{y1:.2}"/>"#
            );
        }
    }
    s.push_str("</g>");
    // Labels and halo ring.
    let half = PI / n as f64;
    let gap = half * 0.12;
    for &l in &leaves {
        let node = &layout.nodes[l];
        let name = node.name.clone().unwrap_or_default();
        let a = node.angle;
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
            escape(&name)
        );
        let value = halo.get(&name).copied().flatten();
        let (a0, a1) = (a - half + gap, a + half - gap);
        let (p0, p1) = (polar(a0, halo_outer), polar(a1, halo_outer));
        let (p2, p3) = (polar(a1, halo_inner), polar(a0, halo_inner));
        let large = if a1 - a0 > PI { 1 } else { 0 };
        let _ = write!(
            s,
            r##"<path d="M{:.2},{:.2} A{halo_outer:.2},{halo_outer:.2} 0 {large} 1 {:.2},{:.2} L{:.2},{:.2} A{halo_inner:.2},{halo_inner:.2} 0 {large} 0 {:.2},{:.2} Z" fill="{}" stroke="#666666" stroke-width="0.5"/>"##,
            p0.0,
            p0.1,
            p1.0,
            p1.1,
            p2.0,
            p2.1,
            p3.0,
            p3.1,
            hex(halo_colour(value))
        );
        if value.is_some_and(|v| v < LOW_HALO) {
            let (mx, my) = polar(a, halo_outer + 6.0);
            let _ = write!(
                s,
                r##"<circle cx="{mx:.2}" cy="{my:.2}" r="2.6" fill="#222222"/>"##
            );
        }
    }
    // Legend: halo scale, low-halo symbol, note on rooting.
    let ly = title_h + w + 8.0;
    let _ = write!(
        s,
        r#"<defs><linearGradient id="halo-scale" x1="0" x2="1" y1="0" y2="0">"#
    );
    for i in 0..=8 {
        let t = i as f64 / 8.0;
        let _ = write!(
            s,
            r#"<stop offset="{t:.3}" stop-color="{}"/>"#,
            hex(halo_colour(Some(t)))
        );
    }
    s.push_str("</linearGradient></defs>");
    let _ = write!(
        s,
        r##"<text x="12" y="{:.1}" font-size="11" fill="#222222">Halo value</text><rect x="80" y="{:.1}" width="120" height="10" fill="url(#halo-scale)" stroke="#666666" stroke-width="0.5"/><text x="80" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">0</text><text x="140" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">0.5</text><text x="200" y="{:.1}" font-size="10" fill="#222222" text-anchor="middle">1</text><circle cx="228" cy="{:.1}" r="2.6" fill="#222222"/><text x="236" y="{:.1}" font-size="11" fill="#222222">halo value below {LOW_HALO}</text>"##,
        ly + 9.0,
        ly,
        ly + 22.0,
        ly + 22.0,
        ly + 22.0,
        ly + 5.0,
        ly + 9.0
    );
    let _ = write!(
        s,
        r##"<text x="12" y="{:.1}" font-size="10" fill="#555555">Unrooted tree, drawn rooted at the midpoint of its longest path. Branch lengths are not drawn.</text>"##,
        ly + 36.0
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
