# qmaws-tui

## Purpose
Terminal mode. In milestone M1: the progress display, with an overall bar and a stage bar (stage number, elapsed time, remaining time, current item) and the run log scrolling above the bars; plain progress lines when the output is not a terminal; quiet and JSON modes. Interactive menus are added in later milestones.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `lib.rs` (crate root and `TerminalDisplay`) |

## Relationships
Implements the `ProgressSink` of `qmaws-engine`. Called by `qmaws-cli`. Will offer exactly the same menu structure as `qmaws-gui`.

## Notes
Progress bars are drawn on standard error with `indicatif`; JSON progress goes to standard output, one object per line.
