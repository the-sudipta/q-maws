# published

## Purpose
Numbers reported by other methods' publications and web pages, one CSV file per source; each row has method, dataset, metric, value, the status "reported" and the exact source location (table or section, or page URL with access date).

## Contents
| Item | Description |
|---|---|
| `afproject_raw/` | The answers of AFproject's results pages, saved as received on 2026-10-03 ([README](afproject_raw/README.md)) |
| `afproject_results.csv` | Every tool's RF, nRF and nQD on the nine AFproject datasets we use (735 rows), from the AFproject results pages |
| `ml_maws_table3.csv` | ML-MAWS (strand-aware IQ-TREE and NoStrand): nRF, nQD, average bootstrap and branches with at least 70% bootstrap, Table III of the ML-MAWS paper |
| `ml_maws_table4.csv` | ML-MAWS time per step and peak memory on Fish mtDNA and E. coli/Shigella, Table IV (Google Colab; not comparable across devices) |
| `ml_maws_text_values.csv` | andi, Mash and CD-MAWS values stated in the text of the ML-MAWS paper |
| `peafowl.csv` | Peafowl's nRF on the four AFproject datasets it shares with us, from the text of its paper, and its run times (Table 1) |

## Relationships
Used for the comparison tables in `results/summary/`. Sources and caveats: `../README.md`.

## Notes
Every value is labelled "reported". Values are copied from tables, text or the data behind a results table, never read off figures.
