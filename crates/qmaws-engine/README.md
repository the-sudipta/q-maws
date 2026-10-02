# qmaws-engine

## Purpose
Run orchestration: the run folder, the stage state machine in `run.json`, atomic hash-verified writes, chunk plans, checkpoints and resume, progress events and time estimates, and (from M8) the audit log. It runs the analysis (from M4: input, MAW extraction, length selection, matrices and quartet counts), the matrix pipeline of `qmaws matrix`, and a toy computation used to test the engine.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Used by `qmaws-tui` (progress display) and `qmaws-cli` (commands). Uses `qmaws-core` for every computation and `qmaws-data` to read inputs. Writes run folders, by default under `results/runs/`.

## Notes
The engine never prints: it reports through a `ProgressSink`, so terminal and GUI show the same events. Design details are in `docs/DESIGN.md` (run folder, atomic writes, resume, root fingerprint, estimates).
