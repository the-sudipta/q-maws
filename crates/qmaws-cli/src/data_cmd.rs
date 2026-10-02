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
    let reg = Registry::builtin();
    let selected = match reg.select(selection) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(2);
        }
    };
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
            let mut display = DownloadDisplay::new(mode, color, &d.id);
            let result = qmaws_data::fetch(
                &data,
                &fetcher,
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
    if failures > 0 {
        eprintln!("{failures} problem(s); see the messages above.");
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
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
    if loaded.findings.iter().any(|f| f.is_error()) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
