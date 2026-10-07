# images

## Purpose
Screenshots of the Q-MAWS window for `docs/USER_GUIDE.md`.

## Contents
| Item | Description |
|---|---|
| `gui_home.png` | Home: the three actions and the recent runs (benchmark runs of M11, two of them running in other processes) |
| `gui_new_run.png` | New run, first step: your own folder or a benchmark dataset, with Browse |
| `gui_run.png` | A run of Fish mtDNA while the quartets are weighed: the live provisional Halo Tree, progress and the live worksheet |
| `gui_details.png` | The run details of the same run: stage, units, speed, times left and the stage list |
| `gui_queue.png` | A queue of three Fish mtDNA runs (seeds 1 to 3) in the window: the Queue card above the run's progress; the species tree during the S2 bootstrap |
| `batch_status.png` | `batch_status.html` of the queue of `gui_queue.png`, in a browser |
| `gui_benchmark.png` | The M11 progress page (Benchmark → M11 progress) on 2026-10-08 with the real benchmark runs: 24 of 70 finished, 2 running |
| `gui_results.png` | The Results library with the Halo Tree of `ecoli_shigella_hgt_seed1` |
| `gui_results_dark.png` | The Results library in the dark theme, with `sim_hgt_250_seed3` |

## Relationships
Taken from the program itself with the development screenshot option (`QMAWS_GUI_SHOT=<file.png>`, with `QMAWS_GUI_SIZE`, `QMAWS_GUI_DELAY`, `QMAWS_GUI_PAGE`, `QMAWS_GUI_RUN`, `QMAWS_GUI_THEME` and `QMAWS_GUI_DETAILS`; see `crates/qmaws-gui/src/app.rs`), which saves the window off the screen and closes it. `batch_status.png` was rendered from the page with Microsoft Edge in headless mode (`--headless=new --screenshot`).

## Notes
Window size 1440 × 900 points at 125% display scaling. Taken on 2026-10-07 with the build of branch `m14`.
