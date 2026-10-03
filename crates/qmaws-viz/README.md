# qmaws-viz

## Purpose
Figures in pure Rust: the Quartet Halo Tree, tanglegram, rectangular tree, SVG, PDF and PNG output, the self-contained interactive HTML tree, and the growth animation. Implemented in milestone M10 (live provisional trees from M9); from M6 it draws the line chart of the H3 simulation.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Used by `qmaws-engine` for run figures and by `qmaws-gui` for display. Produces files in each run's `figures/` folder.

## Notes
Line charts since M6; the circular tree layout, the basic provisional Halo Tree and PNG and PDF output since M9. The final figures come with M10.
