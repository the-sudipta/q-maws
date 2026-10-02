# Design

Software design decisions and their justification. Sections are added by the milestone that makes the decision.

## Workspace layout (M0)

The code is a Cargo workspace with seven crates:

| Crate | Responsibility |
|---|---|
| `qmaws-core` | All science: input, MAW extraction, matrices, quartet counts, weights, amalgamation, support, metrics |
| `qmaws-engine` | Run orchestration: stages, checkpoints, chunks, progress events, audit log |
| `qmaws-data` | Benchmark registry, downloads, manifests, reference trees, Open Tree of Life client |
| `qmaws-viz` | Halo Tree, tanglegram, rectangular tree, HTML export, animation |
| `qmaws-tui` | Terminal menus and progress display |
| `qmaws-gui` | Desktop GUI application |
| `qmaws-cli` | The `qmaws` binary: argument parsing, dispatch to terminal or GUI mode |

Rule: `qmaws-core` has no knowledge of files, terminals or windows. It receives data and returns data. This keeps the science fully testable and guarantees that terminal and GUI modes give identical results.

## Toolchain (M0)

The Rust toolchain is pinned to a specific stable release in `rust-toolchain.toml` (edition 2021), so every developer and CI machine builds with the same compiler. The pin is raised deliberately, in its own commit.

## To be written

- Run directory, checkpoints and resume (M1)
- Optimiser for the conditioned quartet likelihood (M5)
- wQFM-rs algorithm details with references to the wQFM paper (M7)
