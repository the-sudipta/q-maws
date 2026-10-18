# Q-MAWS

[![CI](https://github.com/the-sudipta/q-maws/actions/workflows/ci.yml/badge.svg)](https://github.com/the-sudipta/q-maws/actions/workflows/ci.yml)
[![License: Q-MAWS Source-Available v1.0](https://img.shields.io/badge/license-Q--MAWS%20Source--Available%20v1.0-orange)](LICENSE)

**Q-MAWS: Quartet-based phylogeny from Minimal Absent Word Sets.** Q-MAWS estimates a species tree from whole genomes without sequence alignment, by scoring every four-taxon subset (quartet) of a minimal absent word (MAW) character matrix and combining the weighted quartets into one tree.

> **Version 1.0.0** · all milestones (M0–M14) complete · pre-registered evaluation finished · [Get the tool](#get-the-tool) · [For reviewers and examiners](#for-reviewers-and-examiners) · [Where the code is](docs/ARCHITECTURE.md) · **[Project website](https://the-sudipta.github.io/q-maws/)**

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

## For reviewers and examiners

### Which download do you need?

| You want to… | Download | Notes |
|---|---|---|
| Use Q-MAWS on your own genomes, or try the window | **The release** ([Releases page](https://github.com/the-sudipta/q-maws/releases/latest)) | Ready to run; nothing to install |
| Read the results, tables and figures | Nothing: browse this page, `results/` and `figures/paper/` | |
| Check that no result was altered (SHA-256) | **The whole repository** (`git clone https://github.com/the-sudipta/q-maws`) | About 1 GB, mostly run records; Python 3 for the checker |
| Verify runs, check one part, or reproduce everything | **The whole repository** plus the release program (or build it with `cargo build --release`) | Verification and reproduction also need the input data (section 2) |

Clone with `git` rather than downloading a ZIP: the repository's `.gitattributes` keeps the hashed files byte for byte on every system.

### 1. Check that nothing was altered (SHA-256)

Every result file `X` has a companion `X.sha256` holding its SHA-256 hash.

- **All files at once** (Windows, macOS, Linux):
  ```
  python scripts/check_hashes.py
  ```
  Expected: `2499 files checked, 2499 match, 0 differ or are missing`. Give a folder to check only that part, for example `python scripts/check_hashes.py results/h5`.
- **One file by hand:**
  - Linux or macOS: run `sha256sum results/runs/fish_mito_seed2/report/tree.nwk` and compare it with `cat results/runs/fish_mito_seed2/report/tree.nwk.sha256`.
  - Windows PowerShell: run `(Get-FileHash results\runs\fish_mito_seed2\report\tree.nwk -Algorithm SHA256).Hash.ToLower()` and compare it with `Get-Content results\runs\fish_mito_seed2\report\tree.nwk.sha256`.
- **The release downloads:**
  - Linux or macOS: `sha256sum -c SHA256SUMS.txt` in the download folder.
  - Windows: `Get-FileHash <file> -Algorithm SHA256`, compared with the line in `SHA256SUMS.txt`.
- **The root fingerprint** (`audit/root.txt` in every run) summarises the whole run: the settings and the content hash of every stage (listed in `audit/stages.json`). It is recomputed from the inputs by `qmaws verify --full` (section 2).
- **The input sequences:** each one's hash is in the run's `audit/inputs.json`, and `qmaws verify --inputs` checks them.

### 2. Verify results (minutes to hours)

Verification recomputes parts of a run from its inputs and compares them with the record. The inputs are not stored in the repository, so get them first:

- **AFproject and NCBI datasets:** `qmaws download --dataset <id>` (or `all`). Every archive is checked against its published checksum.
- **H5 datasets:** the branch `h5-data`: `git fetch origin h5-data` and `git checkout origin/h5-data -- h5data`, then unpack `h5data/<id>.fasta.gz` to `data/raw/simulated/<id>/sequences.fasta`. The SHA-256 values are in `data/manifests/simulated/hgt.tsv`.
- **Scaling datasets (H2):** `sh scripts/simulate_scaling.sh <path to iqtree2>`, with the hashes in `data/manifests/simulated/scaling.tsv`. AliSim gives identical data only with the same build (IQ-TREE 2.4.0 for Windows was used).

**In the window (GUI):** open the program, choose **Verify** in the sidebar, then **Browse…** to a run folder, for example `results/runs/fish_mito_seed2`. Choose the kind of check, then **Verify**:

| Kind | What it does | Time |
|---|---|---|
| Quick | The input check, then a random sample of 20 chunks per stage recomputed and compared | Minutes for most runs |
| Full | The whole run recomputed; the root fingerprint compared | As long as the run took |
| One quartet | The full worksheet of four taxa you name, compared with the stored decision | Seconds |
| Inputs only | The sequence files checked against their hashes | Seconds |

The report is shown in the window and saved as `report/verify_<time>.txt` in the run folder. The **Results** library shows each run's Quartet Halo Tree with its nRF, nQD and MSD.

**On the command line (CLI):**
```
qmaws verify --output results/runs/fish_mito_seed2                     # quick check (the default)
qmaws verify --output results/runs/fish_mito_seed2 --full              # recompute everything
qmaws verify --output results/runs/fish_mito_seed2 --quartet A,B,C,D   # one quartet's worksheet
qmaws verify --output results/runs/fish_mito_seed2 --inputs            # only the inputs
qmaws verify --output <run> --input <folder>                           # if the data are elsewhere
```
A verdict `PASS` means every comparison agrees; the exit status is 1 if anything differs. Every committed run already has its own PASS report in `report/`.

### 3. Check one part

| To check… | GUI | CLI |
|---|---|---|
| One quartet's calculation (16 pattern counts, W1, likelihoods, W2c) | Verify → One quartet | `qmaws verify --output <run> --quartet A,B,C,D`; 50 sample worksheets are also in each run's `audit/sample_worksheets.txt` |
| The hand-worked teaching example | — | `qmaws teach --example` |
| The tree error of any tree against a reference | Results library | `qmaws tree-compare --tree <tree.nwk> --reference <reference.nwk>` |
| A run's figures, drawn again | — | `qmaws figures --output <run>` |
| The M11 tables and the H1 test | — | `qmaws summary --h1-pairs mean --h1-zeros wilcoxon --output <folder>`, compared with `results/summary/` |
| H3 (long-branch simulation) | — | `qmaws simulate-h3 --output <folder>`, compared with `results/h3/` |
| H4 (calibration) | — | `qmaws calibration --pair TREE=TRUTH ... --scale 1` (Q-MAWS) or `--scale 100` (UFBoot); the pairs are in `results/h4/README.md` |
| H5 | — | `qmaws h5`, which rebuilds `results/h5/h5_test.tsv` from the run folders |
| H2 (speed) | — | `qmaws h2`, which rebuilds `results/scaling/h2_test.txt` from `times.tsv` |
| The comparison with Mash, CD-MAWS and Peafowl | — | `qmaws baselines`; each tool's own outputs and versions are in `results/baselines/<tool>/<dataset>/` |
| The result charts | — | `python scripts/paper_figures.py`, which redraws `figures/paper/` |
| Equality with ML-MAWS's matrix, wQFM, IQ-TREE and DendroPy | — | `scripts/equivalence.sh`, `scripts/wqfm_check.sh`, `scripts/iqtree_check.sh`, `scripts/metrics_check.py` (they need those programs) |

### 4. Reproduce everything (weeks)

The same inputs, settings and seed give the same **root fingerprint** on Windows, macOS and Linux. A reproduced run can therefore be compared with the committed one by its `audit/root.txt`, and every table and chart built from such runs is identical. The settings of every committed run are in its `run.json`.

<details>
<summary><b>Step by step, with the times measured on the development laptop</b> (Intel Core 5 210H, 12 threads, 8 GB memory)</summary>

1. **Data:** `qmaws download --dataset all` (219 MB to download).
2. **M11 benchmark** (14 datasets × 5 seeds):
   - Seed 1 with the S2 bootstrap: `qmaws run --dataset <id> --seed 1 --bootstrap 100`.
   - Seeds 2 to 5 without it: `qmaws run --dataset <id> --seed <n> --bootstrap 0`.
   - Rhinovirus seed 1 was also run with `--bootstrap 0`.
   - The mean time per run was 89 s (Yersinia HGT) to about 14.6 h (rhinovirus), about 8 days in total; much of it was measured before a Windows speed fix, so it is likely less now.
   - Then the tables and H1: `qmaws summary --h1-pairs mean --h1-zeros wilcoxon`.
3. **H3:** `qmaws simulate-h3 --output results/h3`.
4. **M12 sensitivity:**
   - The weight variants: `qmaws variant --run <seed-1 run> --weights w1|w2a|w2b --output <folder>`.
   - The setting variants: `qmaws run --dataset <id> --seed 1 --bootstrap 0` with `--top-lengths 1|2|4`, `--no-strand`, `--matrix ml` or `--weighting w2-emp`.
   - Every run is listed in `results/sensitivity/settings.tsv`.
5. **H4:** ML-MAWS with IQ-TREE on the five simulated HGT datasets (`scripts/mlmaws_h4.sh`, workflow `mlmaws_h4.yml`), then `qmaws calibration` as in `results/h4/README.md`.
6. **H5** (20 datasets, about one day): for each dataset:
   - `qmaws run --input data/raw/simulated/<id>/sequences.fasta --reference data/manifests/simulated/trees/<id>.nwk --seed 1 --bootstrap 0`;
   - then `qmaws variant --run <run> --weights w1 --output <folder>`;
   - then ML-MAWS (`--strand --iqtree`).
   - Then `qmaws h5`.
7. **Comparison tools:** `sh scripts/baselines.sh mash|cdmaws|peafowl <dataset> <output>` on Linux (workflow `baselines.yml`), then `qmaws baselines`.
8. **M13 speed** (about 1.5 days, one run at a time, nothing else running):
   - `qmaws run --input <scaling data> --seed 1 --bootstrap 0` and ML-MAWS on each scaling dataset.
   - Above 50 taxa, the IQ-TREE command of Amendment 6.
   - Then `qmaws h2`.
   - Run times depend on the computer; the H2 comparison is only meaningful on one computer.
9. **Charts:** `python scripts/paper_figures.py`.

Every result folder's README names the exact command and build. In the window, runs are started with **New run**: choose a benchmark dataset or a folder, a reference tree, the output folder and the settings (seed, S2 replicates), then **Start the run**. A queue of datasets runs one after another and survives restarts (**Resume**). The hypothesis tables are made by the development commands above.

</details>

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
| `site/` | The project website (published on GitHub Pages, rebuilt from the results on every push) ([README](site/README.md)) |
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
