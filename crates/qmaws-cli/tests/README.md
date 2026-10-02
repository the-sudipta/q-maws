# tests (qmaws-cli)

## Purpose
Integration tests that run the real `qmaws` binary, as a user or script would.

## Contents
| Item | Description |
|---|---|
| `data_commands.rs` | The data commands without network: `inspect` (summary, findings with choices, reference name comparison, errors for too few taxa and missing folders), `datasets` (all 14 datasets listed), unknown dataset ids rejected |
| `kill_resume.rs` | Milestone M1 acceptance: a toy run hard-killed 10 times at seeded random moments and resumed finishes with the same root fingerprint as an uninterrupted run; progress output carries remaining-time estimates that are updated during the run |

## Relationships
Cargo builds the `qmaws` binary from `../src/` before these tests and passes its path in `CARGO_BIN_EXE_qmaws`.

## Notes
The size of the toy run depends on the build profile (240 blocks in debug builds, 3,000 in release builds) so that each kill lands while the run is working. Runs are written to the system temporary folder and deleted afterwards. The kill times come from a fixed seed, printed when the test runs with `--nocapture`.
