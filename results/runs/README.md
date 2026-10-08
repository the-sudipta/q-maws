# runs

## Purpose
One folder per run, named `<dataset-or-folder-name>_<YYYY-MM-DD>_<HHMMSS>`, containing `run.json`, `run.log`, `audit/`, `trees/`, `figures/` and `report/`.

## Contents
| Item | Description |
|---|---|
| `ecoli_2026-10-03_080534/` | E. coli/Shigella (29 genomes), default settings, with its figures (M10) |
| `ecoli_shigella_hgt_seed1/` | E. coli/Shigella HGT (27), seed 1, primary configuration, S2 100 replicates (M11) |
| `fish_mito_2026-10-03_092551/` | Fish mtDNA (25), default settings, with its figures and Open Tree of Life group bands (M10) |
| `sim_hgt_0_seed1/` | Simulated HGT = 0 (33), seed 1, primary configuration, S2 100 replicates (M11) |
| `sim_hgt_250_seed1/` | Simulated HGT = 250 (33), seed 1, primary configuration, S2 100 replicates (M11) |
| `sim_hgt_500_seed1/` | Simulated HGT = 500 (33), seed 1, primary configuration, S2 100 replicates (M11) |
| `sim_hgt_750_seed1/` | Simulated HGT = 750 (33), seed 1, primary configuration, S2 100 replicates (M11) |
| `sim_hgt_1000_seed1/` | Simulated HGT = 1000 (33), seed 1, primary configuration, S2 100 replicates (M11) |
| `yersinia_hgt_seed2/` | Yersinia HGT, seed 2, primary configuration, S1 only (M11) |
| `ecoli_shigella_hgt_seed2/` | E. coli/Shigella HGT, seed 2, primary configuration, S1 only (M11) |
| `sim_hgt_0_seed2/` | Simulated HGT = 0 (33), seed 2, primary configuration, S1 only (M11) |
| `sim_hgt_250_seed2/` | Simulated HGT = 250 (33), seed 2, primary configuration, S1 only (M11) |
| `sim_hgt_500_seed2/` | Simulated HGT = 500 (33), seed 2, primary configuration, S1 only (M11) |
| `sim_hgt_750_seed2/` | Simulated HGT = 750 (33), seed 2, primary configuration, S1 only (M11) |
| `sim_hgt_1000_seed2/` | Simulated HGT = 1000 (33), seed 2, primary configuration, S1 only (M11) |
| `yersinia_hgt_seed3/` | Yersinia HGT, seed 3, primary configuration, S1 only (M11) |
| `ecoli_shigella_hgt_seed3/` | E. coli/Shigella HGT, seed 3, primary configuration, S1 only (M11) |
| `sim_hgt_0_seed3/` | Simulated HGT = 0 (33), seed 3, primary configuration, S1 only (M11) |
| `sim_hgt_250_seed3/` | Simulated HGT = 250 (33), seed 3, primary configuration, S1 only (M11) |
| `sim_hgt_500_seed3/` | Simulated HGT = 500 (33), seed 3, primary configuration, S1 only (M11) |
| `sim_hgt_750_seed3/` | Simulated HGT = 750 (33), seed 3, primary configuration, S1 only (M11) |
| `sim_hgt_1000_seed3/` | Simulated HGT = 1000 (33), seed 3, primary configuration, S1 only (M11) |
| `yersinia_hgt_seed4/` | Yersinia HGT, seed 4, primary configuration, S1 only (M11) |
| `ecoli_shigella_hgt_seed4/` | E. coli/Shigella HGT, seed 4, primary configuration, S1 only (M11) |
| `sim_hgt_0_seed4/` | Simulated HGT = 0 (33), seed 4, primary configuration, S1 only (M11) |
| `sim_hgt_250_seed4/` | Simulated HGT = 250 (33), seed 4, primary configuration, S1 only (M11) |
| `sim_hgt_500_seed4/` | Simulated HGT = 500 (33), seed 4, primary configuration, S1 only (M11) |
| `sim_hgt_750_seed4/` | Simulated HGT = 750 (33), seed 4, primary configuration, S1 only (M11) |
| `yersinia_hgt_seed1/` | Yersinia HGT (8), seed 1, primary configuration, S2 100 replicates (M11) |

## Relationships
Written by `crates/qmaws-engine`; checked by `qmaws verify`.

## Notes
M11 benchmark runs are named `<dataset>_seed<n>`. Each run's `work/` folder holds large intermediate files; it is ignored by git and blocked by the pre-commit guard.
