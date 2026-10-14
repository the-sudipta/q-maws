# src (qmaws-engine)

## Purpose
Source code of the run engine. Each module has one responsibility and its own unit tests.

## Contents
| Item | Description |
|---|---|
| `analysis.rs` | The analysis run: stages from input to quartet pattern counts, weights, the tree and its support (S1, halo values, S2 bootstrap stage), checkpoints, chunked counting and weighting in Feistel order, resume, root fingerprint, and the audit files (results, quartet decisions, sample worksheets, environment) |
| `atomic.rs` | Atomic writes (temporary file, sync, rename) with SHA-256 hash files; validity checks; removal of stale temporary files |
| `bootstrap.rs` | S2 bootstrap replicates (weighted counts, W2b or W2c, wQFM-rs): used by the run stage `bootstrap` and, timed, by the cost measurement of `s2-cost` |
| `clock.rs` | UTC date and time without dependencies, for run identifiers and log lines |
| `hash.rs` | SHA-256 helpers with hexadecimal output, one-shot and streaming |
| `launch.rs` | What the terminal menu and the GUI share (plan 5.2, 5.5): run settings, a new run request, the runs on this computer with their percentage done and last interface, the unfinished run with the same data and settings, and the review text, and the user configuration file (extra results folders, the resume queue with its updates) |
| `lib.rs` | Crate root: module list and main re-exports |
| `matrix_pipeline.rs` | From cleaned sequences to `M_full` and `M_ml`: parallel MAW extraction within a memory limit, entropy selection, matrix building, entropy table |
| `readme.rs` | README files of a run folder and its committed subfolders, listing their items (written at the end of a run and after verification) |
| `evaluation.rs` | The reference tree of a run and the comparison of its tree with it (OI-19): stored as `audit/reference.nwk`, `audit/reference.json` and `audit/evaluation.json` with their SHA-256; the checks that `qmaws verify` makes by recomputing nRF, nQD and MSD |
| `figures.rs` | Figures of a finished run (plan 4.7, 4.8): the options found from the run (reference tree, name table, group file), group sources, and every figure file with its formats; the comparison with a reference tree |
| `progress.rs` | Progress events (including the live quartet and provisional tree events for the GUI), the `ProgressSink` trait with its pause request, the cost model and moving-average time estimator, duration formatting |
| `provisional.rs` | The live provisional Halo Tree (plan 4.7): when an update is due (every 5% of quartets or 3 minutes), the overhead budget that doubles the intervals, the saved state for resume, frame and figure paths, `report/convergence.csv` |
| `rundir.rs` | Run folder layout, run identifiers, discovery of unfinished runs |
| `runner.rs` | Starting, stopping and resuming runs; the chunk loop; final outputs and the root fingerprint |
| `state.rs` | The `run.json` record: stages, statuses, chunk plans, throughput, elapsed time; saving and verified loading |
| `store.rs` | Binary formats of MAW lists, the full matrix, count chunks, weight chunks and quartet decisions |
| `testutil.rs` | Temporary folders for unit tests (test builds only) |
| `toy.rs` | The toy computation (SplitMix64 block sums) used to exercise the engine |
| `verify.rs` | Verification of a finished run: input check, quick check of 20 chunks per chunked stage, full recomputation, single quartet with its worksheet; rebuilds the full matrix from the inputs when the work folder is absent; writes `report/verify_<time>.txt` |
| `variant.rs` | Weight variants of a finished run (M12): the tree made again from the stored quartet results with W1, W2a, W2b or W2c, with S1, halo values and the comparison with the run's reference tree; the run folder is not changed |

## Relationships
`runner.rs` uses all other modules. `qmaws-tui` consumes the events defined in `progress.rs`; `qmaws-cli` calls `runner.rs` and `rundir.rs`.

## Notes
Every file the engine writes goes through `atomic.rs`. Unit tests create their folders under the system temporary folder and delete them afterwards.
