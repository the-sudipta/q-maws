# benchmark

## Purpose
Runs of the extended benchmark (docs/PREREGISTRATION.md, Amendments 8 and 8a), all made on the development laptop by `scripts/benchmark_queue.sh`.

## Contents
| Item | Description |
|---|---|
| `runs.tsv` | One row per run: dataset, method, start and end (UTC), wall time in seconds, peak memory in MB, exit code |
| `queue.log` | The queue's log |
| `verify/` | `qmaws verify --quick` reports (release 1.0.0) of every reused run |
| `<dataset>/` | Per dataset: `qmaws/` (Q-MAWS run), `ml_maws/` (ML-MAWS), `iqtree_true/` (IQ-TREE on the true alignment) |
