# qmaws-data

## Purpose
Benchmark dataset registry, downloads with resume and checksum verification, dataset manifests, reference trees, and the Open Tree of Life client. Implemented from milestone M2 onwards.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source: `lib.rs` (crate root) |

## Relationships
Reads and writes the manifests in `data/manifests/` and the trees in `data/references/`. Used by `qmaws-engine` and `qmaws-cli`.

## Notes
Skeleton only (milestone M0): the crate compiles and has trivial tests. Functionality is added in the milestones named above.
