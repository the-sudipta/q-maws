//! The benchmark board (milestone M11): every dataset × seed of the
//! benchmark with its state — finished (with its working time), running (how
//! far, and the time left), stopped, failed, or waiting (with an estimate
//! from the finished runs of the same dataset). Read from the run folders
//! only, so it can be shown while other processes work on the runs; shown
//! by the window (the "M11 progress" page) and written as an HTML page by
//! `qmaws monitor`.

use crate::analysis::{self, AnalysisConfig};
use crate::batch;
use crate::launch::{self, RunSummary};
use std::path::{Path, PathBuf};

/// The datasets of the benchmark, in the order of the M11 queue, and the
/// number of seeds of each. Seed 1 of every dataset except rhinovirus also
/// has the S2 bootstrap (100 replicates); the other seeds have S1 only.
pub const M11_DATASETS: [&str; 14] = [
    "fish_mito",
    "ecoli",
    "ecoli_shigella_hgt",
    "yersinia_hgt",
    "sim_hgt_0",
    "sim_hgt_250",
    "sim_hgt_500",
    "sim_hgt_750",
    "sim_hgt_1000",
    "influenza_a",
    "coronavirus",
    "mammal_mtdna",
    "ebolavirus",
    "rhinovirus",
];
pub const M11_SEEDS: u64 = 5;

/// One dataset of the board.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub taxa: Option<usize>,
    pub cells: Vec<Cell>,
}

/// One run of the board (a dataset and a seed).
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub seed: u64,
    /// The run has the S2 bootstrap.
    pub s2: bool,
    pub state: CellState,
    /// The run folder, when the run exists.
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellState {
    /// Finished: working time in seconds; `earlier` when the run was made
    /// before the benchmark queue (the M10 runs of Fish mtDNA and E. coli).
    Done { seconds: f64, earlier: bool },
    /// Another process is working on it.
    Running {
        percent: f64,
        left_seconds: Option<f64>,
    },
    /// Unfinished and not in use; it continues when its queue reaches it.
    Stopped {
        percent: f64,
        left_seconds: Option<f64>,
    },
    /// The last line of its log is an error.
    Failed { message: String },
    /// Not started; the expected working time from the finished runs of the
    /// same dataset with the same bootstrap setting, if any.
    Waiting { estimate_seconds: Option<f64> },
}

/// The whole board.
#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    pub rows: Vec<Row>,
}

/// Counts of the cells by state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub done: usize,
    pub running: usize,
    pub stopped: usize,
    pub failed: usize,
    pub waiting: usize,
}

impl Counts {
    pub fn total(&self) -> usize {
        self.done + self.running + self.stopped + self.failed + self.waiting
    }
}

impl Board {
    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for cell in self.rows.iter().flat_map(|r| &r.cells) {
            match cell.state {
                CellState::Done { .. } => c.done += 1,
                CellState::Running { .. } => c.running += 1,
                CellState::Stopped { .. } => c.stopped += 1,
                CellState::Failed { .. } => c.failed += 1,
                CellState::Waiting { .. } => c.waiting += 1,
            }
        }
        c
    }

    /// True while a benchmark run is being worked on.
    pub fn active(&self) -> bool {
        self.counts().running > 0
    }

    /// Working time still needed, summed over the runs that are not
    /// finished (runs in parallel workers overlap, so the wall-clock time is
    /// shorter), and whether every such run has an estimate.
    pub fn work_left(&self) -> (f64, bool) {
        let mut sum = 0.0;
        let mut complete = true;
        for cell in self.rows.iter().flat_map(|r| &r.cells) {
            let part = match &cell.state {
                CellState::Done { .. } => Some(0.0),
                CellState::Running { left_seconds, .. }
                | CellState::Stopped { left_seconds, .. } => *left_seconds,
                CellState::Failed { .. } => None,
                CellState::Waiting { estimate_seconds } => *estimate_seconds,
            };
            match part {
                Some(s) => sum += s,
                None => complete = false,
            }
        }
        (sum, complete)
    }
}

/// The run folders of the benchmark: one finished or newest run per dataset
/// and seed, with the primary configuration.
fn runs_by_cell(roots: &[PathBuf]) -> Vec<(String, u64, RunSummary)> {
    let mut found: Vec<(String, u64, RunSummary)> = Vec::new();
    for r in launch::scan(roots) {
        if r.state.kind != analysis::KIND {
            continue;
        }
        let Ok(config) = serde_json::from_value::<AnalysisConfig>(r.state.config.clone()) else {
            continue;
        };
        if !primary(&config) {
            continue;
        }
        let dataset = batch::dataset_name(&r.dir);
        let seed = config.seed;
        match found
            .iter_mut()
            .find(|(d, s, _)| *d == dataset && *s == seed)
        {
            // A finished run wins over an unfinished one.
            Some(slot) if !slot.2.finished && r.finished => slot.2 = r,
            Some(_) => {}
            None => found.push((dataset, seed, r)),
        }
    }
    found
}

/// The primary configuration of the benchmark (any seed, with or without
/// S2): W2-sym weights with W2c, automatic MAW lengths, the strand filter,
/// the full matrix.
fn primary(c: &AnalysisConfig) -> bool {
    c.weighting == analysis::WEIGHTING_SYM
        && c.strand
        && c.lengths.is_none()
        && c.top_lengths == qmaws_core::matrix::TOP_K
        && c.matrix == analysis::MATRIX_FULL
}

/// The last non-empty line of a run's log, if it reports an error.
fn error_line(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("run.log")).ok()?;
    let last = text.lines().rev().find(|l| !l.trim().is_empty())?;
    let lower = last.to_ascii_lowercase();
    (lower.contains("error") || lower.contains("failed")).then(|| last.to_string())
}

/// The board of the M11 benchmark from the run folders under `roots`.
pub fn m11_board(roots: &[PathBuf]) -> Board {
    let registry = qmaws_data::registry::Registry::builtin();
    let runs = runs_by_cell(roots);
    let mut rows: Vec<Row> = M11_DATASETS
        .iter()
        .map(|&id| {
            let ds = registry.dataset(id);
            Row {
                id: id.to_string(),
                name: ds.map_or_else(|| id.to_string(), |d| short_name(&d.name)),
                taxa: ds.map(|d| d.taxa),
                cells: Vec::new(),
            }
        })
        .collect();
    // Working times of the finished runs per dataset and S2 setting.
    let finished_seconds = |id: &str, s2: bool| -> Vec<f64> {
        runs.iter()
            .filter(|(d, _, r)| d == id && r.finished && has_s2(r) == s2)
            .map(|(_, _, r)| r.state.elapsed_seconds)
            .collect()
    };
    for row in &mut rows {
        for seed in 1..=M11_SEEDS {
            let s2 = seed == 1 && row.id != "rhinovirus";
            let run = runs
                .iter()
                .find(|(d, s, _)| *d == row.id && *s == seed)
                .map(|(_, _, r)| r);
            let state = match run {
                Some(r) if r.finished => CellState::Done {
                    seconds: r.state.elapsed_seconds,
                    earlier: !dir_name(&r.dir).ends_with(&format!("_seed{seed}")),
                },
                Some(r) => {
                    let p = analysis::run_progress(&r.dir, &r.state);
                    if launch::in_use(&r.dir) {
                        CellState::Running {
                            percent: p.percent,
                            left_seconds: p.remaining_seconds,
                        }
                    } else if let Some(message) = error_line(&r.dir) {
                        CellState::Failed { message }
                    } else {
                        CellState::Stopped {
                            percent: p.percent,
                            left_seconds: p.remaining_seconds,
                        }
                    }
                }
                None => {
                    let times = finished_seconds(&row.id, s2);
                    CellState::Waiting {
                        estimate_seconds: (!times.is_empty())
                            .then(|| times.iter().sum::<f64>() / times.len() as f64),
                    }
                }
            };
            row.cells.push(Cell {
                seed,
                s2,
                state,
                dir: run.map(|r| r.dir.clone()),
            });
        }
    }
    Board { rows }
}

fn has_s2(r: &RunSummary) -> bool {
    serde_json::from_value::<AnalysisConfig>(r.state.config.clone()).is_ok_and(|c| c.bootstrap > 0)
}

fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The dataset name without the part in brackets ("E. coli/Shigella HGT
/// (27 genomes)" → "E. coli/Shigella HGT").
fn short_name(name: &str) -> String {
    name.split(" (").next().unwrap_or(name).trim().to_string()
}

/// "45 s", "12 min", "3 h 05", "1.4 d".
pub fn short_time(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s < 60 {
        format!("{s} s")
    } else if s < 3600 {
        format!("{} min", (s + 30) / 60)
    } else if s < 86_400 {
        let m = (s + 30) / 60;
        format!("{} h {:02}", m / 60, m % 60)
    } else {
        format!("{:.1} d", s as f64 / 86_400.0)
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The board as a self-contained HTML page that reloads itself every
/// `reload_seconds`; `written` is the local time of writing.
pub fn html(board: &Board, written: &str, reload_seconds: u64) -> String {
    let c = board.counts();
    let total = c.total().max(1);
    let (left, complete) = board.work_left();
    let pct = |n: usize| 100.0 * n as f64 / total as f64;
    let mut grid = String::new();
    for row in &board.rows {
        grid.push_str(&format!(
            "<div class=\"ds\">{}<span>{}</span></div>",
            escape(&row.name),
            row.taxa.map_or(String::new(), |t| format!("{t} taxa"))
        ));
        for cell in &row.cells {
            let s2 = if cell.s2 { " · S2" } else { "" };
            let (class, top, sub, title) = match &cell.state {
                CellState::Done { seconds, earlier } => (
                    if *earlier { "m10" } else { "done" },
                    format!("&#10003; {}", short_time(*seconds)),
                    format!("{}{s2}", if *earlier { "M10 run" } else { "done" }),
                    format!(
                        "finished; working time {} on this computer{}",
                        short_time(*seconds),
                        if *earlier {
                            " (made in milestone M10)"
                        } else {
                            ""
                        }
                    ),
                ),
                CellState::Running {
                    percent,
                    left_seconds,
                } => (
                    "run",
                    format!("&#9654; {percent:.0}%"),
                    left_seconds.map_or("left: estimating".into(), |l| {
                        format!("&asymp;{} left", short_time(l))
                    }),
                    format!(
                        "running: {percent:.1}% done; {}",
                        left_seconds.map_or("time left: estimating".into(), |l| format!(
                            "about {} of work left (measured speed)",
                            short_time(l)
                        ))
                    ),
                ),
                CellState::Stopped {
                    percent,
                    left_seconds,
                } => (
                    "stop",
                    format!("&#10074;&#10074; {percent:.0}%"),
                    left_seconds.map_or("stopped".into(), |l| {
                        format!("&asymp;{} left", short_time(l))
                    }),
                    "stopped; continues from its last checkpoint when its queue reaches it"
                        .to_string(),
                ),
                CellState::Failed { message } => (
                    "fail",
                    "&#10007; failed".to_string(),
                    "see run.log".to_string(),
                    message.clone(),
                ),
                CellState::Waiting { estimate_seconds } => (
                    "wait",
                    estimate_seconds
                        .map_or("waiting".into(), |e| format!("&asymp;{}", short_time(e))),
                    if cell.s2 { "S2".into() } else { String::new() },
                    estimate_seconds.map_or(
                        "waiting; no estimate until this dataset has a finished run like it".into(),
                        |e| {
                            format!(
                                "waiting; about {} (mean of this dataset's finished runs)",
                                short_time(e)
                            )
                        },
                    ),
                ),
            };
            grid.push_str(&format!(
                "<div class=\"c {class}\" title=\"Seed {}{}: {}\">{top}<small>{sub}</small></div>",
                cell.seed,
                if cell.s2 { ", with S2" } else { "" },
                escape(&title)
            ));
        }
    }
    let left_text = if c.done == c.total() {
        "none".to_string()
    } else if complete {
        format!("&asymp; {}", short_time(left))
    } else {
        format!("at least {}", short_time(left))
    };
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="refresh" content="{reload_seconds}">
<title>M11 progress</title>
<style>
:root{{--bg:#f5f4ef;--card:#fff;--ink:#16222f;--soft:#5b6673;--faint:#8a939d;--line:#e2ded3;
--done:#e3f1e8;--done-t:#1f6b3f;--m10:#e4eefa;--m10-t:#1d4f86;--run:#fbefd9;--run-t:#7a4b07;--wait:#efede6;--wait-t:#5b6673;--stop:#ece6f6;--stop-t:#4b3a7a;--fail:#fae3e1;--fail-t:#8b2a20}}
@media (prefers-color-scheme: dark){{:root{{--bg:#0f1720;--card:#16212d;--ink:#e8edf2;--soft:#a3afbb;--faint:#7d8996;--line:#26323f;
--done:#173a28;--done-t:#9fe0b7;--m10:#16304d;--m10-t:#a9cdf5;--run:#3d2d0d;--run-t:#f6c76e;--wait:#1d2733;--wait-t:#a3afbb;--stop:#2a2342;--stop-t:#c9bdf2;--fail:#45201c;--fail-t:#f3aaa1}}}}
body{{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 "Segoe UI",system-ui,sans-serif}}
main{{max-width:1080px;margin:0 auto;padding:28px 20px 40px}}
h1{{font-size:22px;font-weight:500;margin:0}} .sub{{color:var(--soft);margin:2px 0 18px}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;margin-bottom:14px}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:12px 16px}}
.card .l{{color:var(--faint);font-size:13px}} .card .v{{font-size:24px;font-weight:500}} .card .v small{{font-size:13px;color:var(--soft);font-weight:400}}
.bar{{height:9px;border-radius:5px;background:var(--wait);overflow:hidden;display:flex;margin:4px 0 8px}}
.legend{{color:var(--soft);font-size:12.5px;margin-bottom:14px}} .sw{{display:inline-block;width:11px;height:11px;border-radius:3px;vertical-align:-1px;margin:0 5px 0 12px}}
.grid{{display:grid;grid-template-columns:190px repeat({seeds},minmax(0,1fr));gap:5px;background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px}}
.h{{color:var(--faint);font-size:12.5px;padding:0 8px}} .ds{{font-size:14px;padding:6px 4px 0}} .ds span{{display:block;color:var(--faint);font-size:11.5px}}
.c{{border-radius:7px;padding:6px 9px;font-size:13px;line-height:1.35;min-height:38px}} .c small{{display:block;font-size:11.5px;opacity:.85}}
.done{{background:var(--done);color:var(--done-t)}} .m10{{background:var(--m10);color:var(--m10-t)}} .run{{background:var(--run);color:var(--run-t);font-weight:500}}
.wait{{background:var(--wait);color:var(--wait-t)}} .stop{{background:var(--stop);color:var(--stop-t)}} .fail{{background:var(--fail);color:var(--fail-t)}}
.note{{color:var(--faint);font-size:12.5px;margin-top:14px}}
</style></head><body><main>
<h1>M11 progress</h1>
<p class="sub">Q-MAWS benchmark: {datasets} datasets &times; {seeds} seeds, primary configuration. Updated {written} (this computer's time).</p>
<div class="cards">
<div class="card"><div class="l">Finished</div><div class="v">{done} <small>of {total}</small></div></div>
<div class="card"><div class="l">Running</div><div class="v">{running}</div></div>
<div class="card"><div class="l">Waiting</div><div class="v">{waiting}{stopped_text}</div></div>
<div class="card"><div class="l">Failed</div><div class="v">{failed}</div></div>
<div class="card"><div class="l">Work left (sum of runs)</div><div class="v" style="font-size:20px;padding-top:4px">{left_text}</div></div>
</div>
<div class="bar"><i style="width:{p_done:.2}%;background:var(--done-t)"></i><i style="width:{p_run:.2}%;background:var(--run-t)"></i></div>
<div class="legend">{done_pct:.0}% of the runs finished
<span class="sw" style="background:var(--m10)"></span>Done in M10<span class="sw" style="background:var(--done)"></span>Done<span class="sw" style="background:var(--run)"></span>Running<span class="sw" style="background:var(--stop)"></span>Stopped<span class="sw" style="background:var(--wait)"></span>Waiting<span class="sw" style="background:var(--fail)"></span>Failed
&nbsp;&middot;&nbsp; S2: with the 100-replicate bootstrap &nbsp;&middot;&nbsp; &asymp;: estimate</div>
<div class="grid"><div></div>{seed_heads}{grid}</div>
<p class="note">Times are working times on this computer, read from each run's records. Estimates: a running run from its measured speed; a waiting run from the mean of the finished runs of the same dataset with the same bootstrap setting; none before the first such run. Runs of two parallel workers overlap, so the wall-clock time left is shorter than the sum. Point at a cell for details. This page is rewritten every minute by <code>qmaws monitor</code> and reloads itself every {reload_seconds} s; if the time above is old, the updater is not running.</p>
</main></body></html>
"##,
        seeds = M11_SEEDS,
        datasets = board.rows.len(),
        done = c.done,
        total = c.total(),
        running = c.running,
        waiting = c.waiting,
        stopped_text = if c.stopped > 0 {
            format!(" <small>+ {} stopped</small>", c.stopped)
        } else {
            String::new()
        },
        failed = c.failed,
        p_done = pct(c.done),
        p_run = pct(c.running),
        done_pct = pct(c.done),
        seed_heads = (1..=M11_SEEDS)
            .map(|s| format!("<div class=\"h\">Seed {s}</div>"))
            .collect::<String>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(state: CellState) -> Cell {
        Cell {
            seed: 1,
            s2: false,
            state,
            dir: None,
        }
    }

    #[test]
    fn counts_and_work_left() {
        let board = Board {
            rows: vec![Row {
                id: "x".into(),
                name: "X".into(),
                taxa: Some(4),
                cells: vec![
                    cell(CellState::Done {
                        seconds: 100.0,
                        earlier: false,
                    }),
                    cell(CellState::Running {
                        percent: 50.0,
                        left_seconds: Some(60.0),
                    }),
                    cell(CellState::Waiting {
                        estimate_seconds: Some(120.0),
                    }),
                    cell(CellState::Waiting {
                        estimate_seconds: None,
                    }),
                ],
            }],
        };
        let c = board.counts();
        assert_eq!((c.done, c.running, c.waiting, c.total()), (1, 1, 2, 4));
        assert!(board.active());
        assert_eq!(board.work_left(), (180.0, false));
        let page = html(&board, "Thu 08 Oct, 09:00", 60);
        assert!(page.contains("content=\"60\""));
        assert!(page.contains("at least 3 min"));
        assert!(page.contains("1 <small>of 4</small>"));
    }

    #[test]
    fn short_names_and_times() {
        assert_eq!(
            short_name("E. coli/Shigella HGT (27 genomes)"),
            "E. coli/Shigella HGT"
        );
        assert_eq!(short_time(45.0), "45 s");
        assert_eq!(short_time(600.0), "10 min");
        assert_eq!(short_time(11_100.0), "3 h 05");
        assert_eq!(short_time(121_000.0), "1.4 d");
    }
}
