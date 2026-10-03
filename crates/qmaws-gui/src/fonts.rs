//! Fonts of the window. The GUI is built without egui's built-in fonts, so
//! no font files ship with Q-MAWS: a sans-serif and a monospace font
//! installed on the computer are loaded at start. There are no emoji or icon
//! glyphs; buttons use plain text.

use eframe::egui;
use std::sync::Arc;

/// Loads the computer's fonts into `ctx`. Returns false when no font was
/// found (the window cannot show text then).
pub fn install(ctx: &egui::Context) -> bool {
    let sans = qmaws_viz::render::system_font(false);
    let mono = qmaws_viz::render::system_font(true);
    let Some((sans_data, sans_index)) = sans.clone().or_else(|| mono.clone()) else {
        return false;
    };
    let (mono_data, mono_index) = mono.unwrap_or((sans_data.clone(), sans_index));
    let mut defs = egui::FontDefinitions::empty();
    let mut sans_font = egui::FontData::from_owned(sans_data);
    sans_font.index = sans_index;
    let mut mono_font = egui::FontData::from_owned(mono_data);
    mono_font.index = mono_index;
    defs.font_data
        .insert("system-sans".into(), Arc::new(sans_font));
    defs.font_data
        .insert("system-mono".into(), Arc::new(mono_font));
    defs.families.insert(
        egui::FontFamily::Proportional,
        vec!["system-sans".into(), "system-mono".into()],
    );
    defs.families.insert(
        egui::FontFamily::Monospace,
        vec!["system-mono".into(), "system-sans".into()],
    );
    ctx.set_fonts(defs);
    true
}
