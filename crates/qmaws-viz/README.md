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
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
