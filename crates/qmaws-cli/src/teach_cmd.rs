//! The `teach` command: the hand-calculable worksheet.

use qmaws_core::input::RecordMode;
use qmaws_core::newick::{match_names, Tree};
use qmaws_core::teach;
use qmaws_data::loader;
use std::path::Path;
use std::process::ExitCode;

pub fn run(example: bool, input: Option<&Path>, reference: Option<&Path>) -> ExitCode {
    let sheet = if example {
        teach::example()
    } else {
        let Some(path) = input else {
            eprintln!("Error: give --example or --input <folder>");
            return ExitCode::from(2);
        };
        let loaded = match loader::load(path, RecordMode::ConcatenatePerFile) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::from(1);
            }
        };
        // Short sequences are normal in a worksheet; every other error or
        // warning must be resolved first.
        let blocking: Vec<_> = loaded
            .findings
            .iter()
            .filter(|f| f.is_error() || f.is_warning())
            .filter(|f| !matches!(f, qmaws_core::input::Finding::ShortSequence { .. }))
            .collect();
        if !blocking.is_empty() {
            for f in blocking {
                eprintln!("Error: {}", f.message());
            }
            return ExitCode::from(1);
        }
        let mut taxa: Vec<(String, String)> = loaded
            .taxa
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    String::from_utf8_lossy(&t.cleaned.sequence).into_owned(),
                )
            })
            .collect();
        taxa.sort_by(|a, b| a.0.cmp(&b.0));
        let tree = match reference {
            Some(p) => match std::fs::read_to_string(p).map(|t| Tree::parse(t.trim())) {
                Ok(Ok(t)) => {
                    let names: Vec<String> = taxa.iter().map(|t| t.0.clone()).collect();
                    let m = match_names(&t, &names);
                    if !m.is_exact() {
                        eprintln!(
                            "Error: the reference tree's leaves do not match the taxa (missing {:?}, extra {:?})",
                            m.missing_in_tree, m.extra_in_tree
                        );
                        return ExitCode::from(1);
                    }
                    Some(t)
                }
                Ok(Err(e)) => {
                    eprintln!("Error: {}: {e}", p.display());
                    return ExitCode::from(1);
                }
                Err(e) => {
                    eprintln!("Error: {} could not be read: {e}", p.display());
                    return ExitCode::from(1);
                }
            },
            None => None,
        };
        match teach::worksheet(&taxa, tree.as_ref(), None, false) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::from(1);
            }
        }
    };
    print!("{}", sheet.text);
    ExitCode::SUCCESS
}
