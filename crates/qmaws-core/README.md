# qmaws-core

## Purpose
All science of Q-MAWS: input cleaning, MAW extraction, strand filter, length selection, matrices, quartet pattern counts, quartet weighting, amalgamation, support, halo values and evaluation metrics. Implemented from milestone M3 onwards.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `lib.rs` (crate root) |

## Relationships
Used by `qmaws-engine` (orchestration) and through it by the interfaces. Depends on no other workspace crate. It has no knowledge of files, terminals or windows: it receives data and returns data.

## Notes
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
