# User guide

How to use Q-MAWS: menus, commands and screenshots. Completed in milestone M14; sections are added as features are implemented.

## Commands available now

| Command | What it does |
|---|---|
| `qmaws --help`, `qmaws <command> --help` | Prints the available commands and options |
| `qmaws --version` | Prints the version |
| `qmaws toy-run [--output "<folder>"] [--blocks N] [--seed S] [--chunk-seconds T]` | Runs a toy computation that exercises checkpoints, resume and the progress display (development command) |
| `qmaws resume [--output "<run folder>"]` | Resumes an unfinished run. Without `--output`, resumes the only unfinished run in `results/runs`; if there are several, lists them |
| `qmaws datasets [--data-dir "<folder>"]` | Lists the benchmark datasets with taxa, size, reference tree and status (ready, not downloaded, checksum mismatch, not extracted) |
| `qmaws download --dataset <id or all> [--data-dir "<folder>"]` | Downloads, verifies and extracts datasets, then checks the taxon count and the reference tree names. An interrupted download continues where it stopped the next time |
| `qmaws inspect --input "<folder or file>" [--records per-file\|per-record] [--reference "<tree.nwk>"]` | Reads your own sequences and shows the taxa, their lengths and removed characters, and every problem found with its choices; compares names with a reference tree if given |
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

Runs are stored in `results/runs/<name>_<YYYY-MM-DD>_<HHMMSS>` (time in UTC) unless `--output` names another folder. A finished toy run contains `audit/root.txt`, the run's root fingerprint: the same settings give the same fingerprint on every computer.

## Launch scripts

`run.bat` (Windows) and `run.sh` (macOS, Linux) look for the program in `bin/`, then in `target/release/`. If it is missing and Rust's `cargo` is installed, they build it; otherwise they explain where to download a release. With no arguments they start the interactive menu (`qmaws menu`, not implemented yet); with arguments they pass them to the program unchanged. Both work when the folder path contains spaces.
