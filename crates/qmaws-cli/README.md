# qmaws-cli

## Purpose
The `qmaws` binary (`qmaws.exe` on Windows): parses command-line arguments and dispatches to terminal mode, GUI mode, or a direct command. In M0 it supports only `--help` and `--version`.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `main.rs` (binary entry point) |

## Relationships
Depends on every other workspace crate. Started by `run.bat` and `run.sh` in the repository root.

## Notes
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
