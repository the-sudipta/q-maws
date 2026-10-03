# src (qmaws-gui)

## Purpose
Source code of the GUI.

## Contents
| Item | Description |
|---|---|
| `lib.rs` | Crate root and `run`, which opens the window with a `Launch` (runs to start at once, or the main menu) |
| `app.rs` | The window: menu tabs, the steps Data → Reference → Output → Settings → Review → Run, the resume list, the verify screen, and the run view (live Halo Tree with changed branches highlighted, final tree, live worksheet, stage log, progress, Pause, Stop, resume and next-run offers) |
| `controller.rs` | Runs the engine in a worker thread without any drawing: events through a channel, pause and stop flags, a list of runs one after another, the resume queue; used by the window and by the tests (G10) |
| `fonts.rs` | Loads a sans-serif and a monospace font installed on the computer into egui |

## Relationships
`controller.rs` calls `qmaws-engine` (`analysis::start`, `resume_run`); `app.rs` uses `qmaws-engine::launch`, `qmaws-data` and `qmaws-viz::tree`. Called through `qmaws-cli`.

## Notes
The window never blocks: it reads the controller's messages each frame, and downloads and verifications run in their own threads. Closing the window stops a run cleanly; it can be resumed.
