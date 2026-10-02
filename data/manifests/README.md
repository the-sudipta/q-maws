# manifests

## Purpose
Committed records that make every dataset reproducible: the benchmark registry, NCBI accession lists, the download log with URLs and hashes, and the commands and seeds of simulated datasets.

## Contents
| Item | Description |
|---|---|

## Relationships
Read and updated by `crates/qmaws-data`. Describes the files downloaded into `data/raw/`.

## Notes
Empty in milestone M0. The registry `benchmarks.toml`, `download_log.json`, `accessions/` and `simulated/` are added in milestones M2 and M13 (TOML, one file per dataset group), with every field documented in this README.
