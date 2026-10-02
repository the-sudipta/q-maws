# qmaws-tui

## Purpose
Terminal mode: interactive menus with defaults and progress display (overall and stage bars, elapsed and remaining time, current item, scrolling log). Implemented from milestone M1 onwards.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `lib.rs` (crate root) |

## Relationships
Uses `qmaws-engine` for runs and progress events. Called by `qmaws-cli`. Offers exactly the same menu structure as `qmaws-gui`.

## Notes
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
