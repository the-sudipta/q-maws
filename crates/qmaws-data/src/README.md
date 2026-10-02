# src (qmaws-data)

## Purpose
Source code of the data crate: the benchmark registry, downloads, extraction, reference trees and reading of sequence folders from disk.

## Contents
| Item | Description |
|---|---|
| `download.rs` | Downloads with `.part` files, HTTP range resume, retries with back-off, MD5 and SHA-256 verification; link resolution from a web page; the HTTPS fetcher |
| `extract.rs` | Atomic, safe zip extraction with a marker file |
| `lib.rs` | Data folder layout, dataset status, the download log, and `fetch` (download or NCBI fetch, verify, extract, log) |
| `ncbi.rs` | NCBI datasets: accession lists, batched `efetch` requests with tool and email, record checks, the dataset file and its provenance table |
| `loader.rs` | Reading a folder or file of sequences (also `.gz`) into taxa with fingerprints and findings; the folder summary |
| `references.rs` | Built-in reference trees and name tables; reading the tree from an AFproject results page; name translation |
| `registry.rs` | The built-in benchmark registry and its checks |

## Relationships
Uses `qmaws-core` for cleaning and Newick. Compiles in `data/manifests/benchmarks.toml` and `data/references/`. Used by `qmaws-cli` (`datasets`, `download`, `inspect`).

## Notes
Network access is behind the `Fetcher` trait, so the download logic is tested with an in-memory server (resume, restart, retries, checksum mismatch).
