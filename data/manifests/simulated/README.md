# simulated

## Purpose
Recipes and records of the simulated datasets of hypothesis H2 (scaling; plan 6.5, docs/PREREGISTRATION.md Amendment 3). The sequences themselves are not committed (`data/raw/simulated/`); they are made again, byte for byte, from this manifest.

## Contents
| Item | Description |
|---|---|
| `scaling.tsv` | One row per dataset: id, taxa, replicate, seed, sequence length, the AliSim command, SHA-256 of the sequences (after removing trailing spaces from the headers) and of the true tree |
| `trees/` | The true tree of every dataset as written by AliSim |

## Relationships
Written by `scripts/simulate_scaling.sh` with AliSim of IQ-TREE 2.4.0 (`docs/EXTERNAL_TOOLS.md`). The datasets are analysed by Q-MAWS and ML-MAWS on the same computer for H2 (milestone M13).

## Notes
Design: random Yule–Harding trees with m = 25, 50 and 100 taxa (3 replicates each) and m = 200 (1 replicate); Jukes–Cantor model; 1,000,000 letters; AliSim's default branch lengths (exponential, mean 0.1, between 0.001 and 0.999); seed = 1000 × m + replicate. AliSim's defaults are used so that no setting was chosen for the comparison. Running the script twice on 2026-10-05 gave identical files.
