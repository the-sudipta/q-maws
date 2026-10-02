//! Run orchestration for Q-MAWS: stages, checkpoints, chunks, progress events
//! and the audit log.
//!
//! Every output is written atomically with a SHA-256 hash file, so a run can be
//! stopped or killed at any moment and resumed later with identical results.
//! In this milestone the engine runs a toy computation that exercises chunks,
//! checkpoints, resume, progress and time estimates.

pub mod atomic;
pub mod clock;
pub mod hash;
pub mod matrix_pipeline;
pub mod progress;
pub mod rundir;
pub mod runner;
pub mod state;
pub mod toy;

#[cfg(test)]
mod testutil;

pub use progress::{Event, NullSink, ProgressSink, Snapshot};
pub use runner::{resume_run, start_toy_run, EngineError, Outcome, ToyOptions};

/// Name of this crate, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Version of this crate, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(CRATE_NAME, "qmaws-engine");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
