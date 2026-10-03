# .github

## Purpose
GitHub configuration for the repository.

## Contents
| Item | Description |
|---|---|
| `ABOUT.md` | This file |
| `workflows/` | Continuous integration and release workflows ([README](workflows/README.md)) |

## Relationships
Workflows build and test the code in `crates/`.

## Notes
This folder's description is `ABOUT.md`, not `README.md`: GitHub shows `.github/README.md` in place of the root `README.md` on the repository page. `scripts/check-readmes.sh` reads `ABOUT.md` for this folder.

Workflow names, job names and release notes are plain English.
