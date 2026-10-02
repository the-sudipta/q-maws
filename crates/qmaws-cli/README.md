# qmaws-cli

## Purpose
The `qmaws` binary (`qmaws.exe` on Windows): parses command-line arguments and dispatches to terminal mode, GUI mode, or a direct command. Commands so far: `toy-run`, `resume`, `datasets`, `download`, `inspect`, with the global options `--quiet`, `--json-progress` and `--no-color`.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |
| `tests/` | Tests that run the built binary ([README](tests/README.md)) |

## Relationships
Depends on every other workspace crate. Started by `run.bat` and `run.sh` in the repository root. Commands are documented in `docs/USER_GUIDE.md`.

## Notes
Exit status: 0 finished, 1 error, 2 usage error, 3 stopped on request (resumable).
