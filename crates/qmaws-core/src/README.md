# src (qmaws-core)

## Purpose
Source code of the science crate. Each module receives data and returns data; none reads files or talks to the user.

## Contents
| Item | Description |
|---|---|
| `input.rs` | Input reading and cleaning: FASTA or raw text, cleaning rules, record modes, Newick-safe names, validation findings with their choices, renaming of duplicates |
| `lib.rs` | Crate root: module list |
| `newick.rs` | Newick parsing and writing; comparison of leaf names with taxon names |

## Relationships
Used by `qmaws-data` (reading folders and datasets, reference trees) and later by `qmaws-engine` for every stage.

## Notes
Further modules (MAW extraction, matrices, quartet counts, weighting, amalgamation, support, metrics) are added from M3.
