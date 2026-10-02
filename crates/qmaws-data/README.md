# qmaws-data

## Purpose
Benchmark dataset registry, downloads with resume and checksum verification, archive extraction, the download log, reference trees, and reading sequence folders from disk (M2). The Open Tree of Life client follows in a later milestone.

## Contents
| Item | Description |
|---|---|
| `Cargo.toml` | Crate manifest; shared fields come from the workspace manifest |
| `src/` | Source code ([README](src/README.md)) |

## Relationships
Reads and writes the manifests in `data/manifests/` and the trees in `data/references/`. Used by `qmaws-engine` and `qmaws-cli`.

## Notes
Network access goes through the `Fetcher` trait; the registry and reference trees are compiled in. Design details: `docs/DESIGN.md` (data folder, downloads and verification).
