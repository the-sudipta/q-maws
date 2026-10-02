# manifests

## Purpose
Committed records that make every dataset reproducible: the benchmark registry, the log of every download with its URL and checksums, and (from M13) the recipes of simulated datasets. The registry is compiled into the `qmaws` program.

## Contents
| Item | Description |
|---|---|
| `benchmarks.toml` | Benchmark registry: one `[[download]]` per file fetched from the internet, one `[[dataset]]` per benchmark dataset |
| `download_log.json` | One entry per download made with `qmaws download`: URL, date, size, published and computed checksums, result |

## Relationships
Read by `crates/qmaws-data` (registry compiled in with `include_str!`; log appended by `qmaws download`). Describes the files downloaded into `data/raw/`. Reference trees named by datasets are in `data/references/`.

## Notes

### `[[download]]` fields

| Field | Meaning |
|---|---|
| `id` | Download identifier; also the folder name under `data/raw/` |
| `source` | Who publishes the file |
| `page_url` | Page that links to the file (read to find the link; never guessed) |
| `file_name` | Name of the published file |
| `resolved_url` | Direct link found on the page and confirmed by a verified download |
| `kind` | `zip` (extracted after download, archive kept in `data/raw/_archives/`) or `file` (used as downloaded) |
| `published_size` | Size as stated by the source, or the exact byte count for files without a stated size |
| `published_md5` | MD5 published by the source, if it publishes one |
| `pinned_sha256` | SHA-256 the file must have: for AFproject, computed from our first verified download (2026-10-02); for repository files, the SHA-256 of the file at the pinned commit |

A download is accepted only if every given checksum matches. Each download must have `published_md5` or `pinned_sha256`.

### `[[dataset]]` fields

| Field | Meaning |
|---|---|
| `id` | Dataset identifier used in commands (`qmaws download --dataset <id>`) |
| `name` | Display name |
| `download` | Id of the download that contains the dataset |
| `path` | Folder or file inside `data/raw/<download id>/` |
| `layout` | `file_per_taxon` (a folder, one sequence file per taxon) or `record_per_taxon` (one multi-FASTA file, one record per taxon) |
| `taxa` | Expected number of taxa, checked after download |
| `reference` | Reference tree id in `data/references/`, or empty |
| `citation` | Source to cite |

### Findings on the datasets (2026-10-02)

- **AFproject:** all five archives match their published MD5. Every sequence file holds one FASTA record whose header equals the file name. The simulated HGT archive contains one folder per HGT level (`hgt_0`, `hgt_250`, `hgt_500`, `hgt_750`, `hgt_1000`) with 33 single-genome files each, registered as datasets `sim_hgt_0` to `sim_hgt_1000` sharing one download.
- **NCBI datasets of ML-MAWS:** the ML-MAWS repository (commit `0c38db12d9ad271aafcb4940d7558dfcd00925c1`, folder `Data/`) contains the five sequence files ML-MAWS used: `coronavirus.fasta` (34 records), `ebolavirus.fasta` (59), `influenza.fasta` (38), `mammal_mtdna.fasta` (41), `rhinovirus.fasta` (116). It contains no accession lists; headers are names (for example `Cow`, `2_MHV`, `A/duck/Hong Kong/319/1978(H2N2)`), and only the Ebolavirus headers include accession numbers. These files are registered directly, pinned to that commit, and labelled "ML-MAWS data file". Mean lengths of A, C, G, T per record: coronavirus 27,565; ebolavirus 18,932; influenza 1,407; mammal mtDNA 16,647; rhinovirus 7,153. The influenza mean differs from the 13 kb average listed for this dataset in the ML-MAWS paper (Table 2); see `docs/OPEN_ISSUES.md`, OI-6.
- No download from NCBI was needed, so the NCBI E-utilities (and an `email` parameter, decision D10) are not used.
