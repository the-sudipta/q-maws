# sensitivity

## Purpose
Sensitivity of the results to settings (milestone M12; plan Part 8, item 6; docs/PREREGISTRATION.md, Amendment 3): one factor at a time from the primary configuration (`M_full`, W2-sym, W2c, wQFM-rs, S1), on Fish mtDNA, E. coli–Shigella (`ecoli`, 29 genomes), simulated HGT 0 and simulated HGT 500. Factors: weights W1, W2a, W2b; number of MAW lengths K = 1, 2, 4; strand filter off; `M_ml`; W2-emp.

## Contents
| Item | Description |
|---|---|
| `weights.tsv` | nRF, nQD and MSD against the reference tree of the primary run (W2c) and of the W1, W2a and W2b variants of each dataset, with the file each value comes from; the ML-MAWS values reported in its Table III beside them, labelled "reported" |
| `fish_mito/` | Variants of Fish mtDNA |
| `ecoli/` | Variants of E. coli–Shigella (29 genomes) |
| `sim_hgt_0/` | Variants of simulated HGT 0 |
| `sim_hgt_500/` | Variants of simulated HGT 500 |
| `hgt_w1/` | Exploratory: W1 on the other HGT datasets with finished seed-1 runs ([README](hgt_w1/README.md)) |

## Relationships
The weight variants are made from the seed-1 primary runs in `results/runs/` with `qmaws variant` (their stored quartet results are reused). The variants that change the matrix are runs of `qmaws run` with `--top-lengths`, `--no-strand`, `--matrix ml` or `--weighting w2-emp`, added when they are made.

## Notes
Exploratory (Amendment 3): no result here is a pre-registered test. A variant that looks better than the primary configuration is written down as an amendment and tested on newly simulated datasets before any claim is made. ML-MAWS values are copied from its paper and labelled "reported"; they come from another computer and, for nQD, from one ML-MAWS run whose IQ-TREE seed is not known.
