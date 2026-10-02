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
| `deny.toml` format | cargo-deny configuration | License, ban and source policy | Section and key names taken from `deny.template.toml` in the cargo-deny repository (branch `main`) on 2026-10-02. The first CI run failed with `error[wildcard]` on the path dependencies between workspace crates; reproduced locally with cargo-deny 0.20.2 (installed with `cargo install cargo-deny --locked`) and fixed with `allow-wildcard-paths = true`, which then gave `bans ok, licenses ok, sources ok` |
| `rustup toolchain install` | rustup 1.29.1 | Installing the toolchain pinned in `rust-toolchain.toml` on each runner | `rustup toolchain install --help` (rustup 1.29.1): with no argument it installs "the active toolchain", which is the one named in `rust-toolchain.toml` |

## Repositories, services and data sources

### AFproject (https://afproject.org)

- **Used for:** five benchmark archives (nine datasets) and their reference trees.
- **Verified on 2026-10-02:** each dataset page (`/app/benchmark/genome/.../dataset/`) links the archive as `/media/genome/.../dataset/<file>.zip` and states its size and MD5; the links in `data/manifests/benchmarks.toml` were taken from these pages, and every download matched the published MD5. The E. coli (29) and E. coli/Shigella HGT archives have published MD5 values (`de88729e76a47c1de7f06a6c59298cb8`, `e4282d59f4dae2fd6e4914cb747e5566`). The archives contain no reference trees; each results page (`/app/benchmark/genome/.../results/`) embeds the reference tree as a `data:text/plain;charset=utf-8,` link (see `data/references/SOURCE.md`). The server supports HTTP range requests (`Accept-Ranges: bytes`), used to resume downloads; an interrupted 114,629,704-byte download resumed at byte 12,304,384 and verified.
- **Reference package:** `AF-reference_datasets190511.zip`, 4,894,058,545 bytes, last modified 2019-08-09 (HTTP headers); not downloaded. The results archive `AF-results190511.zip` is 12,422,678,494 bytes; not downloaded.
- **Citation:** A. Zielezinski et al., Genome Biology 20:144 (2019).

### ML-MAWS repository (https://github.com/PapriSaha/ML-MAWS)

- **License:** Apache License 2.0 (`LICENSE.txt`, read before any file was used).
- **Cloned** on 2026-10-02 to `../_external/ML-MAWS` (outside this repository) at commit `0c38db12d9ad271aafcb4940d7558dfcd00925c1` (2026-05-09).
- **Used for:** the five NCBI dataset files in `Data/` (downloaded by `qmaws download` from `raw.githubusercontent.com` at that commit; each download matched the SHA-256 of the file at that commit), and the name tables in `Data/*/reference/dataset.json` (see `data/references/SOURCE.md`). No code is used.
- **Read:** `README.md`; `FastaReader.cpp` (input reading: keeps A, C, G, T after upper-casing, with no conversion of U to T; reads `.fasta`, `.fa`, `.fna`, `.fas` from a folder in sorted order; a single-record file takes the header's first word as name, a multi-record file is joined and named after the file; a single multi-FASTA file is read as one taxon per record); `main.cpp` (input may be a folder or a file); `run_all_benchmarks.sh` (the five NCBI datasets are single multi-FASTA files). Further files (suffix automaton, MAW extraction, entropy selection, matrix building) are read in M3.

### GitHub raw file service

- `https://raw.githubusercontent.com/<owner>/<repository>/<commit>/<path>` serves a file at a fixed commit. Verified on 2026-10-02 by downloading the five ML-MAWS data files and comparing their SHA-256 with `git show <commit>:<path>` in the clone: all identical.

### cargo-deny

- Version 0.20.2, installed locally with `cargo install cargo-deny --locked` on 2026-10-02 (development tool only). Used to reproduce the CI dependency check.

### Not used yet

The CD-MAWS suffix automaton implementation, wQFM, IQ-TREE, NCBI E-utilities and Open Tree of Life are added when each is first consulted, including its license.
