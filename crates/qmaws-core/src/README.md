# src (qmaws-core)

## Purpose
Source code of the science crate. Each module receives data and returns data; none reads files or talks to the user.

## Contents
| Item | Description |
|---|---|
| `matrix.rs` | MAW length range and entropy selection, `M_full` and `M_ml` construction, the 50,000-column cap, PHYLIP export |
| `maw.rs` | Suffix automaton, MAW enumeration with lengths and strand filter, compact word codes, the brute-force oracle |
| `amalgamate.rs` | Quartet amalgamation `wQFM-rs` (wQFM v1.4 reimplemented), the weighted quartet consistency score, the exhaustive tree enumeration (G8 oracle), wQFM's input format |
| `bootstrap.rs` | S2 column bootstrap: Poisson(1) column weights from a seeded generator, replicate seeds, weighted pattern counts per weight class with the inclusion–exclusion kernel |
| `control.rs` | Data for the controls: Jukes–Cantor sequences evolved along a random tree, Fisher–Yates shuffles that keep the letter composition, nRF of random trees to a reference |
| `input.rs` | Input reading and cleaning: FASTA or raw text, cleaning rules, record modes, Newick-safe names, validation findings with their choices, renaming of duplicates |
| `lib.rs` | Crate root: module list |
| `sim.rs` | The four-taxon long-branch simulation of H3: per-character simulator, replicate seeds, recovery with ties shared |
| `stats.rs` | Statistics of the H1 test: exact one-sided Wilcoxon signed-rank test (all sign assignments enumerated; zeros dropped or Pratt), Holm correction, median, Hodges–Lehmann estimate, mean and sample SD |
| `support.rs` | S1 support of every internal edge and halo values of every taxon, summed over a stream of weighted quartets in a fixed order; the tree with S1 labels; S2 split frequencies of bootstrap replicate trees |
| `quartet.rs` | Quartet ranks and unranking, the keyed Feistel processing order, co-occurrence tables, the inclusion–exclusion pattern counts with run-time popcount dispatch, the column-scan oracle |
| `teach.rs` | The teaching worksheet: two-letter MAWs, matrix, pattern table, W1 votes, classroom amalgamation, nRF, model example |
| `weight.rs` | Quartet weighting: W1 votes; the two-state model and its pattern probabilities; the conditioned log-likelihood; the branch-length optimiser; W2a, W2b and W2c with per-quartet seeds, SplitMix64 and exact binomial draws |
| `worksheet.rs` | The worksheet of one quartet: co-occurrence counts, pattern counts by inclusion–exclusion written out, W1 votes, W2 fits with branch lengths, W2a, W2b and W2c (audit samples and `qmaws verify --quartet`) |
| `metrics.rs` | Tree comparison: normalised quartet distance (every quartet, four-point condition, unresolved quartets of the reference counted apart) and the matching split distance with the Hungarian algorithm |
| `metrics_check.rs` | Golden test G9: random tree pairs (5 to 60 leaves, some references with polytomies), nQD by enumeration over splits and MSD by brute-force matching as independent oracles |
| `newick.rs` | Newick parsing and writing; comparison of leaf names with taxon names; splits and normalised Robinson–Foulds distance |

## Relationships
Used by `qmaws-data` (reading folders and datasets, reference trees), by `qmaws-engine` for every stage, and by `qmaws-cli` (teaching worksheet, IQ-TREE cross-check).

## Notes
Tree metrics (nRF in `newick.rs`, nQD and MSD in `metrics.rs`) are checked by golden test G9 (`metrics_check.rs`, `qmaws metrics-check`, `scripts/metrics_check.py`).
