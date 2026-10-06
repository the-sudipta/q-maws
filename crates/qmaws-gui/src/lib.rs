//! Desktop GUI application for Q-MAWS (plan 4.10).
//!
//! The window offers the main menu of plan 5.2 (start, resume, verify) and
//! shows a running analysis with the live provisional Halo Tree, the live
//! worksheet, the stage log, progress, Pause and Stop. The engine runs in a
//! worker thread through [`controller::Controller`], which the tests (G10)
//! use without a window.

pub mod app;
pub mod controller;
pub mod figure_view;
pub mod fonts;
pub mod theme;
pub mod widgets;

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
        viewport: {
            let v = egui::ViewportBuilder::default()
                .with_title("Q-MAWS")
                .with_app_id("q-maws")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([1000.0, 640.0])
                .with_maximized(true);
            match qmaws_viz::icon::rgba(256) {
                Ok(rgba) => v.with_icon(egui::IconData {
                    rgba,
                    width: 256,
                    height: 256,
                }),
                Err(_) => v,
            }
        },
        ..Default::default()
    };
    let mut fonts_found = true;
    let result = eframe::run_native(
        "Q-MAWS",
        options,
        Box::new(|cc| {
            fonts_found = fonts::install(&cc.egui_ctx);
            theme::install(&cc.egui_ctx);
            // Development and documentation screenshots: a fixed theme.
            match std::env::var("QMAWS_GUI_THEME").as_deref() {
                Ok("light") => cc.egui_ctx.set_theme(egui::ThemePreference::Light),
                Ok("dark") => cc.egui_ctx.set_theme(egui::ThemePreference::Dark),
                _ => {}
            }
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
