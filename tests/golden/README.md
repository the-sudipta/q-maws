# golden

## Purpose
Golden files: fixed expected outputs that tests compare byte for byte. A golden file changes only with the owner's approval of a documented correction.

## Contents
| Item | Description |
|---|---|
| `fish_mito_root.txt` | Root fingerprint of `qmaws run --dataset fish_mito` with the default settings (seed 1, W2c with 100 resamples, S2 with 100 replicates): golden test G12, checked in CI on Windows, macOS and Linux |
| `worksheet_example.txt` | Output of `qmaws teach --example`: the teaching worksheet of the five-taxon example, golden test G1 |

## Relationships
Compared by tests in `crates/` (`crates/qmaws-core/src/teach.rs` compiles `worksheet_example.txt` in with `include_str!`) and by the job "Cross-platform determinism (G12)" of `.github/workflows/ci.yml` (`fish_mito_root.txt`).

## Notes
`worksheet_example.txt` was written in M4 from the program's output after a test checked every value against the teaching example; the wording differences from the teaching material are listed in `docs/OPEN_ISSUES.md`, OI-12, and the owner approved the file on 2026-10-02. Line endings are LF in the repository; the test accepts CRLF checkouts.

`fish_mito_root.txt` (added in M14) is the root of two default Fish mtDNA runs on the development laptop (Windows, x86-64), on 2026-10-03 with the build of commit 7b1de8a and on 2026-10-07 with the build of branch `m14`; both gave this value.
