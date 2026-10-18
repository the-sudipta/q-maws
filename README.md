# Q-MAWS

[![CI](https://github.com/the-sudipta/q-maws/actions/workflows/ci.yml/badge.svg)](https://github.com/the-sudipta/q-maws/actions/workflows/ci.yml)
[![License: Q-MAWS Source-Available v1.0](https://img.shields.io/badge/license-Q--MAWS%20Source--Available%20v1.0-orange)](LICENSE)

**Q-MAWS: Quartet-based phylogeny from Minimal Absent Word Sets.** Q-MAWS estimates a species tree from whole genomes without sequence alignment, by scoring every four-taxon subset (quartet) of a minimal absent word (MAW) character matrix and combining the weighted quartets into one tree.

> **Version 1.0.0** · all milestones (M0–M14) complete · pre-registered evaluation finished · [Get the tool](#get-the-tool) · [Reproduce the results](#reproduce-or-check-the-results) · [Where the code is](docs/ARCHITECTURE.md)

## What Q-MAWS does

Q-MAWS keeps the MAW character matrix used by ML-MAWS (one row per genome, one column per minimal absent word, value 1 if the word is a MAW of that genome). Instead of a global maximum-likelihood tree search, it:

1. counts, for every quartet of taxa, how often each of the 16 binary column patterns occurs;
2. scores the three possible quartet topologies with a small two-state likelihood model that uses every pattern except 0000, conditioned on the pattern not being 0000;
3. combines the weighted quartets into one species tree with a re-implementation of the wQFM amalgamation algorithm.

What is **not** new: MAW extraction, the MAW matrix itself (ML-MAWS), quartet amalgamation as a concept (wQFM and others), and alignment-free bootstrapping.

## What we found

All hypotheses, tests and criteria were committed before the experiments (`docs/PREREGISTRATION.md`, with seven dated amendments). Results are reported whatever they show.

| Hypothesis | Result |
|---|---|
| H1: lower tree error than ML-MAWS under horizontal gene transfer | Not supported: comparable nRF (lower on 4 of 7 datasets, equal on 3), higher nQD on all 7 (`results/summary/`) |
| H2: faster than ML-MAWS for many taxa (same computer) | Not supported: about 30× faster at 25 taxa and 5–12× at 50, but 2–9× slower at 100 (`results/scaling/`) |
| H3: the conditioned quartet likelihood resists long-branch attraction where simple voting fails | Supported: 100%, 100% and 98.5% recovery against 0% for voting (`results/h3/`) |
| H4: support at least as well calibrated as ML-MAWS's UFBoot | Not supported by 0.00004 (ECE 0.200240 against 0.200200); the column bootstrap S2 has ECE 0.086 (`results/h4/`) |
| H5: the vote-weight variant (v2) beats ML-MAWS on new simulated data | Not supported; the primary configuration was more accurate than ML-MAWS on the same 20 new datasets (pre-registered descriptive comparison; `results/h5/`) |

Against peer-reviewed tools run on the same 29 datasets (`results/baselines/`), Q-MAWS was more accurate than CD-MAWS on the new data, while the k-mer tools Mash and Peafowl were more accurate than every MAW-based method. ML-MAWS as released stops without a tree above 50 taxa with IQ-TREE 2.4.0 (Amendment 6), and Q-MAWS needs about 12 GB of memory at 200 taxa (Amendment 7).

## Get the tool

Ready-to-run bundles for **Windows, macOS (Apple silicon and Intel) and Linux** are on the [Releases page](https://github.com/the-sudipta/q-maws/releases/latest) (current version: **v1.0.0**, with `SHA256SUMS.txt`). Nothing else needs to be installed. Unpack the bundle and:

- **Windows:** double-click `Q-MAWS.exe` for the window, or `run.bat` for the terminal menu.
- **macOS:** double-click `Q-MAWS.app`, or run `./run.sh`.
- **Linux:** double-click `Q-MAWS`, or run `./run.sh`.

How to use the window and the commands: [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## Reproduce or check the results

1. Clone this repository. It holds the source code, every run record, the result tables and the reports.
2. Build: `cargo build --release` (the Rust version is pinned in `rust-toolchain.toml`), or use the release program.
3. Check any committed run without recomputing it: `qmaws verify --output results/runs/<run>`. Add `--full` to recompute the whole run and compare its root fingerprint.
4. Make a run again with the settings in its `run.json`, for example `qmaws run --dataset <id> --seed <n> --bootstrap <b> --output <folder>`. The root fingerprint is the same on Windows, macOS and Linux (golden test G12).

Every folder in `results/` has a README naming the command and the build that made it. The experiments that use other programs (ML-MAWS, IQ-TREE, Mash, CD-MAWS, Peafowl) have their scripts in `scripts/` and their workflows in `.github/workflows/`.

## Where things are

| Looking for | Go to |
|---|---|
| **The method in the code** (each step mapped to its file and functions), the GUI, the CLI, the terminal menu, the CI | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| The mathematics of the method | [docs/METHOD.md](docs/METHOD.md) |
| Hypotheses, tests and criteria fixed before the experiments, with seven dated amendments | [docs/PREREGISTRATION.md](docs/PREREGISTRATION.md) |
| The report of every milestone, M0 to M14 | [docs/milestones/](docs/milestones/README.md) |
| All results and run records | [results/](results/README.md) |
| All documentation | [docs/](docs/README.md) |

## Repository map

| Item | Description |
|---|---|
| `.gitattributes` | Line-ending rules: LF for shell scripts and hooks, CRLF for Windows batch files, no conversion in run folders and control inputs (they are checked byte for byte) |
| `.github/` | Continuous integration, the release workflow and the experiment workflows ([ABOUT](.github/ABOUT.md)) |
| `.zenodo.json` | Metadata for the Zenodo archive of each release (title, author, ORCID, description, keywords); the license is set on Zenodo when the archive is made |
| `.gitignore` | Paths never committed: build output, raw data, run work folders, temporary files |
| `assets/` | The Q-MAWS logo and program icon ([README](assets/README.md)) |
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
| `figures/` | The result charts of the thesis and paper, drawn from `results/` ([README](figures/README.md)) |
| `LICENSE` | Q-MAWS Source-Available License v1.0 |
| `results/` | Run records, experiment results and summary tables ([README](results/README.md)) |
| `run.bat` | Launcher for Windows |
| `run.sh` | Launcher for macOS and Linux |
| `rust-toolchain.toml` | Pinned Rust toolchain version and components |
| `scripts/` | Scripts for the experiments that use other programs, the checks and the commit guard ([README](scripts/README.md)) |
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
