# h4

## Purpose
Hypothesis H4 (docs/PREREGISTRATION.md, Amendment 3): are Q-MAWS support values (S1, seed-1 primary runs) at least as well calibrated as ML-MAWS UFBoot support on the five simulated HGT datasets (true tree known)? ECE over the internal edges of the five datasets pooled, 10 equal-width bins; S2 reported as a secondary result.

## Contents
| Item | Description |
|---|---|
| `mlmaws/` | ML-MAWS outputs of the five datasets (GitHub runner, IQ-TREE 2.4.0) |

## Relationships
Q-MAWS trees: `results/runs/sim_hgt_<level>_seed1/trees/tree_s1.nwk` (and `tree_s2.nwk`); true tree: `data/references/sim_hgt.nwk`; computed with `qmaws calibration`.

## Notes
The reliability tables and the verdict are added when the seed-1 run of simulated HGT 1000 has finished and been verified.
