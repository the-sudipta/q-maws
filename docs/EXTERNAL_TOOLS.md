# External tools

External tools, services and repositories used during development or by the program. For each one: what it is used for, the version, and what was verified (documentation read, license, options), with the date.

## Development toolchain

| Tool | Version | Used for | Verified |
|---|---|---|---|
| Rust (`rustc`, `cargo`) | 1.98.1 | Building and testing the workspace | Version from `rustc --version` and `cargo --version` on 2026-10-02; standard library license from `share/doc/rust/COPYRIGHT-library.html` in the toolchain |
| rustfmt, clippy | shipped with Rust 1.98.1 | Formatting and lint checks | Run on 2026-10-02 |
| Git | 2.56.0 (Git for Windows) | Version control; hooks run with its bundled shell on Windows | Version from `git --version` on 2026-10-02 |

## Repositories, services and data sources

None used yet. Entries for ML-MAWS, the CD-MAWS suffix automaton implementation, wQFM, IQ-TREE, AFproject, NCBI E-utilities and Open Tree of Life are added when each is first consulted, including its license.
