# verify_completed

## Purpose
The completed pre-registered check (Amendment 8) of the 45 reused Q-MAWS runs on the nine AFproject datasets (five seeds each). On the runners, `qmaws verify --quick` stopped at the input check for these runs, because they had recorded their input files under paths of the development laptop (`D:\q-maws\data\raw\...`) and no `--input` was given; those records are in `../verify/`. See the addendum of 2026-11-21 in `docs/PREREGISTRATION.md` (branch `cloud-benchmark`).

## Contents
| Item | Description |
|---|---|
| `<run>.txt` | The report of `qmaws verify --quick --seed 1 --input <AFproject folder>` for one run folder of `results/runs/` |
| `SHA256SUMS.txt` | The SHA-256 value of every report |

## How it was made
- Date: 2026-11-21, on the development laptop (Windows 11).
- Program: the published Windows program of release 1.0.0 (`Q-MAWS-1.0.0-windows-x64.zip`, SHA-256 `1b4dc0a24127def3779ea74828558881d28297c92f6d5b93f50bb6c4d087e9ea`, as in the release's `SHA256SUMS.txt`); each report prints the program version and commit.
- Inputs: the AFproject datasets downloaded again with `qmaws download` (archives checked against the checksums published by AFproject).
- Runs: copies of the committed run folders of `results/runs/` on the branch `cloud-benchmark`; the committed folders were not changed. The paths in the reports are those of the copies.

## Result
All 45 reports end with `Verdict: PASS (every comparison agrees)`: in each run, the input files, the rebuilt matrix, a seeded sample of 20 chunks of the quartet counts and weights, and up to three bootstrap replicates agree with the run record.
