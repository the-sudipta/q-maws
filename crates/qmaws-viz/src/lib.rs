//! Figures for Q-MAWS: Quartet Halo Tree, tanglegram, rectangular tree,
//! interactive HTML export and growth animation.
//!
//! Status: from M6, small-multiple line charts as SVG (`chart`); from M9,
//! the circular cladogram layout and the provisional Halo Tree (`tree`) with
//! PNG and PDF output (`render`). The final tree figures are added in M10.

pub mod chart;
pub mod render;
pub mod tree;

/// Name of this crate, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Version of this crate, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(CRATE_NAME, "qmaws-viz");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
