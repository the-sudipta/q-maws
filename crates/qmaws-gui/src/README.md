# src (qmaws-gui)

## Purpose
Source code of the GUI.

## Contents
| Item | Description |
|---|---|
| `lib.rs` | Crate root and `run`, which opens the window with a `Launch` (runs to start at once, or the main menu) |
| `app.rs` | The window: the sidebar (Home, New run, Resume, Verify, the current run, Results, About, theme and text size), the New run steps Data → Reference → Output → Settings → Review (with the answers to input warnings and the Open Tree of Life download on worker threads), the resume list (runs in use by another process are locked), the verify page, the results library with each finished run's Halo Tree, and the run view (the live or final Halo Tree, progress, stage list with times, live worksheet, stage log, Pause, Stop, resume and next-run offers, export of the final figures) |
| `controller.rs` | Runs the engine in a worker thread without any drawing: events through a channel, pause and stop flags, a list of runs one after another, the resume queue; used by the window and by the tests (G10) |
| `figure_view.rs` | The figure viewer: renders the run's own SVG figure (live provisional tree, final Halo Tree) on a worker thread at high resolution, reloads it when the file changes, opens zoomed onto the title and tree, zoom and pan |
| `fonts.rs` | Loads a sans-serif and a monospace font installed on the computer into egui |
| `theme.rs` | The look of the window: the light ("paper") and dark ("ink") palettes from the logo and the Okabe–Ito colours, type sizes, spacing and the style of every control |
| `widgets.rs` | Building blocks of the pages: cards, buttons, choice cards, stage rows and state marks drawn as shapes, pills, stat tiles, banners, the logo and the sidebar items |

## Relationships
`controller.rs` calls `qmaws-engine` (`analysis::start`, `resume_run`); `app.rs` uses `qmaws-engine::launch`, `qmaws-data` and `qmaws-viz::tree`. Called through `qmaws-cli`.

## Notes
The window never blocks: it reads the controller's messages each frame, and downloads and verifications run in their own threads. Closing the window stops a run cleanly; it can be resumed.
