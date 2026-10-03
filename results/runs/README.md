# runs

## Purpose
One folder per run, named `<dataset-or-folder-name>_<YYYY-MM-DD>_<HHMMSS>`, containing `run.json`, `run.log`, `audit/`, `trees/`, `figures/` and `report/`.

## Contents
| Item | Description |
|---|---|
| `ecoli_2026-10-03_080534/` | E. coli/Shigella (29 genomes), default settings, with its figures (M10) |
| `fish_mito_2026-10-03_092551/` | Fish mtDNA (25), default settings, with its figures and Open Tree of Life group bands (M10) |

## Relationships
Written by `crates/qmaws-engine`; checked by `qmaws verify`.

## Notes
Runs of the benchmark comparison follow in M11. Each run's `work/` folder holds large intermediate files; it is ignored by git and blocked by the pre-commit guard.
