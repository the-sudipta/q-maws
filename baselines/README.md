# baselines

## Purpose
Published results of other methods (ML-MAWS, AFproject tools, Peafowl), each with its exact source, and the files prepared for independent scoring on AFproject.

## Contents
| Item | Description |
|---|---|
| `afproject_submission/` | Prediction files for AFproject and the returned scores ([README](afproject_submission/README.md)) |
| `published/` | Published numbers as CSV files with sources ([README](published/README.md)) |

## Relationships
Compared against the results in `results/`; comparison tables are written to `results/summary/`.

## Notes
Other methods are not re-run to recreate their published results. Values are copied only from tables or text, never read off figures, and are labelled "reported". Discrepancies found in published sources are recorded here neutrally. Filled in milestone M11.

Caveats and discrepancies (recorded 2026-10-03):

- The ML-MAWS paper (arXiv:2606.25584v2) reports Mash, andi and CD-MAWS for most datasets only in bar charts (its Figures 3, 4 and 6); those values are not used. Its text gives values for E. coli/Shigella only (`published/ml_maws_text_values.csv`). The paper's section numbers differ from the project plan's references: the CD-MAWS value is in Section III-C, the run times in Section III-G.
- For E. coli/Shigella, the text of the ML-MAWS paper reports Mash nRF 0.231 and CD-MAWS nRF 0.577, while the AFproject results page lists Mash with nRF 0.15 (k = 26, sketch size 10,000) and CD-MAWS with nRF 0.23 (-k 16 -K 16 -r 1). The paper does not state the parameters it used. andi agrees (0.077 in the paper, 0.08 on AFproject). Both are kept, each with its source.
- The ML-MAWS paper's reference list gives the year of Peafowl as 2019; the article (Zahin et al., BMC Bioinformatics 26) was published on 2025-03-07.
- AFproject gives nRF with two decimals; differences below 0.005 cannot be resolved from it.
- ML-MAWS Table 2 gives the Influenza A dataset an average length of 13 kb. The 38 sequences are the neuraminidase segment (segment 6), 1,350 to 1,467 bp each, as stated by Li, He, He and Yau (2017), the source of the dataset, and as in the file ML-MAWS ran on (mean 1,407 letters); `influenza_a` here is identical in sequence to that file. The 13 kb figure therefore does not describe the data that were analysed (`docs/OPEN_ISSUES.md`, OI-6).
- The ML-MAWS repository's `LICENSE.txt` is the Apache License 2.0, while the header of its `SuffixAutomaton.h` states "License: MIT" with a placeholder author. No ML-MAWS code is copied into Q-MAWS (the behaviour is reimplemented), so this does not affect Q-MAWS (`docs/OPEN_ISSUES.md`, OI-11).
