//! Hidden development command `tree-compare`: nRF, nQD and MSD of any tree
//! (for example an ML-MAWS tree with support labels and branch lengths)
//! against a reference tree, with the same functions as the runs use
//! (plan 2.10). Used for the ML-MAWS runs of H2 and H4.

use qmaws_core::newick::Tree;
use std::path::Path;
use std::process::ExitCode;

fn read(path: &Path) -> Result<Tree, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Tree::parse(text.trim()).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn run(tree: &Path, reference: &Path) -> ExitCode {
    let (t, r) = match (read(tree), read(reference)) {
        (Ok(t), Ok(r)) => (t, r),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("Error: {e}");
            return ExitCode::from(2);
        }
    };
    let c = match qmaws_engine::figures::comparison(&t, &r) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    let m = t.leaf_names().len();
    println!("metric\tvalue\tdetail");
    println!(
        "nRF\t{:.6}\t{} of {} splits differ",
        c.nrf,
        c.rf,
        2 * m.saturating_sub(3)
    );
    println!(
        "nQD\t{:.6}\t{} of {} quartets differ; {} unresolved in the reference left out",
        c.nqd.value, c.nqd.differ, c.nqd.compared, c.nqd.unresolved_in_reference
    );
    println!(
        "MSD\t{}\traw matching split distance for {m} taxa (not normalised)",
        c.msd
    );
    ExitCode::SUCCESS
}
