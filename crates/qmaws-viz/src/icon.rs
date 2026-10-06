//! The program icon: the Q-MAWS logo (a quartet tree whose central split is
//! drawn in orange) and its files for each operating system. The logo is
//! one SVG (`assets/icon/q-maws.svg`); the PNG, Windows `.ico` and macOS
//! `.icns` files are made from it by `qmaws icons` with the same renderer as
//! the figures, so no image tool is needed.

/// The logo as SVG.
pub const LOGO_SVG: &str = include_str!("../../../assets/icon/q-maws.svg");

/// Sizes of the PNG files, in pixels.
pub const PNG_SIZES: [u32; 9] = [16, 24, 32, 48, 64, 128, 256, 512, 1024];

/// Sizes in the Windows icon.
pub const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// macOS icon types and their sizes (all stored as PNG).
pub const ICNS_TYPES: [(&[u8; 4], u32); 7] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"icp6", 64),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
];

/// The logo as a PNG of `size` × `size` pixels.
pub fn png(size: u32) -> Result<Vec<u8>, String> {
    crate::render::png(LOGO_SVG, size)
}

/// The logo as straight RGBA pixels of `size` × `size`.
pub fn rgba(size: u32) -> Result<Vec<u8>, String> {
    crate::render::rgba(LOGO_SVG, size).map(|(_, _, p)| p)
}

/// A Windows `.ico` file holding the given PNG images (size, PNG bytes).
/// Each entry stores its PNG as is, which Windows Vista and later read.
pub fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        let side = if *size >= 256 { 0 } else { *size as u8 }; // 0 means 256
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend_from_slice(data);
    }
    out
}

/// A macOS `.icns` file holding the given PNG images (type, PNG bytes).
pub fn icns(images: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let body: usize = images.iter().map(|(_, d)| 8 + d.len()).sum();
    let mut out = Vec::with_capacity(8 + body);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&((8 + body) as u32).to_be_bytes());
    for (kind, data) in images {
        out.extend_from_slice(*kind);
        out.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
        out.extend_from_slice(data);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_renders_at_every_size() {
        for size in [16, 256] {
            let p = rgba(size).unwrap();
            assert_eq!(p.len(), (size * size * 4) as usize);
        }
        // The corner is transparent (rounded square), the centre is not.
        let p = rgba(64).unwrap();
        assert_eq!(p[3], 0);
        let centre = ((32 * 64 + 32) * 4) as usize;
        assert_eq!(p[centre + 3], 255);
    }

    #[test]
    fn ico_and_icns_headers_point_at_their_images() {
        let a = vec![1u8, 2, 3];
        let b = vec![4u8, 5];
        let ico = ico(&[(16, a.clone()), (256, b.clone())]);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(ico[6], 16);
        assert_eq!(ico[22], 0); // 256 is written as 0
        let first = u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize;
        assert_eq!(first, 6 + 32);
        assert_eq!(&ico[first..first + 3], &a[..]);
        let icns = icns(&[(b"ic07", a.clone())]);
        assert_eq!(&icns[0..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize,
            icns.len()
        );
        assert_eq!(&icns[8..12], b"ic07");
        assert_eq!(&icns[16..], &a[..]);
    }
}
