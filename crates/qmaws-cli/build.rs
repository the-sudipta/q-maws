//! Build script: on Windows with the MSVC linker, gives `qmaws.exe` the
//! Q-MAWS icon, so the program shows its logo in Explorer, on the taskbar
//! and on the desktop. The icon is the committed PNG files in
//! `assets/icon/` (made by `qmaws icons`), packed into a compiled resource
//! file (`.res`) that the linker takes as an input; no resource compiler or
//! extra crate is needed. Other platforms and toolchains get no resource
//! (macOS uses `q-maws.icns` in the app bundle, Linux the PNG files).

use std::path::PathBuf;

/// Sizes stored in the executable, smallest first.
const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

/// One resource entry with numeric type and name, padded to 4 bytes.
fn entry(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // data size
    out.extend_from_slice(&32u32.to_le_bytes()); // header size
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // data version
    out.extend_from_slice(&flags.to_le_bytes()); // memory flags
    out.extend_from_slice(&0x0409u16.to_le_bytes()); // language: English (US)
    out.extend_from_slice(&0u32.to_le_bytes()); // version
    out.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn main() {
    let root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../assets/icon");
    for size in SIZES {
        println!(
            "cargo:rerun-if-changed={}",
            root.join(format!("q-maws-{size}.png")).display()
        );
    }
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !(windows && msvc) {
        return;
    }
    let mut res = Vec::new();
    entry(&mut res, 0, 0, 0, &[]); // a .res file starts with an empty entry
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    group.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    for (i, size) in SIZES.iter().enumerate() {
        let png = std::fs::read(root.join(format!("q-maws-{size}.png")))
            .expect("assets/icon PNG files (run `qmaws icons`)");
        let id = (i + 1) as u16;
        entry(&mut res, RT_ICON, id, 0x1010, &png);
        let side = if *size >= 256 { 0 } else { *size as u8 };
        group.extend_from_slice(&[side, side, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes()); // planes
        group.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        group.extend_from_slice(&(png.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    entry(&mut res, RT_GROUP_ICON, 1, 0x1030, &group);
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("q-maws-icon.res");
    std::fs::write(&out, res).expect("writing the icon resource");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}
