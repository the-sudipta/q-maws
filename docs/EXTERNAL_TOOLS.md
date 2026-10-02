# External tools

External tools, services and repositories used during development or by the program. For each one: what it is used for, the version, and what was verified (documentation read, license, options), with the date.

## Development toolchain

| Tool | Version | Used for | Verified |
|---|---|---|---|
| Rust (`rustc`, `cargo`) | 1.98.1 | Building and testing the workspace | Version from `rustc --version` and `cargo --version` on 2026-10-02; standard library license from `share/doc/rust/COPYRIGHT-library.html` in the toolchain |
| rustfmt, clippy | shipped with Rust 1.98.1 | Formatting and lint checks | Run on 2026-10-02 |
| Git | 2.56.0 (Git for Windows) | Version control; hooks run with its bundled shell on Windows | Version from `git --version` on 2026-10-02 |

## Continuous integration (GitHub Actions)

| Tool | Version | Used for | Verified |
|---|---|---|---|
| `actions/checkout` | `v7` (newest major tag) | Checking out the repository in each CI job | Tags listed with `git ls-remote --tags https://github.com/actions/checkout` on 2026-10-02; usage `uses: actions/checkout@v7` from its README |
| `EmbarkStudios/cargo-deny-action` | `v2` (newest major tag) | Running `cargo deny check licenses bans sources` with `deny.toml` | Tags listed with `git ls-remote` on 2026-10-02; inputs `rust-version`, `command`, `command-arguments` read from its `README.md` and `action.yml` at tag `v2`. It is a Docker action, so it runs on the Linux runner only; its built-in Rust is older, so `rust-version` is set to the pinned 1.98.1 |
| `deny.toml` format | cargo-deny configuration | License, ban and source policy | Section and key names taken from `deny.template.toml` in the cargo-deny repository (branch `main`) on 2026-10-02 |
| `rustup toolchain install` | rustup 1.29.1 | Installing the toolchain pinned in `rust-toolchain.toml` on each runner | `rustup toolchain install --help` (rustup 1.29.1): with no argument it installs "the active toolchain", which is the one named in `rust-toolchain.toml` |

## Repositories, services and data sources

None used yet. Entries for ML-MAWS, the CD-MAWS suffix automaton implementation, wQFM, IQ-TREE, AFproject, NCBI E-utilities and Open Tree of Life are added when each is first consulted, including its license.
