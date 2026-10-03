//! README files of a run folder. Committed run folders follow the
//! repository rule that every folder has a README listing its items, so
//! the engine writes one into the run folder and each of its committed
//! subfolders (`audit/`, `trees/`, `report/`, `figures/`) that holds files.
//! The `work/` folder is never committed and gets none. READMEs are not
//! part of the root fingerprint.

use std::path::Path;

/// Description of a known item of a run folder.
fn describe(folder: &str, name: &str) -> String {
    if let Some(base) = name.strip_suffix(".sha256") {
        return format!("SHA-256 of `{base}`, written after it (validity check)");
    }
    if name.starts_with("verify_") && name.ends_with(".txt") {
        return "Report of `qmaws verify`: every comparison and the verdict".into();
    }
    let known = match (folder, name) {
        ("", "run.json") => "Configuration, input fingerprints, stage states, chunk plans, throughput, working time",
        ("", "run.log") => "Chronological log in English, UTC timestamps",
        ("", "audit/") => "Verification record",
        ("", "trees/") => "Final trees with support values",
        ("", "report/") => "Tree, matrix export, support, bootstrap and halo tables, verification reports",
        ("", "figures/") => "Figures",
        ("audit", "inputs.json") => "Per taxon: name, source file, lengths, removed symbols, cleaned SHA-256; per input file: path, SHA-256, size; MAW length range",
        ("audit", "stages.json") => "Per stage: content hash, start and end time (UTC), summary numbers; number of quartets, seed",
        ("audit", "chunks.json") => "Per chunk of each chunked stage: range, SHA-256 (weights also rounded to 9 significant digits); per bootstrap replicate: seed, SHA-256 of its tree",
        ("audit", "root.txt") => "Root fingerprint",
        ("audit", "results.json") => "Tree, trees with S1 and S2, weights used, amalgamation summary, S1 and S2 per edge, halo value per taxon",
        ("audit", "quartet_decisions.bin.zst") => "Per quartet in rank order: winning topology and its 16-bit weight (zstd)",
        ("audit", "sample_worksheets.txt") => "Worksheets of 50 quartets drawn with a seed, each checked against the stored weights",
        ("audit", "environment.json") => "Program version and commit, device, settings, seeds",
        ("trees", "tree_s1.nwk") => "The tree with S1 as internal labels",
        ("trees", "tree_s2.nwk") => "The tree with S2 (bootstrap split frequencies) as internal labels",
        ("trees", "bootstrap_trees.nwk") => "The tree of every S2 bootstrap replicate, one per line in replicate order",
        ("report", "tree.nwk") => "The tree of wQFM-rs (Newick, unrooted)",
        ("report", "m_ml.phy") => "M_ml in PHYLIP format",
        ("report", "support.tsv") => "S1 of every internal edge with its weights, number of quartets and clade",
        ("report", "halo.tsv") => "Halo value of every taxon with its weights",
        ("report", "bootstrap.tsv") => "S2 of every internal edge: fraction and number of bootstrap replicates containing its split, clade",
        ("report", "s2_cost.txt") => "Measured cost of S2 bootstrap replicates (development)",
        _ => "",
    };
    if known.is_empty() {
        "Output of the run".into()
    } else {
        known.into()
    }
}

fn purpose(folder: &str) -> &'static str {
    match folder {
        "" => "One analysis run of Q-MAWS: its configuration, log, verification record, trees and reports. Written by the program; the large intermediate files in `work/` are not committed.",
        "audit" => "Small verification record of the run (design: `docs/DESIGN.md`, \"Audit files\"). `qmaws verify` checks the run against it.",
        "trees" => "Final trees of the run.",
        "report" => "Tables and reports of the run.",
        _ => "Output of the run.",
    }
}

/// Writes README.md into the run folder and its committed subfolders that
/// hold files, listing their items.
pub fn write_run_readmes(run_dir: &Path) -> std::io::Result<()> {
    for folder in ["", "audit", "trees", "report", "figures"] {
        let dir = if folder.is_empty() {
            run_dir.to_path_buf()
        } else {
            run_dir.join(folder)
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut items: Vec<String> = Vec::new();
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "README.md" || name == "work" || name.ends_with(".tmp") {
                continue;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                // Only folders that will get a README of their own (they hold files).
                let has_files = std::fs::read_dir(e.path())
                    .map(|r| {
                        r.flatten()
                            .any(|x| x.file_type().map(|t| t.is_file()).unwrap_or(false))
                    })
                    .unwrap_or(false);
                if has_files {
                    items.push(format!("{name}/"));
                }
            } else {
                items.push(name);
            }
        }
        if items.is_empty() {
            continue;
        }
        items.sort();
        let title = if folder.is_empty() {
            run_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "run".into())
        } else {
            folder.to_string()
        };
        let mut text = format!(
            "# {title}\n\n## Purpose\n{}\n\n## Contents\n| Item | Description |\n|---|---|\n",
            purpose(folder)
        );
        for item in &items {
            text.push_str(&format!("| `{item}` | {} |\n", describe(folder, item)));
        }
        text.push_str(
            "\n## Relationships\nWritten by `crates/qmaws-engine` (analysis run and `qmaws verify`).\n\n## Notes\nThis README is written by the program and rewritten when the folder changes.\n",
        );
        std::fs::write(dir.join("README.md"), text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn readmes_list_every_item_of_folders_with_files() {
        let tmp = TempDir::new("readme");
        let run = tmp.path().join("fish_run");
        std::fs::create_dir_all(run.join("audit")).unwrap();
        std::fs::create_dir_all(run.join("figures/live")).unwrap();
        std::fs::create_dir_all(run.join("work/chunks")).unwrap();
        std::fs::write(run.join("run.json"), "{}").unwrap();
        std::fs::write(run.join("audit/root.txt"), "x").unwrap();
        std::fs::write(run.join("audit/root.txt.sha256"), "x").unwrap();
        std::fs::write(run.join("work/chunks/a.bin"), "x").unwrap();
        write_run_readmes(&run).unwrap();
        let top = std::fs::read_to_string(run.join("README.md")).unwrap();
        assert!(top.starts_with("# fish_run\n"));
        assert!(top.contains("| `run.json` |") && top.contains("| `audit/` |"));
        assert!(!top.contains("| `work/` |"));
        assert!(!top.contains("`figures/`"), "empty folders are not listed");
        let audit = std::fs::read_to_string(run.join("audit/README.md")).unwrap();
        assert!(audit.contains("| `root.txt` | Root fingerprint |"));
        assert!(audit.contains("SHA-256 of `root.txt`"));
        assert!(!run.join("work/README.md").exists());
        assert!(!run.join("figures/README.md").exists());
    }
}
