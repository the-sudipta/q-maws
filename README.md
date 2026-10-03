# Q-MAWS

**Q-MAWS: Quartet-based phylogeny from Minimal Absent Word Sets.** Q-MAWS estimates a species tree from whole genomes without sequence alignment, by scoring every four-taxon subset (quartet) of a minimal absent word (MAW) character matrix and combining the weighted quartets into one tree.

> **Status: early development (milestone M10, figures).** An analysis runs from sequences to the tree with S1 and S2 support and halo values, in the terminal or in a window, can be verified, and ends with its figures; the benchmark comparison and the releases come in later milestones. This README is a draft and is completed as features are added.

License: Q-MAWS Source-Available License v1.0 (see [License](#license)). Continuous integration runs on Windows, macOS and Linux (`.github/workflows/ci.yml`). A DOI badge will be added when the archive exists (M14).

## What Q-MAWS does

Q-MAWS keeps the MAW character matrix used by ML-MAWS (one row per genome, one column per minimal absent word, value 1 if the word is a MAW of that genome). Instead of a global maximum-likelihood tree search, it:

1. counts, for every quartet of taxa, how often each of the 16 binary column patterns occurs;
2. scores the three possible quartet topologies with a small two-state likelihood model that uses every pattern except 0000, conditioned on the pattern not being 0000;
3. combines the weighted quartets into one species tree with a re-implementation of the wQFM amalgamation algorithm.

The project tests the following research questions. None of them is claimed as a result before it is tested.

1. Is quartet amalgamation on MAW characters a useful alternative to global maximum-likelihood search, particularly under horizontal gene transfer?
2. Does a conditioned quartet likelihood that excludes the 0000 pattern, and uses one-taxon-differs patterns, avoid the long-branch errors of simple informative-pattern voting?
3. Can all quartet pattern counts be computed exactly and quickly by inclusion–exclusion on bitset popcounts?
4. Are the support values of MAW-based phylogeny well calibrated?
5. Does a per-taxon quartet concordance display (the Quartet Halo Tree) reveal conflicting taxa?

What is **not** new: MAW extraction, the MAW matrix itself (ML-MAWS), quartet amalgamation as a concept (wQFM and others), and alignment-free bootstrapping.

## Quick start

Not available yet. When releases exist, the plan is:

- **Windows:** download the release, unpack it, double-click `run.bat`.
- **macOS and Linux:** download the release, unpack it, run `./run.sh`.

Developers can build from source with a Rust toolchain (the version is pinned in `rust-toolchain.toml`):

```
cargo build --release
./run.sh --version        # or: run.bat --version
```

## GUI mode

`qmaws gui` (or choosing "GUI" in the menu, or `--gui` on `run` and `resume`) opens a window with the same menu as the terminal. A new run goes through the steps Data, Reference, Output, Settings, Review and Run. While it runs, the window shows the live provisional Halo Tree (zoom and pan; branches that changed since the previous update are highlighted), the live worksheet of the latest weighed quartet (its 16 pattern counts, W1 weights, W2 log-likelihoods and weights), the stage log, progress with elapsed and remaining time, and Pause and Stop buttons; the final tree is shown at the end. A run started in the window can be resumed in the terminal and the other way round, with the same result. Details: `docs/USER_GUIDE.md`.

## Terminal mode

`qmaws menu` (also `run.bat` or `./run.sh` without arguments) shows the interactive main menu: start a new run, resume an unfinished run (one, or all one after another), verify a run, exit. Every long command shows its stage, progress and time estimate, can be stopped with Ctrl+C and continued with `qmaws resume` (`--all` for every unfinished run).

## Direct commands

Available in this build: `qmaws menu`, `qmaws gui`, `qmaws figures`, `qmaws run` (from sequences to the tree with S1 and S2 support and halo values; `--gui` or `--terminal`), `qmaws resume`, `qmaws verify`, `qmaws datasets`, `qmaws download --dataset <id or all>`, `qmaws inspect --input "<folder>"`, `qmaws matrix`, `qmaws teach`, `qmaws simulate-h3`, `qmaws toy-run` (exercises checkpoints, resume and the progress display), `qmaws --help` and `qmaws --version`. The full command list is in `docs/USER_GUIDE.md`.

## Figures

Every finished run gets its figures in `figures/`: the Quartet Halo Tree (support-coloured branches, the halo ring that shows how well each taxon agrees with the tree, group bands), a rectangular tree, a tanglegram against the reference tree when there is one (with nRF, nQD and MSD), an interactive HTML tree that works offline, an animation of the live provisional trees and a convergence chart. Formats: SVG, PDF and PNG at 300 dots per inch. `qmaws figures --output "<run folder>"` draws them again, optionally with a group file or the Open Tree of Life taxonomy for the group bands. Details: `docs/USER_GUIDE.md`.

## Benchmark datasets

Run `qmaws datasets` for the list and `qmaws download --dataset all` to fetch them (218,853,951 bytes to download; about 660 MB on disk after extraction, measured on 2026-10-02). Nine datasets come from AFproject (https://afproject.org): Fish mtDNA, E. coli/Shigella, E. coli/Shigella HGT, Yersinia HGT, and five simulated HGT levels, each with a reference tree. Five more (coronavirus, ebolavirus, influenza A neuraminidase segments, mammal mtDNA, rhinovirus) are built from NCBI with the accession lists published by Li et al. (2017), without reference trees; their sequences are identical to the data ML-MAWS used. Every download is verified by checksum and recorded in `data/manifests/download_log.json`; sequences are never stored in this repository. Sources and name matching: `data/manifests/README.md`, `data/manifests/accessions/README.md`, `data/references/SOURCE.md`.

## Verifying a run

Every finished run keeps a small verification record in `audit/` (inputs, stage and chunk hashes, the root fingerprint, quartet decisions, 50 sample worksheets, the environment). `qmaws verify` checks a run against it and writes every comparison to `report/verify_<time>.txt`; the exit status is 1 if anything differs.

```
qmaws verify --output "<run folder>"                       # quick check (default)
qmaws verify --output "<run folder>" --full                # recompute everything, compare the root
qmaws verify --output "<run folder>" --quartet A,B,C,D     # one quartet with its worksheet
qmaws verify --output "<run folder>" --inputs              # only the input check
```

- The input check always runs first: it recomputes the hash of every input file and cleaned sequence.
- The quick check recomputes 20 chunks of each chunked stage and 3 bootstrap replicates, drawn with a printed seed (`--seed` repeats the draw).
- If the data were moved, give their new place with `--input "<folder>"`; the root fingerprint does not depend on where the data are.
- A reviewer needs only the run's `run.json`, its `audit/` folder and the raw data: without the `work/` folder the matrix is rebuilt from the inputs.

The committed control runs in `results/controls/runs/` can be checked this way.

## Reproducing the paper's results

To be written (milestones M11 to M13).

## Repository map

| Item | Description |
|---|---|
| `.gitattributes` | Line-ending rules: LF for shell scripts and hooks, CRLF for Windows batch files, no conversion in run folders and control inputs (they are checked byte for byte) |
| `.github/` | GitHub configuration: continuous integration workflows ([ABOUT](.github/ABOUT.md)) |
| `.gitignore` | Paths never committed: build output, raw data, run work folders, temporary files |
| `baselines/` | Published results of other methods, with exact sources ([README](baselines/README.md)) |
| `Cargo.lock` | Exact dependency versions of the Rust workspace |
| `Cargo.toml` | Rust workspace manifest listing the seven crates |
| `CHANGELOG.md` | Changes per tagged version |
| `CITATION.cff` | Citation metadata: title, author, affiliation, ORCID |
| `CONTRIBUTING.md` | Rules for commits, tests and documentation |
| `crates/` | Rust source code, one crate per concern ([README](crates/README.md)) |
| `deny.toml` | Dependency policy (allowed licenses and sources) checked by `cargo deny` in CI |
| `data/` | Dataset manifests and reference trees; raw data is downloaded, not committed ([README](data/README.md)) |
| `docs/` | Design, method, user and milestone documentation ([README](docs/README.md)) |
| `figures/` | Final publication figures ([README](figures/README.md)) |
| `LICENSE` | Q-MAWS Source-Available License v1.0 |
| `results/` | Run records, experiment results and summary tables ([README](results/README.md)) |
| `run.bat` | Launcher for Windows |
| `run.sh` | Launcher for macOS and Linux |
| `rust-toolchain.toml` | Pinned Rust toolchain version and components |
| `scripts/` | Development scripts: pre-commit guard and hook installers ([README](scripts/README.md)) |
| `tests/` | Golden test files and small test inputs ([README](tests/README.md)) |
| `THIRD_PARTY_NOTICES` | Licenses and notices of third-party components |

## Citation

Citation metadata is in `CITATION.cff`. A publication will be added to it when one exists.

## License

Q-MAWS is distributed under the **Q-MAWS Source-Available License v1.0** (`LICENSE`). **This is not an open-source license.** The source code is visible for transparency and scientific verification. Without a separate written license from the copyright holder, the only permitted uses are:

- **Verification use:** reviewing or reproducing results reported in publications by the licensor;
- **Individual academic research use:** by a single person, for non-commercial research, after registering with the licensor (Section 3.2 of the license).

Institutional use, commercial use, and derivative works require a written license. Read `LICENSE` for the exact terms. Third-party components keep their own licenses (`THIRD_PARTY_NOTICES`).

## Contact

Sudipta Kumar Das, Department of Computer Science, AIUB. Email: sudiptakumar400@gmail.com. ORCID: [0009-0000-7521-7763](https://orcid.org/0009-0000-7521-7763). License requests: see Section 12 of `LICENSE`.
