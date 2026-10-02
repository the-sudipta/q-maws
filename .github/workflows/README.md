# workflows

## Purpose
Continuous integration and release workflows run by GitHub Actions.

## Contents
| Item | Description |
|---|---|
| `equivalence.yml` | Development check on Linux, when MAW or matrix code changes: downloads Fish mtDNA and Yersinia HGT, builds ML-MAWS on the runner and runs `scripts/equivalence.sh`; the result of each dataset is shown as an annotation |
| `iqtree.yml` | Development check on Linux, when the weighting code changes: downloads Fish mtDNA and the official IQ-TREE 3.1.4 release on the runner and runs `scripts/iqtree_check.sh`; the comparison is shown as annotations |
| `wqfm.yml` | Development check on Linux, when the amalgamation code changes: downloads Fish mtDNA, checks out the wQFM jar on the runner and runs `scripts/wqfm_check.sh` with the runner's Java; the comparison is shown as annotations |
| `ci.yml` | On every push and pull request: formatting, lints, tests in release mode, the guard self-test and the README check on Windows, macOS and Linux; dependency licenses, bans and sources with `cargo deny` on Linux |

## Relationships
Builds and tests `crates/` with the toolchain pinned in `rust-toolchain.toml`; runs `scripts/test-guard.sh` and `scripts/check-readmes.sh`; reads the dependency policy from `deny.toml`. Release bundles (from M14) include `run.bat`, `run.sh`, `README.md` and `LICENSE`.

## Notes
Basic CI exists from milestone M1. Golden tests G1, G2, G10 and G12 are added with their milestones, and `release.yml` (release binaries for tagged versions) in milestone M14. Workflow and job names are plain English.
