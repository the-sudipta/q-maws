# Q-MAWS

Q-MAWS builds phylogenetic trees without a sequence alignment, quartet by quartet, from the minimal absent words of each genome. Every run keeps a complete record, so its results can be recomputed and verified.

## Starting it

| System | Window | Terminal |
|---|---|---|
| Windows | Double-click `Q-MAWS.exe` | Double-click `run.bat` for the menu, or run `run.bat <command>` in a terminal |
| macOS | Double-click `Q-MAWS.app` | `./run.sh` for the menu, or `./run.sh <command>` |
| Linux | Double-click `Q-MAWS` (or run `./Q-MAWS gui`) | `./run.sh` for the menu, or `./run.sh <command>` |

`run.bat` and `run.sh` with no command open the terminal menu; `run.bat --help` (or `./run.sh --help`) lists every command.

The window and the menu lead through a new run (your own folder with one FASTA file per taxon, or one of the benchmark datasets of the paper), resuming an unfinished run, and verifying a finished one.

## Where the results go

Runs are written to `results/runs/` and downloaded datasets to `data/`, inside this folder when Q-MAWS can write here. Otherwise (an app on macOS, or a folder that cannot be written) they go to `Documents/Q-MAWS` in your home folder.

## The first start

- Windows may show "Windows protected your PC" for a new program: choose "More info", then "Run anyway".
- macOS may say the app is from an unidentified developer: right-click (or Control-click) `Q-MAWS.app`, choose "Open", then "Open" again. This is needed only once.
- Linux: if double-clicking does nothing, mark the file as executable (`chmod +x Q-MAWS run.sh`).

The window uses a font installed on the computer and OpenGL 2 or newer.

## More

Documentation, the source code and the paper's results: https://github.com/the-sudipta/q-maws

`LICENSE` is the Q-MAWS Source-Available License; `THIRD_PARTY_NOTICES` lists the components Q-MAWS is built with and their licenses; `CITATION.cff` says how to cite Q-MAWS.
