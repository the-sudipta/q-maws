# User guide

How to use Q-MAWS: menus, commands and screenshots. Completed in milestone M14; sections are added as features are implemented.

## Commands available now

| Command | What it does |
|---|---|
| `qmaws --help`, `qmaws <command> --help` | Prints the available commands and options |
| `qmaws --version` | Prints the version |
| `qmaws run --dataset <id> | --input "<folder>" [--output "<run folder>"] [--no-strand] [--lengths a,b,c] [--seed S] [--weighting w2-sym|w2-emp|none] [--replicates B] [--bootstrap B]` | Analyses the data as a resumable run: input check, MAW extraction, length selection, matrices, the pattern counts of every quartet, the quartet weights, the tree (wQFM-rs), S1 support and halo values, and S2 bootstrap support. `--weighting`: the two-state symmetric model (default), the model with the frequencies of 0 and 1 of the full matrix, or no weighting. `--replicates`: resamples per quartet for W2c (default 100; 0 skips W2c). `--bootstrap`: S2 column-bootstrap replicates with W2b weights inside (default 100; 0 skips S2). `--seed` sets the quartet order and the resamples. Stop with Ctrl+C and continue with `qmaws resume` |
| `qmaws verify --output "<run folder>" [--full \| --quartet A,B,C,D \| --inputs] [--seed N] [--input "<folder>"]` | Checks a finished run against its `audit/` record: the input check, then the quick check (20 chunks of each chunked stage and 3 bootstrap replicates, drawn with a printed seed), the full recomputation of the root, or one quartet with its worksheet. Writes `report/verify_<time>.txt`; exit status 1 if anything differs. `--input` gives the data's new place if they were moved |
| `qmaws teach --example` or `qmaws teach --input "<folder>" [--reference "<tree.nwk>"]` | Prints the hand-calculable teaching worksheet (see `docs/TEACHING.md`) |
| `qmaws simulate-h3 [--output "<folder>"] [--seed S] [--replicates R] [--w2c-replicates B]` | Runs the long-branch simulation of hypothesis H3 (default: into `results/h3`, seed 1, 200 replicates per setting, 100 W2c resamples) and writes `recovery.csv`, `replicates.csv`, `recovery.svg` and `evaluation.txt`; about one minute |
| `qmaws toy-run [--output "<folder>"] [--blocks N] [--seed S] [--chunk-seconds T]` | Runs a toy computation that exercises checkpoints, resume and the progress display (development command) |
| `qmaws resume [--output "<run folder>"]` | Resumes an unfinished run. Without `--output`, resumes the only unfinished run in `results/runs`; if there are several, lists them |
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

## Stopping and resuming

Press Ctrl+C once to stop after the current step; the run is saved and can be resumed. Press Ctrl+C a second time to exit at once; this is also safe, because every file is written in a way that survives interruption. Closing the terminal or a power cut is equally safe. To continue, run `qmaws resume --output "<run folder>"`; the progress so far is checked and kept.

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
| `audit/` | The verification record (see `qmaws verify`) |
| `run.json`, `run.log` | Settings, stage states and the log |
| `work/` | Large intermediate files; not needed to verify the run, and never committed |

## Launch scripts

`run.bat` (Windows) and `run.sh` (macOS, Linux) look for the program in `bin/`, then in `target/release/`. If it is missing and Rust's `cargo` is installed, they build it; otherwise they explain where to download a release. With no arguments they start the interactive menu (`qmaws menu`, not implemented yet); with arguments they pass them to the program unchanged. Both work when the folder path contains spaces.
