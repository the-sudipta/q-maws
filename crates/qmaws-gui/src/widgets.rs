//! The building blocks of every page: cards, buttons, choice cards, the
//! stage list, status pills, stat tiles, banners and the logo. They take
//! their colours from [`Palette`] and their sizes from the theme, so every
//! page looks the same. Symbols (tick, running, queued) are drawn as shapes,
//! not taken from the font, so they look the same on every computer.

use crate::theme::{self, Palette, CARD_RADIUS, CONTROL_RADIUS};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, TextStyle, TextureHandle, Ui, Vec2,
};

fn font(ui: &Ui, style: TextStyle) -> FontId {
    style.resolve(ui.style())
}

/// A white (or ink) card with a hairline border.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(18))
        .show(ui, add)
        .inner
}

/// A card with less padding, for dense content.
pub fn compact_card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(14, 12))
        .show(ui, add)
        .inner
}

/// The heading block of a page: a small line above (context), the title
/// and an optional line of explanation.
pub fn page_header(ui: &mut Ui, eyebrow: &str, title: &str, subtitle: Option<&str>) {
    let p = Palette::of(ui.ctx());
    if !eyebrow.is_empty() {
        ui.label(
            egui::RichText::new(eyebrow)
                .text_style(theme::caption())
                .color(p.ink_faint),
        );
        ui.add_space(-4.0);
    }
    ui.label(
        egui::RichText::new(title)
            .text_style(theme::title())
            .color(p.ink),
    );
    if let Some(s) = subtitle {
        ui.add_space(-2.0);
        ui.label(egui::RichText::new(s).color(p.ink_soft));
    }
    ui.add_space(10.0);
}

/// A section title inside a page or card.
pub fn section_title(ui: &mut Ui, text: &str) {
    let p = Palette::of(ui.ctx());
    ui.label(
        egui::RichText::new(text)
            .text_style(theme::section())
            .color(p.ink),
    );
}

/// A small grey caption.
pub fn caption(ui: &mut Ui, text: impl Into<String>) {
    let p = Palette::of(ui.ctx());
    ui.label(
        egui::RichText::new(text.into())
            .text_style(theme::caption())
            .color(p.ink_faint),
    );
}

/// Supporting text.
pub fn soft(ui: &mut Ui, text: impl Into<String>) {
    let p = Palette::of(ui.ctx());
    ui.label(egui::RichText::new(text.into()).color(p.ink_soft));
}

fn button_size(ui: &Ui, text: &str, style: TextStyle) -> (Vec2, std::sync::Arc<egui::Galley>) {
    let pad = ui.spacing().button_padding;
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), font(ui, style), Color32::PLACEHOLDER);
    let size = Vec2::new(
        galley.size().x + 2.0 * pad.x,
        (galley.size().y + 2.0 * pad.y).max(ui.spacing().interact_size.y),
    );
    (size, galley)
}

/// The one filled button of a view: the main action.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    primary_button_enabled(ui, text, true)
}

/// The main action, greyed out (but still answering hovers) when `enabled`
/// is false.
pub fn primary_button_enabled(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let p = Palette::of(ui.ctx());
    let (size, galley) = button_size(ui, text, TextStyle::Button);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let fill = if !enabled {
            p.border
        } else if resp.hovered() || resp.is_pointer_button_down_on() {
            p.accent_hover
        } else {
            p.accent
        };
        let fg = if enabled { p.on_accent } else { p.ink_faint };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(CONTROL_RADIUS), fill);
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, fg);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// An outlined button for secondary actions.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    secondary_button_enabled(ui, text, true)
}

/// A secondary button that can be greyed out.
pub fn secondary_button_enabled(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let p = Palette::of(ui.ctx());
    let (size, galley) = button_size(ui, text, TextStyle::Button);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let (fill, stroke) = if !enabled {
            (p.card, p.border)
        } else if resp.hovered() {
            (p.sunken, p.ink_faint)
        } else {
            (p.card, p.border_strong)
        };
        let r = CornerRadius::same(CONTROL_RADIUS);
        ui.painter().rect_filled(rect, r, fill);
        ui.painter()
            .rect_stroke(rect, r, Stroke::new(1.0, stroke), StrokeKind::Inside);
        let fg = if enabled { p.ink } else { p.ink_faint };
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, fg);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A text-only button (links, small toggles).
pub fn quiet_button(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui.ctx());
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        font(ui, TextStyle::Body),
        Color32::PLACEHOLDER,
    );
    let size = galley.size() + Vec2::new(8.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let colour = if resp.hovered() {
        p.accent_hover
    } else {
        p.accent
    };
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, colour);
    if resp.hovered() {
        let y = rect.bottom() - 2.0;
        ui.painter().line_segment(
            [
                Pos2::new(rect.left() + 4.0, y),
                Pos2::new(rect.right() - 4.0, y),
            ],
            Stroke::new(1.0, colour),
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A selectable card for one choice of a step (title and one line).
pub fn choice_card(ui: &mut Ui, title: &str, detail: &str, selected: bool, width: f32) -> Response {
    let p = Palette::of(ui.ctx());
    let title_g = ui.painter().layout_no_wrap(
        title.to_string(),
        font(ui, theme::section()),
        Color32::PLACEHOLDER,
    );
    let detail_g = ui.painter().layout(
        detail.to_string(),
        font(ui, TextStyle::Body),
        Color32::PLACEHOLDER,
        width - 64.0,
    );
    let height = 18.0 + title_g.size().y + 4.0 + detail_g.size().y + 18.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if ui.is_rect_visible(rect) {
        let r = CornerRadius::same(CARD_RADIUS);
        let (fill, stroke) = if selected {
            (p.accent_wash, Stroke::new(1.5, p.accent))
        } else if resp.hovered() {
            (p.card, Stroke::new(1.0, p.ink_faint))
        } else {
            (p.card, Stroke::new(1.0, p.border))
        };
        ui.painter().rect_filled(rect, r, fill);
        ui.painter()
            .rect_stroke(rect, r, stroke, StrokeKind::Inside);
        // Radio mark.
        let c = Pos2::new(
            rect.left() + 22.0,
            rect.top() + 18.0 + title_g.size().y / 2.0,
        );
        ui.painter().circle_stroke(
            c,
            7.5,
            Stroke::new(1.5, if selected { p.accent } else { p.border_strong }),
        );
        if selected {
            ui.painter().circle_filled(c, 4.0, p.accent);
        }
        let x = rect.left() + 42.0;
        let th = title_g.size().y;
        ui.painter()
            .galley(Pos2::new(x, rect.top() + 18.0), title_g, p.ink);
        ui.painter()
            .galley(Pos2::new(x, rect.top() + 22.0 + th), detail_g, p.ink_soft);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The state of a stage or a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Done,
    Running,
    Queued,
    Failed,
}

/// Draws the small state mark: a tick in a green disc, an orange ring with
/// a dot, a grey dot, or a vermilion cross.
pub fn state_mark(ui: &Ui, centre: Pos2, state: State, radius: f32) {
    let p = Palette::of(ui.ctx());
    let painter = ui.painter();
    match state {
        State::Done => {
            painter.circle_filled(centre, radius, p.success);
            let s = radius * 0.5;
            let pts = [
                centre + Vec2::new(-s, 0.05 * s),
                centre + Vec2::new(-0.25 * s, 0.75 * s),
                centre + Vec2::new(s, -0.6 * s),
            ];
            painter.line_segment([pts[0], pts[1]], Stroke::new(1.8, p.card));
            painter.line_segment([pts[1], pts[2]], Stroke::new(1.8, p.card));
        }
        State::Running => {
            painter.circle_stroke(centre, radius - 1.0, Stroke::new(2.0, p.split));
            painter.circle_filled(centre, radius * 0.38, p.split);
        }
        State::Queued => {
            painter.circle_stroke(centre, radius - 1.5, Stroke::new(1.2, p.border_strong));
        }
        State::Failed => {
            painter.circle_filled(centre, radius, p.danger);
            let s = radius * 0.42;
            painter.line_segment(
                [centre + Vec2::new(-s, -s), centre + Vec2::new(s, s)],
                Stroke::new(1.8, p.card),
            );
            painter.line_segment(
                [centre + Vec2::new(-s, s), centre + Vec2::new(s, -s)],
                Stroke::new(1.8, p.card),
            );
        }
    }
}

/// One line of the stage list: mark, name and a right-aligned detail.
pub fn stage_row(ui: &mut Ui, state: State, name: &str, detail: &str) {
    let p = Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    state_mark(
        ui,
        Pos2::new(rect.left() + 8.0, rect.center().y),
        state,
        7.0,
    );
    let (name_colour, detail_colour) = match state {
        State::Queued => (p.ink_faint, p.ink_faint),
        State::Running => (p.ink, p.ink),
        State::Done => (p.ink_soft, p.ink_faint),
        State::Failed => (p.danger, p.danger),
    };
    let body = font(ui, TextStyle::Body);
    let small = font(ui, theme::caption());
    ui.painter().text(
        Pos2::new(rect.left() + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        body,
        name_colour,
    );
    ui.painter().text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        detail,
        small,
        detail_colour,
    );
}

/// A small rounded label with a coloured background.
pub fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font(ui, theme::caption()), fg);
    let size = galley.size() + Vec2::new(18.0, 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(255), bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// A tile with a small label above a value.
pub fn stat(ui: &mut Ui, label: &str, value: &str, width: f32) {
    let p = Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.sunken)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(label)
                        .text_style(theme::caption())
                        .color(p.ink_faint),
                );
                ui.add_space(-6.0);
                ui.label(
                    egui::RichText::new(value)
                        .text_style(theme::section())
                        .color(p.ink),
                );
            });
        });
}

/// A thin rounded progress bar.
pub fn progress_bar(ui: &mut Ui, fraction: f32, height: f32, colour: Color32) {
    let p = Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let r = CornerRadius::same((height / 2.0).round() as u8);
    ui.painter().rect_filled(rect, r, p.sunken);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let filled = Rect::from_min_size(rect.min, Vec2::new((width * f).max(height), height));
        ui.painter().rect_filled(filled, r, colour);
    }
}

/// The kind of a banner.
#[derive(Debug, Clone, Copy)]
pub enum Tone {
    Info,
    Success,
    Warning,
    Danger,
}

/// A full-width message with a coloured edge.
pub fn banner(ui: &mut Ui, tone: Tone, text: &str) {
    let p = Palette::of(ui.ctx());
    let (bg, edge) = match tone {
        Tone::Info => (p.accent_wash, p.accent),
        Tone::Success => (p.success_wash, p.success),
        Tone::Warning => (p.warning_wash, p.split),
        Tone::Danger => (p.danger_wash, p.danger),
    };
    let resp = egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin {
            left: 16,
            right: 14,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).color(p.ink));
        })
        .response;
    let r = resp.rect;
    ui.painter().rect_filled(
        Rect::from_min_max(r.min, Pos2::new(r.left() + 3.0, r.bottom())),
        CornerRadius {
            nw: 8,
            sw: 8,
            ne: 0,
            se: 0,
        },
        edge,
    );
}

/// A label–value line of a summary (review page, run details).
pub fn key_value(ui: &mut Ui, key: &str, value: &str) {
    let p = Palette::of(ui.ctx());
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(170.0, 20.0),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.set_min_width(170.0);
                ui.label(egui::RichText::new(key).color(p.ink_faint));
            },
        );
        ui.label(egui::RichText::new(value).color(p.ink));
    });
}

/// A thin horizontal line.
pub fn rule(ui: &mut Ui) {
    let p = Palette::of(ui.ctx());
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, p.border),
    );
}

/// The logo texture (made once from the SVG).
pub fn logo_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::new("q-maws-logo");
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(t);
    }
    let pixels = qmaws_viz::icon::rgba(256).ok()?;
    let image = egui::ColorImage::from_rgba_unmultiplied([256, 256], &pixels);
    let t = ctx.load_texture("q-maws-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

/// The logo at `size` points.
pub fn logo(ui: &mut Ui, size: f32) {
    if let Some(t) = logo_texture(ui.ctx()) {
        ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::splat(size)));
    }
}

/// An item of the sidebar: text, a marker bar when selected.
pub fn sidebar_item(ui: &mut Ui, text: &str, selected: bool, enabled: bool) -> Response {
    let p = Palette::of(ui.ctx());
    let width = ui.available_width();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 36.0), sense);
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), p.sidebar_active);
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(0.0, 8.0), Vec2::new(3.0, 20.0)),
            CornerRadius::same(2),
            p.split,
        );
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(7),
            p.sidebar_active.gamma_multiply(0.55),
        );
    }
    let colour = if !enabled {
        p.sidebar_faint.gamma_multiply(0.6)
    } else if selected {
        Color32::WHITE
    } else {
        p.sidebar_ink
    };
    ui.painter().text(
        Pos2::new(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font(ui, TextStyle::Body),
        colour,
    );
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A small heading in the sidebar.
pub fn sidebar_heading(ui: &mut Ui, text: &str) {
    let p = Palette::of(ui.ctx());
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(text)
            .text_style(theme::caption())
            .color(p.sidebar_faint),
    );
}
