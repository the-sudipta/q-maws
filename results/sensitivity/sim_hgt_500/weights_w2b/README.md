# weights_w2b

## Purpose
Exploratory sensitivity variant (milestone M12, docs/PREREGISTRATION.md Amendment 3): the tree of the seed-1 primary run of `sim_hgt_500` made again with W2B quartet weights from its stored quartet results, with S1 support and halo values, compared with the run's reference tree.

## Contents
| Item | Description |
|---|---|
| `tree.nwk` | The tree of wQFM-rs with these weights (Newick, unrooted) |
| `tree_s1.nwk` | The tree with S1 support (on these weights) as internal labels |
| `support.tsv` | S1 of every internal edge with its weights, number of quartets and clade |
| `halo.tsv` | Halo value of every taxon |
| `variant.json` | The run used (path and root fingerprint), the weights, the number of weighted quartets, the tree and the comparison with the reference tree |
| `tree.nwk.sha256` | SHA-256 of `tree.nwk`, written after it |
| `tree_s1.nwk.sha256` | SHA-256 of `tree_s1.nwk`, written after it |
| `support.tsv.sha256` | SHA-256 of `support.tsv`, written after it |
| `halo.tsv.sha256` | SHA-256 of `halo.tsv`, written after it |
| `variant.json.sha256` | SHA-256 of `variant.json`, written after it |

## Relationships
Written by `qmaws variant --run <run> --weights w2b --output <this folder>` from the run folder in `results/runs/` named in `variant.json` (its `work/` folder is needed). Summarised in `../../weights.tsv`.

## Notes
Exploratory: not part of a pre-registered test. W1 has no random element, so its tree does not depend on the seed.
