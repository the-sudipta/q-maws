# qmaws-core

## Purpose
All science of Q-MAWS: input cleaning, MAW extraction, strand filter, length selection, matrices, quartet pattern counts, quartet weighting, amalgamation, support, halo values and evaluation metrics. Input cleaning and Newick parsing exist since M2; the other parts are implemented from M3 onwards.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Used by `qmaws-engine` (orchestration) and through it by the interfaces. Depends on no other workspace crate. It has no knowledge of files, terminals or windows: it receives data and returns data.

## Notes
`qmaws-core` has no knowledge of files, terminals or windows: it receives data and returns data.
