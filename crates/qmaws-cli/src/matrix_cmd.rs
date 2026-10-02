//! The `matrix` command: builds `M_full` and `M_ml` for a dataset or folder
//! and writes them with the entropy table and a summary.

use crate::data_cmd::Records;
use qmaws_core::input::RecordMode;
use qmaws_core::matrix;
use qmaws_data::loader;
use qmaws_data::registry::{Layout, Registry};
use qmaws_data::DataDir;
use qmaws_engine::atomic::write_verified;
use qmaws_engine::matrix_pipeline::{build_matrices, entropy_tsv, MatrixOptions};
use std::path::Path;
use std::process::ExitCode;

pub struct MatrixArgs<'a> {
    pub input: Option<&'a Path>,
    pub dataset: Option<&'a str>,
    pub output: &'a Path,
    pub no_strand: bool,
    pub lengths: Option<Vec<usize>>,
    pub records: Records,
    pub data_dir: &'a Path,
    pub quiet: bool,
}

pub fn run(args: MatrixArgs) -> ExitCode {
    let reg = Registry::builtin();
    let data = DataDir::new(args.data_dir);
    let (path, mode) = match (args.input, args.dataset) {
        (Some(p), _) => (p.to_path_buf(), RecordMode::from(args.records)),
        (None, Some(id)) => {
            let Some(ds) = reg.dataset(id) else {
                eprintln!("Error: unknown dataset {id}; run 'qmaws datasets' to see the list");
                return ExitCode::from(2);
            };
            let mode = match ds.layout {
                Layout::FilePerTaxon => RecordMode::ConcatenatePerFile,
                Layout::RecordPerTaxon => RecordMode::OneTaxonPerRecord,
            };
            (data.dataset_path(&reg, ds), mode)
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
    if let Some(f) = loaded.findings.iter().find(|f| f.is_error()) {
        eprintln!("Error: {}", f.message());
        return ExitCode::from(1);
    }
    let warnings: Vec<_> = loaded.findings.iter().filter(|f| f.is_warning()).collect();
    if !warnings.is_empty() {
        for w in &warnings {
            eprintln!("Warning: {}", w.message());
        }
        eprintln!(
            "Error: resolve the warnings first (see qmaws inspect); the matrix is not built."
        );
        return ExitCode::from(1);
    }
    let names: Vec<String> = loaded.taxa.iter().map(|t| t.name.clone()).collect();
    // Hand the sequences over, so they are released after extraction.
    let seqs: Vec<Vec<u8>> = loaded
        .taxa
        .into_iter()
        .map(|t| t.cleaned.sequence)
        .collect();
    let options = MatrixOptions {
        strand: !args.no_strand,
        lengths: args.lengths,
        ..MatrixOptions::default()
    };
    let quiet = args.quiet;
    let log = move |line: String| {
        if !quiet {
            eprintln!("{line}");
        }
    };
    let out = match build_matrices(&names, seqs, &options, &log) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = std::fs::create_dir_all(args.output) {
        eprintln!("Error: could not create {}: {e}", args.output.display());
        return ExitCode::from(1);
    }
    let ml = out.full.select(&out.ml_columns);
    let words = |m: &qmaws_core::matrix::Matrix| {
        let mut s = String::with_capacity(m.lens.iter().map(|&l| l as usize + 1).sum());
        for j in 0..m.columns_len() {
            m.push_word(j, &mut s);
        }
        s
    };
    let full_words = words(&out.full);
    let ml_words = words(&ml);
    let mut summary = serde_json::to_value(&out.summary).expect("summary serialises");
    summary["taxon_names"] = serde_json::json!(names);
    let files = [
        (
            "summary.json",
            serde_json::to_string_pretty(&summary).unwrap() + "\n",
        ),
        ("entropy.tsv", entropy_tsv(&out.summary.entropies)),
        ("m_ml.phy", matrix::to_phylip(&ml, &names)),
        ("m_ml_columns.txt", ml_words),
        ("m_full_columns.txt", full_words),
    ];
    for (name, text) in files {
        let path = args.output.join(name);
        if let Err(e) = write_verified(&path, text.as_bytes()) {
            eprintln!("Error: could not write {}: {e}", path.display());
            return ExitCode::from(1);
        }
    }
    println!(
        "Selected lengths {:?}; M_full {} x {}; M_ml {} x {}; written to {}",
        out.summary.selected_lengths,
        out.summary.taxa,
        out.summary.full_columns,
        out.summary.taxa,
        out.summary.ml_columns,
        args.output.display()
    );
    ExitCode::SUCCESS
}
