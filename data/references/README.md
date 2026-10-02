# references

## Purpose
Reference trees for the benchmark datasets, with the name tables that link AFproject's leaf names to the sequence identifiers in the dataset files. The program uses the `<id>.nwk` trees (compiled in) to compute nRF, nQD and MSD and to draw tanglegrams.

## Contents
| Item | Description |
|---|---|
| `ecoli.afproject.nwk` | E. coli/Shigella (29) tree as published by AFproject; leaf names are already the sequence identifiers |
| `ecoli.nwk` | Same tree, used by the program |
| `ecoli_shigella_hgt.afproject.nwk` | E. coli/Shigella HGT (27) tree as published by AFproject (strain names) |
| `ecoli_shigella_hgt.names.tsv` | AFproject strain name to RefSeq accession, 27 rows |
| `ecoli_shigella_hgt.nwk` | Tree with accessions, used by the program |
| `fish_mito.afproject.nwk` | Fish mtDNA (25) tree as published by AFproject (species names) |
| `fish_mito.names.tsv` | AFproject species name to RefSeq accession, 25 rows |
| `fish_mito.nwk` | Tree with accessions, used by the program |
| `sim_hgt.afproject.nwk` | Simulated HGT (33) tree as published by AFproject, shared by the five HGT levels; names are already the sequence identifiers |
| `sim_hgt.nwk` | Same tree, used by the program |
| `SOURCE.md` | Where each file comes from and how the names were matched |
| `yersinia_hgt.afproject.nwk` | Yersinia HGT (8) tree as published by AFproject (strain names) |
| `yersinia_hgt.names.tsv` | AFproject strain name to sequence identifier, 8 rows |
| `yersinia_hgt.nwk` | Tree with sequence identifiers, used by the program |

## Relationships
Compiled into `crates/qmaws-data` (`references.rs`). Named by the `reference` field of datasets in `data/manifests/benchmarks.toml`. Checked against the taxa of each downloaded dataset by `qmaws download`.

## Notes
Name tables have a header line and two tab-separated columns: `tree_name`, `sequence_id`. A test checks that translating each `.afproject.nwk` tree with its table gives exactly the committed `.nwk` tree; `qmaws download` checks that the tree's leaves equal the dataset's taxon names. Names are never matched by guesswork; see `SOURCE.md`.
