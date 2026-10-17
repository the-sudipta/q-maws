# ML-MAWS on hgt_h010_r5

ML-MAWS (commit `0c38db1`, `--strand --iqtree`, IQ-TREE 2.4.0, all cores) on the laptop; its matrix and IQ-TREE work files are not kept.

| Item | Content |
|---|---|
| `ML_MAWS_tree.newick` | The ML-MAWS tree with UFBoot support (the IQ-TREE `.treefile`) |
| `complexity_report.json` | ML-MAWS's own report of the time and memory of each of its steps |
| `entropy_results.tsv` | ML-MAWS's entropy per MAW length (its length selection) |
| `ml_maws_iqtree.contree` | IQ-TREE consensus tree of the UFBoot replicates |
| `ml_maws_iqtree.iqtree` | IQ-TREE report (model chosen by ModelFinder, likelihood, tree) |
| `ml_maws_iqtree.log` | IQ-TREE log (command, seed, run time, memory) |
| `ml_maws_iqtree.treefile` | IQ-TREE maximum-likelihood tree with UFBoot support |
| `stderr.txt` | The program's messages (standard error) |
| `stdout.txt` | The program's output (standard output) |
