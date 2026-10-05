# audit

## Purpose
Small verification record of the run (design: `docs/DESIGN.md`, "Audit files"). `qmaws verify` checks the run against it.

## Contents
| Item | Description |
|---|---|
| `chunks.json` | Per chunk of each chunked stage: range, SHA-256 (weights also rounded to 9 significant digits); per bootstrap replicate: seed, SHA-256 of its tree |
| `chunks.json.sha256` | SHA-256 of `chunks.json`, written after it (validity check) |
| `environment.json` | Program version and commit, device, settings, seeds |
| `environment.json.sha256` | SHA-256 of `environment.json`, written after it (validity check) |
| `evaluation.json` | nRF, nQD and MSD of `report/tree.nwk` against `reference.nwk`, with every count and the SHA-256 of both trees; recomputed by `qmaws verify` |
| `evaluation.json.sha256` | SHA-256 of `evaluation.json`, written after it (validity check) |
| `inputs.json` | Per taxon: name, source file, lengths, removed symbols, cleaned SHA-256; per input file: path, SHA-256, size; MAW length range |
| `inputs.json.sha256` | SHA-256 of `inputs.json`, written after it (validity check) |
| `otl_taxonomy.json` | Open Tree of Life queries for the group bands: API, taxonomy version, date, names, SHA-256 of every answer |
| `quartet_decisions.bin.zst` | Per quartet in rank order: winning topology and its 16-bit weight (zstd) |
| `quartet_decisions.bin.zst.sha256` | SHA-256 of `quartet_decisions.bin.zst`, written after it (validity check) |
| `reference.json` | Label and source of the reference tree, and the SHA-256 of `reference.nwk` |
| `reference.json.sha256` | SHA-256 of `reference.json`, written after it (validity check) |
| `reference.nwk` | The reference tree the run is compared with, byte for byte as given (the user's file or the benchmark dataset's AFproject tree) |
| `reference.nwk.sha256` | SHA-256 of `reference.nwk`, written after it (validity check) |
| `results.json` | Tree, trees with S1 and S2, weights used, amalgamation summary, S1 and S2 per edge, halo value per taxon |
| `results.json.sha256` | SHA-256 of `results.json`, written after it (validity check) |
| `root.txt` | Root fingerprint |
| `root.txt.sha256` | SHA-256 of `root.txt`, written after it (validity check) |
| `sample_worksheets.txt` | Worksheets of 50 quartets drawn with a seed, each checked against the stored weights |
| `sample_worksheets.txt.sha256` | SHA-256 of `sample_worksheets.txt`, written after it (validity check) |
| `stages.json` | Per stage: content hash, start and end time (UTC), summary numbers; number of quartets, seed |
| `stages.json.sha256` | SHA-256 of `stages.json`, written after it (validity check) |

## Relationships
Written by `crates/qmaws-engine` (analysis run and `qmaws verify`).

## Notes
This README is written by the program and rewritten when the folder changes.
