# qmaws-gui

## Purpose
GUI mode (plan 4.10): a native window with the main menu of plan 5.2 (start, resume, verify, exit), the steps of a new run, and the run view with the live provisional Halo Tree, the live worksheet, the stage log, progress, and Pause and Stop buttons. Implemented in milestone M9.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Uses `qmaws-engine` (in a worker thread, receiving progress events; `launch` for the shared menu logic), `qmaws-data` (folder check, benchmark registry), `qmaws-core` (reference tree names) and `qmaws-viz` (tree layout, halo colours, system fonts). Called by `qmaws-cli` (`qmaws gui`, `--gui`). Offers the same menu structure as `qmaws-tui`.

## Notes
Built with `eframe`/`egui` without egui's built-in fonts: the window loads a font installed on the computer, so no font files ship with Q-MAWS (license decision in `docs/DEPENDENCIES.md`). Buttons carry plain text, because no icon or emoji font is available.
