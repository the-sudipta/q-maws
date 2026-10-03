//! Terminal menus and progress display for Q-MAWS.
//!
//! [`TerminalDisplay`] shows the engine's progress events in a terminal: an
//! overall bar and a stage bar with elapsed time, remaining time and the
//! current item, with the run log scrolling above the bars. When the output is
//! not a terminal (for example a log file or CI), it prints plain progress
//! lines instead. The interactive main menu is in [`menu`].

pub mod menu;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use qmaws_engine::progress::format_duration;
use qmaws_engine::{Event, ProgressSink, Snapshot};
use std::io::{IsTerminal, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Name of this crate, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Version of this crate, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How progress is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Progress bars on a terminal, plain lines otherwise.
    Normal,
    /// Only the final result and errors.
    Quiet,
    /// One JSON object per event on standard output, for scripts.
    Json,
}

/// Interval between plain progress lines when the output is not a terminal.
const PLAIN_INTERVAL: Duration = Duration::from_secs(5);

/// Width of the overall bar's position range (per mille).
const OVERALL_STEPS: u64 = 1000;

pub struct TerminalDisplay {
    mode: DisplayMode,
    bars: Option<Bars>,
    last_plain: Mutex<Option<Instant>>,
}

struct Bars {
    multi: MultiProgress,
    overall: ProgressBar,
    stage: ProgressBar,
}

impl TerminalDisplay {
    /// `color`: use colours in progress bars.
    pub fn new(mode: DisplayMode, color: bool) -> Self {
        let interactive = std::io::stderr().is_terminal();
        let bars = (mode == DisplayMode::Normal && interactive).then(|| Bars::new(color));
        Self {
            mode,
            bars,
            last_plain: Mutex::new(None),
        }
    }

    /// Prints a message above the bars, or as a plain line.
    fn line(&self, message: &str) {
        match &self.bars {
            Some(b) => {
                let _ = b.multi.println(message);
            }
            None => eprintln!("{message}"),
        }
    }

    fn show(&self, s: &Snapshot) {
        match &self.bars {
            Some(b) => b.update(s),
            None => {
                let mut last = self.last_plain.lock().expect("display lock");
                let due = last.is_none_or(|t| t.elapsed() >= PLAIN_INTERVAL);
                let stage_complete = s.stage_units_done == s.stage_units_total;
                if due || stage_complete {
                    *last = Some(Instant::now());
                    eprintln!("{}", plain_line(s));
                }
            }
        }
    }

    fn clear(&self) {
        if let Some(b) = &self.bars {
            b.overall.finish_and_clear();
            b.stage.finish_and_clear();
        }
    }
}

impl Bars {
    fn new(color: bool) -> Self {
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
        let template = if color {
            "{prefix:<34.bold} [{bar:32.cyan/blue}] {percent:>3}% {msg}"
        } else {
            "{prefix:<34} [{bar:32}] {percent:>3}% {msg}"
        };
        let style = ProgressStyle::with_template(template)
            .expect("valid progress template")
            .progress_chars("=> ");
        let overall = multi.add(ProgressBar::new(OVERALL_STEPS).with_style(style.clone()));
        overall.set_prefix("Overall");
        let stage = multi.add(ProgressBar::new(1).with_style(style));
        Self {
            multi,
            overall,
            stage,
        }
    }

    fn update(&self, s: &Snapshot) {
        let pos = (s.overall_fraction * OVERALL_STEPS as f64).round() as u64;
        self.overall.set_position(pos.min(OVERALL_STEPS));
        self.overall.set_message(time_text(s));
        self.stage.set_prefix(format!(
            "Stage {} of {}: {}",
            s.stage_index, s.stage_count, s.stage_name
        ));
        self.stage.set_length(s.stage_units_total.max(1));
        self.stage.set_position(s.stage_units_done);
        self.stage.set_message(s.current_item.clone());
    }
}

fn time_text(s: &Snapshot) -> String {
    let remaining = s
        .remaining_seconds
        .map(format_duration)
        .unwrap_or_else(|| "estimating".to_string());
    format!(
        "elapsed {}, remaining {}",
        format_duration(s.elapsed_seconds),
        remaining
    )
}

/// One-line progress summary for non-interactive output.
pub fn plain_line(s: &Snapshot) -> String {
    format!(
        "Stage {} of {} ({}): {:.0}% | overall {:.0}% | {} | {}",
        s.stage_index,
        s.stage_count,
        s.stage_name,
        s.stage_fraction() * 100.0,
        s.overall_fraction * 100.0,
        time_text(s),
        s.current_item
    )
}

impl ProgressSink for TerminalDisplay {
    fn event(&self, event: &Event) {
        match self.mode {
            DisplayMode::Json => {
                if let Ok(line) = serde_json::to_string(event) {
                    let mut out = std::io::stdout().lock();
                    let _ = writeln!(out, "{line}");
                    let _ = out.flush();
                }
            }
            DisplayMode::Quiet => {}
            DisplayMode::Normal => match event {
                Event::Started {
                    run_id,
                    run_dir,
                    resumed,
                } => {
                    let verb = if *resumed { "Resuming" } else { "Starting" };
                    self.line(&format!("{verb} run {run_id} in {run_dir}"));
                }
                Event::Progress(s) => self.show(s),
                Event::Log { message } => self.line(message),
                Event::Finished { .. } | Event::Stopped => self.clear(),
                // Shown by the GUI; the log already reports provisional trees.
                Event::Quartet(_) | Event::Provisional(_) => {}
            },
        }
    }
}

/// Progress display for one download: a bar with remaining time on a
/// terminal, plain lines otherwise. Counts bytes (with speed) or other units
/// such as records.
pub struct DownloadDisplay {
    mode: DisplayMode,
    bar: Option<ProgressBar>,
    label: String,
    unit: &'static str,
    last_plain: Option<Instant>,
}

impl DownloadDisplay {
    /// A display counting bytes.
    pub fn new(mode: DisplayMode, color: bool, label: &str) -> Self {
        Self::with_unit(mode, color, label, "bytes")
    }

    /// A display counting `unit`, for example "records".
    pub fn with_unit(mode: DisplayMode, color: bool, label: &str, unit: &'static str) -> Self {
        let interactive = std::io::stderr().is_terminal();
        let bar = (mode == DisplayMode::Normal && interactive).then(|| {
            let counts = if unit == "bytes" {
                "{bytes}/{total_bytes} {bytes_per_sec}".to_string()
            } else {
                format!("{{pos}}/{{len}} {unit}")
            };
            let template = if color {
                format!("{{prefix:.bold}} [{{bar:32.cyan/blue}}] {counts}, remaining {{eta}}")
            } else {
                format!("{{prefix}} [{{bar:32}}] {counts}, remaining {{eta}}")
            };
            let style = ProgressStyle::with_template(&template)
                .expect("valid progress template")
                .progress_chars("=> ");
            let bar = ProgressBar::with_draw_target(Some(0), ProgressDrawTarget::stderr())
                .with_style(style);
            bar.set_prefix(label.to_string());
            bar
        });
        Self {
            mode,
            bar,
            label: label.to_string(),
            unit,
            last_plain: None,
        }
    }

    /// Units received so far, out of `total` if known.
    pub fn update(&mut self, done: u64, total: Option<u64>) {
        match self.mode {
            DisplayMode::Quiet => {}
            DisplayMode::Json => {
                let line = serde_json::json!({
                    "event": "download_progress",
                    "item": self.label,
                    "unit": self.unit,
                    "done": done,
                    "total": total,
                });
                println!("{line}");
            }
            DisplayMode::Normal => match &self.bar {
                Some(bar) => {
                    if let Some(t) = total {
                        bar.set_length(t);
                    }
                    bar.set_position(done);
                }
                None => {
                    let due = self
                        .last_plain
                        .is_none_or(|t| t.elapsed() >= PLAIN_INTERVAL);
                    let complete = total.is_some_and(|t| done >= t);
                    if due || complete {
                        self.last_plain = Some(Instant::now());
                        let of = total.map(|t| format!(" of {t}")).unwrap_or_default();
                        eprintln!("{}: {done}{of} {}", self.label, self.unit);
                    }
                }
            },
        }
    }

    pub fn finish(&self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_display_accepts_updates_in_every_mode() {
        for mode in [DisplayMode::Normal, DisplayMode::Quiet, DisplayMode::Json] {
            let mut d = DownloadDisplay::new(mode, false, "fish_mito");
            d.update(0, Some(100));
            d.update(100, Some(100));
            d.update(5, None);
            d.finish();
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            stage_index: 1,
            stage_count: 2,
            stage_name: "toy_count".into(),
            stage_units_done: 25,
            stage_units_total: 100,
            overall_fraction: 0.24,
            elapsed_seconds: 65.0,
            remaining_seconds: Some(200.0),
            current_item: "chunk 3 of 10".into(),
        }
    }

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(CRATE_NAME, "qmaws-tui");
    }

    #[test]
    fn plain_line_shows_stage_overall_and_times() {
        assert_eq!(
            plain_line(&snapshot()),
            "Stage 1 of 2 (toy_count): 25% | overall 24% | elapsed 0:01:05, remaining 0:03:20 | chunk 3 of 10"
        );
    }

    #[test]
    fn missing_estimate_is_shown_as_estimating() {
        let mut s = snapshot();
        s.remaining_seconds = None;
        assert!(time_text(&s).ends_with("remaining estimating"));
    }

    #[test]
    fn displays_accept_every_event() {
        for mode in [DisplayMode::Normal, DisplayMode::Quiet, DisplayMode::Json] {
            let d = TerminalDisplay::new(mode, false);
            d.event(&Event::Log {
                message: "test".into(),
            });
            d.event(&Event::Progress(snapshot()));
            d.event(&Event::Stopped);
        }
    }
}
