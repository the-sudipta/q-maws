# workflows

## Purpose
Continuous integration and release workflows run by GitHub Actions.

## Contents
| Item | Description |
|---|---|
| `equivalence.yml` | Development check on Linux, when MAW or matrix code changes: downloads Fish mtDNA and Yersinia HGT, builds ML-MAWS on the runner and runs `scripts/equivalence.sh`; the result of each dataset is shown as an annotation |
| `iqtree.yml` | Development check on Linux, when the weighting code changes: downloads Fish mtDNA and the official IQ-TREE 3.1.4 release on the runner and runs `scripts/iqtree_check.sh`; the comparison is shown as annotations |
| `wqfm.yml` | Development check on Linux, when the amalgamation code changes: downloads Fish mtDNA, checks out the wQFM jar on the runner and runs `scripts/wqfm_check.sh` with the runner's Java; the comparison is shown as annotations |
| `metrics.yml` | Development check on Linux, when the tree metric code changes: golden test G9. Runs `qmaws metrics-check` on 500 random tree pairs (nQD against enumeration over splits, MSD against brute-force matching), then `scripts/metrics_check.py` with DendroPy 5.1.0 (nRF); the comparison is shown as annotations |
| `mlmaws_h4.yml` | Development experiment for H4, run when its script changes: `scripts/mlmaws_h4.sh` on the five simulated HGT datasets in parallel jobs, then the outputs are committed to a new branch `h4-mlmaws-results-<run id>` (fetched with git and reviewed before they are copied into `results/h4/`) |
| `release.yml` | When a GitHub release is published (or by hand for a tag): checks that the tag is `v` + the workspace version, builds Q-MAWS for Windows (x64), macOS (Apple silicon and Intel) and Linux (x64), assembles each bundle (`Q-MAWS.exe` / `Q-MAWS.app` / `Q-MAWS`, `run.bat` or `run.sh`, `README.md`, `LICENSE`, `THIRD_PARTY_NOTICES`, `CITATION.cff`), attaches the bundles and `SHA256SUMS.txt` to the release, and commits the unpacked bundles to `standalone/` on the default branch |
| `ci.yml` | On every push and pull request: formatting, lints, tests in release mode, the guard self-test and the README check on Windows, macOS and Linux; dependency licenses, bans and sources with `cargo deny` on Linux |

## Relationships
Builds and tests `crates/` with the toolchain pinned in `rust-toolchain.toml`; runs `scripts/test-guard.sh` and `scripts/check-readmes.sh`; reads the dependency policy from `deny.toml`. Release bundles (from M14) include `run.bat`, `run.sh`, `README.md` and `LICENSE`.

## Notes
Basic CI exists from milestone M1. Golden tests G1, G2, G10 and G12 are added with their milestones, and `release.yml` (release bundles) in milestone M14. Workflow and job names are plain English.
