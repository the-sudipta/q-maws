# icon

## Purpose
The Q-MAWS logo and the program icon in every size and format the three operating systems need.

## Contents
| Item | Description |
|---|---|
| `q-maws.svg` | The logo (master); every other file here is made from it |
| `q-maws-16.png` | Logo, 16 × 16 pixels |
| `q-maws-24.png` | Logo, 24 × 24 pixels |
| `q-maws-32.png` | Logo, 32 × 32 pixels |
| `q-maws-48.png` | Logo, 48 × 48 pixels |
| `q-maws-64.png` | Logo, 64 × 64 pixels |
| `q-maws-128.png` | Logo, 128 × 128 pixels |
| `q-maws-256.png` | Logo, 256 × 256 pixels |
| `q-maws-512.png` | Logo, 512 × 512 pixels |
| `q-maws-1024.png` | Logo, 1024 × 1024 pixels |
| `q-maws.ico` | Windows icon (16 to 256 pixels, PNG entries) |
| `q-maws.icns` | macOS icon (16 to 1024 pixels, PNG entries) |

## Relationships
Made by `qmaws icons --output assets/icon` (hidden development command, `crates/qmaws-cli/src/icons_cmd.rs`) with the figure renderer; `crates/qmaws-cli/build.rs` reads the PNG files for the Windows executable; `crates/qmaws-viz/src/icon.rs` embeds `q-maws.svg` for the window icon.

## Notes
After changing `q-maws.svg`, run `qmaws icons` again and commit all files.
