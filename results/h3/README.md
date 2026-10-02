# h3

## Purpose
Results of the built-in four-taxon long-branch simulation that tests hypothesis H3: recovery rates of the true quartet for informative-pattern voting (W1) and for the conditioned quartet likelihood (W2), with every replicate's seed.

## Contents
| Item | Description |
|---|---|
| `report.md` | Short report: design, results table, the pre-registered criterion, interpretation, limitations |
| `recovery.csv` | One row per setting (t_long, N): mean columns left after removing 0000, recovery of W1, W2 and W2c, tied replicates |
| `replicates.csv` | One row per replicate: seed, columns, the 15 pattern counts (0001 to 1111), W1 scores, W2 log-likelihoods, recoveries, W2c weights |
| `recovery.svg` | Figure: recovery rate against the number of simulated characters, one panel per long-branch length |
| `evaluation.txt` | The pre-registered H3 criterion applied to `recovery.csv`, with the verdict, seed and run time |

## Relationships
Written by `qmaws simulate-h3 --output results/h3` (simulator `crates/qmaws-core/src/sim.rs`, weighting `crates/qmaws-core/src/weight.rs`, figure `crates/qmaws-viz/src/chart.rs`) in milestone M6. Hypothesis and criterion: `docs/PREREGISTRATION.md`.

## Notes
Global seed 1, 200 replicates per setting, 100 W2c resamples. Rerunning the command gives byte-identical CSV and SVG files; only the run time in `evaluation.txt` changes.
