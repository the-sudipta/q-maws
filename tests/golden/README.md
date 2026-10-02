# golden

## Purpose
Golden files: fixed expected outputs that tests compare byte for byte. A golden file changes only with the owner's approval of a documented correction.

## Contents
| Item | Description |
|---|---|
| `worksheet_example.txt` | Output of `qmaws teach --example`: the teaching worksheet of the five-taxon example, golden test G1 |

## Relationships
Compared by tests in `crates/` (`crates/qmaws-core/src/teach.rs` compiles this file in with `include_str!`).

## Notes
`worksheet_example.txt` was written in M4 from the program's output after a test checked every value against the teaching example; the wording differences from the teaching material are listed in `docs/OPEN_ISSUES.md`, OI-12, and the owner approved the file on 2026-10-02. Line endings are LF in the repository; the test accepts CRLF checkouts.
