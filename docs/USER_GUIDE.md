# User guide

How to use Q-MAWS: menus, commands and screenshots. Completed in milestone M14; sections are added as features are implemented.

## Commands available now

| Command | What it does |
|---|---|
| `qmaws menu` | The interactive main menu in the terminal (see "Interactive menu"); `run.bat` and `run.sh` without arguments start it |
| `qmaws gui` | Opens the graphical interface with the same menu (see "Graphical interface") |
| `qmaws --help`, `qmaws <command> --help` | Prints the available commands and options |
| `qmaws --version` | Prints the version |
| `qmaws run --dataset <id> | --input "<folder>" [--output "<run folder>"] [--no-strand] [--lengths a,b,c] [--seed S] [--weighting w2-sym|w2-emp|none] [--replicates B] [--bootstrap B]` | Analyses the data as a resumable run: input check, MAW extraction, length selection, matrices, the pattern counts of every quartet, the quartet weights, the tree (wQFM-rs), S1 support and halo values, and S2 bootstrap support. `--weighting`: the two-state symmetric model (default), the model with the frequencies of 0 and 1 of the full matrix, or no weighting. `--replicates`: resamples per quartet for W2c (default 100; 0 skips W2c). `--bootstrap`: S2 column-bootstrap replicates with W2b weights inside (default 100; 0 skips S2). `--seed` sets the quartet order and the resamples. `--no-live-tree` turns off the live provisional tree. `--gui` shows the run in the window, `--terminal` (the default) in the terminal. Stop with Ctrl+C and continue with `qmaws resume` |
| `qmaws verify --output "<run folder>" [--quick \| --full \| --quartet A,B,C,D \| --inputs] [--seed N] [--input "<folder>"]` | Checks a finished run against its `audit/` record: the input check, then the quick check (20 chunks of each chunked stage and 3 bootstrap replicates, drawn with a printed seed), the full recomputation of the root, or one quartet with its worksheet. Writes `report/verify_<time>.txt`; exit status 1 if anything differs. `--input` gives the data's new place if they were moved |
| `qmaws figures --output "<run folder>" [--reference "<tree.nwk>"] [--groups "<groups.tsv>"] [--otl] [--s2]` | Draws the figures of a finished run again (they are drawn after every run). `--reference` gives a tree for the tanglegram (a benchmark dataset's reference tree is found by itself); `--groups` a group file for the group bands; `--otl` takes the groups from the Open Tree of Life taxonomy (needs the internet); `--s2` colours the branches by S2 instead of S1 |
| `qmaws teach --example` or `qmaws teach --input "<folder>" [--reference "<tree.nwk>"]` | Prints the hand-calculable teaching worksheet (see `docs/TEACHING.md`) |
| `qmaws simulate-h3 [--output "<folder>"] [--seed S] [--replicates R] [--w2c-replicates B]` | Runs the long-branch simulation of hypothesis H3 (default: into `results/h3`, seed 1, 200 replicates per setting, 100 W2c resamples) and writes `recovery.csv`, `replicates.csv`, `recovery.svg` and `evaluation.txt`; about one minute |
| `qmaws toy-run [--output "<folder>"] [--blocks N] [--seed S] [--chunk-seconds T]` | Runs a toy computation that exercises checkpoints, resume and the progress display (development command) |
| `qmaws resume [--output "<run folder>" \| --all] [--gui \| --terminal]` | Resumes an unfinished run, in the terminal (default) or the window. Without `--output`, resumes the only unfinished run in `results/runs` and the other results folders you have used; if there are several, lists them. `--all` resumes every unfinished run one after another; the queue is saved, so after a stop the next `qmaws resume --all` continues it |
| `qmaws datasets [--data-dir "<folder>"]` | Lists the benchmark datasets with taxa, size, reference tree and status (ready, not downloaded, checksum mismatch, not extracted) |
| `qmaws download --dataset <id or all> [--data-dir "<folder>"]` | Downloads, verifies and extracts datasets, then checks the taxon count and the reference tree names. An interrupted download continues where it stopped the next time |
| `qmaws inspect --input "<folder or file>" [--records per-file\|per-record] [--reference "<tree.nwk>"]` | Reads your own sequences and shows the taxa, their lengths and removed characters, and every problem found with its choices; compares names with a reference tree if given |
| `qmaws matrix --dataset <id> | --input "<folder>" --output "<folder>" [--no-strand] [--lengths 7,8,9]` | Extracts the minimal absent words, selects the MAW lengths by entropy (or uses the given lengths), builds the full matrix and the ML-MAWS-style matrix, and writes `summary.json`, `entropy.tsv`, `m_ml.phy`, `m_ml_columns.txt` and `m_full_columns.txt` |
| `qmaws inspect --dataset <id> [--compare-with <id>]` | The same for a downloaded benchmark dataset, compared with its built-in reference tree; `--compare-with` also reports which taxa have an identical cleaned sequence in another downloaded dataset |

Options for every command:

| Option | Effect |
|---|---|
| `--quiet` | Shows only the final result and errors |
| `--json-progress` | Prints progress as one JSON object per line on standard output, for scripts |
| `--no-color` | Disables colours |

## Interactive menu

`qmaws menu` (or `run.bat` / `./run.sh` without arguments) shows the main menu:

1. **Start a new run:** your own folder (checked, with a summary of the taxa and every problem found; a reference tree can be given and its names are checked) or a benchmark dataset (downloaded and checked if missing, "All datasets" runs each in turn); the output folder (Enter keeps the default); default or customised settings (weighting method, MAW lengths, strand filter, live provisional tree, random seed, CPU cores, memory limit); if an unfinished run with the same data and settings exists, resume it or start fresh (the earlier run is kept); the review with taxa and quartets; then the interface, GUI or terminal.
2. **Resume an unfinished run:** the unfinished runs with their percentage done and the interface used last (one run is offered directly); any single run or all runs one after another; then the interface (Enter: the one used last). After a single resumed run, the menu offers the next unfinished run.
3. **Verify a run:** a finished run on this computer or a results folder by path; the new place of the data if they were moved; then quick, full, single quartet or input check only. The verdict is shown and the report saved.
4. **Exit.**

In every submenu, `0. Back` returns to the previous menu. Downloading a reference tree from the Open Tree of Life is not available yet, and a reference tree is checked but not used for comparisons yet (later milestones).

## Graphical interface

`qmaws gui` opens the window; `qmaws run ... --gui` and `qmaws resume ... --gui` open it with the run already started. The window has the same menu as tabs: "Start a new run" with the steps Data, Reference, Output, Settings, Review and Run on the left, "Resume an unfinished run", "Verify a run" and "Exit". Paths can be typed, pasted, or given by dropping a folder or file onto the window. If unfinished runs exist when the window opens, it lists them and offers to resume.

During a run:

- **Centre:** the live provisional Halo Tree (scroll to zoom, drag to pan, "Fit" to reset); branches that changed since the previous provisional tree are drawn thicker in blue. When the run finishes, the final tree is shown. Buttons open the figures folder and the run folder.
- **Right:** the live worksheet with the latest weighed quartet (its four taxa, the 16 pattern counts, the W1 weights, the W2 log-likelihoods and the weights used for the tree) and the stage log.
- **Bottom:** overall and stage progress, elapsed and remaining time, the current item, Pause (Continue) and Stop.

The theme follows the system; the buttons at the top right choose light or dark, and "A+" / "A-" change the text size. The window uses a font installed on the computer; if none is found it says so, and the terminal mode can be used instead. Closing the window stops a running analysis cleanly; it can be resumed later.

## Live provisional tree

While the quartets are weighed, Q-MAWS draws a provisional tree from the quartets finished so far: every 5% of the quartets or every 3 minutes, whichever comes first. If drawing takes more than 10% of the time between two updates, both intervals are doubled (shown in the log). The latest tree is in `figures/live/halo_tree_latest.svg`, `.png` and `.pdf`, marked `PROVISIONAL — <x>% of quartets`; small frames are kept in `work/provisional/frames/`. At the end, `report/convergence.csv` lists the nRF of each provisional tree to the final tree. The provisional trees do not change the results; turn them off with `--no-live-tree` or in the settings.

## Figures

When an analysis run finishes, Q-MAWS draws its figures into the run's `figures/` folder:

| File | Figure |
|---|---|
| `halo_tree.svg`, `.pdf`, `.png` | The Quartet Halo Tree: the tree in a circle, branches coloured and widened by S1 support, the halo ring (one segment per taxon, orange for low and purple for high halo values, a dot below 0.6), group bands with their names, and a title block with the settings |
| `rectangular_tree.svg`, `.pdf`, `.png` | The same tree drawn rectangular, with S1/S2 at every node, a halo square and the group beside each name |
| `tanglegram.svg`, `.pdf`, `.png` | The tree beside the reference tree, lines joining the same taxa; dashed vermillion lines mark taxa whose closest relatives differ. The header gives nRF, nQD and MSD (the raw matching split distance, with the number of taxa beside it); the same numbers are in `report/reference_comparison.tsv`. Only when a reference tree is known |
| `interactive_tree.html` | Opens in any web browser, also without internet: zoom with the wheel or buttons, drag to pan, find a taxon, hover for values, switch between the circular and the rectangular layout and between S1 and S2 colours |
| `halo_tree_growth.gif` | Animation of the live provisional trees, ending with the final tree |
| `convergence.svg`, `.pdf`, `.png` | How far each provisional tree was from the final tree (nRF) by the percentage of quartets finished |
| `groups.tsv` | The group of every taxon in the bands, with their source |

PNG files have 300 dots per inch. The colours were chosen to stay distinguishable with the common forms of colour blindness (checked by a simulation in the tests), and width, labels and symbols repeat what the colours show.

**Groups.** Put a file `groups.tsv` in the folder with your sequences (or give one with `qmaws figures --groups`): one line per taxon, the taxon name, a tab, and the group name; a first line `taxon<TAB>group` and lines starting with `#` are skipped. Without a group file, the bands show clades cut from the tree (named Clade A, Clade B, ...). With `qmaws figures --otl`, the groups come from the Open Tree of Life taxonomy (family, order or another rank that gives 2 to 7 groups); this sends the taxon names to the Open Tree of Life and needs the internet, and the query is recorded in `audit/otl_taxonomy.json`.

## Stopping and resuming

In the window, Pause holds the run after the current step (paused time is not counted) and Stop ends it after the current step. In the terminal, press Ctrl+C once to stop after the current step; the run is saved and can be resumed. Press Ctrl+C a second time to exit at once; this is also safe, because every file is written in a way that survives interruption. Closing the terminal or a power cut is equally safe. To continue, run `qmaws resume --output "<run folder>"`; the progress so far is checked and kept. A run can be continued in either interface, whichever it was started in (`--gui` or `--terminal`); the result is the same.

Exit status: 0 when the run finished, 3 when it stopped and can be resumed, 1 on an error, 2 on a usage error.

## Run folders

Runs are stored in `results/runs/<name>_<YYYY-MM-DD>_<HHMMSS>` (time in UTC) unless `--output` names another folder. A finished toy run contains `audit/root.txt`, the run's root fingerprint: the same settings give the same fingerprint on every computer. A finished analysis run contains:

| Item | Content |
|---|---|
| `report/tree.nwk` | The tree |
| `trees/tree_s1.nwk`, `report/support.tsv` | S1 support of every internal edge |
| `trees/tree_s2.nwk`, `report/bootstrap.tsv`, `trees/bootstrap_trees.nwk` | S2 bootstrap support and the replicate trees |
| `report/halo.tsv` | Halo value of every taxon |
| `report/m_ml.phy` | The ML-MAWS-style matrix in PHYLIP format |
| `figures/` | The figures (see "Figures") |
| `figures/live/` | The latest provisional Halo Tree (SVG, PNG, PDF), unless the live tree was turned off |
| `report/reference_comparison.tsv` | nRF, nQD and MSD against the reference tree, when one is known |
| `report/convergence.csv` | nRF of each provisional tree to the final tree |
| `audit/` | The verification record (see `qmaws verify`) |
| `run.json`, `run.log` | Settings, stage states and the log |
| `work/` | Large intermediate files; not needed to verify the run, and never committed |

## Launch scripts

`run.bat` (Windows) and `run.sh` (macOS, Linux) look for the program in `bin/`, then in `target/release/`. If it is missing and Rust's `cargo` is installed, they build it; otherwise they explain where to download a release. With no arguments they start the interactive menu (`qmaws menu`); with arguments they pass them to the program unchanged. Both work when the folder path contains spaces.
