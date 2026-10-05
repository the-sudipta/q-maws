//! Hidden development command `hgt-trees` (docs/PREREGISTRATION.md,
//! Amendment 4): the gene trees of one simulated genome with horizontal
//! transfer, for AliSim's topology-unlinked partitions. Writes
//! `gene_trees.nwk` (one tree per gene, in gene order), `partitions.nex`
//! (one Jukes–Cantor partition per gene) and `transfers.tsv` (the genes with
//! a transfer, the leaves moved and the leaves below the edge they were
//! attached to).

use qmaws_core::hgt;
use qmaws_core::newick::Tree;
use qmaws_core::weight::SplitMix64;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

pub struct HgtArgs<'a> {
    pub species: &'a Path,
    pub genes: usize,
    pub gene_length: usize,
    pub fraction: f64,
    pub seed: u64,
    pub output: &'a Path,
}

pub fn run(a: HgtArgs) -> ExitCode {
    if !(0.0..=1.0).contains(&a.fraction) || a.genes == 0 || a.gene_length == 0 {
        eprintln!(
            "Error: --fraction must be between 0 and 1, --genes and --gene-length at least 1"
        );
        return ExitCode::from(2);
    }
    let species = match std::fs::read_to_string(a.species)
        .map_err(|e| e.to_string())
        .and_then(|t| Tree::parse(t.trim()).map_err(|e| e.to_string()))
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}: {e}", a.species.display());
            return ExitCode::from(2);
        }
    };
    let mut rng = SplitMix64::new(a.seed);
    let chosen = hgt::chosen_genes(a.genes, a.fraction, &mut rng);
    let species_text = species.to_newick(true);
    let mut trees = String::new();
    let mut table = String::from("gene\tmoved\tattached_above\n");
    let mut next = chosen.iter().peekable();
    for g in 0..a.genes {
        if next.peek() == Some(&&g) {
            next.next();
            let (t, x) = hgt::transferred(&species, &mut rng);
            trees.push_str(&t.to_newick(true));
            let _ = writeln!(
                table,
                "{}\t{}\t{}",
                g + 1,
                x.moved.join(","),
                x.target.join(",")
            );
        } else {
            trees.push_str(&species_text);
        }
        trees.push('\n');
    }
    let mut nex = String::from("#nexus\nbegin sets;\n");
    for g in 0..a.genes {
        let start = g * a.gene_length + 1;
        let _ = writeln!(
            nex,
            "    charset gene_{} = {}-{};",
            g + 1,
            start,
            start + a.gene_length - 1
        );
    }
    let parts: Vec<String> = (1..=a.genes).map(|g| format!("JC:gene_{g}")).collect();
    let _ = writeln!(nex, "    charpartition genes = {};\nend;", parts.join(", "));
    if let Err(e) = std::fs::create_dir_all(a.output) {
        eprintln!("Error: {}: {e}", a.output.display());
        return ExitCode::from(1);
    }
    for (name, text) in [
        ("gene_trees.nwk", &trees),
        ("partitions.nex", &nex),
        ("transfers.tsv", &table),
    ] {
        let p = a.output.join(name);
        if let Err(e) = qmaws_engine::atomic::write_atomic(&p, text.as_bytes()) {
            eprintln!("Error: {}: {e}", p.display());
            return ExitCode::from(1);
        }
    }
    println!(
        "{} genes of {} sites; {} with a transfer (seed {}); written to {}",
        a.genes,
        a.gene_length,
        chosen.len(),
        a.seed,
        a.output.display()
    );
    ExitCode::SUCCESS
}
