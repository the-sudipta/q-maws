# paper

## Purpose
The result charts of the thesis and paper, as SVG, drawn from the committed result tables by `scripts/paper_figures.py` (`python scripts/paper_figures.py` draws them again). The figures of each run (Quartet Halo Tree, tanglegram, rectangular tree, convergence) are in that run's `figures/` folder in `results/`.

## Contents
| Item | Description |
|---|---|
| `fig_accuracy_nrf.svg` | nRF per AFproject dataset: Q-MAWS (mean of 5 seeds, 1 SD whisker), ML-MAWS and the best AFproject tool (reported) |
| `fig_accuracy_nqd.svg` | The same for nQD |
| `fig_h1_pairs_nrf.svg` | H1: nRF of Q-MAWS and ML-MAWS on the seven HGT datasets |
| `fig_h1_pairs_nqd.svg` | H1: the same for nQD |
| `h3_recovery.svg` | H3: recovery of the true quartet by voting (W1) and the conditioned likelihood (W2), made by `qmaws simulate-h3` |
| `fig_h4_reliability.svg` | H4: reliability diagram of S1, S2 and ML-MAWS UFBoot on 150 edges |
| `fig_sensitivity_nrf.svg` | M12 sensitivity: nRF of each setting on four datasets |
| `fig_seed_variability.svg` | Mean and SD of nRF and nQD over five seeds |
| `fig_h5_levels_nrf.svg` | H5: median nRF per transfer level for Q-MAWS (primary, v2), ML-MAWS, Mash, CD-MAWS and Peafowl |
| `fig_h5_levels_nqd.svg` | H5: the same for nQD |
| `fig_tools_afproject_nrf.svg` | Q-MAWS against Mash, CD-MAWS and Peafowl on the AFproject datasets (nRF) |
| `fig_tools_afproject_nqd.svg` | The same for nQD |
| `h2_scaling.svg` | H2: wall-clock time against the number of taxa, made by `qmaws h2` |

## Relationships
Source tables: `results/summary/`, `results/sensitivity/`, `results/h4/`, `results/h5/`, `results/baselines/`, `results/scaling/`, `results/h3/`.
