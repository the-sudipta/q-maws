# Controls

Produced by `qmaws controls --seed 1` (hidden development command). Each control is an analysis run in `runs/<control>/` with the default settings (W2-sym, W2c with 100 resamples, wQFM-rs). The first positive control, the hand-calculable worksheet example, is covered by golden tests G1 and G2 (`cargo test`): its sequences have 6 letters, below the 100 that an analysis run accepts. Random-tree levels: nRF of 1000 random binary trees (random stepwise addition, seed 1) to the same reference: mean and 5th percentile.

| Control | Kind | Taxa | nRF to the true or reference tree | Random trees: mean / 5th percentile | Mean S1 (range) | Mean halo |
|---|---|---|---|---|---|---|
| simulated | positive | 16 | 0.154 (4 splits) | 0.984 / 0.923 | 0.936 (0.803 to 1.000) | 0.894 |
| fish_shuffled | negative | 25 | 1.000 (44 splits) | 0.990 / 0.955 | 0.649 (0.523 to 0.866) | 0.513 |
| fish | reference point | 25 | 0.500 (22 splits) | 0.990 / 0.955 | 0.776 (0.619 to 0.988) | 0.628 |

All values: `controls.tsv`.
