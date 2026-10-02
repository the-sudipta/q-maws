# Sources of the reference trees

All files were obtained on 2026-10-02.

## Trees as published by AFproject (`*.afproject.nwk`)

The AFproject dataset archives contain no reference trees. Each AFproject results page shows the reference tree and offers it for download as an embedded link of the form `data:text/plain;charset=utf-8,<tree>` in the "Reference Tree" panel. The `*.afproject.nwk` files are that text, unchanged, followed by a line break.

| File | Results page |
|---|---|
| `fish_mito.afproject.nwk` | https://afproject.org/app/benchmark/genome/std/assembled/fish_mito/results/ |
| `ecoli.afproject.nwk` | https://afproject.org/app/benchmark/genome/std/assembled/ecoli/results/ |
| `ecoli_shigella_hgt.afproject.nwk` | https://afproject.org/app/benchmark/genome/hgt/unsimulated/ecoli_shigella/results/ |
| `yersinia_hgt.afproject.nwk` | https://afproject.org/app/benchmark/genome/hgt/unsimulated/yersinia/results/ |
| `sim_hgt.afproject.nwk` | https://afproject.org/app/benchmark/genome/hgt/simulated/sim_hgt/results/ (one tree for all five HGT levels) |

Citation: A. Zielezinski et al. Benchmarking of alignment-free sequence comparison methods. Genome Biology 20:144 (2019).

## Name tables (`*.names.tsv`)

For three datasets the AFproject tree uses species or strain names, while the dataset's FASTA files use sequence identifiers (RefSeq accessions and similar). The translation comes from the files `Data/<dataset>/reference/dataset.json` in the ML-MAWS repository (https://github.com/PapriSaha/ML-MAWS, commit `0c38db12d9ad271aafcb4940d7558dfcd00925c1`, Apache License 2.0). Each such file lists the sequence identifiers (`seqids`) and the tree names (`treids`) as two parallel lists; row *i* of a name table is (`treids[i]`, `seqids[i]`).

| Name table | ML-MAWS file |
|---|---|
| `fish_mito.names.tsv` | `Data/fish_mtDNA/reference/dataset.json` |
| `ecoli_shigella_hgt.names.tsv` | `Data/ecoli_shigella(HGT)/reference/dataset.json` |
| `yersinia_hgt.names.tsv` | `Data/yersinia-HGT/reference/dataset.json` |

## How the match was verified

The same ML-MAWS folders also contain each reference tree already written with sequence identifiers (`tree.Fischer2013.newick`, `EcoliShigella_MRP_Skippington2011_tree.newick`, `Yersinia_D2S_k9.newick`, `ecoli_ref-tree.newick`, `ref-tree-trimmed.newick`). Translating each AFproject tree with its name table gives a tree that is **identical, character for character after removing whitespace,** to the corresponding ML-MAWS tree, for all five references; for `ecoli` and `sim_hgt`, the AFproject tree itself is identical to the ML-MAWS tree. Two independent publications of the same trees therefore agree on every name. In addition, after download, every dataset's taxon names (from the FASTA headers, which equal the file names) match the leaves of its `.nwk` tree exactly: 25, 29, 27, 8 and 33 of 33 (each HGT level).

## Trees used by the program (`*.nwk`)

The `.nwk` files are the ML-MAWS trees listed above with whitespace removed, followed by a line break; equivalently, the translated AFproject trees. A unit test in `crates/qmaws-data/src/references.rs` repeats the translation and comparison.

## Not used

The AFproject reference package `AF-reference_datasets190511.zip` (4,894,058,545 bytes, https://afproject.org/media/AF-reference_datasets190511.zip) was not downloaded: the trees were found and their names matched without it (decision D14 not needed so far).
