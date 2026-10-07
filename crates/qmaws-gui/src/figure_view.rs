//! The figure viewer: shows an SVG figure of a run (the live provisional
//! Halo Tree while quartets are weighed, the final Halo Tree when the run
//! has finished) exactly as the figure files are drawn, because it renders
//! the same SVG file the run writes. Rendering runs on a worker thread, at
//! a resolution high enough for zooming; the window never waits for it.
//! When the file changes (a new provisional tree), it is rendered again and
//! replaces the picture in place, keeping the zoom.

use crate::theme::{Palette, CARD_RADIUS};
use eframe::egui::{self, CornerRadius, Rect, Stroke, TextureHandle, Vec2};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// A rendered picture: file, its modification time, size and pixels.
struct Rendered {
    path: PathBuf,
    stamp: SystemTime,
    size: [usize; 2],
    pixels: Vec<u8>,
    /// The tree square, as fractions of the figure (left, top, width, height).
    focus: Option<[f32; 4]>,
}

/// The title and tree of a Halo Tree SVG, as fractions of the figure:
/// the figure starts with its title lines (18 units each, plus 10), then
/// the tree as a square as wide as the figure, then the legend. `None` for
/// other drawings.
fn tree_square(svg: &str) -> Option<[f32; 4]> {
    let number = |key: &str| -> Option<f32> {
        let head = svg.get(..svg.find('>')?)?;
        let start = head.find(&format!("{key}=\""))? + key.len() + 2;
        let end = start + head[start..].find('"')?;
        head[start..end].parse().ok()
    };
    let (w, h) = (number("width")?, number("height")?);
    let first_group = svg.find("<g")?;
    let lines = svg[..first_group].matches("<text").count();
    if lines == 0 || w <= 0.0 || h <= w {
        return None;
    }
    // The tree square (with the provisional label in its corner); the
    // title is shown by the window, the legend with "Whole figure".
    let top = 18.0 * lines as f32 + 10.0;
    Some([0.0, top / h, 1.0, w / h])
}

pub struct FigureView {
    path: Option<PathBuf>,
    /// An SVG made in memory (the live tree with changed branches), with
    /// its version, instead of a file.
    text: Option<(u64, Arc<String>)>,
    /// Version of the in-memory SVG shown (or being rendered).
    text_done: Option<u64>,
    width: u32,
    /// Modification time of the file shown (or being rendered).
    stamp: Option<SystemTime>,
    texture: Option<TextureHandle>,
    /// Size of the picture in points at zoom 1.
    size: Vec2,
    pending: Option<Receiver<Result<Rendered, String>>>,
    last_check: Option<Instant>,
    scene: Rect,
    /// The tree square in picture coordinates, when the figure has one.
    focus: Option<Rect>,
    error: Option<String>,
}

impl Default for FigureView {
    fn default() -> Self {
        Self {
            path: None,
            text: None,
            text_done: None,
            width: 0,
            stamp: None,
            texture: None,
            size: Vec2::ZERO,
            pending: None,
            last_check: None,
            scene: Rect::ZERO,
            focus: None,
            error: None,
        }
    }
}

impl FigureView {
    /// Shows `path`, rendered `width` pixels wide. Calling it again with the
    /// same file keeps the picture; a new file replaces it once rendered.
    pub fn show_file(&mut self, path: &Path, width: u32) {
        if self.path.as_deref() != Some(path) || self.width != width || self.text.is_some() {
            self.path = Some(path.to_path_buf());
            self.text = None;
            self.width = width;
            self.stamp = None;
            self.last_check = None;
            self.error = None;
        }
    }

    /// Shows an SVG made in memory; a higher `version` replaces the picture
    /// once rendered.
    pub fn show_svg(&mut self, version: u64, svg: Arc<String>, width: u32) {
        if self.text.as_ref().map(|(v, _)| *v) != Some(version) {
            self.text = Some((version, svg));
            self.path = None;
            self.width = width;
            self.error = None;
        }
    }

    /// True when the picture comes from an SVG made in memory.
    pub fn shows_svg(&self) -> bool {
        self.text.is_some()
    }

    /// Forgets the file (nothing is shown).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// True when a picture is shown or being made.
    pub fn has_picture(&self) -> bool {
        self.texture.is_some() || self.pending.is_some()
    }

    /// Resets zoom and pan to show the whole figure (title and legend).
    pub fn fit(&mut self) {
        self.scene = Rect::ZERO;
    }

    /// Zooms in (`factor` > 1) or out around the centre of the view.
    pub fn zoom(&mut self, factor: f32) {
        if self.scene.is_finite() && self.scene.size() != Vec2::ZERO {
            self.scene = Rect::from_center_size(self.scene.center(), self.scene.size() / factor);
        } else if let Some(f) = self.focus {
            self.scene = Rect::from_center_size(f.center(), f.size() / factor);
        }
    }

    /// Zooms onto the tree itself (the default view of a Halo Tree).
    pub fn focus_tree(&mut self) {
        self.scene = self.focus.unwrap_or(Rect::ZERO);
    }

    /// True when the figure has a tree square to zoom onto.
    pub fn has_tree_view(&self) -> bool {
        self.focus.is_some()
    }

    /// Checks the file (at most once a second) and collects a finished
    /// rendering. `wake` asks the window to repaint when a rendering ends.
    pub fn poll(&mut self, ctx: &egui::Context, wake: &Arc<dyn Fn() + Send + Sync>) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(Ok(r)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(r.size, &r.pixels);
                    let first = self.texture.is_none();
                    match &mut self.texture {
                        Some(t) => t.set(image, egui::TextureOptions::LINEAR),
                        None => {
                            self.texture = Some(ctx.load_texture(
                                "figure",
                                image,
                                egui::TextureOptions::LINEAR,
                            ))
                        }
                    }
                    // Two pixels per point: sharp at zoom 2 on most screens.
                    let new_size = Vec2::new(r.size[0] as f32, r.size[1] as f32) / 2.0;
                    self.focus = r.focus.map(|[x, y, w, h]| {
                        Rect::from_min_size(
                            egui::pos2(x * new_size.x, y * new_size.y),
                            Vec2::new(w * new_size.x, h * new_size.y),
                        )
                    });
                    if first || (new_size - self.size).length() > 1.0 {
                        self.scene = self.focus.unwrap_or(Rect::ZERO);
                    }
                    self.size = new_size;
                    self.stamp = Some(r.stamp);
                    self.error = None;
                    self.pending = None;
                    let _ = r.path;
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
            }
            return;
        }
        if let Some((version, svg)) = self.text.clone() {
            if self.text_done == Some(version) {
                return;
            }
            self.text_done = Some(version);
            let (tx, rx) = mpsc::channel();
            let width = self.width;
            let wake = Arc::clone(wake);
            std::thread::spawn(move || {
                let focus = tree_square(&svg);
                let result = qmaws_viz::render::rgba(&svg, width).map(|(w, h, pixels)| Rendered {
                    path: PathBuf::new(),
                    stamp: SystemTime::UNIX_EPOCH,
                    size: [w as usize, h as usize],
                    pixels,
                    focus,
                });
                let _ = tx.send(result);
                wake();
            });
            self.pending = Some(rx);
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        if self
            .last_check
            .is_some_and(|t| t.elapsed() < Duration::from_millis(900))
        {
            return;
        }
        self.last_check = Some(Instant::now());
        let Ok(stamp) = std::fs::metadata(&path).and_then(|m| m.modified()) else {
            return;
        };
        if self.stamp == Some(stamp) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let width = self.width;
        let wake = Arc::clone(wake);
        std::thread::spawn(move || {
            let result = std::fs::read_to_string(&path)
                .map_err(|e| format!("{}: {e}", path.display()))
                .and_then(|svg| {
                    let focus = tree_square(&svg);
                    qmaws_viz::render::rgba(&svg, width).map(|r| (r, focus))
                })
                .map(|((w, h, pixels), focus)| Rendered {
                    path,
                    stamp,
                    size: [w as usize, h as usize],
                    pixels,
                    focus,
                });
            let _ = tx.send(result);
            wake();
        });
        self.pending = Some(rx);
        // Mark as seen so a file half-written is not rendered twice.
        self.stamp = Some(stamp);
    }

    /// Draws the figure filling the space given: a paper sheet that can be
    /// zoomed (scroll or pinch) and moved (drag). `empty` is shown before
    /// the first picture.
    pub fn show(&mut self, ui: &mut egui::Ui, empty: &str) {
        let p = Palette::of(ui.ctx());
        let rect = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(rect, CornerRadius::same(CARD_RADIUS), p.card);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(CARD_RADIUS),
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
        let Some(texture) = self.texture.clone() else {
            let text = match (&self.error, &self.pending) {
                (Some(e), _) => format!("The figure could not be drawn: {e}"),
                (None, Some(_)) => "Drawing the figure\u{2026}".to_string(),
                (None, None) => empty.to_string(),
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::TextStyle::Body.resolve(ui.style()),
                p.ink_faint,
            );
            ui.allocate_rect(rect, egui::Sense::hover());
            return;
        };
        let size = self.size;
        let inner = rect.shrink(6.0);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        child.set_clip_rect(inner);
        egui::Scene::new()
            .zoom_range(0.15..=6.0)
            .show(&mut child, &mut self.scene, |ui| {
                // The figure is drawn on white paper in both themes, as it
                // is printed.
                let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(r, CornerRadius::ZERO, egui::Color32::WHITE);
                egui::Image::new(&texture).paint_at(ui, r);
            });
        ui.allocate_rect(rect, egui::Sense::hover());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_square_follows_the_title_lines() {
        let svg = r##"<svg width="800" height="1000"><rect/><text>a</text><text>b</text><text>c</text><g></g></svg>"##;
        let [x, y, w, h] = tree_square(svg).unwrap();
        assert_eq!((x, w), (0.0, 1.0));
        assert!((y - 0.064).abs() < 1e-6);
        assert!((h - 0.8).abs() < 1e-6);
        assert!(tree_square("<svg width=\"10\" height=\"10\"><g></g></svg>").is_none());
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;

    /// Runs one frame of a window showing `v` in a 800 × 600 area.
    fn frame(ctx: &egui::Context, v: &mut FigureView) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(800.0, 600.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| v.show(ui, ""));
    }

    #[test]
    fn whole_figure_and_tree_change_the_view() {
        let ctx = egui::Context::default();
        let mut v = FigureView::default();
        let image =
            egui::ColorImage::from_rgba_unmultiplied([200, 400], &vec![255u8; 200 * 400 * 4]);
        v.texture = Some(ctx.load_texture("t", image, egui::TextureOptions::LINEAR));
        v.size = Vec2::new(100.0, 200.0);
        v.focus = Some(Rect::from_min_size(
            egui::pos2(0.0, 20.0),
            Vec2::new(100.0, 100.0),
        ));
        v.scene = v.focus.unwrap();
        frame(&ctx, &mut v);
        frame(&ctx, &mut v);
        let tree = v.scene;
        v.fit();
        frame(&ctx, &mut v);
        frame(&ctx, &mut v);
        let whole = v.scene;
        assert!(
            whole.height() > tree.height() * 1.5,
            "tree {tree:?} whole {whole:?}"
        );
        v.focus_tree();
        frame(&ctx, &mut v);
        frame(&ctx, &mut v);
        assert!(
            (v.scene.height() - tree.height()).abs() < 1.0,
            "back {:?}",
            v.scene
        );
        let before = v.scene.height();
        v.zoom(2.0);
        frame(&ctx, &mut v);
        assert!(
            (v.scene.height() * 2.0 - before).abs() < 1.0,
            "zoom {:?}",
            v.scene
        );
    }
}
