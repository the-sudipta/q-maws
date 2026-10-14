//! The data commands: `datasets`, `download` and `inspect`.

use qmaws_core::input::RecordMode;
use qmaws_core::newick::{match_names, Tree};
use qmaws_data::download::{HttpFetcher, RetryPolicy};
use qmaws_data::loader::{self, LoadedInput};
use qmaws_data::registry::{Dataset, Layout, Registry};
use qmaws_data::{references, DataDir};
use qmaws_engine::clock::UtcDateTime;
use qmaws_tui::{DisplayMode, DownloadDisplay};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Records {
    /// One taxon per file; several records in a file are joined
    PerFile,
    /// One taxon per FASTA record
    PerRecord,
}

impl From<Records> for RecordMode {
    fn from(r: Records) -> Self {
        match r {
            Records::PerFile => RecordMode::ConcatenatePerFile,
            Records::PerRecord => RecordMode::OneTaxonPerRecord,
        }
    }
}

fn mode_for(ds: &Dataset) -> RecordMode {
    match ds.layout {
        Layout::FilePerTaxon => RecordMode::ConcatenatePerFile,
        Layout::RecordPerTaxon => RecordMode::OneTaxonPerRecord,
    }
}

/// `per_file` or `per_record`: how a dataset's files become taxa.
pub fn records_name(ds: &Dataset) -> &'static str {
    match ds.layout {
        Layout::FilePerTaxon => "per_file",
        Layout::RecordPerTaxon => "per_record",
    }
}

/// Downloads a benchmark dataset when it is missing or its check fails
/// (plan 5.2: "Missing or MD5 mismatch → download, verify MD5, use it").
pub fn ensure_downloaded(id: &str, data_dir: &Path, mode: DisplayMode) -> Result<(), ExitCode> {
    let reg = Registry::builtin();
    let Some(ds) = reg.dataset(id) else {
        eprintln!("Error: unknown dataset {id}");
        return Err(ExitCode::from(crate::EXIT_USAGE));
    };
    let d = reg.download(&ds.download).expect("registry is checked");
    let status = qmaws_data::status(&DataDir::new(data_dir), d);
    if status == qmaws_data::Status::Ready {
        return Ok(());
    }
    if mode == DisplayMode::Normal {
        println!("{id}: {status}; downloading and checking it first.");
    }
    match fetch_selected(id, data_dir, mode, false) {
        Ok(0) => Ok(()),
        Ok(n) => {
            eprintln!("Error: {id}: {n} download problem(s)");
            Err(ExitCode::from(crate::EXIT_ERROR))
        }
        Err(e) => {
            eprintln!("Error: {id}: {e}");
            Err(ExitCode::from(crate::EXIT_ERROR))
        }
    }
}

/// Checks that a reference tree's leaves are exactly the taxa of the input
/// (after the answers to its warnings), as plan 5.4 asks.
pub fn check_reference_names(
    input: &Path,
    records: &str,
    choices: &qmaws_engine::analysis::InputChoices,
    reference: &qmaws_engine::launch::RunReference,
) -> Result<(), String> {
    let mode = if records == "per_record" {
        RecordMode::OneTaxonPerRecord
    } else {
        RecordMode::ConcatenatePerFile
    };
    let mut loaded = loader::load(input, mode).map_err(|e| e.to_string())?;
    loader::apply_choices(&mut loaded, choices.rename_duplicates, &choices.skip);
    let names: Vec<String> = loaded.taxa.iter().map(|t| t.name.clone()).collect();
    let tree =
        qmaws_core::newick::Tree::parse(reference.newick.trim()).map_err(|e| e.to_string())?;
    let m = qmaws_core::newick::match_names(&tree, &names);
    if m.is_exact() {
        return Ok(());
    }
    let mut parts = Vec::new();
    for (label, list) in [
        ("taxa missing in the tree", &m.missing_in_tree),
        ("leaves without a taxon", &m.extra_in_tree),
        ("leaf names used more than once", &m.duplicated_in_tree),
    ] {
        if !list.is_empty() {
            parts.push(format!("{label}: {}", list.join(", ")));
        }
    }
    Err(format!(
        "the reference tree's leaves do not match the taxa ({})",
        parts.join("; ")
    ))
}

/// `qmaws datasets`
pub fn datasets(data_dir: &Path) -> ExitCode {
    let reg = Registry::builtin();
    let data = DataDir::new(data_dir);
    let mut statuses = std::collections::BTreeMap::new();
    println!(
        "{:<20} {:>5}  {:<13}  {:<20}  {:<16}  Name",
        "Dataset", "Taxa", "Size", "Reference tree", "Status"
    );
    for ds in &reg.datasets {
        let d = reg.download(&ds.download).expect("registry is checked");
        let status = statuses
            .entry(d.id.clone())
            .or_insert_with(|| qmaws_data::status(&data, d))
            .clone();
        let reference = if ds.reference.is_empty() {
            "none"
        } else {
            ds.reference.as_str()
        };
        println!(
            "{:<20} {:>5}  {:<13}  {:<20}  {:<16}  {}",
            ds.id,
            ds.taxa,
            d.published_size,
            reference,
            status.to_string(),
            ds.name
        );
    }
    println!();
    println!("Data folder: {}", data.root().display());
    println!("Download with: qmaws download --dataset <id or all>");
    ExitCode::SUCCESS
}

/// `qmaws download`
pub fn download(selection: &str, data_dir: &Path, mode: DisplayMode, color: bool) -> ExitCode {
    match fetch_selected(selection, data_dir, mode, color) {
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(2)
        }
        Ok(0) => ExitCode::SUCCESS,
        Ok(failures) => {
            eprintln!("{failures} problem(s); see the messages above.");
            ExitCode::from(1)
        }
    }
}

/// Downloads, verifies and checks the selected datasets for the menu and
/// the GUI; an error when there was a problem.
pub fn download_for_menu(id: &str, data_dir: &Path, mode: DisplayMode) -> Result<(), String> {
    match fetch_selected(id, data_dir, mode, false)? {
        0 => Ok(()),
        n => Err(format!("{n} problem(s); see the messages in the terminal")),
    }
}

/// Downloads the selection; returns the number of problems.
fn fetch_selected(
    selection: &str,
    data_dir: &Path,
    mode: DisplayMode,
    color: bool,
) -> Result<usize, String> {
    let reg = Registry::builtin();
    let selected = reg.select(selection).map_err(|e| e.to_string())?;
    let data = DataDir::new(data_dir);
    let fetcher = HttpFetcher::new();
    let mut done_downloads = BTreeSet::new();
    let mut failures = 0;
    for ds in &selected {
        let d = reg.download(&ds.download).expect("registry is checked");
        if done_downloads.insert(d.id.clone()) {
            let say = |s: String| {
                if mode == DisplayMode::Normal {
                    println!("{s}");
                }
            };
            say(format!(
                "{}: {} ({}) from {}",
                d.id, d.file_name, d.published_size, d.source
            ));
            let unit = if d.kind == qmaws_data::registry::DownloadKind::Ncbi {
                "records"
            } else {
                "bytes"
            };
            let mut display = DownloadDisplay::with_unit(mode, color, &d.id, unit);
            let result = qmaws_data::fetch(
                &data,
                &fetcher,
                &reg,
                d,
                &UtcDateTime::now().iso8601(),
                RetryPolicy::default(),
                &mut |p| display.update(p.bytes_done, p.total_size),
            );
            display.finish();
            match result {
                Ok(f) => {
                    let c = f.downloaded.checksums();
                    let how = match &f.downloaded {
                        qmaws_data::download::Downloaded::AlreadyPresent(_) => {
                            "already present, checksums verified".to_string()
                        }
                        qmaws_data::download::Downloaded::Fetched {
                            resumed_from: 0, ..
                        } => "downloaded, checksums verified".to_string(),
                        qmaws_data::download::Downloaded::Fetched { resumed_from, .. } => {
                            format!("download resumed at byte {resumed_from}, checksums verified")
                        }
                    };
                    say(format!(
                        "  {how}: {} bytes, MD5 {}, SHA-256 {}{}",
                        c.bytes,
                        c.md5,
                        c.sha256,
                        if f.extracted { "; extracted" } else { "" }
                    ));
                }
                Err(e) => {
                    eprintln!("Error: {}: {e}", d.id);
                    failures += 1;
                    continue;
                }
            }
        }
        if !check_dataset(&reg, &data, ds, mode) {
            failures += 1;
        }
    }
    Ok(failures)
}

/// Reads a downloaded dataset and checks its taxon count and reference
/// tree names. Returns false on a problem.
fn check_dataset(reg: &Registry, data: &DataDir, ds: &Dataset, mode: DisplayMode) -> bool {
    let path = data.dataset_path(reg, ds);
    let loaded = match loader::load(&path, mode_for(ds)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: {}: {e}", ds.id);
            return false;
        }
    };
    let mut ok = true;
    let mut line = format!("{}: {} taxa", ds.id, loaded.taxa.len());
    if loaded.taxa.len() != ds.taxa {
        eprintln!(
            "Error: {}: {} taxa found, but the registry expects {}",
            ds.id,
            loaded.taxa.len(),
            ds.taxa
        );
        ok = false;
    }
    if let Some(r) = references::reference(&ds.reference) {
        let tree = r.tree().expect("committed reference trees parse");
        let names: Vec<String> = loaded.taxa.iter().map(|t| t.name.clone()).collect();
        let m = match_names(&tree, &names);
        if m.is_exact() {
            line.push_str(&format!(
                "; all {} names match the reference tree {}",
                names.len(),
                r.id
            ));
        } else {
            eprintln!(
                "Error: {}: taxon names differ from reference tree {}: missing in tree {:?}, extra in tree {:?}, duplicated {:?}",
                ds.id, r.id, m.missing_in_tree, m.extra_in_tree, m.duplicated_in_tree
            );
            ok = false;
        }
    } else {
        line.push_str("; no reference tree");
    }
    let warnings = loaded.findings.iter().filter(|f| f.is_warning()).count();
    let fixes =
        loaded.findings.len() - warnings - loaded.findings.iter().filter(|f| f.is_error()).count();
    line.push_str(&format!(
        "; {warnings} warning(s), {fixes} automatic name fix(es) (see qmaws inspect --dataset {})",
        ds.id
    ));
    if mode == DisplayMode::Normal {
        println!("  {line}");
    }
    ok
}

fn print_findings(loaded: &LoadedInput) {
    if loaded.findings.is_empty() {
        println!("No problems found.");
        return;
    }
    for f in &loaded.findings {
        let kind = if f.is_error() {
            "Error"
        } else if f.is_warning() {
            "Warning"
        } else {
            "Fixed automatically"
        };
        println!("{kind}: {}", f.message());
        if !f.choices().is_empty() {
            println!("  Choices: {}", f.choices().join(" / "));
        }
    }
}

/// `qmaws inspect`
pub fn inspect(
    input: Option<&Path>,
    dataset: Option<&str>,
    reference: Option<&Path>,
    compare_with: Option<&str>,
    records: Records,
    data_dir: &Path,
) -> ExitCode {
    let reg = Registry::builtin();
    let data = DataDir::new(data_dir);
    let (path, mode, builtin_reference) = match (input, dataset) {
        (Some(p), _) => (p.to_path_buf(), RecordMode::from(records), None),
        (None, Some(id)) => {
            let Some(ds) = reg.dataset(id) else {
                eprintln!("Error: unknown dataset {id}; run 'qmaws datasets' to see the list");
                return ExitCode::from(2);
            };
            (
                data.dataset_path(&reg, ds),
                mode_for(ds),
                references::reference(&ds.reference),
            )
        }
        (None, None) => unreachable!("clap requires --input or --dataset"),
    };
    let loaded = match loader::load(&path, mode) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    print!("{}", loader::summary(&loaded));
    println!();
    print_findings(&loaded);

    let tree = match (reference, builtin_reference) {
        (Some(p), _) => match std::fs::read_to_string(p) {
            Ok(text) => match Tree::parse(text.trim()) {
                Ok(t) => Some((p.display().to_string(), t)),
                Err(e) => {
                    eprintln!("Error: {}: {e}", p.display());
                    return ExitCode::from(1);
                }
            },
            Err(e) => {
                eprintln!("Error: {} could not be read: {e}", p.display());
                return ExitCode::from(1);
            }
        },
        (None, Some(r)) => Some((
            format!("built-in reference {}", r.id),
            r.tree().expect("committed reference trees parse"),
        )),
        (None, None) => None,
    };
    if let Some((label, tree)) = tree {
        let names: Vec<String> = loaded.taxa.iter().map(|t| t.name.clone()).collect();
        let m = match_names(&tree, &names);
        println!();
        if m.is_exact() {
            println!("Reference tree ({label}): all {} names match.", names.len());
        } else {
            println!("Reference tree ({label}): names do not match.");
            if !m.missing_in_tree.is_empty() {
                println!("  Taxa not in the tree: {}", m.missing_in_tree.join(", "));
            }
            if !m.extra_in_tree.is_empty() {
                println!(
                    "  Tree leaves without a taxon: {}",
                    m.extra_in_tree.join(", ")
                );
            }
            if !m.duplicated_in_tree.is_empty() {
                println!(
                    "  Leaves that occur more than once: {}",
                    m.duplicated_in_tree.join(", ")
                );
            }
        }
    }
    if let Some(other_id) = compare_with {
        let Some(other) = reg.dataset(other_id) else {
            eprintln!("Error: unknown dataset {other_id}; run 'qmaws datasets' to see the list");
            return ExitCode::from(2);
        };
        let other_loaded = match loader::load(&data.dataset_path(&reg, other), mode_for(other)) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error: {other_id}: {e}");
                return ExitCode::from(1);
            }
        };
        println!();
        print!("{}", compare(&loaded, &other_loaded, other_id));
    }
    if loaded.findings.iter().any(|f| f.is_error()) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Compares the cleaned sequences of two inputs: which taxa have an
/// identical sequence in the other input, and under which name.
pub fn compare(a: &LoadedInput, b: &LoadedInput, b_label: &str) -> String {
    let mut by_hash: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for (t, h) in b.taxa.iter().zip(&b.cleaned_sha256) {
        by_hash.entry(h.as_str()).or_default().push(t.name.as_str());
    }
    let mut identical = 0;
    let mut renamed = Vec::new();
    let mut unmatched = Vec::new();
    for (t, h) in a.taxa.iter().zip(&a.cleaned_sha256) {
        match by_hash.get(h.as_str()) {
            Some(names) => {
                identical += 1;
                if !names.contains(&t.name.as_str()) {
                    renamed.push(format!("  {} = {}", t.name, names.join(" / ")));
                }
            }
            None => unmatched.push(format!("  {}", t.name)),
        }
    }
    let mut out = format!(
        "Comparison with {b_label}: {identical} of {} taxa have an identical cleaned sequence there ({} taxa in {b_label}).\n",
        a.taxa.len(),
        b.taxa.len()
    );
    if !renamed.is_empty() {
        out.push_str("Identical sequence under another name:\n");
        out.push_str(&renamed.join("\n"));
        out.push('\n');
    }
    if !unmatched.is_empty() {
        out.push_str("No identical sequence:\n");
        out.push_str(&unmatched.join("\n"));
        out.push('\n');
    }
    out
}
