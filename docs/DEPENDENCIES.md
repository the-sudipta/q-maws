# Dependencies

Every crate used by the workspace is listed here before it is added, with its license and the reason for using it.

## License policy

| Allowed | Forbidden |
|---|---|
| MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0 (file-level, unmodified), BSL-1.0 (from M9) | GPL, LGPL, AGPL, SSPL, and any license that would require the Q-MAWS source to be released under its terms; font licenses (OFL-1.1, Ubuntu-font-1.0) |

GUI decision (owner, 2026-10-03, M9): the GUI uses `eframe`/`egui` with egui's built-in fonts turned off, so the crate `epaint_default_fonts` (fonts under OFL-1.1 and the Ubuntu Font Licence) is not used and no font files ship with Q-MAWS; the window loads a font installed on the computer. BSL-1.0 (Boost Software License, permissive) is allowed because `eframe` always uses `clipboard-win` and `error-code` (BSL-1.0) for its clipboard support on Windows. OFL-1.1 and Ubuntu-font-1.0 stay excluded.

Pure-Rust crates are preferred to keep cross-compilation simple. The policy is enforced in continuous integration by `cargo deny` with `deny.toml` (licenses, banned and duplicate crates, allowed sources). A crate offered under a choice of licenses is accepted when one of the choices is allowed.

## Direct dependencies

Versions are those in `Cargo.lock`. "Maintenance checked" means: the version used is the newest release on crates.io on the stated date, and the crate has a public repository.

| Crate | Version | License | Used by | Reason | Maintenance checked |
|---|---|---|---|---|---|
| `serde` (feature `derive`) | 1.0.229 | MIT OR Apache-2.0 | `qmaws-engine` | Serialisation of `run.json` and other records | 2026-10-02 |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | `qmaws-engine`, `qmaws-tui`, `qmaws-gui` (from M9); tests of `qmaws-cli` | JSON for `run.json`, audit files and `--json-progress` output | 2026-10-02 |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 | `qmaws-engine`, `qmaws-core` (from M5) | SHA-256 of every output file and of the root fingerprint; per-quartet seeds of W2c (pure Rust, RustCrypto) | 2026-10-02 |
| `indicatif` | 0.18.6 | MIT | `qmaws-tui` | Terminal progress bars (overall and stage bars, log lines above the bars) | 2026-10-02 |
| `dialoguer` (no default features) | 0.12.0 | MIT | `qmaws-tui` (from M9) | Interactive terminal menus (plan 4.9, 5.2): numbered choices, text input with defaults, yes/no questions. The default features (editor, password, fuzzy select, history, completion) are not needed | 2026-10-03 |
| `eframe` (features `accesskit`, `glow`, `wayland`, `x11`; no default features) | 0.36.2 | MIT OR Apache-2.0 | `qmaws-gui` (from M9) | The native window of the GUI (plan 4.10) with `egui`: panels, buttons, progress bars, zoom and pan (`egui::Scene`), light and dark themes, screen-reader support (`accesskit`). Drawn with OpenGL (`glow`), which needs no further system library on the release platforms; the default `wgpu` renderer is not used. The default feature `default_fonts` is off (see the GUI decision above); `links` and `web_screen_reader` are not needed | 2026-10-03 |
| `clap` (feature `derive`) | 4.6.7 | MIT OR Apache-2.0 | `qmaws-cli` | Command-line parsing, help and version output | 2026-10-02 |
| `ctrlc` | 3.5.2 | MIT/Apache-2.0 | `qmaws-cli` | Ctrl+C handling on Windows, macOS and Linux for clean stopping | 2026-10-02 |
| `toml` | 1.1.6 | MIT OR Apache-2.0 | `qmaws-data` | Reading the benchmark registry `benchmarks.toml` | 2026-10-02 |
| `ureq` (features `rustls-no-provider`, `_ring`, `platform-verifier`; no default features) | 3.4.2 | MIT OR Apache-2.0 | `qmaws-data` | HTTPS downloads with range requests (resume), pure-Rust TLS (rustls with ring) verified against the operating system's certificate store | 2026-10-02 |
| `md-5` | 0.11.0 | MIT OR Apache-2.0 | `qmaws-data` | MD5 checksums, as published by AFproject | 2026-10-02 |
| `zip` (feature `deflate` only) | 8.6.0 | MIT | `qmaws-data` | Extracting dataset archives (newest stable release; 9.0.0 is a pre-release) | 2026-10-02 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 | `qmaws-data` | Reading `.gz` sequence files | 2026-10-02 |
| `qmaws-core` (workspace) | – | Q-MAWS license | `qmaws-data`, `qmaws-engine`, `qmaws-viz` (from M9) | Input cleaning, Newick parsing, MAWs, matrices, quartets | – |
| `qmaws-data` (workspace) | – | Q-MAWS license | `qmaws-engine` | Reading the input folder of an analysis run | – |
| `libm` | 0.2.16 | MIT | `qmaws-core` | Pure-Rust `log2` for the entropy of MAW lengths, and (from M5) `exp`, `log`, `log1p`, `expm1` and `lgamma` for quartet weighting, so results are identical on every platform | 2026-10-02 |
| `rayon` | 1.12.0 | MIT OR Apache-2.0 | `qmaws-engine` | Extracting the MAWs of several taxa in parallel, with a bounded number of threads | 2026-10-02 |
| `sysinfo` (features `system` and `disk`) | 0.39.6 | MIT | `qmaws-engine` | Measuring the available memory to set the default memory limit (70%), and the free disk space before the quartet stages (plan 4.12; `disk` added 2026-10-05) | 2026-10-02 |
| `zstd` | 0.14.0 | BSD-3-Clause (also the bundled Zstandard C library in `zstd-sys`) | `qmaws-engine` (from M8) | Compressing `audit/quartet_decisions.bin.zst`, the format named in the design. Not pure Rust: the C library is compiled from the bundled source by `cc` on every platform, with no system library needed. | 2026-10-02 |
| `resvg` (features `text`, `system-fonts`, `memmap-fonts`; no default features) | 0.45.1 | Apache-2.0 OR MIT | `qmaws-viz` (from M9) | SVG to PNG for the live provisional tree and its frames (pure Rust, with `tiny-skia`). Text is drawn with the fonts installed on the computer; no font files ship with Q-MAWS. Version 0.45.1 rather than the newest (0.48.1) because `svg2pdf` 0.13.0 is built on `usvg` 0.45, and both must share one `usvg`. The default feature `raster-images` (GIF, WebP and JPEG decoders) is not needed | 2026-10-03 |
| `gif` | 0.14.2 | MIT OR Apache-2.0 | `qmaws-viz` (from M10) | Encoding the growth animation `halo_tree_growth.gif` (pure Rust, image-rs project); its default feature `color_quant` (NeuQuant, MIT) reduces each frame to 256 colours | 2026-10-03 |
| `svg2pdf` (feature `text`; no default features) | 0.13.0 | MIT OR Apache-2.0 | `qmaws-viz` (from M9) | SVG to PDF for the live provisional tree (pure Rust, with `pdf-writer`); the default features `image` and `filters` are not needed | 2026-10-03 |
| `rfd` | 0.17.2 | MIT | `qmaws-gui` (from M14) | The system's folder and file dialogs behind the window's Browse buttons (sequence folder, reference tree, run folder, results folder, export folder): the Windows and macOS dialogs, and the desktop portal on Linux (with `pollster`, Apache-2.0 OR MIT, to wait for the portal's answer). The portal's D-Bus crates (`zbus` and others) are already used by `eframe`, so `rfd` adds two crates (`rfd`, `pollster`) | 2026-10-07 |

`ureq` is used without its default features, because the default adds `webpki-roots`, whose certificate data is licensed CDLA-Permissive-2.0, a license not on the allowed list. Certificates are instead verified by the operating system (`rustls-platform-verifier`).

## Indirect dependencies

The direct dependencies bring in further crates; 380 crates in total are used to build Q-MAWS for the release platforms (2026-10-03; 378 before `gif` and `color_quant` were added in M10, 183 before `eframe` was added in M9, 181 before `dialoguer`, 129 before `resvg` and `svg2pdf`, 123 before `zstd` was added in M8). Most of the crates added with `eframe` are platform layers of the window: `winit` and `glutin` (windows and OpenGL on each system), `accesskit` (screen readers), `x11rb`, `wayland-*` and `smithay-*` (Linux display servers), `zbus` and `atspi` (Linux accessibility bus), `objc2-*` (macOS), `arboard` (clipboard, with `image` for copied images). Each is listed with its license, authors and source in `THIRD_PARTY_NOTICES`. Declared licenses (2026-10-03): MIT, Apache-2.0, ISC, BSD-2-Clause (`arrayref`, from M9), BSD-3-Clause, Zlib and Unicode-3.0, alone or as alternatives; `ring` is "Apache-2.0 AND ISC"; `memchr` is "Unlicense OR MIT" and is used under MIT; `adler2` is "0BSD OR MIT OR Apache-2.0" and is used under MIT. From M9: BSL-1.0 (`clipboard-win`, `error-code`); `self_cell` is "Apache-2.0 OR GPL-2.0-only" and is used under Apache-2.0; `zune-core` and `zune-jpeg` are "MIT OR Apache-2.0 OR Zlib" and `glow`, `raw-window-handle`, `dpi` and `cursor-icon` offer MIT among their choices, so all are used under MIT. `cargo deny check licenses bans sources` passes. M14 (2026-10-07): `rfd` and `pollster` added (MIT; Apache-2.0 OR MIT); it was set aside in M9 because of the Linux portal stack, which `eframe` has brought in since.

## Considered and not used

| Crate | Reason |
|---|---|
| `chrono`, `time` | Only a UTC date and time are needed (run identifiers, log lines); a short tested function in `qmaws-engine` does this without a dependency |
