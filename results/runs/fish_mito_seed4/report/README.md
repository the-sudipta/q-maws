# report

## Purpose
Tables and reports of the run.

## Contents
| Item | Description |
|---|---|
| `convergence.csv` | nRF of every provisional tree to the final tree, by percentage of quartets finished |
| `halo.tsv` | Halo value of every taxon with its weights |
| `halo.tsv.sha256` | SHA-256 of `halo.tsv`, written after it (validity check) |
| `m_ml.phy` | M_ml in PHYLIP format |
| `m_ml.phy.sha256` | SHA-256 of `m_ml.phy`, written after it (validity check) |
| `reference_comparison.tsv` | nRF, nQD and MSD of the tree against the reference tree |
| `reference_comparison.tsv.sha256` | SHA-256 of `reference_comparison.tsv`, written after it (validity check) |
| `support.tsv` | S1 of every internal edge with its weights, number of quartets and clade |
| `support.tsv.sha256` | SHA-256 of `support.tsv`, written after it (validity check) |
| `tree.nwk` | The tree of wQFM-rs (Newick, unrooted) |
| `tree.nwk.sha256` | SHA-256 of `tree.nwk`, written after it (validity check) |
| `verify_2026-10-10_172047.txt` | Report of `qmaws verify`: every comparison and the verdict |
| `verify_2026-10-10_172047.txt.sha256` | SHA-256 of `verify_2026-10-10_172047.txt`, written after it (validity check) |

## Relationships
Written by `crates/qmaws-engine` (analysis run and `qmaws verify`).

## Notes
This README is written by the program and rewritten when the folder changes.
