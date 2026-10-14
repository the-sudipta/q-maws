# summary

## Purpose
Final comparison tables between Q-MAWS results and published baselines, with every per-dataset value.

## Contents
| Item | Description |
|---|---|
| `runs.tsv` | Every finished run of the primary configuration: dataset, seed, folder, root hash, nRF, nQD, MSD, mean S1, mean S2, working time |
| `qmaws.tsv` | Q-MAWS per dataset: mean and sample SD over seeds of nRF, nQD, MSD (raw), mean S1 and working time; mean S2 of seed 1 |
| `comparison.tsv` | Q-MAWS nRF and nQD beside the reported values of ML-MAWS (Table III), Peafowl and AFproject (best value, number of tools, tools strictly lower than Q-MAWS at AFproject's precision) |
| `h1.tsv` | H1 test per metric: n, zeros, W+, W-, one-sided exact p, Holm-adjusted p, median difference, Hodges–Lehmann estimate |
| `h1_pairs.tsv` | H1 pairs: Q-MAWS value under the pairing rule, reported ML-MAWS strand-aware value, difference |

## Comparison
Q-MAWS: mean ± sample SD over the seeds listed. Other columns are reported values (ML-MAWS: Table III of its paper; Peafowl: text of its paper; AFproject: best value among the tools on its results page). Lower is better. Datasets without a reference tree have no nRF or nQD.

| Dataset | Metric | Q-MAWS | Seeds | ML-MAWS strand (reported) | ML-MAWS NoStrand (reported) | Peafowl (reported) | AFproject best (reported) |
|---|---|---|---|---|---|---|---|
| fish_mito | nRF | 0.455 ± 0.000 | 1,2,3,4,5 | 0.409 | 0.182 | 0.05 | 0.05 |
| fish_mito | nQD | 0.334 ± 0.039 | 1,2,3,4,5 | 0.185 | 0.070 | - | 0.0017 |
| ecoli | nRF | 0.638 ± 0.021 | 1,2,3,4,5 | 0.538 | 0.654 | 0.23 | 0.04 |
| ecoli | nQD | 0.414 ± 0.004 | 1,2,3,4,5 | 0.247 | 0.417 | - | 0.0021 |
| ecoli_shigella_hgt | nRF | 0.583 ± 0.000 | 1,2,3,4,5 | 0.583 | 0.750 | 0.17 | 0.08 |
| ecoli_shigella_hgt | nQD | 0.398 ± 0.006 | 1,2,3,4,5 | 0.321 | 0.406 | - | 0.0040 |
| yersinia_hgt | nRF | 1.000 ± 0.000 | 1,2,3,4,5 | 1.000 | 1.000 | 1.00 | 0.00 |
| yersinia_hgt | nQD | 0.729 ± 0.000 | 1,2,3,4,5 | 0.671 | 0.657 | - | 0.0000 |
| sim_hgt_0 | nRF | 0.533 ± 0.000 | 1,2,3,4,5 | 0.533 | 0.733 | - | 0.00 |
| sim_hgt_0 | nQD | 0.513 ± 0.031 | 1,2,3,4,5 | 0.212 | 0.374 | - | 0.0000 |
| sim_hgt_250 | nRF | 0.527 ± 0.015 | 1,2,3,4,5 | 0.700 | 0.633 | - | 0.00 |
| sim_hgt_250 | nQD | 0.494 ± 0.022 | 1,2,3,4,5 | 0.357 | 0.358 | - | 0.0000 |
| sim_hgt_500 | nRF | 0.527 ± 0.015 | 1,2,3,4,5 | 0.733 | 0.667 | - | 0.00 |
| sim_hgt_500 | nQD | 0.484 ± 0.050 | 1,2,3,4,5 | 0.403 | 0.435 | - | 0.0000 |
| sim_hgt_750 | nRF | 0.513 ± 0.018 | 1,2,3,4,5 | 0.733 | 0.600 | - | 0.03 |
| sim_hgt_750 | nQD | 0.510 ± 0.060 | 1,2,3,4,5 | 0.379 | 0.422 | - | 0.0032 |
| sim_hgt_1000 | nRF | 0.567 ± 0.000 | 1,2,3,4,5 | 0.700 | 0.600 | - | 0.13 |
| sim_hgt_1000 | nQD | 0.531 ± 0.066 | 1,2,3,4,5 | 0.314 | 0.412 | - | 0.1013 |
| influenza_a | nRF | - | 1,2,3,4,5 | - | - | - | - |
| influenza_a | nQD | - | 1,2,3,4,5 | - | - | - | - |
| coronavirus | nRF | - | 1,2,3,4,5 | - | - | - | - |
| coronavirus | nQD | - | 1,2,3,4,5 | - | - | - | - |
| mammal_mtdna | nRF | - | 1,2,3,4,5 | - | - | - | - |
| mammal_mtdna | nQD | - | 1,2,3,4,5 | - | - | - | - |
| ebolavirus | nRF | - | 1,2,3,4,5 | - | - | - | - |
| ebolavirus | nQD | - | 1,2,3,4,5 | - | - | - | - |
| rhinovirus | nRF | - | 1,2,3,4,5 | - | - | - | - |
| rhinovirus | nQD | - | 1,2,3,4,5 | - | - | - | - |

## H1
Pairing value: Mean; zero differences: Wilcoxon (OI-18). One-sided exact Wilcoxon signed-rank test of Q-MAWS minus ML-MAWS strand-aware (reported), Holm across the two metrics. Criterion: adjusted p < 0.05 for at least one metric, and median difference < 0. With 7 datasets the power of the test is limited.

| Metric | n | Zeros | W+ | p (one-sided) | p (Holm) | Median difference | Hodges–Lehmann | Criterion met for this metric |
|---|---|---|---|---|---|---|---|---|
| nRF | 7 | 1 | 3 | 0.0781 | 0.1562 | -0.1333 | -0.1031 | no |
| nQD | 7 | 0 | 28 | 1.0000 | 1.0000 | 0.1309 | 0.1354 | no |

H1 criterion met: **no**. Per-dataset pairs: `h1_pairs.tsv`.

## Relationships
Built by the hidden command `qmaws summary` (`crates/qmaws-cli/src/summary_cmd.rs`) from `results/runs/` and `baselines/published/` (milestone M11).

## Notes
Only finished runs of the primary configuration (W2-sym, W2c with 100 resamples, strand filter on, entropy-selected lengths) enter the tables. S2 is run on seed 1 only (owner decision, 2026-10-03). Losses are reported alongside wins.
