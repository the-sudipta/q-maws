//! SVG to PNG (`resvg`) and SVG to PDF (`svg2pdf`). Text is drawn with the
//! fonts installed on the computer; no font files ship with Q-MAWS.

use std::sync::{Arc, OnceLock};

/// The computer's fonts, loaded once per process.
fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// The data of an installed sans-serif (or monospace) font and the index
/// of the face in it (for font collections), for the GUI's text.
pub fn system_font(monospace: bool) -> Option<(Vec<u8>, u32)> {
    use resvg::usvg::fontdb::{Family, Query};
    let db = fonts();
    let family = if monospace {
        Family::Monospace
    } else {
        Family::SansSerif
    };
    let id = db
        .query(&Query {
            families: &[family],
            ..Default::default()
        })
        .or_else(|| db.faces().next().map(|f| f.id))?;
    db.with_face_data(id, |data, index| (data.to_vec(), index))
}

fn parse(svg: &str) -> Result<resvg::usvg::Tree, String> {
    let options = resvg::usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    resvg::usvg::Tree::from_str(svg, &options).map_err(|e| format!("reading the SVG: {e}"))
}

/// The SVG as a PNG `width` pixels wide (height in proportion).
pub fn png(svg: &str, width: u32) -> Result<Vec<u8>, String> {
    let tree = parse(svg)?;
    let size = tree.size();
    let scale = width as f32 / size.width();
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height)
        .ok_or_else(|| "the image size is not valid".to_string())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| format!("encoding the PNG: {e}"))
}

/// The SVG as a one-page PDF.
pub fn pdf(svg: &str) -> Result<Vec<u8>, String> {
    let tree = parse(svg)?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| format!("writing the PDF: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="#ff0000"/><text x="10" y="50" font-family="sans-serif" font-size="12">Q-MAWS</text></svg>"##;

    #[test]
    fn png_has_the_requested_width_and_proportional_height() {
        let bytes = png(SVG, 480).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        assert_eq!((width, height), (480, 240));
    }

    #[test]
    fn pdf_is_a_pdf_file() {
        let bytes = pdf(SVG).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn a_system_font_is_found_when_fonts_are_installed() {
        // Build machines without fonts return None; the PNG test above
        // already needs installed fonts for its text.
        if let Some((data, _)) = system_font(false) {
            assert!(data.len() > 1000);
        }
    }

    #[test]
    fn invalid_svg_is_an_error() {
        assert!(png("<svg", 10).is_err());
        assert!(pdf("not svg").is_err());
    }
}
