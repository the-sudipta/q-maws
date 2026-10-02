# src (qmaws-cli)

## Purpose
Source code of the `qmaws` binary.

## Contents
| Item | Description |
|---|---|
| `data_cmd.rs` | The commands `datasets`, `download` and `inspect` |
| `matrix_cmd.rs` | The command `matrix` |
| `teach_cmd.rs` | The command `teach` |
| `main.rs` | Argument parsing, Ctrl+C handling, and the commands `run`, `toy-run` and `resume` |

## Relationships
Calls `qmaws-engine` for runs, `qmaws-data` for datasets and input, and `qmaws-tui` for progress display.

## Notes
Exit status: 0 success, 1 error, 2 usage error, 3 run stopped on request (resumable).
