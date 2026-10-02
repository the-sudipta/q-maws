//! The command `simulate-h3`: the four-taxon long-branch experiment of
//! hypothesis H3 (plan 2.7.6). Writes `recovery.csv` (one row per setting),
//! `replicates.csv` (one row per replicate, with its seed and counts),
//! `recovery.svg` (the figure) and `evaluation.txt` (the pre-registered
//! criterion applied to the results).

use qmaws_core::sim::{self, Replicate};
use qmaws_viz::chart::{self, Chart, Marker, Panel, Reference, Series};
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

pub struct H3Args<'a> {
    pub output: &'a Path,
    pub seed: u64,
    pub replicates: u32,
    pub w2c_replicates: u32,
    pub quiet: bool,
}

/// Summary of one setting.
struct Setting {
    t_long: f64,
    characters: u64,
    replicates: u32,
    mean_columns: f64,
    w1: f64,
    w1_ties: u32,
    w2: f64,
    w2_ties: u32,
    w2c_mean_true: f64,
    w2c: f64,
}

fn summarise(reps: &[Replicate]) -> Setting {
    let n = reps.len() as f64;
    let mean = |f: &dyn Fn(&Replicate) -> f64| reps.iter().map(f).sum::<f64>() / n;
    let w2c_share = |r: &Replicate| match r.w2c {
        Some(w) => {
            let best = w[0].max(w[1]).max(w[2]);
            let k = w.iter().filter(|&&v| v == best).count() as f64;
            if w[0] == best {
                1.0 / k
            } else {
                0.0
            }
        }
        None => 1.0 / 3.0,
    };
    Setting {
        t_long: reps[0].t_long,
        characters: reps[0].characters,
        replicates: reps.len() as u32,
        mean_columns: mean(&|r| r.counts.iter().sum::<u64>() as f64),
        w1: mean(&|r| r.w1_recovery),
        w1_ties: reps
            .iter()
            .filter(|r| r.w1_recovery > 0.0 && r.w1_recovery < 1.0)
            .count() as u32,
        w2: mean(&|r| r.w2_recovery),
        w2_ties: reps
            .iter()
            .filter(|r| r.w2_recovery > 0.0 && r.w2_recovery < 1.0)
            .count() as u32,
        w2c_mean_true: mean(&|r| r.w2c.map_or(1.0 / 3.0, |w| w[0])),
        w2c: mean(&w2c_share),
    }
}

fn csv_float(v: f64) -> String {
    format!("{v:.6}")
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn run(args: H3Args) -> ExitCode {
    match run_inner(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_inner(args: &H3Args) -> Result<(), String> {
    std::fs::create_dir_all(args.output).map_err(|e| format!("{}: {e}", args.output.display()))?;
    let started = Instant::now();
    let mut settings = Vec::new();
    let mut rows = String::from(
        "t_long\tcharacters\treplicate\tseed\tcolumns\tcounts\tw1_scores\tw1_recovery\tw2_log_likelihoods\tw2_recovery\tw2c_weights\n",
    );
    for (ti, &t_long) in sim::LONG_BRANCHES.iter().enumerate() {
        for (ni, &characters) in sim::CHARACTERS.iter().enumerate() {
            let t0 = Instant::now();
            let reps: Vec<Replicate> = (0..args.replicates)
                .map(|r| {
                    let seed = sim::replicate_seed(args.seed, ti as u64, ni as u64, r as u64);
                    sim::run_replicate(t_long, characters, r, seed, args.w2c_replicates)
                })
                .collect();
            for r in &reps {
                let join = |v: &[String]| v.join(",");
                let _ = writeln!(
                    rows,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    r.t_long,
                    r.characters,
                    r.replicate,
                    r.seed,
                    r.counts.iter().sum::<u64>(),
                    join(
                        &r.counts[1..]
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                    ),
                    join(
                        &r.w1_scores
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                    ),
                    csv_float(r.w1_recovery),
                    r.log_likelihoods
                        .map_or("NA".into(), |ll| join(&ll.map(|v| format!("{v:.6}")))),
                    csv_float(r.w2_recovery),
                    r.w2c.map_or("NA".into(), |w| join(&w.map(csv_float))),
                );
            }
            let s = summarise(&reps);
            if !args.quiet {
                println!(
                    "t_long {t_long}, N {characters}: W1 {:.3}, W2 {:.3}, W2c {:.3} ({:.1} s)",
                    s.w1,
                    s.w2,
                    s.w2c,
                    t0.elapsed().as_secs_f64()
                );
            }
            settings.push(s);
        }
    }
    let mut recovery = String::from(
        "t_long\tcharacters\treplicates\tmean_columns_after_removing_0000\tw1_recovery\tw1_tied_replicates\tw2_recovery\tw2_tied_replicates\tw2c_mean_weight_of_true_topology\tw2c_recovery\n",
    );
    for s in &settings {
        let _ = writeln!(
            recovery,
            "{}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.t_long,
            s.characters,
            s.replicates,
            s.mean_columns,
            csv_float(s.w1),
            s.w1_ties,
            csv_float(s.w2),
            s.w2_ties,
            csv_float(s.w2c_mean_true),
            csv_float(s.w2c)
        );
    }
    // Pre-registered criterion: in every setting (t_long) where W1 recovery
    // is below 50% at N = 100,000, W2 recovery is at least 95% at N = 100,000.
    let n_max = *sim::CHARACTERS.last().expect("settings");
    let mut evaluation = format!(
        "H3 criterion: in every setting where W1 recovery is below 50% at N = {n_max} characters, W2 recovery is at least 95% at N = {n_max}.\n\n"
    );
    let mut challenged = 0;
    let mut failed = 0;
    for s in settings.iter().filter(|s| s.characters == n_max) {
        let applies = s.w1 < 0.5;
        let ok = s.w2 >= 0.95;
        if applies {
            challenged += 1;
            if !ok {
                failed += 1;
            }
        }
        let _ = writeln!(
            evaluation,
            "t_long {}: W1 {:.3}, W2 {:.3} -> {}",
            s.t_long,
            s.w1,
            s.w2,
            match (applies, ok) {
                (false, _) => "criterion does not apply (W1 not below 50%)",
                (true, true) => "W2 at least 95%: met",
                (true, false) => "W2 below 95%: NOT met",
            }
        );
    }
    let verdict = if challenged == 0 {
        "No setting has W1 recovery below 50%; the criterion is not challenged (H3 not tested by this experiment)."
    } else if failed == 0 {
        "Supported: the criterion is met in every setting where it applies."
    } else {
        "Not supported: the criterion fails in at least one setting where it applies."
    };
    let _ = writeln!(
        evaluation,
        "\nSettings where the criterion applies: {challenged}; failed: {failed}.\nVerdict: {verdict}"
    );
    let _ = writeln!(
        evaluation,
        "\nGlobal seed {}; {} replicates per setting; {} W2c resamples; program version {}; computed in {:.1} s.",
        args.seed,
        args.replicates,
        args.w2c_replicates,
        env!("CARGO_PKG_VERSION"),
        started.elapsed().as_secs_f64()
    );
    let figure = chart::to_svg(&figure(&settings));
    write(&args.output.join("recovery.csv"), &recovery)?;
    write(&args.output.join("replicates.csv"), &rows)?;
    write(&args.output.join("recovery.svg"), &figure)?;
    write(&args.output.join("evaluation.txt"), &evaluation)?;
    if !args.quiet {
        print!("\n{evaluation}");
        println!(
            "Wrote recovery.csv, replicates.csv, recovery.svg and evaluation.txt to {}",
            args.output.display()
        );
    }
    Ok(())
}

fn figure(settings: &[Setting]) -> Chart {
    let panels = sim::LONG_BRANCHES
        .iter()
        .map(|&t| {
            let of = |f: &dyn Fn(&Setting) -> f64| {
                settings
                    .iter()
                    .filter(|s| s.t_long == t)
                    .map(|s| (s.characters as f64, f(s)))
                    .collect::<Vec<_>>()
            };
            Panel {
                title: format!("long branches a, c: t = {t}"),
                series: vec![
                    Series {
                        label: "W1 (informative-pattern votes)".into(),
                        colour: chart::ORANGE,
                        dashed: true,
                        marker: Marker::Square,
                        points: of(&|s| s.w1),
                    },
                    Series {
                        label: "W2 (conditioned likelihood, W2-sym)".into(),
                        colour: chart::BLUE,
                        dashed: false,
                        marker: Marker::Circle,
                        points: of(&|s| s.w2),
                    },
                ],
            }
        })
        .collect();
    Chart {
        title: "Recovery of the true quartet ab|cd (200 replicates per point)".into(),
        x_label: "characters simulated before removing 0000 (log scale)".into(),
        y_label: "recovery rate".into(),
        x_ticks: sim::CHARACTERS.iter().map(|&n| n as f64).collect(),
        y_range: (0.0, 1.0),
        y_ticks: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        references: vec![
            Reference {
                y: 0.95,
                label: "0.95".into(),
            },
            Reference {
                y: 0.5,
                label: "0.5".into(),
            },
        ],
        panels,
    }
}
