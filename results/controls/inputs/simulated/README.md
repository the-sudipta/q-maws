# simulated

## Purpose
16 sequences of 20000 bases evolved along a random binary tree under Jukes–Cantor (branch lengths uniform in 0.01 to 0.1, seed 1).

## Contents
| Item | Description |
|---|---|
| `sequences/` | The simulated sequences, one FASTA file per taxon |
| `true_tree.nwk` | The true tree with branch lengths (expected substitutions per site) |

## Relationships
Written by the hidden command `qmaws controls` (`crates/qmaws-cli/src/controls_cmd.rs`).

## Notes
Recreated by the command with the same seed.
