//! Rectangular cladograms (plan 4.8.3) and the tanglegram (plan 4.8.2).
//!
//! Both use the display root of [`TreeLayout`]: leaves in drawing order
//! from top to bottom, internal nodes at the mean height of their children
//! and at a horizontal position by depth, with every leaf aligned on the
//! right (left in the mirrored tree of a tanglegram).

use crate::colour::{hex, support_colour, support_width};
use crate::tree::{escape, halo_colour, node_support, EdgeSupport, Groups, TreeLayout, LOW_HALO};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Vertical distance between leaves, in pixels.
const ROW: f64 = 16.0;

/// Positions of every node: x from 0 (root) to 1 (leaves), y in rows.
pub(crate) fn positions(layout: &TreeLayout) -> (Vec<f64>, Vec<f64>) {
    let n = layout.nodes.len();
    let mut y = vec![0.0; n];
    let mut x = vec![0.0; n];
    for (i, &l) in layout.leaves().iter().enumerate() {
        y[l] = i as f64;
    }
    // Post-order: children before parents.
    let mut order = Vec::new();
    let mut stack = vec![(0usize, false)];
    while let Some((v, expanded)) = stack.pop() {
        if expanded || layout.nodes[v].children.is_empty() {
            order.push(v);
        } else {
            stack.push((v, true));
            for &c in layout.nodes[v].children.iter().rev() {
                stack.push((c, false));
            }
        }
    }
    for &v in &order {
        let node = &layout.nodes[v];
        if !node.children.is_empty() {
            y[v] = node.children.iter().map(|&c| y[c]).sum::<f64>() / node.children.len() as f64;
        }
        x[v] = layout.radius(v, 1.0);
    }
    (x, y)
}

/// Text and layers of a rectangular tree.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RectStyle {
    pub title: Vec<String>,
    /// Branch colour and width, and the first number at each internal node.
    pub support: Option<EdgeSupport>,
    /// A second support value written after the first (for example S2).
    pub second: Option<EdgeSupport>,
    pub groups: Option<Groups>,
}

/// The rectangular tree as SVG, with support values at the internal nodes,
/// a halo square before each taxon name and group colours after it.
pub fn rectangular_svg(
    layout: &TreeLayout,
    halo: &BTreeMap<String, Option<f64>>,
    style: &RectStyle,
) -> String {
    let all = layout.taxa();
    let leaves = layout.leaves();
    let (px, py) = positions(layout);
    let longest = all.iter().map(|s| s.chars().count()).max().unwrap_or(1) as f64;
    let group_w = style
        .groups
        .as_ref()
        .map(|g| {
            20.0 + 6.5
                * g.names()
                    .iter()
                    .map(|s| s.chars().count())
                    .max()
                    .unwrap_or(0) as f64
        })
        .unwrap_or(0.0);
    let tree_w = 520.0;
    let label_w = 34.0 + longest * 7.0;
    let w = 20.0 + tree_w + label_w + group_w + 20.0;
    let title_h = 18.0 * style.title.len() as f64 + 14.0;
    let legend_h = 60.0;
    let h = title_h + ROW * leaves.len() as f64 + legend_h;
    let x_of = |v: usize| 20.0 + tree_w * px[v];
    let y_of = |v: usize| title_h + ROW * (py[v] + 0.5);
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.1} {h:.1}" font-family="sans-serif">"#
    );
    let _ = write!(
        s,
        r##"<rect width="{w:.1}" height="{h:.1}" fill="#ffffff"/>"##
    );
    for (i, line) in style.title.iter().enumerate() {
        let (size, weight) = if i == 0 { (15, "bold") } else { (12, "normal") };
        let _ = write!(
            s,
            r##"<text x="12" y="{:.1}" font-size="{size}" font-weight="{weight}" fill="#222222">{}</text>"##,
            20.0 + 18.0 * i as f64,
            escape(line)
        );
    }
    s.push_str(r#"<g fill="none" stroke-linecap="square">"#);
    for (v, node) in layout.nodes.iter().enumerate() {
        for &c in &node.children {
            let (colour, width) = match &style.support {
                Some(sup) if !layout.nodes[c].children.is_empty() => {
                    let val = node_support(layout, c, &all, sup);
                    (hex(support_colour(val)), support_width(val))
                }
                _ => ("#444444".to_string(), 1.2),
            };
            let _ = write!(
                s,
                r#"<path d="M{:.2},{:.2} V{:.2} H{:.2}" stroke="{colour}" stroke-width="{width:.2}"/>"#,
                x_of(v),
                y_of(v),
                y_of(c),
                x_of(c)
            );
        }
    }
    s.push_str("</g>");
    // Support values at internal nodes.
    if let Some(sup) = &style.support {
        for v in 1..layout.nodes.len() {
            if layout.nodes[v].children.is_empty() {
                continue;
            }
            let first = node_support(layout, v, &all, sup);
            let second = style
                .second
                .as_ref()
                .and_then(|s2| node_support(layout, v, &all, s2));
            let text = match (first, second) {
                (Some(a), Some(b)) => format!("{a:.2}/{b:.2}"),
                (Some(a), None) => format!("{a:.2}"),
                (None, Some(b)) => format!("-/{b:.2}"),
                (None, None) => continue,
            };
            let _ = write!(
                s,
                r##"<text x="{:.2}" y="{:.2}" font-size="9" fill="#333333" text-anchor="end">{}</text>"##,
                x_of(v) - 2.0,
                y_of(v) - 3.0,
                text
            );
        }
    }
    // Halo squares, names and groups.
    let lx = 20.0 + tree_w;
    for &l in &leaves {
        let name = layout.nodes[l].name.clone().unwrap_or_default();
        let y = y_of(l);
        let value = halo.get(&name).copied().flatten();
        let _ = write!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="10" height="10" fill="{}" stroke="#666666" stroke-width="0.5"/>"##,
            lx + 4.0,
            y - 5.0,
            hex(halo_colour(value))
        );
        let marker = if value.is_some_and(|v| v < LOW_HALO) {
            " \u{25cf}"
        } else {
            ""
        };
        let _ = write!(
            s,
            r##"<text x="{:.1}" y="{:.1}" font-size="11" fill="#222222" dominant-baseline="central">{}{marker}</text>"##,
            lx + 18.0,
            y,
            escape(&name)
        );
        if let Some(groups) = &style.groups {
            if let Some(g) = groups.of.get(&name) {
                let gx = lx + label_w;
                let _ = write!(
                    s,
                    r##"<rect x="{gx:.1}" y="{:.1}" width="12" height="{ROW}" fill="{}"/><text x="{:.1}" y="{y:.1}" font-size="10" font-style="italic" fill="#222222" dominant-baseline="central">{}</text>"##,
                    y - ROW / 2.0,
                    hex(groups.colour_of(g)),
                    gx + 16.0,
                    escape(g)
                );
            }
        }
    }
    // Legend.
    let ly = title_h + ROW * leaves.len() as f64 + 10.0;
    let mut legend =
        String::from("Square: halo value (orange low, purple high; \u{25cf} below 0.6).");
    if let Some(sup) = &style.support {
        legend.push_str(&format!(
            " Numbers at nodes: {}{}. Branch colour and width: {} (light thin: low; dark thick: high).",
            sup.label,
            style
                .second
                .as_ref()
                .map(|s2| format!("/{}", s2.label))
                .unwrap_or_default(),
            sup.label
        ));
    }
    let _ = write!(
        s,
        r##"<text x="12" y="{:.1}" font-size="10" fill="#555555">{}</text>"##,
        ly + 10.0,
        escape(&legend)
    );
    let mut note = String::from("Unrooted tree, drawn rooted at the midpoint of its longest path; branch lengths are not drawn.");
    if let Some(g) = &style.groups {
        note.push_str(&format!(" Groups: {}.", g.source));
    }
    let _ = write!(
        s,
        r##"<text x="12" y="{:.1}" font-size="10" fill="#555555">{}</text>"##,
        ly + 26.0,
        escape(&note)
    );
    s.push_str("</svg>\n");
    s
}

/// Number of crossing connector lines: pairs of taxa in opposite order in
/// the two leaf orders.
pub fn crossings(left: &[String], right: &[String]) -> usize {
    let pos: BTreeMap<&str, usize> = right
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let r: Vec<usize> = left
        .iter()
        .filter_map(|n| pos.get(n.as_str()).copied())
        .collect();
    let mut count = 0;
    for i in 0..r.len() {
        for j in i + 1..r.len() {
            if r[i] > r[j] {
                count += 1;
            }
        }
    }
    count
}

fn leaf_order(layout: &TreeLayout) -> Vec<String> {
    layout
        .leaves()
        .iter()
        .map(|&l| layout.nodes[l].name.clone().unwrap_or_default())
        .collect()
}

/// Greedy untangling: at every internal node of either tree, tries every
/// rotation of its children's order, forwards and reversed, and keeps the
/// one with the fewest crossings; repeats until nothing improves (at most
/// 20 passes). Returns the crossings left.
pub fn untangle(left: &mut TreeLayout, right: &mut TreeLayout) -> usize {
    let mut best = crossings(&leaf_order(left), &leaf_order(right));
    for _ in 0..20 {
        let before = best;
        for side in 0..2 {
            let n = if side == 0 {
                left.nodes.len()
            } else {
                right.nodes.len()
            };
            for v in 0..n {
                let original = {
                    let tree = if side == 0 { &*left } else { &*right };
                    tree.nodes[v].children.clone()
                };
                if original.len() < 2 {
                    continue;
                }
                let mut keep = original.clone();
                for reversed in [false, true] {
                    for shift in 0..original.len() {
                        let mut order = original.clone();
                        order.rotate_left(shift);
                        if reversed {
                            order.reverse();
                        }
                        let tree = if side == 0 { &mut *left } else { &mut *right };
                        tree.nodes[v].children = order.clone();
                        let c = crossings(&leaf_order(left), &leaf_order(right));
                        if c < best {
                            best = c;
                            keep = order;
                        }
                    }
                }
                let tree = if side == 0 { &mut *left } else { &mut *right };
                tree.nodes[v].children = keep;
            }
        }
        if best == before || best == 0 {
            break;
        }
    }
    best
}

/// Text of a tanglegram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TanglegramStyle {
    pub title: Vec<String>,
    /// Names above the two trees.
    pub left_label: String,
    pub right_label: String,
}

/// The tanglegram as SVG: `left` (our tree) and `right` (the reference)
/// as rectangular trees facing each other, lines joining the same taxa.
/// Taxa in `differ` (whose closest relatives differ between the trees) are
/// joined by dashed vermillion lines, the others by grey lines. Call [`untangle`]
/// first to reduce crossings.
pub fn tanglegram_svg(
    left: &TreeLayout,
    right: &TreeLayout,
    differ: &BTreeSet<String>,
    style: &TanglegramStyle,
) -> String {
    let (lx, ly) = positions(left);
    let (rx, ry) = positions(right);
    let n = left.leaves().len();
    let longest = left
        .taxa()
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1) as f64;
    let tree_w = 300.0;
    let label_w = 10.0 + longest * 6.6;
    let gap = 160.0;
    let w = 20.0 + 2.0 * (tree_w + label_w) + gap + 20.0;
    let title_h = 18.0 * style.title.len() as f64 + 34.0;
    let h = title_h + ROW * n as f64 + 50.0;
    let lx_of = |v: usize| 20.0 + tree_w * lx[v];
    let rx_of = |v: usize| w - 20.0 - tree_w * rx[v];
    let y_l = |v: usize| title_h + ROW * (ly[v] + 0.5);
    let y_r = |v: usize| title_h + ROW * (ry[v] + 0.5);
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.1} {h:.1}" font-family="sans-serif">"#
    );
    let _ = write!(
        s,
        r##"<rect width="{w:.1}" height="{h:.1}" fill="#ffffff"/>"##
    );
    for (i, line) in style.title.iter().enumerate() {
        let (size, weight) = if i == 0 { (15, "bold") } else { (12, "normal") };
        let _ = write!(
            s,
            r##"<text x="12" y="{:.1}" font-size="{size}" font-weight="{weight}" fill="#222222">{}</text>"##,
            20.0 + 18.0 * i as f64,
            escape(line)
        );
    }
    let _ = write!(
        s,
        r##"<text x="20" y="{:.1}" font-size="12" font-weight="bold" fill="#222222">{}</text><text x="{:.1}" y="{:.1}" font-size="12" font-weight="bold" fill="#222222" text-anchor="end">{}</text>"##,
        title_h - 10.0,
        escape(&style.left_label),
        w - 20.0,
        title_h - 10.0,
        escape(&style.right_label)
    );
    let draw =
        |s: &mut String, t: &TreeLayout, x: &dyn Fn(usize) -> f64, y: &dyn Fn(usize) -> f64| {
            s.push_str(r##"<g fill="none" stroke="#333333" stroke-width="1.2">"##);
            for (v, node) in t.nodes.iter().enumerate() {
                for &c in &node.children {
                    let _ = write!(
                        s,
                        r#"<path d="M{:.2},{:.2} V{:.2} H{:.2}"/>"#,
                        x(v),
                        y(v),
                        y(c),
                        x(c)
                    );
                }
            }
            s.push_str("</g>");
        };
    draw(&mut s, left, &lx_of, &y_l);
    draw(&mut s, right, &rx_of, &y_r);
    let right_y: BTreeMap<String, f64> = right
        .leaves()
        .iter()
        .map(|&l| (right.nodes[l].name.clone().unwrap_or_default(), y_r(l)))
        .collect();
    let left_end = 20.0 + tree_w + label_w;
    let right_start = w - 20.0 - tree_w - label_w;
    for &l in &left.leaves() {
        let name = left.nodes[l].name.clone().unwrap_or_default();
        let y0 = y_l(l);
        let _ = write!(
            s,
            r##"<text x="{:.1}" y="{y0:.1}" font-size="11" fill="#222222" dominant-baseline="central">{}</text>"##,
            20.0 + tree_w + 6.0,
            escape(&name)
        );
        if let Some(&y1) = right_y.get(&name) {
            let (colour, dash, width) = if differ.contains(&name) {
                ("#d55e00", r#" stroke-dasharray="5 3""#, 1.6)
            } else {
                ("#999999", "", 1.0)
            };
            let _ = write!(
                s,
                r#"<line x1="{:.1}" y1="{y0:.1}" x2="{:.1}" y2="{y1:.1}" stroke="{colour}" stroke-width="{width}"{dash}/>"#,
                left_end + 4.0,
                right_start - 4.0
            );
        }
    }
    for &l in &right.leaves() {
        let name = right.nodes[l].name.clone().unwrap_or_default();
        let _ = write!(
            s,
            r##"<text x="{:.1}" y="{:.1}" font-size="11" fill="#222222" text-anchor="end" dominant-baseline="central">{}</text>"##,
            w - 20.0 - tree_w - 6.0,
            y_r(l),
            escape(&name)
        );
    }
    let ly0 = title_h + ROW * n as f64 + 16.0;
    let _ = write!(
        s,
        r##"<line x1="12" y1="{:.1}" x2="42" y2="{:.1}" stroke="#d55e00" stroke-width="1.6" stroke-dasharray="5 3"/><text x="48" y="{:.1}" font-size="10" fill="#555555" dominant-baseline="central">taxon whose closest relatives (smallest clade around it) differ between the trees</text><line x1="12" y1="{:.1}" x2="42" y2="{:.1}" stroke="#999999"/><text x="48" y="{:.1}" font-size="10" fill="#555555" dominant-baseline="central">taxon placed the same way in both trees. Both trees are unrooted, drawn rooted at their midpoints; child order rotated to reduce crossings.</text>"##,
        ly0,
        ly0,
        ly0,
        ly0 + 16.0,
        ly0 + 16.0,
        ly0 + 16.0
    );
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
    fn positions_put_leaves_in_rows_and_parents_between_children() {
        let l = layout("((A,B),(C,D),E);");
        let (x, y) = positions(&l);
        let leaves = l.leaves();
        for (i, &v) in leaves.iter().enumerate() {
            assert_eq!(y[v], i as f64);
            assert_eq!(x[v], 1.0);
        }
        assert_eq!(x[0], 0.0);
        for (v, node) in l.nodes.iter().enumerate() {
            if !node.children.is_empty() {
                let mean =
                    node.children.iter().map(|&c| y[c]).sum::<f64>() / node.children.len() as f64;
                assert_eq!(y[v], mean);
            }
        }
    }

    #[test]
    fn crossings_count_inversions() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(crossings(&s(&["A", "B", "C"]), &s(&["A", "B", "C"])), 0);
        assert_eq!(crossings(&s(&["A", "B", "C"]), &s(&["C", "B", "A"])), 3);
        assert_eq!(
            crossings(&s(&["A", "B", "C", "D"]), &s(&["B", "A", "C", "D"])),
            1
        );
    }

    #[test]
    fn untangling_identical_trees_removes_every_crossing() {
        let mut a = layout("(((A,B),(C,D)),((E,F),(G,H)));");
        let mut b = layout("(((H,G),(F,E)),((D,C),(B,A)));");
        assert_eq!(untangle(&mut a, &mut b), 0);
        // Different trees keep the crossings they need, never more than at
        // the start.
        let mut c = layout("(((A,E),(C,D)),((B,F),(G,H)));");
        let mut d = layout("(((H,G),(F,E)),((D,C),(B,A)));");
        let start = crossings(&leaf_order(&c), &leaf_order(&d));
        assert!(untangle(&mut c, &mut d) <= start);
    }

    #[test]
    fn rectangular_svg_has_names_support_and_groups() {
        let l = layout("((A,B),(C,D),(E,F));");
        let all = l.taxa();
        let mut sup = EdgeSupport {
            label: "S1".into(),
            values: BTreeMap::new(),
        };
        sup.values.insert(
            crate::tree::canonical_split(&["A".to_string(), "B".to_string()], &all),
            0.876,
        );
        let groups = Groups {
            source: "groups.tsv".into(),
            of: [("A", "Alpha"), ("B", "Alpha"), ("C", "Beta")]
                .into_iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        };
        let halo: BTreeMap<String, Option<f64>> =
            all.iter().map(|t| (t.clone(), Some(0.9))).collect();
        let svg = rectangular_svg(
            &l,
            &halo,
            &RectStyle {
                title: vec!["T".into()],
                support: Some(sup),
                second: None,
                groups: Some(groups),
            },
        );
        for t in ["A", "B", "C", "D", "E", "F"] {
            assert!(svg.contains(&format!(">{t}</text>")), "{t}");
        }
        assert!(svg.contains(">0.88</text>"));
        assert!(svg.contains("Alpha") && svg.contains("Groups: groups.tsv"));
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>\n"));
    }

    #[test]
    fn tanglegram_marks_differing_taxa() {
        let a = layout("((A,B),(C,D),(E,F));");
        let b = layout("((A,B),(C,E),(D,F));");
        let differ: BTreeSet<String> = ["C", "D", "E", "F"].iter().map(|s| s.to_string()).collect();
        let svg = tanglegram_svg(
            &a,
            &b,
            &differ,
            &TanglegramStyle {
                title: vec!["T".into()],
                left_label: "Q-MAWS".into(),
                right_label: "Reference".into(),
            },
        );
        // Four dashed lines plus the legend sample.
        assert_eq!(svg.matches("stroke-dasharray").count(), 5);
        assert_eq!(svg.matches(">A</text>").count(), 2);
    }
}
