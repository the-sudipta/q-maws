//! Progress events and time estimates.
//!
//! The engine reports progress through a [`ProgressSink`]. Terminal and GUI
//! displays implement the sink; the engine never prints anything itself.
//!
//! Estimates: each stage has an estimated cost in seconds, derived from its
//! remaining work units and its throughput. Throughput starts from a
//! calibration measurement (or the value stored in `run.json` on resume) and is
//! updated after every chunk with an exponentially weighted moving average.

use serde::Serialize;
use std::collections::BTreeMap;

/// A point-in-time view of a run's progress.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    /// 1-based index of the current stage.
    pub stage_index: usize,
    pub stage_count: usize,
    pub stage_name: String,
    pub stage_units_done: u64,
    pub stage_units_total: u64,
    /// Overall completion from 0 to 1, weighted by estimated stage cost.
    pub overall_fraction: f64,
    /// Working time over all sessions of this run, in seconds.
    pub elapsed_seconds: f64,
    /// Estimated remaining time in seconds, when an estimate exists.
    pub remaining_seconds: Option<f64>,
    /// What is being processed now, for example `chunk 12 of 40 (blocks 440 to 479)`.
    pub current_item: String,
}

impl Snapshot {
    pub fn stage_fraction(&self) -> f64 {
        if self.stage_units_total == 0 {
            1.0
        } else {
            self.stage_units_done as f64 / self.stage_units_total as f64
        }
    }
}

/// Everything the engine reports while a run is in progress.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A run started or resumed.
    Started {
        run_id: String,
        run_dir: String,
        resumed: bool,
    },
    /// Updated progress and estimate.
    Progress(Snapshot),
    /// A line for the scrolling log.
    Log { message: String },
    /// The run completed; `root` is the root fingerprint.
    Finished { root: String },
    /// The run stopped on request; it can be resumed.
    Stopped,
}

/// Receives progress events. Implementations must be cheap: the engine calls
/// them from its working thread.
pub trait ProgressSink {
    fn event(&self, event: &Event);
}

/// A sink that ignores every event.
pub struct NullSink;

impl ProgressSink for NullSink {
    fn event(&self, _event: &Event) {}
}

/// Weight of the newest measurement in the moving average of throughput.
pub const EWMA_ALPHA: f64 = 0.3;

/// Cost model and throughput tracker for a sequence of stages.
#[derive(Debug, Clone)]
pub struct Estimator {
    stages: Vec<StageCost>,
}

#[derive(Debug, Clone)]
struct StageCost {
    name: String,
    units_total: u64,
    units_done: u64,
    /// Units per second; `None` until measured or calibrated.
    rate: Option<f64>,
}

impl Estimator {
    /// `stages`: (name, total work units) in run order.
    pub fn new(stages: &[(&str, u64)]) -> Self {
        Self {
            stages: stages
                .iter()
                .map(|(name, total)| StageCost {
                    name: (*name).to_string(),
                    units_total: *total,
                    units_done: 0,
                    rate: None,
                })
                .collect(),
        }
    }

    fn get(&mut self, name: &str) -> &mut StageCost {
        self.stages
            .iter_mut()
            .find(|s| s.name == name)
            .expect("stage known to the estimator")
    }

    /// Sets the starting throughput of a stage (calibration or stored value).
    pub fn set_rate(&mut self, name: &str, units_per_second: f64) {
        if units_per_second.is_finite() && units_per_second > 0.0 {
            self.get(name).rate = Some(units_per_second);
        }
    }

    pub fn rate(&self, name: &str) -> Option<f64> {
        self.stages.iter().find(|s| s.name == name)?.rate
    }

    /// Current throughput of every stage that has one.
    pub fn rates(&self) -> BTreeMap<String, f64> {
        self.stages
            .iter()
            .filter_map(|s| s.rate.map(|r| (s.name.clone(), r)))
            .collect()
    }

    /// Sets how many units of a stage are already done (for example on resume).
    pub fn set_done(&mut self, name: &str, units_done: u64) {
        let s = self.get(name);
        s.units_done = units_done.min(s.units_total);
    }

    /// Records that `units` were completed in `seconds`, and updates the
    /// moving average of throughput.
    pub fn record(&mut self, name: &str, units: u64, seconds: f64) {
        let s = self.get(name);
        s.units_done = (s.units_done + units).min(s.units_total);
        if units > 0 && seconds > 0.0 {
            let measured = units as f64 / seconds;
            s.rate = Some(match s.rate {
                Some(old) => EWMA_ALPHA * measured + (1.0 - EWMA_ALPHA) * old,
                None => measured,
            });
        }
    }

    fn cost(units: u64, rate: Option<f64>) -> Option<f64> {
        if units == 0 {
            Some(0.0)
        } else {
            rate.map(|r| units as f64 / r)
        }
    }

    /// Estimated remaining seconds over all stages, if every stage with
    /// remaining work has a throughput.
    pub fn remaining_seconds(&self) -> Option<f64> {
        self.stages
            .iter()
            .map(|s| Self::cost(s.units_total - s.units_done, s.rate))
            .sum()
    }

    /// Overall completion from 0 to 1, weighting each stage by its estimated
    /// cost. Stages without a throughput count by unit share.
    pub fn overall_fraction(&self) -> f64 {
        let mut done = 0.0;
        let mut total = 0.0;
        let all_rated = self.stages.iter().all(|s| s.rate.is_some());
        for s in &self.stages {
            let (d, t) = if all_rated {
                let r = s.rate.unwrap_or(1.0);
                (s.units_done as f64 / r, s.units_total as f64 / r)
            } else {
                (s.units_done as f64, s.units_total as f64)
            };
            done += d;
            total += t;
        }
        if total <= 0.0 {
            1.0
        } else {
            (done / total).clamp(0.0, 1.0)
        }
    }
}

/// Formats seconds as `H:MM:SS`.
pub fn format_duration(seconds: f64) -> String {
    let s = if seconds.is_finite() && seconds > 0.0 {
        seconds.round() as u64
    } else {
        0
    };
    format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_time_needs_rates() {
        let mut e = Estimator::new(&[("a", 100), ("b", 10)]);
        assert_eq!(e.remaining_seconds(), None);
        e.set_rate("a", 10.0);
        assert_eq!(e.remaining_seconds(), None);
        e.set_rate("b", 5.0);
        assert_eq!(e.remaining_seconds(), Some(12.0));
    }

    #[test]
    fn records_update_progress_and_moving_average() {
        let mut e = Estimator::new(&[("a", 100)]);
        e.record("a", 10, 1.0);
        assert_eq!(e.rate("a"), Some(10.0));
        e.record("a", 10, 0.5); // measured 20 per second
        let expected = EWMA_ALPHA * 20.0 + (1.0 - EWMA_ALPHA) * 10.0;
        assert!((e.rate("a").unwrap() - expected).abs() < 1e-12);
        assert!((e.overall_fraction() - 0.2).abs() < 1e-12);
        assert!((e.remaining_seconds().unwrap() - 80.0 / expected).abs() < 1e-9);
    }

    #[test]
    fn overall_fraction_is_weighted_by_cost() {
        // Stage a: 100 units at 100/s = 1 s. Stage b: 100 units at 1/s = 100 s.
        let mut e = Estimator::new(&[("a", 100), ("b", 100)]);
        e.set_rate("a", 100.0);
        e.set_rate("b", 1.0);
        e.set_done("a", 100);
        assert!((e.overall_fraction() - 1.0 / 101.0).abs() < 1e-12);
        e.set_done("b", 100);
        assert_eq!(e.overall_fraction(), 1.0);
        assert_eq!(e.remaining_seconds(), Some(0.0));
    }

    #[test]
    fn invalid_rates_are_ignored() {
        let mut e = Estimator::new(&[("a", 1)]);
        e.set_rate("a", 0.0);
        e.set_rate("a", f64::NAN);
        assert_eq!(e.rate("a"), None);
    }

    #[test]
    fn durations_are_formatted() {
        assert_eq!(format_duration(0.0), "0:00:00");
        assert_eq!(format_duration(59.6), "0:01:00");
        assert_eq!(format_duration(3_725.0), "1:02:05");
        assert_eq!(format_duration(-3.0), "0:00:00");
    }

    #[test]
    fn events_serialise_as_tagged_json() {
        let json = serde_json::to_string(&Event::Log {
            message: "hello".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"event":"log","message":"hello"}"#);
    }
}
