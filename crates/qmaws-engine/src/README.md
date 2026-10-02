# src (qmaws-engine)

## Purpose
Source code of the run engine. Each module has one responsibility and its own unit tests.

## Contents
| Item | Description |
|---|---|
| `atomic.rs` | Atomic writes (temporary file, sync, rename) with SHA-256 hash files; validity checks; removal of stale temporary files |
| `clock.rs` | UTC date and time without dependencies, for run identifiers and log lines |
| `hash.rs` | SHA-256 helpers with hexadecimal output, one-shot and streaming |
| `lib.rs` | Crate root: module list and main re-exports |
| `rundir.rs` | Run folder layout, run identifiers, discovery of unfinished runs |
| `state.rs` | The `run.json` record: stages, statuses, chunk plans, throughput, elapsed time; saving and verified loading |
| `testutil.rs` | Temporary folders for unit tests (test builds only) |

## Relationships
`runner.rs` uses all other modules. `qmaws-tui` consumes the events defined in `progress.rs`; `qmaws-cli` calls `runner.rs` and `rundir.rs`.

## Notes
Every file the engine writes goes through `atomic.rs`. Unit tests create their folders under the system temporary folder and delete them afterwards.
