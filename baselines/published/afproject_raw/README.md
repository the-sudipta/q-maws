# afproject_raw

## Purpose
The data behind AFproject's results tables, saved as received on 2026-10-03, so that every row of `../afproject_results.csv` can be checked.

## Contents
| Item | Description |
|---|---|
| `table1_2.json` | E. coli/Shigella (29), `ajax/table1/?dataset_id=2` |
| `table1_3.json` | Fish mtDNA (25), `ajax/table1/?dataset_id=3` |
| `table1_6.json` | E. coli/Shigella HGT (27), `ajax/table1/?dataset_id=6` |
| `table1_7.json` | Yersinia HGT (8), `ajax/table1/?dataset_id=7` |
| `table1_8.json` | Simulated HGT, averages over the five levels, `ajax/table1/?dataset_id=8` (not used in the comparison) |
| `table2_15.json` | Simulated HGT level 0, `ajax/table2/?dataset_id=8&subdataset_id=15` |
| `table2_16.json` | Simulated HGT level 250, `ajax/table2/?dataset_id=8&subdataset_id=16` |
| `table2_17.json` | Simulated HGT level 500, `ajax/table2/?dataset_id=8&subdataset_id=17` |
| `table2_18.json` | Simulated HGT level 750, `ajax/table2/?dataset_id=8&subdataset_id=18` |
| `table2_19.json` | Simulated HGT level 1000, `ajax/table2/?dataset_id=8&subdataset_id=19` |

## Relationships
Requests under `https://afproject.org/app/benchmark/genome/results/`, made by the results pages listed in `data/references/SOURCE.md`; turned into `../afproject_results.csv` with the SHA-256 of each file in its source column.

## Notes
The results pages fill their tables from these requests; the request names were read from the page script `results.js`. nRF is given with two decimals and nQD with four, as on the pages.
