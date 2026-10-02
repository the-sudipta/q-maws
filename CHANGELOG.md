# Changelog

All notable changes to Q-MAWS are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Each tagged version has its own section. Milestone tags have the form `v0.<n>-m<xx>-<short-name>`; the first public release is `v1.0.0`.

## [Unreleased]

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
