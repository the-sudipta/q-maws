# fish_mito

## Purpose
Exploratory sensitivity variants of dataset `fish_mito` (milestone M12, docs/PREREGISTRATION.md Amendment 3).

## Contents
| Item | Description |
|---|---|
| `weights_w1/` | The tree with W1 weights (votes of the split patterns) |
| `weights_w2a/` | The tree with W2a weights (best topology, weighted by its log-likelihood lead) |
| `weights_w2b/` | The tree with W2b weights (normalised likelihoods) |
| `lengths_k1/` | Run with K = 1 MAW length (--top-lengths 1), seed 1, S1 only |
| `lengths_k2/` | Run with K = 2 MAW lengths (--top-lengths 2), seed 1, S1 only |
| `lengths_k4/` | Run with K = 4 MAW lengths (--top-lengths 4), seed 1, S1 only |
| `strand_off/` | Run with strand filter off (--no-strand), seed 1, S1 only |
| `matrix_ml/` | Run with M_ml instead of M_full (--matrix ml), seed 1, S1 only |
| `model_w2emp/` | Run with W2-emp instead of W2-sym (--weighting w2-emp), seed 1, S1 only |

## Relationships
Made from the seed-1 primary run of `fish_mito` in `results/runs/`. Summarised in `../weights.tsv` (weights) and `../settings.tsv` (runs).

## Notes
The runs were made by the M12 queue (seed 1, S1 only, `--memory-limit 2`) with the build of commit ef7b226 and verified with `qmaws verify --quick` (PASS).
