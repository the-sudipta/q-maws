# crates

## Purpose
Rust source code of Q-MAWS, organised as a Cargo workspace with one crate per concern. The science lives in `qmaws-core`; the other crates orchestrate runs, obtain data, draw figures, and provide the terminal, GUI and command-line interfaces.

## Contents
| Item | Description |
|---|---|
| `qmaws-cli/` | The `qmaws` binary: argument parsing and dispatch to terminal or GUI mode ([README](qmaws-cli/README.md)) |
| `qmaws-core/` | All science: input, MAW extraction, matrices, quartet counts, weights, amalgamation, support, metrics ([README](qmaws-core/README.md)) |
| `qmaws-data/` | Benchmark registry, downloads, manifests, reference trees, Open Tree of Life client ([README](qmaws-data/README.md)) |
| `qmaws-engine/` | Run orchestration: stages, checkpoints, chunks, progress events, audit log ([README](qmaws-engine/README.md)) |
| `qmaws-gui/` | Desktop GUI application ([README](qmaws-gui/README.md)) |
| `qmaws-tui/` | Terminal menus and progress display ([README](qmaws-tui/README.md)) |
| `qmaws-viz/` | Halo Tree, tanglegram, rectangular tree, HTML export, animation ([README](qmaws-viz/README.md)) |

## Relationships
The workspace manifest is `Cargo.toml` in the repository root. `qmaws-cli` depends on all other crates. `qmaws-engine`, `qmaws-tui` and `qmaws-gui` will depend on `qmaws-core`; `qmaws-core` depends on no other workspace crate.

## Notes
`qmaws-core` has no knowledge of files, terminals or windows: it receives data and returns data (see `docs/DESIGN.md`). Shared package fields (version, edition, authors, license file) are set once in the workspace manifest.
