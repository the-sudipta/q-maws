# H5: Q-MAWS v2 against ML-MAWS on new simulated data

Confirmatory test of hypothesis H5 (`docs/PREREGISTRATION.md`, Amendment 4) on the 20 simulated HGT datasets made after the amendment (30 taxa; transfer levels h = 0, 0.1, 0.2, 0.4; 5 replicates each; data on the branch `h5-data`, manifest `data/manifests/simulated/hgt.tsv`). All runs on the development laptop by the H5 queue (Q-MAWS build 826812e; ML-MAWS commit `0c38db1`, `--strand --iqtree`, IQ-TREE 2.4.0), finished 2026-10-14 19:11 UTC.

| Path | Content |
|---|---|
| `<dataset>/qmaws/` | Q-MAWS run, primary configuration, seed 1, S1, with the species tree as reference (the large `work/` folder is not committed) |
| `<dataset>/v2/` | Q-MAWS v2: the W1 tree of the same run (`qmaws variant --weights w1`) |
| `<dataset>/ml_maws/` | ML-MAWS tree with UFBoot support, IQ-TREE log and report |
| `h5_values.tsv` | nRF and nQD of v2, the primary configuration and ML-MAWS against the species tree, every dataset |
| `h5_test.tsv` | The pre-registered test (v2 minus ML-MAWS) and the descriptive comparison of the primary configuration, made by `qmaws h5` |

## Result

| Comparison | Metric | Pairs (zeros) | Median difference | p (one-sided) | p (Holm) | Criterion |
|---|---|---|---|---|---|---|
| **v2 minus ML-MAWS (H5, confirmatory)** | nRF | 20 (1) | −0.019 | 0.535 | 0.535 | not met |
| | nQD | 20 (0) | −0.017 | 0.184 | 0.368 | not met |
| Primary minus ML-MAWS (pre-registered, descriptive) | nRF | 20 (1) | −0.130 | 0.0016 | 0.0031 | — |
| | nQD | 20 (0) | −0.046 | 0.0053 | 0.0053 | — |

**H5 is not supported:** Q-MAWS v2 (W1 weights) is not more accurate than ML-MAWS on the new data. The exploratory M12 finding that W1 beats the primary configuration does not hold on data that no part of the method has seen: the primary configuration (W2c) has a lower nRF than v2 on 15 of the 20 datasets (3 equal, 2 higher). As pre-registered, W1 is reported as an exploratory observation only.

The primary configuration, compared with ML-MAWS in the same way as a pre-registered descriptive result, has a lower nRF on 15 datasets (4 higher, 1 equal) and a lower nQD on 14 (6 higher).

Median nRF per transfer level (5 datasets each):

| Level | Q-MAWS primary | Q-MAWS v2 | ML-MAWS |
|---|---|---|---|
| h = 0 | 0.148 | 0.259 | 0.333 |
| h = 0.1 | 0.185 | 0.333 | 0.222 |
| h = 0.2 | 0.185 | 0.259 | 0.259 |
| h = 0.4 | 0.148 | 0.333 | 0.148 |

The peer-reviewed tools on the same 20 datasets are in `results/baselines/` (Amendment 5): Peafowl median nRF 0.000 at every level, Mash 0.000 to 0.074, CD-MAWS 0.222 to 0.296.

## Verification

The 20 Q-MAWS runs are verified with `qmaws verify --quick` after M13, whose timing runs started on the same laptop when H5 finished (verification during M13 would change its measured times).
