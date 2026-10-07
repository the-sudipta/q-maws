# afproject_submission

## Purpose
Prediction files in the format required by each AFproject dataset upload page, and the results AFproject returned, with access dates.

## Contents
| Item | Dataset | Upload page | Copied from | Run root |
|---|---|---|---|---|
| `fish_mito.nwk` | fish_mito | https://afproject.org/app/benchmark/genome/std/assembled/fish_mito/ | `results/runs/fish_mito_2026-10-03_092551/report/tree.nwk` | `adcc945a65f01c6504fe641171feb448b3b982ec65b690eaa299abda64cf7dac` |
| `ecoli.nwk` | ecoli | https://afproject.org/app/benchmark/genome/std/assembled/ecoli/ | `results/runs/ecoli_2026-10-03_080534/report/tree.nwk` | `36e7569de82dc87b84b126b29ca50dd0135b7b824a9cd53ddf6eda6d5dba0247` |
| `ecoli_shigella_hgt.nwk` | ecoli_shigella_hgt | https://afproject.org/app/benchmark/genome/hgt/unsimulated/ecoli_shigella/ | `results/runs/ecoli_shigella_hgt_seed1/report/tree.nwk` | `06e962b1a4b23879c7c64ecedaa1a81cf3d4f7801cf73a95454a0f68090e1650` |
| `yersinia_hgt.nwk` | yersinia_hgt | https://afproject.org/app/benchmark/genome/hgt/unsimulated/yersinia/ | `results/runs/yersinia_hgt_seed1/report/tree.nwk` | `537c28c5b2afcfa2aedf6a1f24b3c2987e635ceefc3b8241d3f31c8b194a560e` |
| `sim_hgt_0.nwk` | sim_hgt_0 | https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 0) | `results/runs/sim_hgt_0_seed1/report/tree.nwk` | `33ac4eca60c8987a23f8534f3d90952023bb4fa6a5d4bc7e2228d16b31619306` |
| `sim_hgt_250.nwk` | sim_hgt_250 | https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 250) | `results/runs/sim_hgt_250_seed1/report/tree.nwk` | `47190db6119363a82d088179a12efcd9a36c0c2ab341fa239d2c9367c7fde385` |
| `sim_hgt_500.nwk` | sim_hgt_500 | https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 500) | `results/runs/sim_hgt_500_seed1/report/tree.nwk` | `15341f49286266accf0064d0fb1ab68c45ea7f22339ceb2cd36a9453a3274129` |
| `sim_hgt_750.nwk` | sim_hgt_750 | https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 750) | `results/runs/sim_hgt_750_seed1/report/tree.nwk` | `e52b066be94028a022506fd7cd7d91e400750cb9b65f5acdd562446cd2006e71` |
| `sim_hgt_1000.nwk` | sim_hgt_1000 | https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/ (HGT level 1000) | `results/runs/sim_hgt_1000_seed1/report/tree.nwk` | `c2640414d3113b0e10d49d7df7cb6d3231806148bb5082f0f27ced249b287288` |

## Relationships
Written by the hidden command `qmaws summary --afproject` (`crates/qmaws-cli/src/summary_cmd.rs`) from the seed-1 runs in `results/runs/`. The owner uploads the files manually.

## Notes
Format (read on the upload and dataset pages, 2026-10-03): AFproject accepts a TSV of pairwise distances, a PHYLIP distance matrix, or a tree in Newick format; branch lengths are optional. The files here are Newick trees without branch lengths or support values, with leaves named by the FASTA file names of the dataset without the extension. The simulated HGT page takes one file per HGT level (0, 250, 500, 750, 1000) in one submission. Form fields: method name `Q-MAWS`; method parameters `qmaws run --dataset <id> --seed 1` (primary configuration: W2-sym, W2c with 100 resamples, wQFM-rs, strand filter on). AFproject scores with its own tools (EMBOSS ftreedist); its returned scores are recorded here with the access date once uploaded.
