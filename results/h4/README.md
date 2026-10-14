# h4

## Purpose
Hypothesis H4 (docs/PREREGISTRATION.md, Amendment 3): are Q-MAWS support values (S1, seed-1 primary runs) at least as well calibrated as ML-MAWS UFBoot support on the five simulated HGT datasets (true tree known)? ECE over the internal edges of the five datasets pooled, 10 equal-width bins; S2 reported as a secondary result.

## Contents
| Item | Description |
|---|---|
| `calibration_s1.tsv` | Reliability table and ECE of Q-MAWS S1 (primary) |
| `calibration_s2.tsv` | Reliability table and ECE of Q-MAWS S2 (secondary) |
| `calibration_ufboot.tsv` | Reliability table and ECE of ML-MAWS UFBoot on its reported tree `ML_MAWS_tree.newick` (comparison of H4) |
| `calibration_ufboot_consensus.tsv` | The same for the IQ-TREE UFBoot consensus tree `ml_maws_iqtree.contree` (descriptive only) |
| `mlmaws/` | ML-MAWS outputs of the five datasets (GitHub runner, IQ-TREE 2.4.0) |
| `calibration_s1.tsv.sha256` | SHA-256 of `calibration_s1.tsv` |
| `calibration_s2.tsv.sha256` | SHA-256 of `calibration_s2.tsv` |
| `calibration_ufboot.tsv.sha256` | SHA-256 of `calibration_ufboot.tsv` |
| `calibration_ufboot_consensus.tsv.sha256` | SHA-256 of `calibration_ufboot_consensus.tsv` |

## Relationships
Q-MAWS trees: `results/runs/sim_hgt_<level>_seed1/trees/tree_s1.nwk` and `tree_s2.nwk`; true tree: `data/references/sim_hgt.nwk`; computed with `qmaws calibration --pair TREE=TRUTH ... --scale 1` (Q-MAWS) or `--scale 100` (UFBoot).

## Notes
Result (150 internal edges, 30 per dataset):

| Support | ECE |
|---|---|
| Q-MAWS S1 (primary) | 0.200240 |
| ML-MAWS UFBoot, reported tree | 0.200200 |
| Q-MAWS S2 (secondary) | 0.086067 |
| ML-MAWS UFBoot, consensus tree (descriptive) | 0.192467 |

**H4 is not supported:** the criterion ECE(Q-MAWS S1) ≤ ECE(ML-MAWS UFBoot) fails by 0.00004; the two are practically equal. S1 is too low on true splits (support 0.5–0.7 where every such split is true) and its edges below 0.5 are mostly false, so S1 ranks splits well but is not a probability. S2 (secondary, not the tested quantity) has less than half the ECE of either.

The comparison uses ML-MAWS's reported tree, the tree whose topology ML-MAWS outputs and whose nRF is evaluated, with the UFBoot values IQ-TREE maps onto it; the consensus tree is a different topology and is shown only for completeness.
