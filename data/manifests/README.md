# manifests

## Purpose
Committed records that make every dataset reproducible: the benchmark registry, the accession lists of the NCBI datasets, the log of every download with its URL and checksums, and (from M13) the recipes of simulated datasets. The registry and the accession lists are compiled into the `qmaws` program.

## Contents
| Item | Description |
|---|---|
| `accessions/` | Accession lists of the NCBI datasets, with pinned record versions ([README](accessions/README.md)) |
| `benchmarks.toml` | Benchmark registry: NCBI settings, one `[[download]]` per file or record set fetched from the internet, one `[[dataset]]` per benchmark dataset |
| `download_log.json` | One entry per download made with `qmaws download`: URL, date, size, published and computed checksums, result |
| `simulated/` | Recipes, seeds, checksums and true trees of the simulated scaling datasets of H2 ([README](simulated/README.md)) |

## Relationships
Read by `crates/qmaws-data` (registry and accession lists compiled in with `include_str!`; log appended by `qmaws download`). Describes the files downloaded into `data/raw/`. Reference trees named by datasets are in `data/references/`.

## Notes

### `[ncbi]` settings

`tool` and `email` are sent with every NCBI E-utilities request, as NCBI asks. The email is the owner's contact address (decision D10, 2026-10-02).

### `[[download]]` fields

| Field | Meaning |
|---|---|
| `id` | Download identifier; also the folder name under `data/raw/` |
| `source` | Who publishes the file |
| `page_url` | Page that links to the file (read to find the link; never guessed); for NCBI downloads, the publication of the accession list |
| `file_name` | Name of the published file, or of the file built from NCBI records |
| `resolved_url` | Direct link found on the page and confirmed by a verified download; for NCBI downloads, the E-utilities `efetch` endpoint |
| `kind` | `zip` (extracted after download, archive kept in `data/raw/_archives/`), `file` (used as downloaded) or `ncbi` (records fetched by accession and written as one multi-FASTA file) |
| `accessions` | For `ncbi`: the accession list in `accessions/` |
| `published_size` | Size as stated by the source, the exact byte count for files without a stated size, or the number of records |
| `published_md5` | MD5 published by the source, if it publishes one |
| `pinned_sha256` | SHA-256 the file must have: for AFproject, computed from our first verified download (2026-10-02); for repository files, the SHA-256 of the file at the pinned commit; for NCBI, computed from the file built from the pinned record versions |

A download is accepted only if every given checksum matches. Each `zip` and `file` download must have `published_md5` or `pinned_sha256`.

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
- **NCBI datasets:** built from NCBI Nucleotide with the accession lists of Li, He, He and Yau (2017), Supplementary Tables S1 to S5 (`accessions/README.md`), as the owner decided (decision D5, 2026-10-02). Every one of the 288 records was returned, and every sequence is identical to the file ML-MAWS used.
- **Influenza A:** Li et al. (2017) state that they used "Segment 6 gene encoding N (neuraminidase)" of the 38 viruses; their table lists lengths of 1,350 to 1,467 bp, and every NCBI record title names segment 6 or neuraminidase. The dataset is therefore the neuraminidase segment, about 1.4 kb per virus; the 13 kb average listed in the ML-MAWS paper (Table 2) does not describe these data (`docs/OPEN_ISSUES.md`, OI-6).
- **ML-MAWS data files (reference only):** the ML-MAWS repository (commit `0c38db12d9ad271aafcb4940d7558dfcd00925c1`, folder `Data/`) contains the five files ML-MAWS used, without accession lists. They stay registered as `coronavirus_mlmaws`, `ebolavirus_mlmaws`, `influenza_a_mlmaws`, `mammal_mtdna_mlmaws` and `rhinovirus_mlmaws`, only to document and check that the NCBI datasets contain the same sequences. Mean A/C/G/T letters per record: coronavirus 27,565; ebolavirus 18,932; influenza 1,407; mammal mtDNA 16,647; rhinovirus 7,153.
- **Download log:** the first entries for `coronavirus`, `ebolavirus`, `influenza_a`, `mammal_mtdna` and `rhinovirus` (URL `raw.githubusercontent.com/...`) were made before these ids were given to the NCBI datasets; they are the ML-MAWS files now registered with the suffix `_mlmaws`. Entries with the `efetch` URL are the NCBI datasets.
