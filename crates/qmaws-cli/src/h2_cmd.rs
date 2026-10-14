//! Hidden development command `h2`: the evaluation of hypothesis H2
//! (docs/PREREGISTRATION.md, Amendment 3). Reads `times.tsv`, written by the
//! M13 queue (one row per attempt of a program on a scaling dataset), and
//! compares the wall-clock time of Q-MAWS (a complete run with S1) and of
//! ML-MAWS (IQ-TREE with 1,000 UFBoot included), both on the same computer.
//!
//! Time of a dataset: Q-MAWS = sum of the wall times of all its attempts (a
//! stopped run is resumed, so the attempts add up to one run); ML-MAWS = wall
//! time of its last successful attempt (a stopped ML-MAWS run starts again
//! from the beginning). Success criterion: Q-MAWS faster in every replicate
//! with m >= 100. The crossover m is the smallest m from which the mean
//! Q-MAWS time is lower than the mean ML-MAWS time at that m and at every
//! larger m. Writes `h2_values.tsv`, `h2_test.txt` and `scaling.svg`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

/// The scaling datasets of Amendment 3.
pub fn datasets() -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (m, reps) in [(25, 3), (50, 3), (100, 3), (200, 1)] {
        for r in 1..=reps {
            out.push((format!("scaling_m{m}_r{r}"), m));
        }
    }
    out
}

/// Time and peak memory of both programs on one dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub dataset: String,
    pub taxa: usize,
    pub qmaws_seconds: f64,
    pub qmaws_attempts: usize,
    pub qmaws_peak_mb: Option<f64>,
    pub ml_seconds: f64,
    pub ml_peak_mb: Option<f64>,
}

#[derive(Default)]
struct Acc {
    q_wall: f64,
    q_attempts: usize,
    q_ok: bool,
    q_peak: Option<f64>,
    ml: Option<(f64, Option<f64>)>,
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Builds the pairs from the text of `times.tsv`. Columns (by header name):
/// dataset, program, seconds_wall, exit, and optionally peak_mb.
pub fn pairs(times: &str) -> Result<(Vec<Pair>, Vec<String>), String> {
    let mut lines = times
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header: Vec<&str> = lines
        .next()
        .ok_or("times.tsv is empty")?
        .split('\t')
        .collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (cd, cp, cw, ce) = match (
        col("dataset"),
        col("program"),
        col("seconds_wall"),
        col("exit"),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return Err("times.tsv needs the columns dataset, program, seconds_wall, exit".into()),
    };
    let cm = col("peak_mb");
    let mut acc: BTreeMap<String, Acc> = BTreeMap::new();
    for (i, line) in lines.enumerate() {
        let f: Vec<&str> = line.split('\t').collect();
        let get = |c: usize| f.get(c).copied().unwrap_or("");
        let wall: f64 = get(cw)
            .parse()
            .map_err(|_| format!("times.tsv row {}: bad seconds_wall", i + 2))?;
        let ok = get(ce) == "0";
        let peak = cm.and_then(|c| get(c).parse::<f64>().ok());
        let a = acc.entry(get(cd).to_string()).or_default();
        match get(cp) {
            "Q-MAWS" => {
                a.q_wall += wall;
                a.q_attempts += 1;
                a.q_ok |= ok;
                a.q_peak = max_opt(a.q_peak, peak);
            }
            "ML-MAWS" if ok => a.ml = Some((wall, peak)),
            "ML-MAWS" => {}
            other => return Err(format!("times.tsv row {}: unknown program {other}", i + 2)),
        }
    }
    let mut out = Vec::new();
    let mut missing = Vec::new();
    for (id, m) in datasets() {
        match acc.get(&id) {
            Some(a) if a.q_ok && a.ml.is_some() => {
                let (ml_seconds, ml_peak_mb) = a.ml.unwrap();
                out.push(Pair {
                    dataset: id,
                    taxa: m,
                    qmaws_seconds: a.q_wall,
                    qmaws_attempts: a.q_attempts,
                    qmaws_peak_mb: a.q_peak,
                    ml_seconds,
                    ml_peak_mb,
                });
            }
            Some(a) if !a.q_ok => missing.push(format!("{id}: Q-MAWS not finished")),
            Some(_) => missing.push(format!("{id}: ML-MAWS not finished")),
            None => missing.push(format!("{id}: not started")),
        }
    }
    Ok((out, missing))
}

/// The pre-registered criterion and the crossover.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub supported: bool,
    pub large_replicates: usize,
    pub large_faster: usize,
    pub crossover: Option<usize>,
}

pub fn verdict(pairs: &[Pair]) -> Verdict {
    let large: Vec<&Pair> = pairs.iter().filter(|p| p.taxa >= 100).collect();
    let faster = large
        .iter()
        .filter(|p| p.qmaws_seconds < p.ml_seconds)
        .count();
    let mut sizes: Vec<usize> = pairs.iter().map(|p| p.taxa).collect();
    sizes.sort_unstable();
    sizes.dedup();
    let mean = |m: usize, f: fn(&Pair) -> f64| {
        let v: Vec<f64> = pairs.iter().filter(|p| p.taxa == m).map(f).collect();
        v.iter().sum::<f64>() / v.len() as f64
    };
    let ahead: Vec<bool> = sizes
        .iter()
        .map(|&m| mean(m, |p| p.qmaws_seconds) < mean(m, |p| p.ml_seconds))
        .collect();
    let crossover = (0..sizes.len())
        .find(|&i| ahead[i..].iter().all(|&a| a))
        .map(|i| sizes[i]);
    Verdict {
        supported: !large.is_empty() && faster == large.len(),
        large_replicates: large.len(),
        large_faster: faster,
        crossover,
    }
}

/// Time (log scale) against the number of taxa, one dot per run.
pub fn figure(pairs: &[Pair]) -> String {
    let (w, h, l, r, t, b) = (640.0, 420.0, 80.0, 20.0, 50.0, 60.0);
    let (pw, ph) = (w - l - r, h - t - b);
    let all: Vec<f64> = pairs
        .iter()
        .flat_map(|p| [p.qmaws_seconds, p.ml_seconds])
        .filter(|v| *v > 0.0)
        .collect();
    let lo = all
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min)
        .max(1.0)
        .log10()
        .floor();
    let hi = all
        .iter()
        .cloned()
        .fold(1.0, f64::max)
        .log10()
        .ceil()
        .max(lo + 1.0);
    let x = |m: usize| l + pw * ((m as f64).log2() - 25f64.log2()) / (200f64.log2() - 25f64.log2());
    let y = |s: f64| t + ph - ph * (s.max(1.0).log10() - lo) / (hi - lo);
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" font-family=\"Arial, sans-serif\" font-size=\"12\">\n\
         <rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>\n\
         <text x=\"{}\" y=\"24\" text-anchor=\"middle\" font-size=\"15\" font-weight=\"bold\">Wall-clock time against the number of taxa (same computer)</text>\n",
        w / 2.0
    );
    let mut e = lo as i32;
    while e <= hi as i32 {
        let yy = y(10f64.powi(e));
        let _ = writeln!(
            s,
            "<line x1=\"{l}\" y1=\"{yy:.1}\" x2=\"{}\" y2=\"{yy:.1}\" stroke=\"#ddd\"/><text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">10^{e} s</text>",
            w - r,
            l - 6.0,
            yy + 4.0
        );
        e += 1;
    }
    for m in [25, 50, 100, 200] {
        let _ = writeln!(
            s,
            "<text x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{m}</text>",
            x(m),
            t + ph + 18.0
        );
    }
    let _ = writeln!(
        s,
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">number of taxa m</text>",
        l + pw / 2.0,
        h - 12.0
    );
    for (k, (name, color, f)) in [
        (
            "Q-MAWS (S1)",
            "#4C6EF5",
            (|p: &Pair| p.qmaws_seconds) as fn(&Pair) -> f64,
        ),
        ("ML-MAWS (IQ-TREE, UFBoot 1,000)", "#F08C00", |p: &Pair| {
            p.ml_seconds
        }),
    ]
    .into_iter()
    .enumerate()
    {
        for p in pairs {
            let _ = writeln!(
                s,
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"{color}\" fill-opacity=\"0.75\"><title>{}: {:.0} s</title></circle>",
                x(p.taxa) + if k == 0 { -4.0 } else { 4.0 },
                y(f(p)),
                p.dataset,
                f(p)
            );
        }
        let _ = writeln!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" fill=\"{color}\"/><text x=\"{}\" y=\"{}\">{name}</text>",
            l + 10.0,
            t + 6.0 + 18.0 * k as f64,
            l + 28.0,
            t + 17.0 + 18.0 * k as f64
        );
    }
    s.push_str("</svg>\n");
    s
}

/// Fills the peak memory from `memory.tsv` (columns dataset, program,
/// peak_mb; written beside the queue by `m13_memory.ps1`): the largest value
/// of each dataset and program over its attempts.
pub fn apply_memory(pairs: &mut [Pair], memory: &str) {
    for line in memory.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        let (Some(d), Some(p), Some(Ok(mb))) =
            (f.first(), f.get(1), f.get(2).map(|v| v.parse::<f64>()))
        else {
            continue;
        };
        for pair in pairs.iter_mut().filter(|x| x.dataset == *d) {
            match *p {
                "Q-MAWS" => pair.qmaws_peak_mb = max_opt(pair.qmaws_peak_mb, Some(mb)),
                "ML-MAWS" => pair.ml_peak_mb = max_opt(pair.ml_peak_mb, Some(mb)),
                _ => {}
            }
        }
    }
}

fn fmt_mb(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.0}")).unwrap_or_else(|| "NA".into())
}

pub fn run(dir: &Path) -> ExitCode {
    let path = dir.join("times.tsv");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };
    let (mut pairs, missing) = match pairs(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::from(1);
        }
    };
    if !missing.is_empty() {
        println!(
            "H2 needs all {} datasets; {} are not complete:",
            datasets().len(),
            missing.len()
        );
        for m in &missing {
            println!("  {m}");
        }
        return ExitCode::from(1);
    }
    if let Ok(memory) = std::fs::read_to_string(dir.join("memory.tsv")) {
        apply_memory(&mut pairs, memory.trim_start_matches('\u{feff}'));
    }
    let mut rows = String::from(
        "dataset\ttaxa\tqmaws_seconds\tqmaws_attempts\tqmaws_peak_mb\tml_maws_seconds\tml_maws_peak_mb\tqmaws_faster\n",
    );
    for p in &pairs {
        let _ = writeln!(
            rows,
            "{}\t{}\t{:.0}\t{}\t{}\t{:.0}\t{}\t{}",
            p.dataset,
            p.taxa,
            p.qmaws_seconds,
            p.qmaws_attempts,
            fmt_mb(p.qmaws_peak_mb),
            p.ml_seconds,
            fmt_mb(p.ml_peak_mb),
            if p.qmaws_seconds < p.ml_seconds {
                "yes"
            } else {
                "no"
            }
        );
    }
    let v = verdict(&pairs);
    let text = format!(
        "H2 (Amendment 3): Q-MAWS faster than ML-MAWS in every replicate with m >= 100\n\
         Replicates with m >= 100: {}; Q-MAWS faster in {}\n\
         Crossover m (mean Q-MAWS time lower at this m and every larger m): {}\n\
         H2 {}\n",
        v.large_replicates,
        v.large_faster,
        v.crossover
            .map(|m| m.to_string())
            .unwrap_or_else(|| "none".into()),
        if v.supported {
            "SUPPORTED"
        } else {
            "NOT SUPPORTED"
        }
    );
    for (name, body) in [
        ("h2_values.tsv", rows),
        ("h2_test.txt", text.clone()),
        ("scaling.svg", figure(&pairs)),
    ] {
        if let Err(e) = qmaws_engine::atomic::write_verified(&dir.join(name), body.as_bytes()) {
            eprintln!("Error: {name}: {e}");
            return ExitCode::from(1);
        }
    }
    print!("{text}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(rows: &[(&str, &str, f64, i32)]) -> String {
        let mut s = String::from(
            "dataset\tprogram\tseconds_engine\tseconds_wall\texit\tnote\tdate_utc\tpeak_mb\n",
        );
        for (d, p, w, e) in rows {
            let _ = writeln!(s, "{d}\t{p}\tNA\t{w}\t{e}\tnew\t2026-10-14\t100");
        }
        s
    }

    fn complete(q: impl Fn(usize) -> f64, ml: impl Fn(usize) -> f64) -> String {
        let mut rows = Vec::new();
        let ids = datasets();
        for (id, m) in &ids {
            rows.push((id.as_str(), "Q-MAWS", q(*m), 0));
            rows.push((id.as_str(), "ML-MAWS", ml(*m), 0));
        }
        times(&rows)
    }

    #[test]
    fn there_are_ten_datasets_in_the_amendment_order() {
        let d = datasets();
        assert_eq!(d.len(), 10);
        assert_eq!(d[0], ("scaling_m25_r1".to_string(), 25));
        assert_eq!(d[9], ("scaling_m200_r1".to_string(), 200));
    }

    #[test]
    fn faster_for_large_m_is_supported_with_the_right_crossover() {
        // Q-MAWS slower at 25 and 50, faster from 100.
        let t = complete(|m| 2500.0 + m as f64 * 10.0, |m| (m * m) as f64 * 0.5);
        let (p, missing) = pairs(&t).unwrap();
        assert!(missing.is_empty());
        let v = verdict(&p);
        assert!(v.supported);
        assert_eq!((v.large_replicates, v.large_faster), (4, 4));
        assert_eq!(v.crossover, Some(100));
    }

    #[test]
    fn one_slower_large_replicate_fails_the_criterion() {
        let mut t = complete(|_| 10.0, |_| 20.0);
        t = t.replace(
            "scaling_m200_r1\tQ-MAWS\tNA\t10\t",
            "scaling_m200_r1\tQ-MAWS\tNA\t30\t",
        );
        let (p, _) = pairs(&t).unwrap();
        let v = verdict(&p);
        assert!(!v.supported);
        assert_eq!(v.large_faster, 3);
        assert_eq!(v.crossover, None);
    }

    #[test]
    fn resumed_qmaws_attempts_add_up_and_failed_ml_maws_attempts_are_ignored() {
        let t = times(&[
            ("scaling_m25_r1", "Q-MAWS", 100.0, 1),
            ("scaling_m25_r1", "Q-MAWS", 50.0, 0),
            ("scaling_m25_r1", "ML-MAWS", 999.0, 1),
            ("scaling_m25_r1", "ML-MAWS", 70.0, 0),
        ]);
        let (p, missing) = pairs(&t).unwrap();
        assert_eq!(missing.len(), 9);
        assert_eq!(p.len(), 1);
        assert_eq!(
            (p[0].qmaws_seconds, p[0].qmaws_attempts, p[0].ml_seconds),
            (150.0, 2, 70.0)
        );
    }

    #[test]
    fn memory_rows_give_the_largest_peak_per_program() {
        let (mut p, _) = pairs(&complete(|_| 1.0, |_| 2.0)).unwrap();
        apply_memory(
            &mut p,
            "dataset\tprogram\tpeak_mb\tdate_utc\n\
             scaling_m25_r1\tQ-MAWS\t300\tx\n\
             scaling_m25_r1\tQ-MAWS\t500\tx\n\
             scaling_m25_r1\tML-MAWS\t900\tx\n",
        );
        assert_eq!(
            (p[0].qmaws_peak_mb, p[0].ml_peak_mb),
            (Some(500.0), Some(900.0))
        );
        assert_eq!(p[1].ml_peak_mb, Some(100.0));
    }

    #[test]
    fn the_figure_is_a_complete_svg() {
        let (p, _) = pairs(&complete(|m| m as f64 * 10.0, |m| m as f64 * 20.0)).unwrap();
        let svg = figure(&p);
        assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("<circle").count(), 20);
    }
}
