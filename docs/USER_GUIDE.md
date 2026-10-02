# User guide

How to use Q-MAWS: menus, commands and screenshots. Completed in milestone M14; sections are added as features are implemented.

## Commands available now

| Command | What it does |
|---|---|
| `qmaws --help` | Prints the available options |
| `qmaws --version` | Prints the version |

Any other command prints an error saying it is not available in this development build.

## Launch scripts

`run.bat` (Windows) and `run.sh` (macOS, Linux) look for the program in `bin/`, then in `target/release/`. If it is missing and Rust's `cargo` is installed, they build it; otherwise they explain where to download a release. With no arguments they start the interactive menu (`qmaws menu`, not implemented yet); with arguments they pass them to the program unchanged. Both work when the folder path contains spaces.
