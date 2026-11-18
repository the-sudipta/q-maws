# cloud

## Purpose
The extended benchmark (docs/PREREGISTRATION.md, Amendments 8 to 10) on GitHub-hosted runners: making the inputs again, installing every method at its pinned version, scheduling the runs in waves of parallel jobs, continuing unfinished runs, and committing every finished run to the branch `benchmark-cloud-state`. These scripts only schedule and record; the methods, commands, settings, datasets and analyses are the ones fixed in Amendments 8 to 9a, run through the same scripts as on the laptop (`bench_tools.sh`, `bench_refs.sh`, the `bench_*.py` analyses).

## Contents
| Item | Description |
|---|---|
| `config/` | The method table and the run request ([README](config/README.md)) |
| `bench_cloud.py` | Command line used by the workflows: `plan`, `run`, `merge`, `inputs`, `status` |
| `catalog.py` | What runs: the datasets, the methods and the list of tasks (one task = one run folder under `results/benchmark/`) |
| `state.py` | Which tasks are finished (from the run folders' own markers) and the run tables |
| `planner.py` | Plans one wave: unfinished tasks whose prerequisites and smoke test are finished, packed into at most 20 jobs |
| `executor.py` | Runs one job's tasks before the job's deadline and writes the results, run rows and host rows |
| `runners.py` | One function per kind of run (the Linux methods, Q-MAWS, ML-MAWS, the IQ-TREE reference, the references of Amendment 9, checks of reused runs, matrices) |
| `hostinfo.py` | The computer of a job: processor, cores, memory, disk, image, a 10-second processor calibration |
| `merge.py` | Adds a wave's results to the state, picks the tuned settings when tuning is complete, seals finished runs |
| `install_tools.sh` | Installs every program at its pinned version (Ubuntu packages, release downloads checked by SHA-256, builds from pinned commits) |
| `inputs_windows.sh` | Makes the simulated datasets again with IQ-TREE 2.4.0 and Q-MAWS 1.0.0 for Windows and checks them against the committed SHA-256 values |
| `inputs_linux.sh` | Downloads the AFproject and NCBI data, takes the H5 data from `h5-data`, and packs every input in one archive per group |
| `analyse.sh` | Runs the committed analysis scripts on the finished runs and writes their tables into the state |
| `logged.sh` | Runs a workflow step and, if it fails, publishes the end of its output as an error annotation (readable without signing in) |
| `STATE_README.md` | The README of the branch `benchmark-cloud-state` |

## Relationships
Run by `.github/workflows/benchmark_cloud.yml` (inputs, waves, analysis) and `benchmark_wave.yml` (one wave). Calls `scripts/bench_tools.sh`, `scripts/bench_refs.sh`, `scripts/simulate_benchmark.sh`, `scripts/simulate_scaling.sh`, `scripts/bench_tune_choose.py`, `scripts/bench_hash.py` and the analysis scripts. Writes the layout of `results/benchmark/`.

## Notes
- **Start:** change `config/run.json` on the branch `cloud-benchmark` and push. `smoke` runs every method once on `bm_tune_r1`; `full` runs everything (up to 12 waves of at most 6 hours each per push; push again to continue).
- **Finished, failed, interrupted:** a run is finished when its folder has its marker (`done.txt`, `failed.txt`, or `audit/root.txt` for Q-MAWS). A run stopped only because its job ran out of time is not a result: it is continued (Q-MAWS, ML-MAWS's IQ-TREE step, the IQ-TREE reference) or started again in a later job. A run that fails three times for another reason is a result: failed.
- **Time limit:** 5 h 30 min per run of the methods run through `bench_tools.sh`; none for Q-MAWS, ML-MAWS and the IQ-TREE reference (Amendments 8a and 10).
- **Processors:** several tools compile with `-march=native`, so the built tools are cached per processor model.
- Only the Python standard library is used.
