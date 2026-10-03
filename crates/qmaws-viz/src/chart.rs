//! Small-multiple line charts as SVG: one panel per setting, a logarithmic
//! x axis, a linear y axis, line series with markers and dashed reference
//! lines. Colours are from the Okabe–Ito palette (colour-blind safe), and
//! every series also differs by line style and marker shape, so colour is
//! never the only carrier of meaning.

use std::fmt::Write as _;

/// Okabe–Ito blue.
pub const BLUE: &str = "#0072B2";
/// Okabe–Ito orange.
pub const ORANGE: &str = "#E69F00";
/// Okabe–Ito bluish green.
pub const GREEN: &str = "#009E73";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Circle,
    Square,
    Triangle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub label: String,
    pub colour: &'static str,
    pub dashed: bool,
    pub marker: Marker,
    /// (x, y) points; x > 0 (logarithmic axis).
    pub points: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub title: String,
    pub series: Vec<Series>,
}

/// A horizontal reference line across every panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    pub y: f64,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    /// Tick positions on the x axis (also the axis range: first to last).
    pub x_ticks: Vec<f64>,
    /// y axis range.
    pub y_range: (f64, f64),
    pub y_ticks: Vec<f64>,
    pub references: Vec<Reference>,
    pub panels: Vec<Panel>,
}

const PANEL_W: f64 = 260.0;
const PANEL_H: f64 = 220.0;
const GAP: f64 = 60.0;
const LEFT: f64 = 70.0;
const TOP: f64 = 60.0;
const BOTTOM: f64 = 60.0;
const LEGEND_H: f64 = 40.0;

/// Escapes text for SVG.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Number for a tick label: integers with thousands separators, others
/// without trailing zeros.
fn tick_label(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        let s = format!("{}", v as i64);
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn marker(out: &mut String, m: Marker, x: f64, y: f64, colour: &str) {
    let _ = match m {
        Marker::Circle => writeln!(
            out,
            r#"<circle cx="{x:.2}" cy="{y:.2}" r="4" fill="{colour}"/>"#
        ),
        Marker::Square => writeln!(
            out,
            r#"<rect x="{:.2}" y="{:.2}" width="8" height="8" fill="{colour}"/>"#,
            x - 4.0,
            y - 4.0
        ),
        Marker::Triangle => writeln!(
            out,
            r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{colour}"/>"#,
            x,
            y - 5.0,
            x - 5.0,
            y + 4.0,
            x + 5.0,
            y + 4.0
        ),
    };
}

/// The chart as a standalone SVG document.
pub fn to_svg(chart: &Chart) -> String {
    let n = chart.panels.len().max(1) as f64;
    let width = LEFT + n * PANEL_W + (n - 1.0) * GAP + 40.0;
    let height = TOP + PANEL_H + BOTTOM + LEGEND_H;
    let (x0, x1) = (
        chart.x_ticks.first().copied().unwrap_or(1.0).log10(),
        chart.x_ticks.last().copied().unwrap_or(10.0).log10(),
    );
    let (y0, y1) = chart.y_range;
    let px = |left: f64, x: f64| left + (x.log10() - x0) / (x1 - x0) * PANEL_W;
    let py = |y: f64| TOP + (1.0 - (y - y0) / (y1 - y0)) * PANEL_H;
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" font-family="Helvetica, Arial, sans-serif" font-size="12">"#
    );
    let _ = writeln!(out, r#"<rect width="100%" height="100%" fill="white"/>"#);
    let _ = writeln!(
        out,
        r#"<text x="{:.1}" y="24" text-anchor="middle" font-size="15" font-weight="bold">{}</text>"#,
        width / 2.0,
        esc(&chart.title)
    );
    for (i, panel) in chart.panels.iter().enumerate() {
        let left = LEFT + i as f64 * (PANEL_W + GAP);
        let _ = writeln!(
            out,
            r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="13">{}</text>"#,
            left + PANEL_W / 2.0,
            TOP - 10.0,
            esc(&panel.title)
        );
        // Frame, grid and ticks.
        let _ = writeln!(
            out,
            r##"<rect x="{left:.2}" y="{TOP:.2}" width="{PANEL_W:.2}" height="{PANEL_H:.2}" fill="none" stroke="#444" stroke-width="1"/>"##
        );
        for &yt in &chart.y_ticks {
            let y = py(yt);
            let _ = writeln!(
                out,
                r##"<line x1="{left:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="#ddd" stroke-width="0.8"/>"##,
                left + PANEL_W
            );
            if i == 0 {
                let _ = writeln!(
                    out,
                    r#"<text x="{:.2}" y="{:.2}" text-anchor="end">{}</text>"#,
                    left - 6.0,
                    y + 4.0,
                    tick_label(yt)
                );
            }
        }
        for &xt in &chart.x_ticks {
            let x = px(left, xt);
            let _ = writeln!(
                out,
                r##"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="#444" stroke-width="1"/>"##,
                TOP + PANEL_H,
                TOP + PANEL_H + 5.0
            );
            let _ = writeln!(
                out,
                r#"<text x="{x:.2}" y="{:.2}" text-anchor="middle">{}</text>"#,
                TOP + PANEL_H + 18.0,
                tick_label(xt)
            );
        }
        for r in &chart.references {
            let y = py(r.y);
            let _ = writeln!(
                out,
                r##"<line x1="{left:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="#777" stroke-width="1" stroke-dasharray="2 3"/>"##,
                left + PANEL_W
            );
            let _ = writeln!(
                out,
                r##"<text x="{:.2}" y="{:.2}" text-anchor="start" font-size="10" fill="#555">{}</text>"##,
                left + 4.0,
                y - 3.0,
                esc(&r.label)
            );
        }
        for s in &panel.series {
            let pts: Vec<String> = s
                .points
                .iter()
                .map(|&(x, y)| format!("{:.2},{:.2}", px(left, x), py(y)))
                .collect();
            let dash = if s.dashed {
                r#" stroke-dasharray="6 4""#
            } else {
                ""
            };
            let _ = writeln!(
                out,
                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2"{dash}/>"#,
                pts.join(" "),
                s.colour
            );
            for &(x, y) in &s.points {
                marker(&mut out, s.marker, px(left, x), py(y), s.colour);
            }
        }
    }
    // Axis labels.
    let _ = writeln!(
        out,
        r#"<text x="{:.1}" y="{:.1}" text-anchor="middle">{}</text>"#,
        LEFT + (n * PANEL_W + (n - 1.0) * GAP) / 2.0,
        TOP + PANEL_H + 42.0,
        esc(&chart.x_label)
    );
    let _ = writeln!(
        out,
        r#"<text x="18" y="{:.1}" text-anchor="middle" transform="rotate(-90 18 {:.1})">{}</text>"#,
        TOP + PANEL_H / 2.0,
        TOP + PANEL_H / 2.0,
        esc(&chart.y_label)
    );
    // Legend, from the first panel's series.
    if let Some(first) = chart.panels.first() {
        let y = TOP + PANEL_H + BOTTOM + 14.0;
        let mut x = LEFT;
        for s in &first.series {
            let dash = if s.dashed {
                r#" stroke-dasharray="6 4""#
            } else {
                ""
            };
            let _ = writeln!(
                out,
                r#"<line x1="{x:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{}" stroke-width="2"{dash}/>"#,
                x + 30.0,
                s.colour
            );
            marker(&mut out, s.marker, x + 15.0, y, s.colour);
            let _ = writeln!(
                out,
                r#"<text x="{:.2}" y="{:.2}">{}</text>"#,
                x + 38.0,
                y + 4.0,
                esc(&s.label)
            );
            x += 40.0 + 7.0 * s.label.chars().count() as f64 + 30.0;
        }
    }
    out.push_str("</svg>\n");
    out
}

/// The convergence report as a chart (plan 4.7): nRF of each provisional
/// tree to the final tree against the percentage of quartets finished,
/// on linear axes (0 to 100%, 0 to 1).
pub fn convergence_svg(points: &[(f64, f64)], title: &str) -> String {
    let (left, top, pw, ph) = (70.0, 50.0, 520.0, 300.0);
    let (w, h) = (left + pw + 30.0, top + ph + 70.0);
    let px = |x: f64| left + x.clamp(0.0, 100.0) / 100.0 * pw;
    let py = |y: f64| top + (1.0 - y.clamp(0.0, 1.0)) * ph;
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif"><rect width="{w}" height="{h}" fill="#ffffff"/><text x="12" y="24" font-size="15" font-weight="bold" fill="#222222">{}</text>"##,
        esc(title)
    );
    for i in 0..=5 {
        let y = i as f64 / 5.0;
        let _ = write!(
            s,
            r##"<line x1="{left}" y1="{0:.1}" x2="{1:.1}" y2="{0:.1}" stroke="#e5e5e5"/><text x="{2:.1}" y="{3:.1}" font-size="11" fill="#333333" text-anchor="end">{y:.1}</text>"##,
            py(y),
            left + pw,
            left - 6.0,
            py(y) + 4.0
        );
    }
    for i in 0..=5 {
        let x = i as f64 * 20.0;
        let _ = write!(
            s,
            r##"<line x1="{0:.1}" y1="{top}" x2="{0:.1}" y2="{1:.1}" stroke="#e5e5e5"/><text x="{0:.1}" y="{2:.1}" font-size="11" fill="#333333" text-anchor="middle">{x:.0}</text>"##,
            px(x),
            top + ph,
            top + ph + 16.0
        );
    }
    let _ = write!(
        s,
        r##"<rect x="{left}" y="{top}" width="{pw}" height="{ph}" fill="none" stroke="#555555"/><text x="{0:.1}" y="{1:.1}" font-size="12" fill="#222222" text-anchor="middle">Quartets finished (%)</text><text x="18" y="{2:.1}" font-size="12" fill="#222222" text-anchor="middle" transform="rotate(-90 18 {2:.1})">nRF to the final tree</text>"##,
        left + pw / 2.0,
        top + ph + 36.0,
        top + ph / 2.0
    );
    if !points.is_empty() {
        let path: Vec<String> = points
            .iter()
            .map(|&(x, y)| format!("{:.2},{:.2}", px(x), py(y)))
            .collect();
        let _ = write!(
            s,
            r##"<polyline points="{}" fill="none" stroke="#0072b2" stroke-width="2"/>"##,
            path.join(" ")
        );
        for &(x, y) in points {
            let _ = write!(
                s,
                r##"<circle cx="{:.2}" cy="{:.2}" r="3.5" fill="#0072b2"/>"##,
                px(x),
                py(y)
            );
        }
    }
    let _ = writeln!(
        s,
        r##"<text x="12" y="{:.1}" font-size="10" fill="#555555">Each point is a provisional tree from the quartets finished so far (seeded random order); 0 means the final tree.</text></svg>"##,
        h - 10.0
    );
    s
}

#[cfg(test)]
mod convergence_tests {
    use super::*;

    #[test]
    fn convergence_chart_has_one_point_per_tree() {
        let svg = convergence_svg(&[(5.0, 0.6), (50.0, 0.3), (95.0, 0.0)], "Fish <mtDNA>");
        assert_eq!(svg.matches("<circle").count(), 3);
        assert!(svg.contains("Fish &lt;mtDNA&gt;"));
        assert!(convergence_svg(&[], "x").contains("</svg>"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> Chart {
        Chart {
            title: "T & <test>".into(),
            x_label: "x".into(),
            y_label: "y".into(),
            x_ticks: vec![100.0, 1000.0, 10000.0],
            y_range: (0.0, 1.0),
            y_ticks: vec![0.0, 0.5, 1.0],
            references: vec![Reference {
                y: 0.95,
                label: "0.95".into(),
            }],
            panels: vec![
                Panel {
                    title: "a".into(),
                    series: vec![Series {
                        label: "W1".into(),
                        colour: ORANGE,
                        dashed: true,
                        marker: Marker::Square,
                        points: vec![(100.0, 0.0), (10000.0, 1.0)],
                    }],
                },
                Panel {
                    title: "b".into(),
                    series: vec![],
                },
            ],
        }
    }

    #[test]
    fn svg_is_well_formed_and_maps_the_axes() {
        let svg = to_svg(&chart());
        assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>\n"));
        assert!(svg.contains("T &amp; &lt;test&gt;"));
        // x = 100 at the left edge, x = 10,000 at the right; y = 0 at the
        // bottom, y = 1 at the top.
        let bottom = TOP + PANEL_H;
        assert!(svg.contains(&format!(
            "points=\"{:.2},{:.2} {:.2},{:.2}\"",
            LEFT,
            bottom,
            LEFT + PANEL_W,
            TOP
        )));
        assert_eq!(svg.matches("<polyline").count(), 1);
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains(">10,000<"));
        // Every opened element is closed.
        assert_eq!(svg.matches("<svg").count(), svg.matches("</svg>").count());
    }

    #[test]
    fn tick_labels() {
        assert_eq!(tick_label(100000.0), "100,000");
        assert_eq!(tick_label(0.5), "0.5");
        assert_eq!(tick_label(0.95), "0.95");
        assert_eq!(tick_label(1.0), "1");
    }
}
