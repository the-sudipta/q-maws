# qmaws-engine

## Purpose
Run orchestration: the run folder, the stage state machine in `run.json`, atomic hash-verified writes, chunk plans, checkpoints and resume, progress events and time estimates, and (from M8) the audit log. In milestone M1 it runs a toy computation that exercises all of this without any science.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Used by `qmaws-tui` (progress display) and `qmaws-cli` (commands). Will use `qmaws-core` for every computation and `qmaws-data` for inputs. Writes run folders, by default under `results/runs/`.

## Notes
The engine never prints: it reports through a `ProgressSink`, so terminal and GUI show the same events. Design details are in `docs/DESIGN.md` (run folder, atomic writes, resume, root fingerprint, estimates).
