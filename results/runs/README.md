# runs

## Purpose
One folder per run, named `<dataset-or-folder-name>_<YYYY-MM-DD>_<HHMMSS>`, containing `run.json`, `run.log`, `audit/`, `trees/`, `figures/` and `report/`.

## Contents
| Item | Description |
|---|---|

## Relationships
Written by `crates/qmaws-engine`; checked by `qmaws verify`.

## Notes
Empty in milestone M0. Each run's `work/` folder holds large intermediate files; it is ignored by git and blocked by the pre-commit guard.
