//! The batch progress view (plan 5.5.1): when several runs execute as one
//! queue (`qmaws run --dataset all`, `qmaws resume --all`, a queue in the
//! window), the user sees the whole batch, not only the current run.
//!
//! [`Batch`] is the one model behind the three places that show it: the
//! window, the terminal ([`Batch::terminal`]) and the offline page
//! `batch_status.html` ([`Batch::html`]), a single self-contained file that
//! reloads itself every 30 seconds. It is read from the runs' `run.json`
//! files, so it survives a stop and a resume of the queue.
//!
//! Estimates: a finished run counts with its working time; the running run
//! with its working time plus the engine's estimate of the time left; a run
//! not started yet with the mean working time of the finished runs of the
//! same dataset (and the same S2 setting), or, before any, by scaling the
//! time per quartet of the finished runs to its number of quartets
//! (labelled "rough"). Every time shown is an estimate and says so.

use crate::clock::UtcDateTime;
use crate::launch::RunSummary;
use crate::progress::Snapshot;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// File name of the offline status page, written next to the run folders.
pub const STATUS_FILE: &str = "batch_status.html";
/// Seconds between two automatic reloads of the status page.
pub const REFRESH_SECONDS: u64 = 30;

/// Where a run of the batch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Running,
    Finished,
    Stopped,
    Failed,
}

impl Phase {
    /// Letter for plain text.
    pub fn letter(self) -> char {
        match self {
            Phase::Queued => 'Q',
            Phase::Running => 'R',
            Phase::Finished => 'F',
            Phase::Stopped => 'S',
            Phase::Failed => 'X',
        }
    }

    /// Symbol for the terminal and the page.
    pub fn symbol(self) -> &'static str {
        match self {
            Phase::Queued => "\u{00b7}",
            Phase::Running => "\u{25b6}",
            Phase::Finished => "\u{2713}",
            Phase::Stopped => "\u{2717}",
            Phase::Failed => "\u{2717}",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Phase::Queued => "queued",
            Phase::Running => "running",
            Phase::Finished => "finished",
            Phase::Stopped => "stopped",
            Phase::Failed => "failed",
        }
    }
}

/// One run of the batch.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub dir: PathBuf,
    /// Dataset (row of the grid).
    pub dataset: String,
    /// Seed (column of the grid), when known.
    pub seed: Option<u64>,
    /// The run includes the S2 bootstrap.
    pub s2: bool,
    /// Number of taxa, when known (for rough estimates).
    pub taxa: Option<usize>,
    pub phase: Phase,
    /// Fraction done, 0 to 1.
    pub done: f64,
    /// Working time so far, in seconds.
    pub elapsed: f64,
    /// Expected total working time, in seconds.
    pub expected: Option<f64>,
    /// The expectation is a rough size-based estimate.
    pub rough: bool,
}

/// The current run's details.
#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    pub index: usize,
    pub stage: String,
    pub stage_fraction: f64,
    pub remaining: Option<f64>,
}

/// The state of a queue of runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub entries: Vec<Entry>,
    pub current: Option<Current>,
}

/// The dataset of a run folder: its name without a `_seed<n>` or a
/// `_<date>_<time>` suffix.
pub fn dataset_name(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(i) = name.rfind("_seed") {
        if name[i + 5..].chars().all(|c| c.is_ascii_digit()) && name.len() > i + 5 {
            return name[..i].to_string();
        }
    }
    // `<name>_YYYY-MM-DD_HHMMSS` from the default output folder.
    let bytes = name.as_bytes();
    if name.len() > 18 {
        let tail = &name[name.len() - 18..];
        let t = tail.as_bytes();
        let digits = |r: std::ops::Range<usize>| t[r].iter().all(u8::is_ascii_digit);
        if t[0] == b'_'
            && digits(1..5)
            && t[5] == b'-'
            && digits(6..8)
            && t[8] == b'-'
            && digits(9..11)
            && t[11] == b'_'
            && digits(12..18)
        {
            let _ = bytes;
            return name[..name.len() - 18].to_string();
        }
    }
    name
}

fn quartets(m: usize) -> f64 {
    if m < 4 {
        return 0.0;
    }
    let m = m as f64;
    m * (m - 1.0) * (m - 2.0) * (m - 3.0) / 24.0
}

impl Batch {
    /// The batch of `dirs` (in queue order). `running` is the index of the
    /// run in progress; `failed` lists runs that ended with an error;
    /// `stopped` the run that was stopped, if any.
    pub fn read(
        dirs: &[PathBuf],
        running: Option<usize>,
        stopped: Option<usize>,
        failed: &[usize],
    ) -> Self {
        let mut entries: Vec<Entry> = dirs
            .iter()
            .enumerate()
            .map(|(i, dir)| {
                let summary = RunSummary::read(dir);
                let (seed, s2, taxa) = summary
                    .as_ref()
                    .and_then(|s| {
                        serde_json::from_value::<crate::analysis::AnalysisConfig>(
                            s.state.config.clone(),
                        )
                        .ok()
                        .map(|c| {
                            (
                                Some(c.seed),
                                c.bootstrap > 0,
                                Some(s.state.inputs.len()).filter(|&n| n > 0),
                            )
                        })
                    })
                    .unwrap_or((None, false, None));
                let finished = summary.as_ref().is_some_and(|s| s.finished);
                let phase = if failed.contains(&i) {
                    Phase::Failed
                } else if finished {
                    Phase::Finished
                } else if running == Some(i) {
                    Phase::Running
                } else if stopped == Some(i) {
                    Phase::Stopped
                } else {
                    Phase::Queued
                };
                let elapsed = summary.as_ref().map_or(0.0, |s| s.state.elapsed_seconds);
                let done = if finished {
                    1.0
                } else {
                    summary.as_ref().map_or(0.0, |s| s.percent / 100.0)
                };
                Entry {
                    dir: dir.clone(),
                    dataset: dataset_name(dir),
                    seed,
                    s2,
                    taxa,
                    phase,
                    done,
                    elapsed,
                    expected: finished.then_some(elapsed),
                    rough: false,
                }
            })
            .collect();
        estimate(&mut entries);
        Batch {
            entries,
            current: None,
        }
    }

    /// Takes the progress of the running run.
    pub fn set_progress(&mut self, index: usize, s: &Snapshot) {
        if let Some(e) = self.entries.get_mut(index) {
            e.phase = Phase::Running;
            e.done = s.overall_fraction.clamp(0.0, 1.0);
            e.elapsed = s.elapsed_seconds;
            if let Some(r) = s.remaining_seconds {
                e.expected = Some(s.elapsed_seconds + r);
                e.rough = false;
            }
        }
        self.current = Some(Current {
            index,
            stage: s.stage_name.clone(),
            stage_fraction: s.stage_fraction(),
            remaining: s.remaining_seconds,
        });
    }

    pub fn finished(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.phase == Phase::Finished)
            .count()
    }

    /// Batch progress from 0 to 1, weighted by each run's expected time
    /// (by count when some runs have no estimate yet).
    pub fn fraction(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        if self.entries.iter().all(|e| e.expected.is_some()) {
            let total: f64 = self.entries.iter().map(|e| e.expected.unwrap_or(0.0)).sum();
            if total > 0.0 {
                let done: f64 = self
                    .entries
                    .iter()
                    .map(|e| e.expected.unwrap_or(0.0) * e.done)
                    .sum();
                return (done / total).clamp(0.0, 1.0);
            }
        }
        self.entries.iter().map(|e| e.done).sum::<f64>() / self.entries.len() as f64
    }

    /// Estimated working time left, and whether every run has an estimate
    /// (when not, the time is a lower bound).
    pub fn left_seconds(&self) -> (f64, bool) {
        let mut left = 0.0;
        let mut complete = true;
        for e in &self.entries {
            if e.phase == Phase::Finished {
                continue;
            }
            match e.expected {
                Some(x) => left += (x - e.elapsed).max(0.0),
                None => complete = false,
            }
        }
        (left, complete)
    }

    /// The run being worked on, 1-based, for "run 7 of 70".
    fn position(&self) -> usize {
        match &self.current {
            Some(c) => c.index + 1,
            None => (self.finished() + 1).min(self.entries.len()),
        }
    }

    /// Datasets (rows, in queue order) and seeds (columns, ascending).
    pub fn grid(&self) -> (Vec<String>, Vec<Option<u64>>) {
        let mut rows: Vec<String> = Vec::new();
        let mut cols: Vec<Option<u64>> = Vec::new();
        for e in &self.entries {
            if !rows.contains(&e.dataset) {
                rows.push(e.dataset.clone());
            }
            if !cols.contains(&e.seed) {
                cols.push(e.seed);
            }
        }
        cols.sort();
        (rows, cols)
    }

    fn cell(&self, dataset: &str, seed: Option<u64>) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.dataset == dataset && e.seed == seed)
    }

    /// Groups of the queue for the phase bars: "with S2" and "without S2"
    /// when both occur, else one group. (label, finished, total)
    pub fn phases(&self) -> Vec<(String, usize, usize)> {
        let with: Vec<&Entry> = self.entries.iter().filter(|e| e.s2).collect();
        let without: Vec<&Entry> = self.entries.iter().filter(|e| !e.s2).collect();
        let count = |v: &[&Entry]| v.iter().filter(|e| e.phase == Phase::Finished).count();
        if with.is_empty() || without.is_empty() {
            return vec![("All runs".into(), self.finished(), self.entries.len())];
        }
        vec![
            ("Runs with S2".into(), count(&with), with.len()),
            ("Runs without S2".into(), count(&without), without.len()),
        ]
    }

    /// The summary line, for example
    /// `Batch: 15.3% | run 7 of 70 | left 2 d 21 h | finish Thu 08 Oct, 02:10` (local time).
    pub fn summary_line(&self, now: &UtcDateTime) -> String {
        let (left, complete) = self.left_seconds();
        let left_text = if self.entries.iter().all(|e| e.phase == Phase::Finished) {
            "nothing left".to_string()
        } else if left == 0.0 && !complete {
            "left: estimating".to_string()
        } else {
            format!(
                "left {}{}",
                if complete { "" } else { "at least " },
                long_duration(left)
            )
        };
        let finish = if complete && left > 0.0 {
            format!(" | finish {}", finish_text(now, left))
        } else {
            String::new()
        };
        format!(
            "Batch: {:.1}% | run {} of {} | {left_text}{finish}",
            100.0 * self.fraction(),
            self.position(),
            self.entries.len()
        )
    }

    /// The terminal view: summary line, current run, grid and phases. With
    /// `color`, coloured symbols; without, letters only and no escape codes.
    pub fn terminal(&self, color: bool, now: &UtcDateTime) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", self.summary_line(now));
        if let Some(c) = &self.current {
            if let Some(e) = self.entries.get(c.index) {
                let _ = writeln!(
                    s,
                    "Current: {} (seed {}{}), {} {:.0}%{}",
                    e.dataset,
                    e.seed.map_or("-".into(), |x| x.to_string()),
                    if e.s2 { ", with S2" } else { "" },
                    crate::analysis::stage_label(&c.stage),
                    100.0 * c.stage_fraction,
                    c.remaining
                        .map(|r| format!(", {} left", long_duration(r)))
                        .unwrap_or_default()
                );
            }
        }
        let (rows, cols) = self.grid();
        let name_w = rows
            .iter()
            .map(|r| r.chars().count())
            .max()
            .unwrap_or(7)
            .max(7);
        let cell_w = 13;
        let _ = write!(s, "{:name_w$}", "");
        for c in &cols {
            let head = c.map_or("run".to_string(), |x| format!("seed {x}"));
            let _ = write!(s, "  {head:<cell_w$}");
        }
        s.push('\n');
        for r in &rows {
            let _ = write!(s, "{r:name_w$}");
            for &c in &cols {
                let text = match self.cell(r, c) {
                    None => String::new(),
                    Some(e) => cell_text(e, color),
                };
                // Pad by visible width (escape codes do not count).
                let visible = strip_ansi(&text).chars().count();
                let _ = write!(s, "  {text}{}", " ".repeat(cell_w.saturating_sub(visible)));
            }
            s.push('\n');
        }
        for (label, done, total) in self.phases() {
            let _ = writeln!(s, "{label}: {done} of {total} runs");
        }
        s.push_str("Times are estimates.\n");
        s
    }

    /// The offline status page: one self-contained HTML file (no network,
    /// no external fonts or scripts) that reloads itself every 30 s and
    /// says when it may be out of date.
    pub fn html(&self, now: &UtcDateTime, now_unix: i64) -> String {
        let (rows, cols) = self.grid();
        let (left, complete) = self.left_seconds();
        let mut s = String::new();
        let _ = write!(
            s,
            r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="refresh" content="{REFRESH_SECONDS}">
<title>Q-MAWS batch: {pct:.1}%</title>
<style>
:root{{--bg:#f6f4ef;--card:#fff;--sunken:#efece4;--line:#e3ded2;--ink:#0b2545;--soft:#4a5b74;--faint:#8692a4;
--blue:#0072b2;--green:#007f5c;--greenw:#e1f2eb;--orange:#e69f00;--orangew:#fbf0d6;--red:#b84a00;--redw:#fae8de;--grey:#c9c3b6}}
@media (prefers-color-scheme: dark){{:root{{--bg:#0d1624;--card:#131f31;--sunken:#0a121f;--line:#223148;--ink:#e7edf5;--soft:#a7b5c8;--faint:#6c7d95;
--blue:#56b4e9;--green:#3fc29a;--greenw:#0f2e28;--orange:#f0b02a;--orangew:#3a2e10;--red:#f08a4b;--redw:#3a1f14;--grey:#30435f}}}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;font-variant-numeric:tabular-nums}}
main{{max-width:1080px;margin:0 auto;padding:32px 24px 48px}}
header{{display:flex;align-items:center;gap:14px;margin-bottom:22px}}
.mark{{width:40px;height:40px;border-radius:10px;background:#0b2545;flex:none}}
h1{{font-size:24px;font-weight:500;margin:0}} .sub{{color:var(--soft);margin:0}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px;margin:0 0 14px}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:16px 18px}}
.label{{color:var(--faint);font-size:12.5px}} .value{{font-size:26px;font-weight:500}} .value.when{{font-size:21px;white-space:nowrap;padding-top:4px}}
.bar{{height:8px;border-radius:4px;background:var(--sunken);overflow:hidden;margin-top:10px}}
.bar>i{{display:block;height:100%;background:var(--blue);border-radius:4px}}
table{{width:100%;border-collapse:separate;border-spacing:0 6px}}
th{{text-align:left;font-weight:400;color:var(--faint);font-size:12.5px;padding:0 8px}}
td{{padding:0 4px}} td.name{{padding-right:14px;white-space:nowrap}}
.cell{{border-radius:7px;padding:6px 10px;font-size:13px;white-space:nowrap}}
.F{{background:var(--greenw);color:var(--green)}} .R{{background:var(--orangew);color:var(--orange);font-weight:500}}
.Q{{background:var(--sunken);color:var(--faint)}} .S,.X{{background:var(--redw);color:var(--red)}}
.note{{color:var(--faint);font-size:12.5px;margin-top:18px}}
.stale{{display:none;background:var(--redw);color:var(--red);border-radius:8px;padding:10px 14px;margin-bottom:14px}}
.phase{{display:flex;justify-content:space-between;font-size:13px;color:var(--soft)}}
</style></head>
<body data-written="{now_unix}"><main>
<div class="stale" id="stale">This page has not been updated for more than {stale} s: the queue may have stopped. Resume it with <code>qmaws resume --all</code>.</div>
<header><svg class="mark" viewBox="0 0 130 130" aria-hidden="true"><rect width="130" height="130" rx="28" fill="#0b2545"/><g stroke="#fff" stroke-width="8" stroke-linecap="round"><line x1="40" y1="65" x2="17" y2="32"/><line x1="40" y1="65" x2="17" y2="98"/><line x1="90" y1="65" x2="113" y2="32"/><line x1="90" y1="65" x2="113" y2="98"/></g><line x1="40" y1="65" x2="90" y2="65" stroke="#e69f00" stroke-width="10" stroke-linecap="round"/><circle cx="17" cy="32" r="12" fill="#56b4e9"/><circle cx="17" cy="98" r="12" fill="#009e73"/><circle cx="113" cy="32" r="12" fill="#d55e00"/><circle cx="113" cy="98" r="12" fill="#f0e442"/></svg>
<div><h1>Q-MAWS batch</h1><p class="sub">{line}</p></div></header>
"##,
            pct = 100.0 * self.fraction(),
            stale = 3 * REFRESH_SECONDS,
            line = escape(&self.summary_line(now)),
        );
        let left_text = if self.entries.iter().all(|e| e.phase == Phase::Finished) {
            "none".to_string()
        } else if left == 0.0 && !complete {
            "estimating".to_string()
        } else {
            format!(
                "{}{}",
                if complete { "" } else { "at least " },
                long_duration(left)
            )
        };
        let _ = write!(
            s,
            r#"<div class="cards">
<div class="card"><div class="label">Batch progress</div><div class="value">{:.1}%</div><div class="bar"><i style="width:{:.2}%"></i></div></div>
<div class="card"><div class="label">Runs finished</div><div class="value">{} of {}</div></div>
<div class="card"><div class="label">Time left (estimate)</div><div class="value">{}</div></div>
<div class="card"><div class="label">Estimated finish</div><div class="value when" id="finish">{}</div></div>
</div>
"#,
            100.0 * self.fraction(),
            100.0 * self.fraction(),
            self.finished(),
            self.entries.len(),
            escape(&left_text),
            if complete && left > 0.0 {
                finish_text(now, left)
            } else {
                "\u{2014}".into()
            }
        );
        if let Some(c) = &self.current {
            if let Some(e) = self.entries.get(c.index) {
                let _ = write!(
                    s,
                    r#"<div class="card" style="margin-bottom:14px"><div class="label">Current run</div>
<div style="font-size:17px">{} &middot; seed {}{} &middot; {} {:.0}%</div>
<div class="bar"><i style="width:{:.2}%"></i></div>
<div class="label" style="margin-top:6px">Working time {}{}</div></div>
"#,
                    escape(&e.dataset),
                    e.seed.map_or("-".into(), |x| x.to_string()),
                    if e.s2 { " &middot; with S2" } else { "" },
                    escape(crate::analysis::stage_label(&c.stage)),
                    100.0 * c.stage_fraction,
                    100.0 * e.done,
                    long_duration(e.elapsed),
                    c.remaining
                        .map(|r| format!(", {} left", about_duration(r)))
                        .unwrap_or_default()
                );
            }
        }
        s.push_str("<div class=\"card\"><table><tr><th></th>");
        for c in &cols {
            let _ = write!(
                s,
                "<th>{}</th>",
                c.map_or("Run".to_string(), |x| format!("Seed {x}"))
            );
        }
        s.push_str("</tr>\n");
        for r in &rows {
            let _ = write!(s, "<tr><td class=\"name\">{}</td>", escape(r));
            for &c in &cols {
                match self.cell(r, c) {
                    None => s.push_str("<td></td>"),
                    Some(e) => {
                        let _ = write!(
                            s,
                            "<td><div class=\"cell {}\" title=\"{}\">{} {}</div></td>",
                            e.phase.letter(),
                            escape(&e.dir.display().to_string()),
                            e.phase.symbol(),
                            escape(&cell_detail(e))
                        );
                    }
                }
            }
            s.push_str("</tr>\n");
        }
        s.push_str("</table>\n");
        for (label, done, total) in self.phases() {
            let pct = if total == 0 {
                0.0
            } else {
                100.0 * done as f64 / total as f64
            };
            let _ = writeln!(
                s,
                "<div style=\"margin-top:12px\"><div class=\"phase\"><span>{}</span><span>{done} of {total} runs</span></div><div class=\"bar\"><i style=\"width:{pct:.2}%\"></i></div></div>",
                escape(&label)
            );
        }
        let _ = write!(
            s,
            r#"</div>
<p class="note">Written {} (this computer's time). This page reloads every {REFRESH_SECONDS} s. Every time is an estimate, measured from the runs that have finished; "rough" marks estimates scaled by the number of quartets.</p>
</main>
<script>
(function(){{var w=Number(document.body.dataset.written)*1000;function check(){{if(Date.now()-w>{stale}000){{document.getElementById("stale").style.display="block";}}}}check();setInterval(check,5000);}})();
</script>
</body></html>
"#,
            crate::clock::local_clock(now.unix_seconds()),
            stale = 3 * REFRESH_SECONDS,
        );
        s
    }

    /// Writes the status page atomically into `folder`.
    pub fn write_html(&self, folder: &Path) -> std::io::Result<PathBuf> {
        let now = UtcDateTime::now();
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let path = folder.join(STATUS_FILE);
        std::fs::create_dir_all(folder)?;
        let tmp = folder.join(format!("{STATUS_FILE}.tmp"));
        std::fs::write(&tmp, self.html(&now, unix))?;
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

/// The folder for the status page: the common parent of the run folders.
pub fn status_folder(dirs: &[PathBuf]) -> PathBuf {
    let parents: Vec<PathBuf> = dirs
        .iter()
        .map(|d| d.parent().map(Path::to_path_buf).unwrap_or_default())
        .collect();
    match parents.first() {
        Some(first) if parents.iter().all(|p| p == first) => first.clone(),
        _ => PathBuf::from(crate::rundir::DEFAULT_RUNS_ROOT),
    }
}

fn estimate(entries: &mut [Entry]) {
    // Mean working time of the finished runs of each dataset and S2 setting.
    let finished: Vec<(String, bool, f64, Option<usize>)> = entries
        .iter()
        .filter(|e| e.phase == Phase::Finished && e.elapsed > 0.0)
        .map(|e| (e.dataset.clone(), e.s2, e.elapsed, e.taxa))
        .collect();
    let per_quartet: Vec<f64> = finished
        .iter()
        .filter_map(|(_, _, t, m)| {
            m.map(|m| t / quartets(m))
                .filter(|x| x.is_finite() && *x > 0.0)
        })
        .collect();
    let per_quartet = (!per_quartet.is_empty())
        .then(|| per_quartet.iter().sum::<f64>() / per_quartet.len() as f64);
    for e in entries.iter_mut() {
        if e.expected.is_some() {
            continue;
        }
        let same: Vec<f64> = finished
            .iter()
            .filter(|(d, s2, _, _)| *d == e.dataset && *s2 == e.s2)
            .map(|(_, _, t, _)| *t)
            .collect();
        if !same.is_empty() {
            e.expected = Some(same.iter().sum::<f64>() / same.len() as f64);
        } else if let (Some(pq), Some(m)) = (per_quartet, e.taxa) {
            e.expected = Some(pq * quartets(m));
            e.rough = true;
        } else if e.done > 0.02 && e.elapsed > 0.0 {
            // A run partly done: scale its own time.
            e.expected = Some(e.elapsed / e.done);
            e.rough = true;
        }
    }
}

fn cell_detail(e: &Entry) -> String {
    match e.phase {
        Phase::Finished => short_duration(e.elapsed),
        Phase::Running => format!("{:.0}%", 100.0 * e.done),
        Phase::Queued | Phase::Stopped => match e.expected {
            Some(x) => format!(
                "{}{}",
                if e.rough { "~" } else { "" },
                short_duration((x - e.elapsed).max(0.0))
            ),
            None => e.phase.word().into(),
        },
        Phase::Failed => "failed".into(),
    }
}

fn cell_text(e: &Entry, color: bool) -> String {
    let detail = cell_detail(e);
    if !color {
        return format!("{} {detail}", e.phase.letter());
    }
    let code = match e.phase {
        Phase::Finished => "32",
        Phase::Running => "33;1",
        Phase::Queued => "90",
        Phase::Stopped | Phase::Failed => "31",
    };
    format!("\u{1b}[{code}m{} {detail}\u{1b}[0m", e.phase.symbol())
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// "under 1 min", "45 min", "3 h 05 min", "2 d 21 h".
pub fn long_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m:02} min")
    } else if m > 0 {
        format!("{m} min")
    } else {
        "under 1 min".into()
    }
}

/// [`long_duration`] as an estimate: "about 3 min", but "under 1 min".
pub fn about_duration(seconds: f64) -> String {
    let d = long_duration(seconds);
    if d.starts_with("under") {
        d
    } else {
        format!("about {d}")
    }
}

/// Like [`long_duration`], shorter, for grid cells.
fn short_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s >= 86_400 {
        format!("{} d {} h", s / 86_400, (s % 86_400) / 3600)
    } else if s >= 3600 {
        format!("{} h {:02} m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{} min", s / 60)
    } else {
        format!("{s} s")
    }
}

/// The local clock time `left` seconds after `now`.
fn finish_text(now: &UtcDateTime, left: f64) -> String {
    crate::clock::local_clock(now.unix_seconds() + left.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_read_naturally() {
        assert_eq!(about_duration(20.0), "under 1 min");
        assert_eq!(about_duration(185.0), "about 3 min");
        assert_eq!(about_duration(7500.0), "about 2 h 05 min");
    }

    fn entry(
        dataset: &str,
        seed: u64,
        phase: Phase,
        done: f64,
        elapsed: f64,
        expected: Option<f64>,
    ) -> Entry {
        Entry {
            dir: PathBuf::from(format!("runs/{dataset}_seed{seed}")),
            dataset: dataset.into(),
            seed: Some(seed),
            s2: seed == 1,
            taxa: Some(10),
            phase,
            done,
            elapsed,
            expected,
            rough: false,
        }
    }

    fn sample() -> Batch {
        let mut entries = vec![
            entry("fish", 1, Phase::Finished, 1.0, 100.0, Some(100.0)),
            entry("fish", 2, Phase::Running, 0.5, 25.0, Some(50.0)),
            entry("fish", 3, Phase::Queued, 0.0, 0.0, None),
            entry("ecoli", 1, Phase::Queued, 0.0, 0.0, None),
        ];
        entries[1].s2 = false;
        entries[2].s2 = false;
        let mut b = Batch {
            entries,
            current: None,
        };
        estimate(&mut b.entries);
        b
    }

    #[test]
    fn dataset_names_drop_seed_and_date_suffixes() {
        assert_eq!(dataset_name(Path::new("runs/sim_hgt_0_seed3")), "sim_hgt_0");
        assert_eq!(
            dataset_name(Path::new("runs/fish_mito_2026-10-03_092551")),
            "fish_mito"
        );
        assert_eq!(dataset_name(Path::new("runs/my_run")), "my_run");
        assert_eq!(dataset_name(Path::new("runs/x_seedling")), "x_seedling");
    }

    #[test]
    fn estimates_come_from_finished_runs_or_are_rough() {
        let b = sample();
        // fish seed 3 (no S2): no finished fish run without S2, so rough by
        // quartets: 100 s for C(10,4) quartets scales to the same size.
        let f3 = &b.entries[2];
        assert!(f3.rough);
        assert!((f3.expected.unwrap() - 100.0).abs() < 1e-9);
        // ecoli seed 1 has no finished run of its own: rough as well.
        assert!(b.entries[3].rough);
    }

    #[test]
    fn the_summary_weights_runs_by_their_expected_time() {
        let b = sample();
        // Expected 100 + 50 + 100 + 100 = 350; done 100 + 25 = 125.
        assert!((b.fraction() - 125.0 / 350.0).abs() < 1e-9);
        let (left, complete) = b.left_seconds();
        assert!(complete);
        assert!((left - 225.0).abs() < 1e-9);
        assert_eq!(b.finished(), 1);
        let (rows, cols) = b.grid();
        assert_eq!(rows, vec!["fish".to_string(), "ecoli".to_string()]);
        assert_eq!(cols, vec![Some(1), Some(2), Some(3)]);
        assert_eq!(b.phases().len(), 2);
    }

    #[test]
    fn the_plain_terminal_view_has_no_escape_codes() {
        let b = sample();
        let now = UtcDateTime::from_unix_seconds(0);
        let plain = b.terminal(false, &now);
        assert!(!plain.contains('\u{1b}'));
        assert!(plain.contains("F 1 min") || plain.contains("F 100 s"));
        assert!(plain.starts_with("Batch: 35.7% | run 2 of 4"));
        let colour = b.terminal(true, &now);
        assert!(colour.contains('\u{1b}'));
    }

    #[test]
    fn the_page_is_self_contained_and_written_atomically() {
        let b = sample();
        let html = b.html(&UtcDateTime::from_unix_seconds(0), 0);
        assert!(html.starts_with("<!doctype html>"));
        assert!(!html.contains("http://") && !html.contains("https://"));
        assert!(html.contains("http-equiv=\"refresh\""));
        assert!(html.contains("qmaws resume --all"));
        let dir = std::env::temp_dir().join(format!("qmaws_batch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = b.write_html(&dir).unwrap();
        assert_eq!(path, dir.join(STATUS_FILE));
        assert!(!dir.join(format!("{STATUS_FILE}.tmp")).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stopped_and_resumed_queue_keeps_its_totals() {
        let mut b = sample();
        b.entries[1].phase = Phase::Stopped;
        let before = (b.entries.len(), b.finished());
        b.entries[1].phase = Phase::Running;
        assert_eq!((b.entries.len(), b.finished()), before);
    }
}
