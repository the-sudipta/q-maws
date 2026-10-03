# src (qmaws-tui)

## Purpose
Source code of the terminal mode: the progress display and the interactive main menu.

## Contents
| Item | Description |
|---|---|
| `lib.rs` | Crate root and `TerminalDisplay`: progress bars, plain progress lines, quiet and JSON modes, download progress |
| `menu.rs` | The main menu of plan 5.2 (new run, resume, verify, exit): asks through the `Prompter` trait (`dialoguer` in the terminal, a script in tests) and returns a `MenuAction` for the program to carry out |

## Relationships
Uses `qmaws-engine` (`launch`, events), `qmaws-data` (folder check, benchmark registry) and `qmaws-core` (reference tree names). Called by `qmaws-cli`.

## Notes
The menu never starts work itself; the GUI offers the same structure through `qmaws-engine::launch`.
