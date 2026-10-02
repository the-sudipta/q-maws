# Changelog

All notable changes to Q-MAWS are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Each tagged version has its own section. Milestone tags have the form `v0.<n>-m<xx>-<short-name>`; the first public release is `v1.0.0`.

## [Unreleased]

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
