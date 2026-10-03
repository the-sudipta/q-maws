# Changelog

All notable changes to Q-MAWS are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Each tagged version has its own section. Milestone tags have the form `v0.<n>-m<xx>-<short-name>`; the first public release is `v1.0.0`.

## [Unreleased]

## [v0.9-m09-gui] - 2026-10-03

### Added

- Graphical interface (`qmaws gui`, `--gui` on `run` and `resume`): the main menu as tabs, the steps of a new run, the live provisional Halo Tree with changed branches highlighted, the live worksheet, the stage log, progress, Pause and Stop; unfinished runs offered on launch. Built with `eframe`/`egui` without built-in fonts (a system font is loaded).
- Interactive terminal menu (`qmaws menu`, started by `run.bat` and `run.sh` without arguments): start, resume (one run, or all one after another), verify, exit.
- Pause: the engine waits between units of work without counting the time.
- Live provisional tree while quartets are weighed (`figures/live/`, frames, `report/convergence.csv`); `--no-live-tree`.
- `qmaws resume --all` with a saved queue; `--gui` and `--terminal`; `qmaws verify --quick`; a CPU core limit in the menu settings.
- Golden test G10: terminal and GUI give the same root; runs move between the interfaces at any stop.

### Changed

- `qmaws resume` without `--output` also finds runs in the results folders used before (user configuration file).
- BSL-1.0 added to the allowed licenses (clipboard support of `eframe`).

## [v0.8-m08-support] - 2026-10-03

### Added

- S1 support of every internal edge and halo values of every taxon (stage `support`), and S2 column-bootstrap support (stage `bootstrap`: 100 replicates by default with W2b inside, `--bootstrap`; decision D7).
- Audit record in every run folder: `results.json`, `quartet_decisions.bin.zst`, 50 sample worksheets, `environment.json`, and per-stage times and summary numbers in `stages.json`.
- `qmaws verify`: input check, quick check, full recomputation of the root, and single quartet with its worksheet.
- Positive and negative controls in `results/controls/` (hidden command `controls`); S2 cost measurement (hidden command `s2-cost`); golden test G11 (Fish mtDNA killed 10 times and resumed).

### Changed

- W2c counts a resample whose three fits are stars (internal branch at the lower bound) as a three-way tie (OI-14; pre-registration Amendment 1). H3 was run again: verdict unchanged.
- Root fingerprint format v2: independent of where the data are kept.

## [v0.7-m07-amalgamation] - 2026-10-02

### Added

- Quartet amalgamation with `wQFM-rs`, a Rust implementation of wQFM (Apache License 2.0): analysis runs now end with the stage `amalgamate` and write `report/tree.nwk`. Exhaustive tree oracle (G8); G2 complete. Identical to the original wQFM jar on 23 comparison inputs (development check on GitHub Actions).

## [v0.6-m06-h3] - 2026-10-02

### Added

- Long-branch simulation of hypothesis H3 (`qmaws simulate-h3`): per-character simulator, 2,400 seeded replicates, recovery table, figure and report in `results/h3/`. H3 is supported: at 100,000 characters W1 recovered the true quartet in 0% of replicates and W2 in 98.5% to 100%.
- SVG line charts in `qmaws-viz`.

## [v0.5-m05-weighting] - 2026-10-02

### Added

- Quartet weighting: W1 votes; conditioned quartet maximum likelihood under the two-state model (W2-sym, W2-emp) with an exact-derivative Newton optimiser; W2a, W2b and W2c (100 multinomial resamples with per-quartet seeds); golden test G3.
- Analysis runs gain the resumable stage `quartet_weight` and the options `--weighting` and `--replicates`.
- IQ-TREE cross-check of the quartet likelihood on GitHub Actions (development only).

## [v0.4-m04-counting] - 2026-10-02

### Added

- Quartet ranks and unranking (G6), a keyed Feistel processing order (G7), and the inclusion–exclusion pattern-count kernel with run-time popcount dispatch (G5).
- Analysis runs (`qmaws run`): resumable stages from input to the pattern counts of every quartet, chunked in a seeded random order; root fingerprint independent of chunk size.
- Teaching worksheet (`qmaws teach --example`, `qmaws teach --input`), golden test G1; G2 up to the pattern table.
- Newick splits and normalised Robinson–Foulds distance.

## [v0.3-m03-matrices] - 2026-10-02

### Added

- MAW extraction with a suffix automaton (lengths up to 64 letters), strand filter, and a brute-force oracle; golden test G4 (2,000 random strings).
- Length selection by entropy and the adaptive length range as in ML-MAWS; `M_full` and `M_ml` (constant columns removed, 50,000-column cap) with PHYLIP export.
- Parallel extraction within a memory limit (70% of the available memory), memory estimate before the matrix is built.
- Command `qmaws matrix`.
- Equivalence check with ML-MAWS on GitHub Actions: `M_ml` byte-identical on Fish mtDNA and Yersinia HGT.

## [v0.2-m02-data] - 2026-10-02

### Added

- Input reading and cleaning (FASTA and raw text, `.gz`, cleaning rules, Newick-safe names) with all validation findings.
- Newick parser and writer; name comparison between trees and taxa.
- Benchmark registry with 19 datasets: nine AFproject datasets, five NCBI datasets built from published accession lists (pinned record versions), and the five ML-MAWS data files as reference only.
- NCBI datasets fetched with E-utilities (batched, rate-limited, tool and email), verified against pinned checksums and identical in sequence to the files ML-MAWS used.
- Downloads with resume, retries and MD5 and SHA-256 verification; safe archive extraction; download log.
- Reference trees for the AFproject datasets, with name tables checked against two independent sources.
- Commands `qmaws datasets`, `qmaws download` and `qmaws inspect` (with `--compare-with`).
- One taxon per record is named by its whole header line.

## [v0.1-m01-engine] - 2026-10-02

### Added

- Run engine: run folders, `run.json` stage state machine, atomic hash-verified writes, frozen chunk plans, resume after interruption or hard kill, root fingerprint independent of chunk boundaries.
- Progress events and time estimates (calibration, moving-average throughput, cost-weighted overall progress).
- Terminal progress display with overall and stage bars; `--quiet`, `--json-progress` and `--no-color`.
- Commands `qmaws toy-run` and `qmaws resume`; Ctrl+C stops cleanly, a second Ctrl+C exits at once.
- Continuous integration on Windows, macOS and Linux, and a dependency license check with `cargo deny`.

## [v0.0-m00-foundation] - 2026-10-02

### Added

- Repository skeleton: Cargo workspace with seven empty crates, folder READMEs, ignore and line-ending rules.
- Pre-commit guard with hook installers and a self-test.
- Launch scripts `run.bat` and `run.sh`.
- `qmaws --help` and `qmaws --version`.
- Pre-registration of hypotheses H1 to H4 (`docs/PREREGISTRATION.md`).
- Citation metadata (`CITATION.cff`).
