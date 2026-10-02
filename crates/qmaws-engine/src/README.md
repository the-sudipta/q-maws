# src (qmaws-engine)

## Purpose
Source code of the run engine. Each module has one responsibility and its own unit tests.

## Contents
| Item | Description |
|---|---|
| `analysis.rs` | The analysis run: stages from input to quartet pattern counts, weights and the tree, checkpoints, chunked counting and weighting in Feistel order, resume, root fingerprint |
| `atomic.rs` | Atomic writes (temporary file, sync, rename) with SHA-256 hash files; validity checks; removal of stale temporary files |
| `clock.rs` | UTC date and time without dependencies, for run identifiers and log lines |
| `hash.rs` | SHA-256 helpers with hexadecimal output, one-shot and streaming |
| `lib.rs` | Crate root: module list and main re-exports |
| `matrix_pipeline.rs` | From cleaned sequences to `M_full` and `M_ml`: parallel MAW extraction within a memory limit, entropy selection, matrix building, entropy table |
| `progress.rs` | Progress events, the `ProgressSink` trait, the cost model and moving-average time estimator, duration formatting |
| `rundir.rs` | Run folder layout, run identifiers, discovery of unfinished runs |
| `runner.rs` | Starting, stopping and resuming runs; the chunk loop; final outputs and the root fingerprint |
| `state.rs` | The `run.json` record: stages, statuses, chunk plans, throughput, elapsed time; saving and verified loading |
| `store.rs` | Binary formats of MAW lists, the full matrix, count chunks and weight chunks |
| `testutil.rs` | Temporary folders for unit tests (test builds only) |
| `toy.rs` | The toy computation (SplitMix64 block sums) used to exercise the engine |

## Relationships
`runner.rs` uses all other modules. `qmaws-tui` consumes the events defined in `progress.rs`; `qmaws-cli` calls `runner.rs` and `rundir.rs`.

## Notes
Every file the engine writes goes through `atomic.rs`. Unit tests create their folders under the system temporary folder and delete them afterwards.
