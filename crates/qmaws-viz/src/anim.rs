//! The growth animation `halo_tree_growth.gif` (plan 4.7): the frames of
//! the live provisional tree, then the final tree, as an animated GIF that
//! repeats. Colours are reduced to 256 per frame with the NeuQuant
//! quantiser of the `gif` crate.

/// Encodes PNG images (same width and height) as a looping GIF. Each frame
/// is shown `delay_ms` milliseconds, the last one `last_ms`.
pub fn gif_from_pngs(frames: &[Vec<u8>], delay_ms: u32, last_ms: u32) -> Result<Vec<u8>, String> {
    let first = frames.first().ok_or("no frames")?;
    let decode = |bytes: &[u8]| {
        resvg::tiny_skia::Pixmap::decode_png(bytes).map_err(|e| format!("reading a frame: {e}"))
    };
    let size = decode(first)?;
    let (w, h) = (size.width(), size.height());
    let (gw, gh) = (
        u16::try_from(w).map_err(|_| "frame too wide for a GIF")?,
        u16::try_from(h).map_err(|_| "frame too high for a GIF")?,
    );
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, gw, gh, &[]).map_err(|e| e.to_string())?;
        enc.set_repeat(gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;
        for (i, png) in frames.iter().enumerate() {
            let mut pixmap = decode(png)?;
            if (pixmap.width(), pixmap.height()) != (w, h) {
                // Another size (for example a frame drawn by an earlier
                // version): placed at the top left of a white frame.
                let mut canvas = resvg::tiny_skia::Pixmap::new(w, h).ok_or("bad frame size")?;
                canvas.fill(resvg::tiny_skia::Color::WHITE);
                canvas.draw_pixmap(
                    0,
                    0,
                    pixmap.as_ref(),
                    &resvg::tiny_skia::PixmapPaint::default(),
                    resvg::tiny_skia::Transform::identity(),
                    None,
                );
                pixmap = canvas;
            }
            // Frames are opaque, so premultiplied and straight RGBA agree.
            let mut rgba = pixmap.data().to_vec();
            let mut frame = gif::Frame::from_rgba_speed(gw, gh, &mut rgba, 10);
            let ms = if i + 1 == frames.len() {
                last_ms
            } else {
                delay_ms
            };
            frame.delay = (ms / 10).min(u16::MAX as u32) as u16;
            enc.write_frame(&frame).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(colour: &str) -> Vec<u8> {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="{colour}"/></svg>"#
        );
        crate::render::png(&svg, 40).unwrap()
    }

    #[test]
    fn frames_become_a_looping_gif() {
        let gif = gif_from_pngs(&[png("#ff0000"), png("#0000ff")], 600, 3000).unwrap();
        assert!(gif.starts_with(b"GIF89a"));
        // Logical screen 40 x 20.
        assert_eq!(u16::from_le_bytes([gif[6], gif[7]]), 40);
        assert_eq!(u16::from_le_bytes([gif[8], gif[9]]), 20);
        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let mut dec = opts.read_info(&gif[..]).unwrap();
        let mut delays = Vec::new();
        while let Some(f) = dec.read_next_frame().unwrap() {
            delays.push(f.delay);
        }
        assert_eq!(delays, vec![60, 300]);
    }

    #[test]
    fn frames_of_another_size_are_placed_on_a_white_frame() {
        let other = crate::render::png(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40"/></svg>"#,
            40,
        )
        .unwrap();
        let gif = gif_from_pngs(&[png("#ff0000"), other], 600, 3000).unwrap();
        assert_eq!(u16::from_le_bytes([gif[8], gif[9]]), 20);
        assert!(gif_from_pngs(&[], 600, 3000).is_err());
    }
}
