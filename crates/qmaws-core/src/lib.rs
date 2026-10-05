//! All science of Q-MAWS: input cleaning, MAW extraction, matrices, quartet
//! pattern counts, weighting, amalgamation, support and evaluation metrics.
//!
//! This crate has no knowledge of files, terminals or windows. It receives
//! data and returns data.

pub mod amalgamate;
pub mod bootstrap;
pub mod calibration;
pub mod control;
pub mod input;
pub mod matrix;
pub mod maw;
pub mod metrics;
pub mod metrics_check;
pub mod newick;
pub mod quartet;
pub mod sim;
pub mod stats;
pub mod support;
pub mod teach;
pub mod weight;
pub mod worksheet;

/// Name of this crate, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Version of this crate, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(CRATE_NAME, "qmaws-core");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
