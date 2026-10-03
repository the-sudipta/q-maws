# qmaws-tui

## Purpose
Terminal mode. In milestone M1: the progress display, with an overall bar and a stage bar (stage number, elapsed time, remaining time, current item) and the run log scrolling above the bars; plain progress lines when the output is not a terminal; quiet and JSON modes. From M9: the interactive main menu of plan 5.2 (`qmaws menu`).

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Implements the `ProgressSink` of `qmaws-engine`. Called by `qmaws-cli`. Offers the same menu structure as `qmaws-gui`; both use `qmaws-engine::launch`.

## Notes
Progress bars are drawn on standard error with `indicatif`; JSON progress goes to standard output, one object per line.
