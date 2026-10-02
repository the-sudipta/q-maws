# src (qmaws-core)

## Purpose
Source code of the science crate. Each module receives data and returns data; none reads files or talks to the user.

## Contents
| Item | Description |
|---|---|
| `matrix.rs` | MAW length range and entropy selection, `M_full` and `M_ml` construction, the 50,000-column cap, PHYLIP export |
| `maw.rs` | Suffix automaton, MAW enumeration with lengths and strand filter, compact word codes, the brute-force oracle |
| `input.rs` | Input reading and cleaning: FASTA or raw text, cleaning rules, record modes, Newick-safe names, validation findings with their choices, renaming of duplicates |
| `lib.rs` | Crate root: module list |
| `sim.rs` | The four-taxon long-branch simulation of H3: per-character simulator, replicate seeds, recovery with ties shared |
| `quartet.rs` | Quartet ranks and unranking, the keyed Feistel processing order, co-occurrence tables, the inclusion–exclusion pattern counts with run-time popcount dispatch, the column-scan oracle |
| `teach.rs` | The teaching worksheet: two-letter MAWs, matrix, pattern table, W1 votes, classroom amalgamation, nRF, model example |
| `weight.rs` | Quartet weighting: W1 votes; the two-state model and its pattern probabilities; the conditioned log-likelihood; the branch-length optimiser; W2a, W2b and W2c with per-quartet seeds, SplitMix64 and exact binomial draws |
| `newick.rs` | Newick parsing and writing; comparison of leaf names with taxon names; splits and normalised Robinson–Foulds distance |

## Relationships
Used by `qmaws-data` (reading folders and datasets, reference trees), by `qmaws-engine` for every stage, and by `qmaws-cli` (teaching worksheet, IQ-TREE cross-check).

## Notes
Further modules (amalgamation, support, metrics) are added from M7.
