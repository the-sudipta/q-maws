# hgt_w1

## Purpose
Exploratory: W1 weights on the HGT datasets whose seed-1 runs are finished and that are not among the four datasets of the M12 design (docs/PREREGISTRATION.md, Amendment 3), to see whether the advantage of W1 seen there also appears under horizontal gene transfer.

## Contents
| Item | Description |
|---|---|
| `w1_hgt.tsv` | nRF, nQD and MSD of the W1 variant and of the primary run (W2c) of each dataset, with the source file of every value, and the ML-MAWS values of its Table III labelled "reported" |
| `sim_hgt_250/` | W1 variant of simulated HGT 250 |
| `ecoli_shigella_hgt/` | W1 variant of E. coli/Shigella HGT (27 genomes) |
| `yersinia_hgt/` | W1 variant of Yersinia HGT (8 genomes) |

## Relationships
Made from the seed-1 runs in `results/runs/` with `qmaws variant`. W1 variants of simulated HGT 0 and 500 are in `../sim_hgt_0/` and `../sim_hgt_500/`.

## Notes
Exploratory: these results suggest a variant to test, they do not test it. The W1 tree of simulated HGT 250 is identical to that of simulated HGT 500 (the two runs and their inputs differ; the five simulated datasets share one species tree). Simulated HGT 750 and 1000 are added when their runs finish.
