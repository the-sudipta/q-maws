# assets

## Purpose
Artwork shipped with Q-MAWS: the logo and the program icon.

## Contents
| Item | Description |
|---|---|
| `icon/` | The Q-MAWS logo (SVG master) and the icon files made from it |

## Relationships
`crates/qmaws-viz/src/icon.rs` embeds the logo; `crates/qmaws-cli/build.rs` puts the icon into the Windows executable; the release packages use the `.icns` (macOS) and PNG (Linux) files.

## Notes
The logo is a quartet tree: four taxa (colour-blind safe Okabe–Ito colours) and the split between them, drawn in orange.
