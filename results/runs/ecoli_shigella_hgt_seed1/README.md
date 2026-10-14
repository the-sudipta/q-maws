# ecoli_shigella_hgt_seed1

## Purpose
One analysis run of Q-MAWS: its configuration, log, verification record, trees and reports. Written by the program; the large intermediate files in `work/` are not committed.

## Contents
| Item | Description |
|---|---|
| `audit/` | Verification record |
| `figures/` | Figures |
| `report/` | Tree, matrix export, support, bootstrap and halo tables, verification reports |
| `run.json` | Configuration, input fingerprints, stage states, chunk plans, throughput, working time |
| `run.json.sha256` | SHA-256 of `run.json`, written after it (validity check) |
| `run.log` | Chronological log in English, UTC timestamps |
| `trees/` | Final trees with support values |

## Relationships
Written by `crates/qmaws-engine` (analysis run and `qmaws verify`).

## Notes
This README is written by the program and rewritten when the folder changes.
