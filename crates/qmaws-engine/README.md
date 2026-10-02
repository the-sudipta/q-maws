# qmaws-engine

## Purpose
Run orchestration: the run directory, the stage state machine in `run.json`, atomic writes, chunk plans, checkpoints and resume, progress events and time estimates, and the audit log. Implemented from milestone M1 onwards.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `lib.rs` (crate root) |

## Relationships
Used by `qmaws-tui`, `qmaws-gui` and `qmaws-cli`. Will use `qmaws-core` for every computation and `qmaws-data` for inputs.

## Notes
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
