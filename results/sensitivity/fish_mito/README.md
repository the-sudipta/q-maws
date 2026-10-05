# fish_mito

## Purpose
Exploratory sensitivity variants of dataset `fish_mito` (milestone M12, docs/PREREGISTRATION.md Amendment 3).

## Contents
| Item | Description |
|---|---|
| `weights_w1/` | The tree with W1 weights (votes of the split patterns) |
| `weights_w2a/` | The tree with W2a weights (best topology, weighted by its log-likelihood lead) |
| `weights_w2b/` | The tree with W2b weights (normalised likelihoods) |

## Relationships
Made from the seed-1 primary run of `fish_mito` in `results/runs/`. Summarised in `../weights.tsv`.

## Notes
The variants that need new runs (number of lengths, strand filter, `M_ml`, W2-emp) are added when those runs are made.
