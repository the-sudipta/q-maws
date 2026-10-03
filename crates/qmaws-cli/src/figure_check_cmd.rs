//! Hidden development command `figure-check`: draws the Halo Tree, the
//! rectangular tree and the interactive page for the taxon names of a
//! downloaded dataset on a random tree with random values, to check that
//! the text stays readable for many taxa (M10 acceptance: 116 taxa of
//! Rhinovirus). The figures say in their title that the tree and the values
//! are random; they are not results.

use qmaws_data::registry::{Layout, Registry};
use qmaws_data::DataDir;
use qmaws_viz::tree::{canonical_split, EdgeSupport, TreeLayout};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

/// SplitMix64 for the random tree and values.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub fn run(dataset: &str, data_dir: &Path, output: &Path, seed: u64) -> ExitCode {
    let reg = Registry::builtin();
    let Some(ds) = reg.dataset(dataset) else {
        eprintln!("Error: unknown dataset {dataset}");
        return ExitCode::from(2);
    };
    let mode = match ds.layout {
        Layout::FilePerTaxon => qmaws_core::input::RecordMode::ConcatenatePerFile,
        Layout::RecordPerTaxon => qmaws_core::input::RecordMode::OneTaxonPerRecord,
    };
    let path = DataDir::new(data_dir).dataset_path(&reg, ds);
    let names: Vec<String> = match qmaws_data::loader::load(&path, mode) {
        Ok(l) => l.taxa.iter().map(|t| t.name.clone()).collect(),
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    let mut rng = Rng(seed);
    // Random stepwise addition of the taxa.
    let mut groups: Vec<String> = names.clone();
    while groups.len() > 3 {
        let i = rng.below(groups.len());
        let a = groups.swap_remove(i);
        let j = rng.below(groups.len());
        let b = groups.swap_remove(j);
        groups.push(format!("({a},{b})"));
    }
    let newick = format!("({});", groups.join(","));
    let layout = match TreeLayout::from_newick(&newick) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    let all = layout.taxa();
    let halo: BTreeMap<String, Option<f64>> = all
        .iter()
        .map(|t| (t.clone(), Some(0.3 + 0.7 * rng.unit())))
        .collect();
    let mut s1 = EdgeSupport {
        label: "S1".into(),
        values: BTreeMap::new(),
    };
    for v in 1..layout.nodes.len() {
        if !layout.nodes[v].children.is_empty() {
            s1.values
                .insert(canonical_split(&layout.clade(v), &all), rng.unit());
        }
    }
    let groups = qmaws_viz::groups::automatic_groups(&layout);
    let m = all.len();
    let title = vec![
        format!("Layout check: {} taxon names of {dataset}", m),
        "RANDOM tree, halo values and support: not a result".to_string(),
    ];
    let size = (600.0 + 6.0 * m as f64).clamp(800.0, 1600.0);
    let svg = qmaws_viz::tree::halo_tree_svg(
        &layout,
        &halo,
        &qmaws_viz::tree::HaloTreeStyle {
            title: title.clone(),
            size,
            support: Some(s1.clone()),
            groups: Some(groups.clone()),
            ..Default::default()
        },
    );
    let rect = qmaws_viz::rect::rectangular_svg(
        &layout,
        &halo,
        &qmaws_viz::rect::RectStyle {
            title: title.clone(),
            support: Some(s1.clone()),
            second: None,
            groups: Some(groups.clone()),
        },
    );
    let html = qmaws_viz::html::interactive_html(&layout, &halo, &[s1], Some(&groups), &title);
    if let Err(e) = std::fs::create_dir_all(output) {
        eprintln!("Error: {e}");
        return ExitCode::from(1);
    }
    let write = |name: &str, bytes: &[u8]| std::fs::write(output.join(name), bytes);
    let width = |svg: &str| -> u32 {
        svg.split("width=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .and_then(|w| w.parse::<f64>().ok())
            .map_or(2400, |w| (w * 300.0 / 96.0).round() as u32)
    };
    let result = (|| -> Result<(), String> {
        write("halo_tree.svg", svg.as_bytes()).map_err(|e| e.to_string())?;
        write("halo_tree.png", &qmaws_viz::render::png(&svg, width(&svg))?)
            .map_err(|e| e.to_string())?;
        write("rectangular_tree.svg", rect.as_bytes()).map_err(|e| e.to_string())?;
        write(
            "rectangular_tree.png",
            &qmaws_viz::render::png(&rect, width(&rect))?,
        )
        .map_err(|e| e.to_string())?;
        write("interactive_tree.html", html.as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            println!("{m} taxa; figures in {}", output.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}
