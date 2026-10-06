//! The look of the window: two palettes ("paper" and "ink"), type sizes,
//! spacing and the style of every control. Every page takes its colours from
//! [`Palette`], so the light and dark themes stay consistent.
//!
//! The colours follow the logo: a deep navy ink, the Okabe–Ito colour-blind
//! safe hues for meaning (blue for actions and progress, bluish green for
//! finished, orange for running, vermilion for problems), and the orange of
//! the logo's central split as the one accent of the sidebar.

use eframe::egui::{
    self, epaint::Shadow, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle,
    Vec2,
};

/// The colours of one theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Window background.
    pub canvas: Color32,
    /// Cards and panels.
    pub card: Color32,
    /// Inputs, tiles and other sunken areas.
    pub sunken: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    /// Main text.
    pub ink: Color32,
    /// Supporting text.
    pub ink_soft: Color32,
    /// Captions and hints.
    pub ink_faint: Color32,
    /// Actions and progress.
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Text on the accent colour.
    pub on_accent: Color32,
    pub accent_wash: Color32,
    /// The orange of the logo's central split; also "running".
    pub split: Color32,
    pub success: Color32,
    pub success_wash: Color32,
    pub danger: Color32,
    pub danger_wash: Color32,
    pub warning_wash: Color32,
    pub sidebar: Color32,
    pub sidebar_ink: Color32,
    pub sidebar_faint: Color32,
    pub sidebar_active: Color32,
}

impl Palette {
    /// Warm paper with navy ink.
    pub const LIGHT: Palette = Palette {
        dark: false,
        canvas: Color32::from_rgb(0xF6, 0xF4, 0xEF),
        card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        sunken: Color32::from_rgb(0xEF, 0xEC, 0xE4),
        border: Color32::from_rgb(0xE3, 0xDE, 0xD2),
        border_strong: Color32::from_rgb(0xCB, 0xC4, 0xB4),
        ink: Color32::from_rgb(0x0B, 0x25, 0x45),
        ink_soft: Color32::from_rgb(0x4A, 0x5B, 0x74),
        ink_faint: Color32::from_rgb(0x86, 0x92, 0xA4),
        accent: Color32::from_rgb(0x00, 0x72, 0xB2),
        accent_hover: Color32::from_rgb(0x00, 0x5E, 0x93),
        on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        accent_wash: Color32::from_rgb(0xE3, 0xEF, 0xF7),
        split: Color32::from_rgb(0xE6, 0x9F, 0x00),
        success: Color32::from_rgb(0x00, 0x7F, 0x5C),
        success_wash: Color32::from_rgb(0xE1, 0xF2, 0xEB),
        danger: Color32::from_rgb(0xB8, 0x4A, 0x00),
        danger_wash: Color32::from_rgb(0xFA, 0xE8, 0xDE),
        warning_wash: Color32::from_rgb(0xFB, 0xF0, 0xD6),
        sidebar: Color32::from_rgb(0x0B, 0x25, 0x45),
        sidebar_ink: Color32::from_rgb(0xD5, 0xDE, 0xEA),
        sidebar_faint: Color32::from_rgb(0x84, 0x99, 0xB6),
        sidebar_active: Color32::from_rgb(0x1A, 0x40, 0x6C),
    };

    /// Night ink: the same hues, lighter where they carry text.
    pub const DARK: Palette = Palette {
        dark: true,
        canvas: Color32::from_rgb(0x0D, 0x16, 0x24),
        card: Color32::from_rgb(0x13, 0x1F, 0x31),
        sunken: Color32::from_rgb(0x0A, 0x12, 0x1F),
        border: Color32::from_rgb(0x22, 0x31, 0x48),
        border_strong: Color32::from_rgb(0x30, 0x43, 0x61),
        ink: Color32::from_rgb(0xE7, 0xED, 0xF5),
        ink_soft: Color32::from_rgb(0xA7, 0xB5, 0xC8),
        ink_faint: Color32::from_rgb(0x6C, 0x7D, 0x95),
        accent: Color32::from_rgb(0x56, 0xB4, 0xE9),
        accent_hover: Color32::from_rgb(0x7C, 0xC6, 0xF0),
        on_accent: Color32::from_rgb(0x06, 0x1B, 0x2E),
        accent_wash: Color32::from_rgb(0x12, 0x2D, 0x47),
        split: Color32::from_rgb(0xF0, 0xB0, 0x2A),
        success: Color32::from_rgb(0x3F, 0xC2, 0x9A),
        success_wash: Color32::from_rgb(0x0F, 0x2E, 0x28),
        danger: Color32::from_rgb(0xF0, 0x8A, 0x4B),
        danger_wash: Color32::from_rgb(0x3A, 0x1F, 0x14),
        warning_wash: Color32::from_rgb(0x3A, 0x2E, 0x10),
        sidebar: Color32::from_rgb(0x07, 0x10, 0x1C),
        sidebar_ink: Color32::from_rgb(0xD5, 0xDE, 0xEA),
        sidebar_faint: Color32::from_rgb(0x6F, 0x83, 0x9E),
        sidebar_active: Color32::from_rgb(0x16, 0x2B, 0x48),
    };

    /// The palette of the current theme.
    pub fn of(ctx: &egui::Context) -> Palette {
        if ctx.theme() == egui::Theme::Dark {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

/// Corner radius of cards.
pub const CARD_RADIUS: u8 = 10;
/// Corner radius of buttons and inputs.
pub const CONTROL_RADIUS: u8 = 7;

/// Named text styles beyond egui's own.
pub fn display() -> TextStyle {
    TextStyle::Name("display".into())
}
pub fn title() -> TextStyle {
    TextStyle::Name("title".into())
}
pub fn section() -> TextStyle {
    TextStyle::Name("section".into())
}
pub fn caption() -> TextStyle {
    TextStyle::Name("caption".into())
}
pub fn figure() -> TextStyle {
    TextStyle::Name("figure".into())
}

/// Applies both themes to the context; egui switches between them with the
/// system or the user's choice.
pub fn install(ctx: &egui::Context) {
    ctx.style_mut_of(egui::Theme::Light, |s| style(s, &Palette::LIGHT));
    ctx.style_mut_of(egui::Theme::Dark, |s| style(s, &Palette::DARK));
}

fn style(s: &mut egui::Style, p: &Palette) {
    use FontFamily::{Monospace, Proportional};
    s.text_styles = [
        (TextStyle::Small, FontId::new(12.0, Proportional)),
        (TextStyle::Body, FontId::new(14.5, Proportional)),
        (TextStyle::Button, FontId::new(14.0, Proportional)),
        (TextStyle::Heading, FontId::new(20.0, Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, Monospace)),
        (display(), FontId::new(34.0, Proportional)),
        (title(), FontId::new(24.0, Proportional)),
        (section(), FontId::new(16.5, Proportional)),
        (caption(), FontId::new(12.0, Proportional)),
        (figure(), FontId::new(30.0, Proportional)),
    ]
    .into();

    let sp = &mut s.spacing;
    sp.item_spacing = Vec2::new(10.0, 9.0);
    sp.button_padding = Vec2::new(14.0, 7.0);
    sp.interact_size = Vec2::new(40.0, 32.0);
    sp.window_margin = Margin::same(16);
    sp.menu_margin = Margin::same(8);
    sp.indent = 18.0;
    sp.slider_width = 220.0;
    sp.text_edit_width = 420.0;
    sp.combo_width = 420.0;
    sp.scroll.bar_width = 7.0;
    sp.scroll.floating = true;

    let v = &mut s.visuals;
    *v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.override_text_color = Some(p.ink);
    v.weak_text_color = Some(p.ink_faint);
    v.panel_fill = p.canvas;
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    v.menu_corner_radius = CornerRadius::same(CONTROL_RADIUS);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 28 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 100 } else { 24 }),
    };
    v.extreme_bg_color = p.sunken;
    v.text_edit_bg_color = Some(p.card);
    v.faint_bg_color = p.sunken;
    v.code_bg_color = p.sunken;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.split;
    v.error_fg_color = p.danger;
    v.selection.bg_fill = p.accent_wash;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.striped = false;
    v.slider_trailing_fill = true;
    v.indent_has_left_vline = false;

    let radius = CornerRadius::same(CONTROL_RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.card;
    w.noninteractive.weak_bg_fill = p.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = p.sunken;
    w.inactive.weak_bg_fill = p.card;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border_strong);
    w.inactive.fg_stroke = Stroke::new(1.0, p.ink);
    w.inactive.corner_radius = radius;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.sunken;
    w.hovered.weak_bg_fill = p.sunken;
    w.hovered.bg_stroke = Stroke::new(1.0, p.ink_faint);
    w.hovered.fg_stroke = Stroke::new(1.0, p.ink);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.border;
    w.active.weak_bg_fill = p.border;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.ink);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open = w.active;
}
