# qmaws-gui

## Purpose
GUI mode: native desktop window with the step panels, live Halo Tree, live worksheet panel, progress, and Pause and Stop buttons. Implemented in milestone M9.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `lib.rs` (crate root) |

## Relationships
Uses `qmaws-engine` (in a worker thread, receiving progress events) and `qmaws-viz`. Called by `qmaws-cli`. Offers exactly the same menu structure as `qmaws-tui`.

## Notes
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
