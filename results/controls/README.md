# controls

## Purpose
Positive controls (hand-calculable example, simulated tree-like data) and negative controls (each sequence shuffled independently, preserving its letter composition).

## Contents
| Item | Description |
|---|---|
| `summary.md` | Table of the controls: nRF to the true or reference tree, the nRF level of random trees, S1 and halo values |
| `controls.tsv` | The same values with all digits and each run's root fingerprint |
| `inputs/simulated/` | 16 sequences of 20,000 bases evolved along a random tree (Jukes–Cantor, seed 1) and the true tree `true_tree.nwk` |
| `inputs/fish_shuffled/` | The 25 Fish mtDNA sequences, each shuffled on its own (seed 1) |
| `runs/simulated/`, `runs/fish_shuffled/`, `runs/fish/` | The analysis runs: `run.json`, `run.log`, `audit/`, `trees/`, `report/` (their `work/` folders are not committed) |

## Relationships
Produced by the hidden command `qmaws controls` (milestone M8) from the downloaded Fish mtDNA (`data/raw/fish_mito`) and its reference tree (`data/references/fish_mito.nwk`). The worksheet example, the first positive control, is covered by golden tests G1 and G2 instead (its sequences are too short for an analysis run).

## Notes
Recreate with `qmaws controls` after removing `inputs/` and `runs/`; the same seed gives the same inputs and root fingerprints. Each run can be checked with `qmaws verify --output results/controls/runs/<control>`.
