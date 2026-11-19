# benchmark

## Purpose
Runs of the extended benchmark (docs/PREREGISTRATION.md, Amendments 8 to 10). The runs made on the development laptop were lost before any was committed and are not used (Amendment 10); every run is made again on GitHub-hosted runners by `.github/workflows/benchmark_cloud.yml`, which commits them to the branch `benchmark-cloud-state`. They are copied here, unchanged and with their seals, when the benchmark is complete.

## Contents
| Item | Description |
|---|---|
| `.gitignore` | Lock, marker and temporary files of the laptop queues, never committed |
| `runs.tsv` | One row per run: dataset, method, start and end (UTC), wall time in seconds, peak memory in MB, exit code |
| `hosts.tsv` | One row per attempt of a run: the computer (processor, cores, memory, free disk, runner image, a 10-second processor calibration) and how the attempt ended |
| `versions/` | The version of every program, one table per processor model of the runners |
| `inputs/` | The check of the simulated data made again against the committed SHA-256 values |
| `verify/` | `qmaws verify --quick` reports (release 1.0.0) of every reused run |
| `smoke/` | The installation check of every method on `bm_tune_r1` |
| `tuning/` | Every tuning value on the six tuning datasets, and `chosen.tsv` (the tuned value of each method) |
| `<dataset>/` | Per evaluation dataset: one folder per method (`qmaws/`, `ml_maws/`, `iqtree_true/`, the Linux methods, `<method>_tuned/`, and the references `fasttree_genes/`, `caster_site/`, `treeqmc/`, `wtreeqmc/`) |
| `speed/` | Every method alone on the 9 speed datasets |
| `w2c1000/` | Q-MAWS with 1,000 W2c resamples on the five H5 datasets with h = 0.2 |
| `dependence/` | The MAW matrices of the dependence analysis |
| `summary/` | The analysis: accuracy table, H6, medians, ranks, effect sizes, calibration, halo, dependence, speed, W2c with 1,000 resamples |
| `benchmark.sha256` | The SHA-256 of every run's seal (`files.sha256`) and its root fingerprint |

## Relationships
Made by `scripts/cloud/` with `scripts/bench_tools.sh` and `scripts/bench_refs.sh`; analysed by `scripts/bench_collect.py`, `bench_stats.py`, `bench_extra.py`, `bench_dependence.py`, `bench_speed.py` and `bench_w2c1000.py`; checked by `scripts/bench_hash.py verify`.

## Notes
A run folder is finished when it has `done.txt` (a result tree), `failed.txt` (the reason, for example "not run (time)") or, for Q-MAWS, `audit/root.txt`. Files here keep their bytes on every system (`.gitattributes`), so the seals stay valid after a checkout on Windows.
