# workflows

## Purpose
Continuous integration and release workflows: `ci.yml` (formatting, lints, tests and golden tests on Windows, macOS and Linux) and `release.yml` (release binaries for tagged versions).

## Contents
| Item | Description |
|---|---|

## Relationships
Build and test `crates/`; release bundles include `run.bat`, `run.sh`, `README.md` and `LICENSE`.

## Notes
Empty in milestone M0. Basic CI is added in milestone M1; full CI and releases in milestone M14.
