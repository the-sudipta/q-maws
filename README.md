# benchmark-cloud-state

## Purpose
The runs of the extended benchmark made on GitHub-hosted runners (docs/PREREGISTRATION.md, Amendment 10 on the branch `cloud-benchmark`). The workflow `benchmark_cloud.yml` commits every finished run here after each wave of jobs, so no finished run can be lost and an interrupted benchmark continues where it stopped.

## Contents
| Item | Description |
|---|---|
| `results/benchmark/` | Run folders in the layout of Amendments 8 to 9a (`<dataset>/<method>/`, `tuning/`, `speed/`, `smoke/`, `verify/`, `w2c1000/`, `dependence/`), each sealed with SHA-256 (`files.sha256`, `benchmark.sha256`) |
| `results/benchmark/runs.tsv` | One row per finished run: dataset, method, start and end (UTC), wall time in seconds, peak memory in MB, exit code |
| `results/benchmark/hosts.tsv` | One row per attempt: the computer it ran on (processor, cores, memory, disk, runner image, a 10-second processor calibration) and how it ended |
| `results/benchmark/versions/` | The version of every program, per processor model of the runners |
| `results/benchmark/inputs/` | The check of the simulated data made again against the committed SHA-256 values |
| `results/benchmark/summary/` | The analysis (written when every run is finished) |
| `cloud/` | Bookkeeping of the workflow only: input sizes, the workflow run holding the inputs, unfinished runs to continue, retry counts, a log of the waves |

## Relationships
Made by `.github/workflows/benchmark_cloud.yml` and `scripts/cloud/` on the branch `cloud-benchmark`. The finished results are copied into `results/benchmark/` of the main line when the benchmark is complete.

## Notes
Only the workflow commits here. Runs are never edited after they are committed; `python scripts/bench_hash.py verify` checks every seal.
