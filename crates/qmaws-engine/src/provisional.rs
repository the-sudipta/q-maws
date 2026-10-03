//! The live provisional Halo Tree (plan 4.7).
//!
//! Quartets are weighed in a seeded random order, so the quartets finished
//! at any time are a fair sample of all quartets. While the stage
//! `quartet_weight` runs, the engine amalgamates the finished quartets every
//! 5% of the quartets or every 3 minutes, whichever comes first, computes
//! provisional halo values and draws the provisional Halo Tree with a
//! watermark:
//!
//! - `figures/live/halo_tree_latest.svg`, `.png` and `.pdf`, overwritten at
//!   each update (atomic writes);
//! - `work/provisional/frames/frame_0001.png`, ... (480 pixels wide);
//! - `work/provisional/state.json`: the intervals and every provisional tree,
//!   so a resumed run continues from the last saved state.
//!
//! Overhead budget: when one update takes more than 10% of the time since
//! the previous one, both intervals are doubled and the change is logged.
//! After the amalgamation, `report/convergence.csv` gives the nRF of every
//! provisional tree to the final tree.
//!
//! Nothing here is part of the root fingerprint: the provisional trees depend
//! on timing, the results do not.

use crate::atomic;
use crate::rundir::RunDir;
use crate::runner::{io_err, EngineError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Update every this many percent of the quartets ...
pub const INTERVAL_PERCENT: f64 = 5.0;
/// ... or after this many seconds, whichever comes first.
pub const INTERVAL_SECONDS: f64 = 180.0;
/// Largest share of the elapsed interval that an update may take.
pub const OVERHEAD_BUDGET: f64 = 0.10;
/// Width of the saved frames in pixels.
pub const FRAME_WIDTH: u32 = 480;
/// Width of `halo_tree_latest.png` in pixels.
pub const LATEST_WIDTH: u32 = 1200;

/// One provisional tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub frame: u32,
    /// Percentage of quartets finished.
    pub percent: f64,
    pub quartets: u64,
    pub newick: String,
    /// Time the update took, in seconds.
    pub seconds: f64,
}

/// `work/provisional/state.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub enabled: bool,
    pub interval_percent: f64,
    pub interval_seconds: f64,
    pub frames: Vec<Frame>,
}

impl State {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            interval_percent: INTERVAL_PERCENT,
            interval_seconds: INTERVAL_SECONDS,
            frames: Vec::new(),
        }
    }
}

pub(crate) fn state_file(dir: &RunDir) -> PathBuf {
    dir.work().join("provisional").join("state.json")
}

pub(crate) fn frame_file(dir: &RunDir, frame: u32) -> PathBuf {
    dir.work()
        .join("provisional")
        .join("frames")
        .join(format!("frame_{frame:04}.png"))
}

pub(crate) fn latest_file(dir: &RunDir, extension: &str) -> PathBuf {
    dir.figures()
        .join("live")
        .join(format!("halo_tree_latest.{extension}"))
}

/// Reads the saved state; `None` if there is none or it is damaged.
pub(crate) fn load(dir: &RunDir) -> Option<State> {
    let bytes = atomic::read_verified(&state_file(dir))?;
    serde_json::from_slice(&bytes).ok()
}

/// The Newick text of the last provisional tree saved in a run folder; the
/// GUI compares the next tree with it after a resume.
pub fn last_tree(run_dir: &std::path::Path) -> Option<String> {
    load(&RunDir::new(run_dir))?
        .frames
        .last()
        .map(|f| f.newick.clone())
}

/// Scheduling of the updates within one session.
pub(crate) struct Live {
    pub state: State,
    /// Time of the previous update, or of the start of this session.
    last: Instant,
}

impl Live {
    /// The saved state, or a new one with `enabled` (the run's setting).
    pub fn open(dir: &RunDir, enabled: bool) -> Self {
        Self {
            state: load(dir).unwrap_or_else(|| State::new(enabled)),
            last: Instant::now(),
        }
    }

    fn last_percent(&self) -> f64 {
        self.state.frames.last().map_or(0.0, |f| f.percent)
    }

    /// Whether an update is due with `done` of `total` quartets finished.
    pub fn due(&self, done: u64, total: u64) -> bool {
        if !self.state.enabled || done == 0 || done >= total {
            return false;
        }
        let percent = 100.0 * done as f64 / total as f64;
        percent >= self.last_percent() + self.state.interval_percent
            || self.last.elapsed().as_secs_f64() >= self.state.interval_seconds
    }

    /// Records an update that took `seconds`; returns a log line when the
    /// intervals were doubled to keep within the overhead budget.
    pub fn record(&mut self, frame: Frame) -> Option<String> {
        let interval = self.last.elapsed().as_secs_f64();
        let seconds = frame.seconds;
        self.state.frames.push(frame);
        self.last = Instant::now();
        (seconds > OVERHEAD_BUDGET * interval).then(|| {
            self.state.interval_percent *= 2.0;
            self.state.interval_seconds *= 2.0;
            format!(
                "Provisional tree took {seconds:.1} s of a {interval:.1} s interval (over {:.0}%); updating every {}% or {} s from now on.",
                OVERHEAD_BUDGET * 100.0,
                self.state.interval_percent,
                self.state.interval_seconds
            )
        })
    }

    pub fn save(&self, dir: &RunDir) -> Result<(), EngineError> {
        let p = state_file(dir);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let text = serde_json::to_string_pretty(&self.state).expect("state serialises") + "\n";
        atomic::write_verified(&p, text.as_bytes())
            .map(|_| ())
            .map_err(io_err(&p))
    }
}

/// `report/convergence.csv`: for every provisional tree, the percentage of
/// quartets and the nRF to the final tree.
pub(crate) fn convergence_csv(frames: &[Frame], final_tree: &str) -> Result<String, EngineError> {
    use qmaws_core::newick::{nrf, Tree};
    let parse = |t: &str| {
        Tree::parse(t.trim()).map_err(|e| EngineError::Invalid(format!("provisional tree: {e}")))
    };
    let last = parse(final_tree)?;
    let mut out = String::from("frame,percent_of_quartets,quartets,nrf_to_final\n");
    for f in frames {
        let (_, value) = nrf(&parse(&f.newick)?, &last);
        out.push_str(&format!(
            "{},{:.2},{},{:.6}\n",
            f.frame, f.percent, f.quartets, value
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn frame(n: u32, percent: f64, newick: &str, seconds: f64) -> Frame {
        Frame {
            frame: n,
            percent,
            quartets: n as u64 * 10,
            newick: newick.into(),
            seconds,
        }
    }

    #[test]
    fn due_every_interval_percent_and_never_at_the_end() {
        let dir = TempDir::new("prov_due");
        let mut live = Live::open(&RunDir::new(dir.path()), true);
        assert!(!live.due(0, 1000));
        assert!(!live.due(49, 1000));
        assert!(live.due(50, 1000));
        assert!(!live.due(1000, 1000));
        live.state.frames.push(frame(1, 5.0, "(A,B,(C,D));", 0.0));
        assert!(!live.due(99, 1000));
        assert!(live.due(100, 1000));
    }

    #[test]
    fn disabled_is_never_due() {
        let dir = TempDir::new("prov_off");
        let live = Live::open(&RunDir::new(dir.path()), false);
        assert!(!live.due(500, 1000));
    }

    #[test]
    fn slow_updates_double_both_intervals() {
        let dir = TempDir::new("prov_budget");
        let mut live = Live::open(&RunDir::new(dir.path()), true);
        // An update that took far longer than the (near zero) interval.
        let line = live.record(frame(1, 5.0, "(A,B,(C,D));", 100.0));
        assert!(line.is_some());
        assert_eq!(live.state.interval_percent, 10.0);
        assert_eq!(live.state.interval_seconds, 360.0);
        // A free update keeps them.
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(live.record(frame(2, 15.0, "(A,B,(C,D));", 0.0)).is_none());
        assert_eq!(live.state.interval_percent, 10.0);
    }

    #[test]
    fn state_survives_a_restart() {
        let dir = TempDir::new("prov_state");
        let run = RunDir::new(dir.path());
        let mut live = Live::open(&run, true);
        live.record(frame(1, 5.0, "(A,B,(C,D));", 0.0));
        live.save(&run).unwrap();
        let again = Live::open(&run, false);
        assert_eq!(again.state, live.state);
        assert!(again.state.enabled, "the saved setting wins on resume");
    }

    #[test]
    fn the_last_saved_tree_is_found() {
        let dir = TempDir::new("prov_last");
        let run = RunDir::new(dir.path());
        assert_eq!(last_tree(dir.path()), None);
        let mut live = Live::open(&run, true);
        live.record(frame(1, 5.0, "(A,B,(C,D));", 0.0));
        live.record(frame(2, 10.0, "(A,C,(B,D));", 0.0));
        live.save(&run).unwrap();
        assert_eq!(last_tree(dir.path()).as_deref(), Some("(A,C,(B,D));"));
    }

    #[test]
    fn convergence_lists_nrf_to_the_final_tree() {
        let frames = vec![
            frame(1, 5.0, "((A,B),(C,E),(D,F));", 0.1),
            frame(2, 10.0, "((A,B),(C,D),(E,F));", 0.1),
        ];
        let csv = convergence_csv(&frames, "((A,B),(C,D),(E,F));").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "frame,percent_of_quartets,quartets,nrf_to_final");
        assert_eq!(lines[1], "1,5.00,10,0.666667");
        assert_eq!(lines[2], "2,10.00,20,0.000000");
    }
}
