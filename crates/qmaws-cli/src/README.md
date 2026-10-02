# src (qmaws-cli)

## Purpose
Source code of the `qmaws` binary.

## Contents
| Item | Description |
|---|---|
| `controls_cmd.rs` | The hidden command `controls`: simulated (positive) and shuffled (negative) controls with Fish mtDNA as a reference point, written to `results/controls/` |
| `data_cmd.rs` | The commands `datasets`, `download` and `inspect` |
| `h3_cmd.rs` | The command `simulate-h3`: runs the H3 experiment and writes its tables, figure and evaluation |
| `iqtree_cmd.rs` | The hidden development commands `iqtree-export` and `iqtree-compare` of the IQ-TREE cross-check |
| `matrix_cmd.rs` | The command `matrix` |
| `s2_cmd.rs` | The hidden development command `s2-cost`: times S2 bootstrap replicates of a finished run and writes `report/s2_cost.txt` |
| `wqfm_cmd.rs` | The hidden development commands `wqfm-export` and `wqfm-compare` of the wQFM jar comparison |
| `teach_cmd.rs` | The command `teach` |
| `verify_cmd.rs` | The command `verify`: input check, quick, full and single-quartet verification of a finished run; prints the report |
| `main.rs` | Argument parsing, Ctrl+C handling, and the commands `run`, `toy-run` and `resume` |

## Relationships
Calls `qmaws-engine` for runs, `qmaws-data` for datasets and input, and `qmaws-tui` for progress display.

## Notes
Exit status: 0 success, 1 error (for `verify`: also a failed comparison), 2 usage error, 3 run stopped on request (resumable).
