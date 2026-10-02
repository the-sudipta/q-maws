# results

## Purpose
Committed records of runs and experiments: per-run audit records, trees, reports and figures, and the results of the long-branch simulation, controls, sensitivity analysis, and the final comparison tables.

## Contents
| Item | Description |
|---|---|
| `controls/` | Positive and negative control results ([README](controls/README.md)) |
| `h3/` | Long-branch simulation results for hypothesis H3 ([README](h3/README.md)) |
| `runs/` | One folder per run ([README](runs/README.md)) |
| `sensitivity/` | Sensitivity analysis results ([README](sensitivity/README.md)) |
| `summary/` | Final comparison tables ([README](summary/README.md)) |

## Relationships
Produced by the `qmaws` program (`crates/`). Compared with `baselines/`; final figures go to `figures/paper/`.

## Notes
Every number here comes from a logged run. Large intermediate files (`results/runs/*/work/`) are ignored by git.
