# sim_hgt_0

## Purpose
Outputs of ML-MAWS on simulated HGT 0 for hypothesis H4 (docs/PREREGISTRATION.md, Amendment 3), made on a GitHub Linux runner by `.github/workflows/mlmaws_h4.yml` (run 37358527191) and copied unchanged from branch `h4-mlmaws-results-37358527191`.

## Contents
| Item | Description |
|---|---|
| `ML_MAWS_tree.newick` | The ML-MAWS tree (IQ-TREE `.treefile`) with UFBoot support (0 to 100) as internal labels |
| `ml_maws_iqtree.contree` | IQ-TREE consensus tree of the UFBoot replicates |
| `ml_maws_iqtree.iqtree` | IQ-TREE report (model chosen, likelihoods) |
| `ml_maws_iqtree.log` | IQ-TREE log, with the seed it chose |
| `complexity_report.json` | ML-MAWS time and memory per step on the runner (not used for H2) |
| `entropy_results.tsv` | ML-MAWS entropy of each MAW length |
| `stdout.txt` | ML-MAWS standard output |
| `stderr.txt` | ML-MAWS messages |
| `provenance.txt` | Dataset, versions, commands, IQ-TREE archive SHA-256, seed, runner CPU and date |

## Relationships
Compared with the true tree `data/references/sim_hgt.nwk`; calibration in `../../`.

## Notes
ML-MAWS passes no seed to IQ-TREE, so a new run can give another tree; the seed of this run is in `provenance.txt`.
