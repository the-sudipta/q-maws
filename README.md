# Q-MAWS

**Q-MAWS: Quartet-based phylogeny from Minimal Absent Word Sets.** Q-MAWS estimates a species tree from whole genomes without sequence alignment, by scoring every four-taxon subset (quartet) of a minimal absent word (MAW) character matrix and combining the weighted quartets into one tree.

> **Status: early development (milestone M1, engine skeleton).** The run engine (checkpoints, resume, progress display) works with a toy computation. No analysis can be run yet. This README is a draft and is completed as features are added.

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

To be written (milestone M9).

## Terminal mode

To be written (milestones M1 to M8).

## Direct commands

Available in this build: `qmaws toy-run` (exercises checkpoints, resume and the progress display), `qmaws resume`, `qmaws --help` and `qmaws --version`. The full command list is in `docs/USER_GUIDE.md`.

## Benchmark datasets

To be written (milestone M2). Datasets are downloaded by the program and verified by checksum; they are never stored in this repository.

## Verifying a run

To be written (milestone M8).

## Reproducing the paper's results

To be written (milestones M11 to M13).

## Repository map

| Item | Description |
|---|---|
| `.gitattributes` | Line-ending rules: LF for shell scripts and hooks, CRLF for Windows batch files |
| `.github/` | GitHub configuration: continuous integration workflows ([README](.github/README.md)) |
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
