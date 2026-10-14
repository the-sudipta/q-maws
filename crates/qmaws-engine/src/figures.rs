//! Final figures of a finished analysis run (plan 4.7 and 4.8), drawn by
//! `qmaws-viz` from the run's reports:
//!
//! | File in `figures/` | Content |
//! |---|---|
//! | `halo_tree.svg/.pdf/.png` | Quartet Halo Tree: branches coloured and widened by S1 (or S2), halo ring, group bands, title block |
//! | `rectangular_tree.svg/.pdf/.png` | Rectangular cladogram with S1/S2 at the nodes, halo squares and groups |
//! | `tanglegram.svg/.pdf/.png` | Our tree against the reference tree, when there is one, with nRF, nQD and MSD |
//! | `interactive_tree.html` | Self-contained page: zoom, pan, search, tooltips, circular or rectangular layout |
//! | `halo_tree_growth.gif` | The provisional trees and the final tree, when frames exist |
//! | `convergence.svg/.pdf/.png` | nRF of each provisional tree to the final tree |
//! | `groups.tsv` | The groups of the bands and their source |
//!
//! PNG files have 300 dots per inch for the SVG's size read at 96 dots per
//! inch. Groups come from a group file, else from the Open Tree of Life
//! taxonomy when asked for, else from the tree itself. Nothing here is part
//! of the root fingerprint.

use crate::rundir::RunDir;
use crate::runner::{io_err, EngineError};
use crate::state::RunState;
use qmaws_core::newick::Tree;
use qmaws_viz::tree::{canonical_split, EdgeSupport, Groups, TreeLayout};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Branch support shown in the static figures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportChoice {
    S1,
    S2,
}

/// A reference tree for the tanglegram and the comparison of the run.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceTree {
    /// For example `AFproject reference tree (fish_mito)`.
    pub label: String,
    /// Where it came from (a file path or a dataset).
    pub source: String,
    /// The Newick text as given; stored in `audit/reference.nwk`.
    pub newick: String,
    pub tree: Tree,
}

impl ReferenceTree {
    /// Reads a Newick text; the label and source describe it.
    pub fn parse(newick: &str, label: &str, source: &str) -> Result<Self, String> {
        Ok(Self {
            label: label.to_string(),
            source: source.to_string(),
            newick: newick.to_string(),
            tree: Tree::parse(newick.trim()).map_err(|e| e.to_string())?,
        })
    }
}

/// What the figures use besides the run itself.
#[derive(Debug, Clone, PartialEq)]
pub struct FigureOptions {
    pub reference: Option<ReferenceTree>,
    /// A `taxon<TAB>group` file.
    pub groups_file: Option<PathBuf>,
    /// Look up groups in the Open Tree of Life taxonomy (network).
    pub otl: bool,
    /// Taxon of the run to the name used for the Open Tree of Life search
    /// (for example the species name of a benchmark accession).
    pub search_names: BTreeMap<String, String>,
    pub support: SupportChoice,
}

impl Default for FigureOptions {
    fn default() -> Self {
        Self {
            reference: None,
            groups_file: None,
            otl: false,
            search_names: BTreeMap::new(),
            support: SupportChoice::S1,
        }
    }
}

/// Dots per inch of the PNG files.
pub const PNG_DPI: f64 = 300.0;

/// The options found from the run itself: the reference tree and name
/// table of a benchmark dataset in `data_dir`, and a `groups.tsv` in the
/// input folder.
pub fn auto_options(run_dir: &Path, data_dir: &Path) -> FigureOptions {
    let mut o = FigureOptions::default();
    let Ok(state) = RunState::load(&RunDir::new(run_dir).run_json()) else {
        return o;
    };
    let input = state.config["input"].as_str().unwrap_or("").to_string();
    let input_path = PathBuf::from(&input);
    if input_path.is_dir() && input_path.join("groups.tsv").is_file() {
        o.groups_file = Some(input_path.join("groups.tsv"));
    }
    let reg = qmaws_data::registry::Registry::builtin();
    let data = qmaws_data::DataDir::new(data_dir);
    let same = |p: PathBuf| {
        std::path::absolute(&p)
            .is_ok_and(|a| a == std::path::absolute(&input_path).unwrap_or_default())
    };
    if let Some(ds) = reg
        .datasets
        .iter()
        .find(|ds| same(data.dataset_path(&reg, ds)))
    {
        if let Some(r) = qmaws_data::references::reference(&ds.reference) {
            if let Ok(tree) = r.tree() {
                o.reference = Some(ReferenceTree {
                    label: format!("AFproject reference tree ({})", r.id),
                    source: format!(
                        "data/references/{}.nwk (AFproject tree of {}, leaves as sequence ids)",
                        r.id, ds.id
                    ),
                    newick: r.newick.to_string(),
                    tree,
                });
            }
            if let Some(tsv) = r.names_tsv {
                if let Ok(names) = qmaws_data::references::parse_names(tsv) {
                    // Table: published name -> sequence id.
                    o.search_names = names.into_iter().map(|(name, id)| (id, name)).collect();
                }
            }
        }
    }
    // A reference stored in the run (given when the run started, or by an
    // earlier `qmaws figures --reference`) comes first.
    if let Ok(Some(r)) = crate::evaluation::stored_reference(run_dir) {
        o.reference = Some(ReferenceTree {
            label: r.info.label,
            source: r.info.source,
            newick: String::from_utf8_lossy(&r.newick).into_owned(),
            tree: r.tree,
        });
    }
    o
}

/// Files written and remarks for the log.
#[derive(Debug, Clone, Default)]
pub struct Written {
    pub files: Vec<PathBuf>,
    pub notes: Vec<String>,
}

fn read(dir: &Path, rel: &str) -> Result<String, EngineError> {
    let p = dir.join(rel);
    std::fs::read_to_string(&p).map_err(io_err(&p))
}

/// Rows of a TSV report without its header.
fn rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

/// Support by canonical split from a table with the value in column
/// `value` and the clade (comma-separated) in the last column.
fn support_from(text: &str, value: usize, label: &str, all: &[String]) -> EdgeSupport {
    let mut values = BTreeMap::new();
    for r in rows(text) {
        let (Some(v), Some(clade)) = (r.get(value).and_then(|x| x.parse::<f64>().ok()), r.last())
        else {
            continue;
        };
        let clade: Vec<String> = clade.split(',').map(str::to_string).collect();
        values.insert(canonical_split(&clade, all), v);
    }
    EdgeSupport {
        label: label.into(),
        values,
    }
}

fn write_file(path: &Path, bytes: &[u8], out: &mut Written) -> Result<(), EngineError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err(parent))?;
    }
    crate::atomic::write_atomic(path, bytes).map_err(io_err(path))?;
    out.files.push(path.to_path_buf());
    Ok(())
}

/// SVG, PDF and PNG (300 dpi) of one figure.
fn write_formats(
    figures: &Path,
    stem: &str,
    svg: &str,
    out: &mut Written,
) -> Result<(), EngineError> {
    let width = svg_width(svg).unwrap_or(800.0);
    let png = qmaws_viz::render::png(svg, (width * PNG_DPI / 96.0).round() as u32)
        .map_err(EngineError::Invalid)?;
    let pdf = qmaws_viz::render::pdf(svg).map_err(EngineError::Invalid)?;
    write_file(&figures.join(format!("{stem}.svg")), svg.as_bytes(), out)?;
    write_file(&figures.join(format!("{stem}.pdf")), &pdf, out)?;
    write_file(&figures.join(format!("{stem}.png")), &png, out)?;
    Ok(())
}

fn svg_width(svg: &str) -> Option<f64> {
    let start = svg.find("width=\"")? + 7;
    let end = svg[start..].find('"')? + start;
    svg[start..end].parse().ok()
}

/// One line describing the run's settings, for the title blocks.
fn settings_line(state: &RunState) -> String {
    let c = &state.config;
    let weighting = match c["weighting"].as_str().unwrap_or("") {
        "w2_emp" => "W2-emp",
        "none" => "no weighting",
        _ => "W2-sym",
    };
    let lengths = match c["lengths"].as_array() {
        Some(l) if !l.is_empty() => format!(
            "MAW lengths {}",
            l.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => "MAW lengths by entropy".into(),
    };
    format!(
        "{weighting}, W2c {} resamples, S2 {} replicates, strand filter {}, {lengths}, seed {}",
        c["replicates"].as_u64().unwrap_or(0),
        c["bootstrap"].as_u64().unwrap_or(0),
        if c["strand"].as_bool().unwrap_or(true) {
            "on"
        } else {
            "off"
        },
        c["seed"].as_u64().unwrap_or(1)
    )
}

/// Draws every figure of a finished run into `figures/`.
pub fn render(
    run_dir: &Path,
    opts: &FigureOptions,
    log: &mut dyn FnMut(&str),
) -> Result<Written, EngineError> {
    let dir = RunDir::new(run_dir);
    let state = RunState::load(&dir.run_json()).map_err(|error| EngineError::State {
        path: run_dir.to_path_buf(),
        error,
    })?;
    if !run_dir.join("audit").join("root.txt").exists() {
        return Err(EngineError::Invalid(
            "the run has not finished; figures are drawn from a finished run".into(),
        ));
    }
    let mut out = Written::default();
    let figures = dir.figures();
    let newick = read(run_dir, "report/tree.nwk")?;
    let tree = Tree::parse(newick.trim()).map_err(|e| EngineError::Invalid(e.to_string()))?;
    let layout = TreeLayout::from_tree(&tree);
    let all = layout.taxa();
    let m = all.len();
    let halo: BTreeMap<String, Option<f64>> = rows(&read(run_dir, "report/halo.tsv")?)
        .into_iter()
        .filter_map(|r| Some((r.first()?.clone(), r.get(1).and_then(|v| v.parse().ok()))))
        .collect();
    let s1 = support_from(&read(run_dir, "report/support.tsv")?, 2, "S1", &all);
    let s2 = read(run_dir, "report/bootstrap.tsv")
        .ok()
        .map(|t| support_from(&t, 2, "S2", &all))
        .filter(|s| !s.values.is_empty());
    let shown = match (opts.support, &s2) {
        (SupportChoice::S2, Some(s)) => s.clone(),
        (SupportChoice::S2, None) => {
            out.notes
                .push("S2 is not available in this run; branches show S1.".into());
            s1.clone()
        }
        _ => s1.clone(),
    };
    let second = if shown.label == "S1" {
        s2.clone()
    } else {
        Some(s1.clone())
    };
    let quartets = qmaws_core::quartet::quartet_count(m);

    // Groups: a group file, else the Open Tree of Life, else the tree.
    let mut groups: Option<Groups> = None;
    if let Some(file) = &opts.groups_file {
        match std::fs::read_to_string(file) {
            Ok(text) => {
                let (of, problems) = qmaws_viz::groups::read_group_file(&text, &all);
                for p in problems {
                    out.notes.push(format!("{}: {p}", file.display()));
                }
                if of.is_empty() {
                    out.notes
                        .push(format!("{}: no usable groups", file.display()));
                } else {
                    groups = Some(Groups {
                        source: file
                            .file_name()
                            .map_or("group file".into(), |n| n.to_string_lossy().into_owned()),
                        of,
                    });
                }
            }
            Err(e) => out.notes.push(format!(
                "the group file {} cannot be read: {e}",
                file.display()
            )),
        }
    }
    if groups.is_none() && opts.otl {
        log("Looking up the taxa in the Open Tree of Life taxonomy...");
        let taxa: Vec<(String, String)> = all
            .iter()
            .map(|t| {
                let name = opts.search_names.get(t).unwrap_or(t);
                (t.clone(), qmaws_data::otl::search_name(name))
            })
            .collect();
        let fetcher = qmaws_data::download::HttpFetcher::new();
        let date = crate::clock::UtcDateTime::now().iso8601();
        match qmaws_data::otl::taxonomy_groups(&fetcher, &taxa, &date) {
            Ok(t) => {
                let text =
                    serde_json::to_string_pretty(&t.record).expect("record serialises") + "\n";
                write_file(
                    &dir.audit().join("otl_taxonomy.json"),
                    text.as_bytes(),
                    &mut out,
                )?;
                out.notes.push(format!(
                    "Open Tree of Life taxonomy {}: {} of {m} taxa matched, {} without a match, {} ambiguous.",
                    t.taxonomy_version,
                    t.matched,
                    t.unmatched.len(),
                    t.ambiguous.len()
                ));
                match t.rank {
                    Some(rank) => {
                        groups = Some(Groups {
                            source: format!("Open Tree of Life taxonomy {}, {rank}", t.taxonomy_version),
                            of: t.groups,
                        })
                    }
                    None => out.notes.push(
                        "The Open Tree of Life taxonomy gives no rank with 2 to 7 groups; the groups are clades of the tree.".into(),
                    ),
                }
            }
            Err(e) => out.notes.push(format!(
                "The Open Tree of Life could not be used ({e}); the groups are clades of the tree."
            )),
        }
    }
    let groups = groups.unwrap_or_else(|| qmaws_viz::groups::automatic_groups(&layout));
    let mut gtsv = format!("# source: {}\ntaxon\tgroup\n", groups.source);
    for (t, g) in &groups.of {
        gtsv.push_str(&format!("{t}\t{g}\n"));
    }
    write_file(&figures.join("groups.tsv"), gtsv.as_bytes(), &mut out)?;

    let date = state
        .created_utc
        .get(..10)
        .unwrap_or(&state.created_utc)
        .to_string();
    let title = |first: String| -> Vec<String> {
        vec![
            first,
            format!(
                "{m} taxa, {} quartets; {}",
                qmaws_data::loader::group_thousands(quartets),
                settings_line(&state)
            ),
            format!("Run of {date} (UTC), Q-MAWS {}", state.program_version),
        ]
    };

    // Halo Tree.
    let size = (600.0 + 6.0 * m as f64).clamp(800.0, 1600.0);
    let halo_style = qmaws_viz::tree::HaloTreeStyle {
        title: title(format!("Quartet Halo Tree: {}", state.run_id)),
        watermark: None,
        size,
        support: Some(shown.clone()),
        groups: Some(groups.clone()),
    };
    let svg = qmaws_viz::tree::halo_tree_svg(&layout, &halo, &halo_style);
    write_formats(&figures, "halo_tree", &svg, &mut out)?;
    log("Figure: Quartet Halo Tree (SVG, PDF, PNG).");

    // Rectangular tree.
    let rect_style = qmaws_viz::rect::RectStyle {
        title: title(format!("Rectangular tree: {}", state.run_id)),
        support: Some(shown.clone()),
        second: second.clone(),
        groups: Some(groups.clone()),
    };
    let svg = qmaws_viz::rect::rectangular_svg(&layout, &halo, &rect_style);
    write_formats(&figures, "rectangular_tree", &svg, &mut out)?;
    log("Figure: rectangular tree (SVG, PDF, PNG).");

    // Tanglegram against the reference.
    if let Some(reference) = &opts.reference {
        match comparison(&tree, &reference.tree) {
            Ok(c) => {
                let mut left = layout.clone();
                let mut right = TreeLayout::from_tree(&reference.tree);
                let crossings = qmaws_viz::rect::untangle(&mut left, &mut right);
                let header = format!(
                    "nRF {:.3} ({} of {} splits differ); nQD {:.3} ({} of {} quartets differ{}); MSD {} (raw, {m} taxa)",
                    c.nrf,
                    c.rf,
                    2 * m.saturating_sub(3),
                    c.nqd.value,
                    c.nqd.differ,
                    c.nqd.compared,
                    if c.nqd.unresolved_in_reference > 0 {
                        format!("; {} unresolved in the reference, left out", c.nqd.unresolved_in_reference)
                    } else {
                        String::new()
                    },
                    c.msd
                );
                let mut t = title(format!(
                    "Tanglegram: {} against the reference",
                    state.run_id
                ));
                t.insert(1, header.clone());
                let svg = qmaws_viz::rect::tanglegram_svg(
                    &left,
                    &right,
                    &c.differ,
                    &qmaws_viz::rect::TanglegramStyle {
                        title: t,
                        left_label: "Q-MAWS tree".into(),
                        right_label: reference.label.clone(),
                    },
                );
                write_formats(&figures, "tanglegram", &svg, &mut out)?;
                let tsv = format!(
                    "metric\tvalue\tdetail\nnRF\t{:.6}\t{} of {} splits differ\nnQD\t{:.6}\t{} of {} quartets differ; {} unresolved in the reference left out\nMSD\t{}\traw matching split distance for {m} taxa (not normalised)\nreference\t{}\t\ncrossings\t{crossings}\tconnector crossings left in the tanglegram\n",
                    c.nrf,
                    c.rf,
                    2 * m.saturating_sub(3),
                    c.nqd.value,
                    c.nqd.differ,
                    c.nqd.compared,
                    c.nqd.unresolved_in_reference,
                    c.msd,
                    reference.label
                );
                let path = dir.report().join("reference_comparison.tsv");
                crate::atomic::write_verified(&path, tsv.as_bytes()).map_err(io_err(&path))?;
                out.files.push(path);
                log(&format!(
                    "Figure: tanglegram against the {}: {header}.",
                    reference.label
                ));
                // The reference and the comparison, for `qmaws verify`.
                let stored = match crate::evaluation::stored_reference(run_dir) {
                    Ok(Some(s)) if s.newick == reference.newick.as_bytes() => s,
                    _ => {
                        let s = crate::evaluation::store_reference(
                            run_dir,
                            reference.newick.as_bytes(),
                            &reference.label,
                            &reference.source,
                            None,
                        )?;
                        log(&format!(
                            "Reference tree stored in audit/reference.nwk ({}).",
                            reference.source
                        ));
                        s
                    }
                };
                let evaluation = crate::evaluation::evaluate(run_dir, &stored)?;
                crate::evaluation::write(run_dir, &evaluation)?;
            }
            Err(e) => out.notes.push(format!("No tanglegram: {e}")),
        }
    }

    // Interactive page.
    let mut supports = vec![s1.clone()];
    if let Some(s) = &s2 {
        supports.push(s.clone());
    }
    if shown.label == "S2" {
        supports.reverse();
    }
    let page = qmaws_viz::html::interactive_html(
        &layout,
        &halo,
        &supports,
        Some(&groups),
        &title(format!("Quartet Halo Tree: {}", state.run_id)),
    );
    write_file(
        &figures.join("interactive_tree.html"),
        page.as_bytes(),
        &mut out,
    )?;
    log("Figure: interactive tree (HTML).");

    // Convergence and growth animation from the live provisional trees.
    if let Some(p) = crate::provisional::load(&dir) {
        if !p.frames.is_empty() {
            let points: Vec<(f64, f64)> = match read(run_dir, "report/convergence.csv") {
                Ok(text) => text
                    .lines()
                    .skip(1)
                    .filter_map(|l| {
                        let f: Vec<&str> = l.split(',').collect();
                        Some((f.get(1)?.parse().ok()?, f.get(3)?.parse().ok()?))
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
            if !points.is_empty() {
                let svg = qmaws_viz::chart::convergence_svg(
                    &points,
                    &format!("Convergence of the provisional trees: {}", state.run_id),
                );
                write_formats(&figures, "convergence", &svg, &mut out)?;
                log("Figure: convergence of the provisional trees (SVG, PDF, PNG).");
            }
            let mut frames = Vec::new();
            for f in &p.frames {
                if let Ok(bytes) = std::fs::read(crate::provisional::frame_file(&dir, f.frame)) {
                    frames.push(bytes);
                }
            }
            if frames.len() == p.frames.len() {
                // Last frame: the final tree in the style of the frames.
                let style = qmaws_viz::tree::HaloTreeStyle {
                    title: vec![
                        format!("Final Halo Tree: {}", state.run_id),
                        format!("{m} taxa, all {quartets} quartets"),
                    ],
                    size: 800.0,
                    ..Default::default()
                };
                let svg = qmaws_viz::tree::halo_tree_svg(&layout, &halo, &style);
                frames.push(
                    qmaws_viz::render::png(&svg, crate::provisional::FRAME_WIDTH)
                        .map_err(EngineError::Invalid)?,
                );
                match qmaws_viz::anim::gif_from_pngs(&frames, 800, 3000) {
                    Ok(gif) => {
                        write_file(&figures.join("halo_tree_growth.gif"), &gif, &mut out)?;
                        log("Figure: growth animation (GIF).");
                    }
                    Err(e) => out.notes.push(format!("No growth animation: {e}")),
                }
            } else {
                out.notes.push(
                    "No growth animation: the frames in work/provisional/frames/ are missing."
                        .into(),
                );
            }
        }
    }
    crate::readme::write_run_readmes(run_dir).map_err(io_err(run_dir))?;
    Ok(out)
}

/// Our tree against a reference.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub rf: usize,
    pub nrf: f64,
    pub nqd: qmaws_core::metrics::QuartetDistance,
    pub msd: u64,
    /// Taxa whose closest relatives differ: the smallest clade around the
    /// taxon (the smallest side of a split that contains it) is not the
    /// same in the two trees.
    pub differ: BTreeSet<String>,
}

pub fn comparison(ours: &Tree, reference: &Tree) -> Result<Comparison, String> {
    let nqd = qmaws_core::metrics::nqd(ours, reference)?;
    let msd = qmaws_core::metrics::msd(ours, reference)?;
    let (rf, nrf) = qmaws_core::newick::nrf(ours, reference);
    let names = ours.leaf_names();
    let closest = |t: &Tree| -> BTreeMap<String, Vec<String>> {
        let splits = t.splits();
        names
            .iter()
            .map(|x| {
                let best = splits
                    .iter()
                    .map(|s| {
                        if s.contains(x) {
                            s.clone()
                        } else {
                            let mut other: Vec<String> =
                                names.iter().filter(|y| !s.contains(y)).cloned().collect();
                            other.sort();
                            other
                        }
                    })
                    .min_by_key(|side| side.len())
                    .unwrap_or_default();
                (x.clone(), best)
            })
            .collect()
    };
    let (ca, cb) = (closest(ours), closest(reference));
    let differ: BTreeSet<String> = names
        .iter()
        .filter(|x| ca.get(*x) != cb.get(*x))
        .cloned()
        .collect();
    Ok(Comparison {
        rf,
        nrf,
        nqd,
        msd,
        differ,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_of_trees_with_one_moved_taxon() {
        let a = Tree::parse("((A,B),(C,D),(E,F));").unwrap();
        let b = Tree::parse("((A,B),(C,E),(D,F));").unwrap();
        let c = comparison(&a, &b).unwrap();
        assert_eq!(c.rf, 4);
        assert!((c.nrf - 4.0 / 6.0).abs() < 1e-12);
        assert!(c.nqd.differ > 0);
        assert!(c.msd > 0);
        // A and B are sisters in both trees; C, D, E and F are not.
        assert!(!c.differ.contains("A") && !c.differ.contains("B"));
        for x in ["C", "D", "E", "F"] {
            assert!(c.differ.contains(x), "{x}");
        }
        let same = comparison(&a, &a).unwrap();
        assert_eq!((same.rf, same.msd, same.nqd.differ), (0, 0, 0));
        assert!(same.differ.is_empty());
    }

    #[test]
    fn support_tables_are_read_by_split() {
        let all: Vec<String> = ["A", "B", "C", "D", "E"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let s = support_from(
            "edge\tsize\ts1\tclade\n1\t2\t0.8\tA,B\n2\t2\tx\tC,D\n",
            2,
            "S1",
            &all,
        );
        assert_eq!(s.values.len(), 1);
        assert_eq!(
            s.values[&canonical_split(&["A".into(), "B".into()], &all)],
            0.8
        );
    }

    #[test]
    fn svg_width_is_read() {
        assert_eq!(
            svg_width(r#"<svg xmlns="x" width="812.5" height="3">"#),
            Some(812.5)
        );
        assert_eq!(svg_width("<svg>"), None);
    }
}
