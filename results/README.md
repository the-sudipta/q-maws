# results

## Purpose
Committed records of runs and experiments: per-run audit records, trees, reports and figures, and the results of the long-branch simulation, controls, sensitivity analysis, and the final comparison tables.

## Contents
| Item | Description |
|---|---|
| `controls/` | Positive and negative control results ([README](controls/README.md)) |
| `h3/` | Long-branch simulation results for hypothesis H3 ([README](h3/README.md)) |
| `h4/` | Hypothesis H4: calibration of Q-MAWS support against ML-MAWS UFBoot on the simulated HGT datasets ([README](h4/README.md)) |
| `runs/` | One folder per run ([README](runs/README.md)) |
| `sensitivity/` | Sensitivity analysis results ([README](sensitivity/README.md)) |
| `h5/` | H5: Q-MAWS v2 against ML-MAWS on 20 new simulated datasets ([README](h5/README.md)) |
| `baselines/` | Mash, CD-MAWS and Peafowl on the AFproject and H5 datasets ([README](baselines/README.md)) |
| `scaling/` | M13: scaling and H2 ([README](scaling/README.md)) |
| `summary/` | Final comparison tables ([README](summary/README.md)) |

## Relationships
Produced by the `qmaws` program (`crates/`). Compared with `baselines/`; final figures go to `figures/paper/`.

## Notes
Every number here comes from a logged run. Large intermediate files (`results/runs/*/work/`) are ignored by git.
