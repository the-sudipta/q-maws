# Controls

Produced by `qmaws controls --seed 1` (hidden development command). Each control is an analysis run in `runs/<control>/` with the default settings (W2-sym, W2c with 100 resamples, wQFM-rs, S2 with 100 bootstrap replicates and W2b inside). The first positive control, the hand-calculable worksheet example, is covered by golden tests G1 and G2 (`cargo test`): its sequences have 6 letters, below the 100 that an analysis run accepts. Random-tree levels: nRF of 1000 random binary trees (random stepwise addition, seed 1) to the same reference: mean and 5th percentile.

| Control | Kind | Taxa | nRF to the true or reference tree | Random trees: mean / 5th percentile | Mean S1 (range) | Mean S2 (range) | Mean halo |
|---|---|---|---|---|---|---|---|
| simulated | positive | 16 | 0.308 (8 splits) | 0.984 / 0.923 | 0.655 (0.459 to 0.971) | 0.755 (0.000 to 1.000) | 0.488 |
| fish_shuffled | negative | 25 | 0.909 (40 splits) | 0.990 / 0.955 | 0.333 (0.333 to 0.333) | 0.001 (0.000 to 0.020) | 0.333 |
| fish | reference point | 25 | 0.455 (20 splits) | 0.990 / 0.955 | 0.577 (0.392 to 0.980) | 0.279 (0.000 to 1.000) | 0.433 |

All values: `controls.tsv`.
