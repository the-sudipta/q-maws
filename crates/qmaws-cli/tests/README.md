# tests (qmaws-cli)

## Purpose
Integration tests that run the real `qmaws` binary, as a user or script would.

## Contents
| Item | Description |
|---|---|
| `data_commands.rs` | The data commands without network: `inspect` (summary, findings with choices, reference name comparison, errors for too few taxa and missing folders), `datasets` (all 14 datasets listed), unknown dataset ids rejected; `run`: options checked before anything is written (`--dataset all` with `--output`, unknown datasets, `--cores 0`, a negative memory limit), and input warnings answered with `--skip` (stopped without an answer; with it, the taxon is left out, recorded in `audit/inputs.json` and `run.json`, and the run verifies) |
| `interfaces.rs` | Golden test G10 and interface switching on the 16-taxon simulated control input (`results/controls/inputs/simulated/`): a terminal run and a run through the GUI's controller give the same root; a run started in the terminal, hard-killed in the weighting stage and resumed in the GUI, and a run started in the GUI, stopped there and resumed in the terminal, give that root too, and `run.json` records the interface used last. Runs with 10 W2c resamples and 3 S2 replicates; the ignored `g10_default_settings` uses the default settings |
| `kill_resume.rs` | A toy run hard-killed 10 times (M1 acceptance) and an analysis run of synthetic genomes hard-killed 6 times, each resumed, finish with the same root fingerprint as an uninterrupted run with another chunk size; progress output carries remaining-time estimates that are updated during the run; golden test G11 (ignored by default; needs the downloaded Fish mtDNA): a Fish mtDNA run hard-killed 10 times at random moments and resumed gives the root of an uninterrupted run; `qmaws verify` passes on a finished run (quick check, single quartet), rejects a quartet of two names, and fails the input check after an input file is changed |
| `reference.rs` | A run with the user's reference tree (`--reference`, plan 5.4) on the 16-taxon simulated control: a tree whose leaves are not the taxa is refused before the run; the true tree is stored byte for byte in `audit/reference.nwk`, compared in `audit/evaluation.json` and the tanglegram, and recomputed by `qmaws verify`; a changed number in the stored comparison fails verification (OI-19) |

## Relationships
Cargo builds the `qmaws` binary from `../src/` before these tests and passes its path in `CARGO_BIN_EXE_qmaws`.

## Notes
The size of the toy run depends on the build profile (240 blocks in debug builds, 3,000 in release builds) so that each kill lands while the run is working. Runs are written to the system temporary folder and deleted afterwards. The kill times come from a fixed seed, printed when the test runs with `--nocapture`.
