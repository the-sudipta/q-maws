//! Desktop GUI application for Q-MAWS (plan 4.10).
//!
//! The window offers the main menu of plan 5.2 (start, resume, verify) and
//! shows a running analysis with the live provisional Halo Tree, the live
//! worksheet, the stage log, progress, Pause and Stop. The engine runs in a
//! worker thread through [`controller::Controller`], which the tests (G10)
//! use without a window.

pub mod app;
pub mod controller;
pub mod fonts;

pub use app::{DownloadFn, Launch};
pub use controller::{Controller, Job, Message};

use eframe::egui;

/// Name of this crate, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Version of this crate, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Opens the window and returns when it is closed. A run still working when
/// the window closes is stopped cleanly and can be resumed.
pub fn run(launch: Launch) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Q-MAWS {VERSION}"))
            .with_inner_size([1240.0, 700.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    let mut fonts_found = true;
    let result = eframe::run_native(
        "Q-MAWS",
        options,
        Box::new(|cc| {
            fonts_found = fonts::install(&cc.egui_ctx);
            Ok(Box::new(app::App::new(&cc.egui_ctx, launch)))
        }),
    );
    if !fonts_found {
        return Err("no font is installed on this computer, so the window cannot show text; use the terminal mode instead".into());
    }
    result.map_err(|e| format!("the window could not be opened: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(CRATE_NAME, "qmaws-gui");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
