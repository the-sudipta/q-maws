# src (qmaws-cli)

## Purpose
Source code of the `qmaws` binary.

## Contents
| Item | Description |
|---|---|
| `compare_cmd.rs` | Hidden `tree-compare`: nRF, nQD and MSD of any Newick tree (support labels and branch lengths allowed) against a reference tree, for the ML-MAWS runs of H2 and H4 |
| `controls_cmd.rs` | The hidden command `controls`: simulated (positive) and shuffled (negative) controls with Fish mtDNA as a reference point, written to `results/controls/` |
| `data_cmd.rs` | The commands `datasets`, `download` and `inspect` |
| `figures_cmd.rs` | The command `figures`, and the figures drawn after every finished analysis run |
| `figure_check_cmd.rs` | The hidden command `figure-check`: figures for the taxon names of a dataset on a random tree, to check readability |
| `h3_cmd.rs` | The command `simulate-h3`: runs the H3 experiment and writes its tables, figure and evaluation |
| `iqtree_cmd.rs` | The hidden development commands `iqtree-export` and `iqtree-compare` of the IQ-TREE cross-check |
| `matrix_cmd.rs` | The command `matrix` |
| `metrics_check_cmd.rs` | Hidden `metrics-check`: golden test G9, random tree pairs with nQD and MSD checked against independent oracles and `pairs.tsv` for the DendroPy nRF check |
| `menu_cmd.rs` | The commands `menu` (carries out the terminal menu's choice in the chosen interface, offers the next unfinished run) and `gui`; resuming a list or queue of runs in the terminal; the runs of `resume --all` |
| `s2_cmd.rs` | The hidden development command `s2-cost`: times S2 bootstrap replicates of a finished run and writes `report/s2_cost.txt` |
| `wqfm_cmd.rs` | The hidden development commands `wqfm-export` and `wqfm-compare` of the wQFM jar comparison |
| `summary_cmd.rs` | The hidden development command `summary`: comparison tables of milestone M11 in `results/summary/` from finished runs and published values, the H1 test (`--h1-pairs`, `--h1-zeros`), and the AFproject upload files (`--afproject`) |
| `teach_cmd.rs` | The command `teach` |
| `verify_cmd.rs` | The command `verify`: input check, quick, full and single-quartet verification of a finished run; prints the report |
| `main.rs` | Argument parsing, Ctrl+C handling, and the commands `run`, `toy-run` and `resume` (with `--gui` or `--terminal`) |

## Relationships
Calls `qmaws-engine` for runs, `qmaws-data` for datasets and input, `qmaws-tui` for the menu and progress display, and `qmaws-gui` for the window.

## Notes
Exit status: 0 success, 1 error (for `verify`: also a failed comparison), 2 usage error, 3 run stopped on request (resumable).
