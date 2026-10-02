# Design

Software design decisions and their justification. Sections are added by the milestone that makes the decision.

## Workspace layout (M0)

The code is a Cargo workspace with seven crates:

| Crate | Responsibility |
|---|---|
| `qmaws-core` | All science: input, MAW extraction, matrices, quartet counts, weights, amalgamation, support, metrics |
| `qmaws-engine` | Run orchestration: stages, checkpoints, chunks, progress events, audit log |
| `qmaws-data` | Benchmark registry, downloads, manifests, reference trees, Open Tree of Life client |
| `qmaws-viz` | Halo Tree, tanglegram, rectangular tree, HTML export, animation |
| `qmaws-tui` | Terminal menus and progress display |
| `qmaws-gui` | Desktop GUI application |
| `qmaws-cli` | The `qmaws` binary: argument parsing, dispatch to terminal or GUI mode |

Rule: `qmaws-core` has no knowledge of files, terminals or windows. It receives data and returns data. This keeps the science fully testable and guarantees that terminal and GUI modes give identical results.

## Toolchain (M0)

The Rust toolchain is pinned to a specific stable release in `rust-toolchain.toml` (edition 2021), so every developer and CI machine builds with the same compiler. The pin is raised deliberately, in its own commit.

## Run folder (M1)

Every run lives in its own folder, by default `results/runs/<name>_<YYYY-MM-DD>_<HHMMSS>`. The date and time are in UTC, so run names do not depend on the computer's time zone; if the folder exists, `_2`, `_3`, ... is appended. Layout:

| Path | Content |
|---|---|
| `run.json` (+ `run.json.sha256`) | Configuration, input fingerprints, stage states, chunk plans, measured throughput, accumulated working time, last interface |
| `run.log` | Chronological log in English, one line per event, UTC timestamps |
| `work/maws/`, `work/matrix/`, `work/chunks/`, `work/support/`, `work/provisional/` | Large intermediate files; never committed |
| `audit/` | Small verification record, including `root.txt` (root fingerprint) and `chunks.json` |
| `trees/`, `figures/` (with `live/`), `report/` | Final trees, figures and reports |

## Atomic writes and validity (M1)

Every output `<name>` is written to `<name>.tmp`, flushed, synced, and renamed to `<name>`; then its SHA-256 is written the same way to `<name>.sha256`. Before the data rename, the old hash file is deleted. A file is valid only if it exists and matches its hash file; anything else is treated as not done and recomputed. Leftover `*.tmp` files are deleted at the start of every session. On Unix, the parent folder is synced after each rename.

One exception keeps a run resumable after a kill at the worst moment: if `run.json` exists but `run.json.sha256` is missing, the process was killed between the data rename and the hash write. The rename is atomic, so `run.json` is complete, and it is accepted; the next save restores the hash. A hash file that exists but does not match is damage, and the run is refused with an explanation.

## Stages, chunks and resume (M1)

- Each stage in `run.json` is `pending`, `running` or `done`, with the SHA-256 of its output files. The full pipeline has 11 stages (`ingest` to `finalize`).
- Chunked stages split their work units into contiguous chunks of equal size (the last may be smaller). The chunk plan is chosen when the run starts, from a short calibration and a target chunk duration (default 3 seconds), and stored in `run.json`; it never changes on resume.
- A chunk is done exactly when its file in `work/chunks/` is valid. Resuming loads and verifies `run.json`, verifies the input fingerprints (refusing to continue if an input changed), removes `*.tmp` files, checks every chunk and every final output, resets damaged units, and continues with the first unfinished unit. `run.json` is saved after every chunk.
- Stopping: the first Ctrl+C asks the engine to stop before the next unit (the current chunk is finished and saved); the second exits at once. Both are safe because every write is atomic. Exit status: 0 finished, 3 stopped (resumable), 1 error, 2 usage error.

## Root fingerprint (M1)

The root fingerprint is the SHA-256 of a short, line-based text listing the fingerprint format, the run kind, the SHA-256 of the configuration, and the content hash of each stage. The content hash of a chunked stage is the SHA-256 of all chunk outputs concatenated in order. It therefore does **not** depend on where the chunk boundaries are, which are chosen per device by calibration. Per-chunk hashes are recorded separately in `audit/chunks.json` for verification. See `docs/OPEN_ISSUES.md`, OI-4.

## Progress and time estimates (M1)

- The engine sends events to a progress sink (`Started`, `Progress`, `Log`, `Finished`, `Stopped`); it never prints. Terminal and GUI displays implement the sink.
- Each stage has an estimated cost: remaining units divided by throughput (units per second). Throughput starts from the calibration, or from the value stored in `run.json` on resume, and is updated after every chunk with an exponentially weighted moving average (weight 0.3 for the newest measurement). Overall completion is weighted by estimated stage cost. Working time accumulates over sessions.
- The terminal display shows an overall bar and a stage bar with stage number, elapsed time, remaining time and the current item, and prints log lines above the bars. When the output is not a terminal, it prints a plain progress line at most every 5 seconds and at the end of each stage. `--quiet` prints only the result; `--json-progress` prints one JSON object per event on standard output; `--no-color` disables colours.

## Toy run (M1)

`qmaws toy-run` exercises the engine without any science. Its work is split into blocks of 1,048,576 steps; block `b` is the wrapping sum of SplitMix64 outputs over its steps, stored as 8 bytes. Stages: `toy_count` (chunked) and `finalize` (writes `audit/chunks.json`, `audit/root.txt`, `report/toy_result.json`). The same seed and block count give the same root fingerprint on every device, whatever the chunk size.

## Input reading and cleaning (M2)

- `qmaws-core::input` works on file names and bytes only. A file whose first non-whitespace character is `>` is FASTA (a record's name is the first word of its header when files are taxa, and the whole header line when records are taxa); anything else is raw sequence text for one taxon named after the file. Accepted extensions: `.txt`, `.fa`, `.fasta`, `.fna`, `.fas`, each optionally followed by `.gz` (case-insensitive).
- Cleaning, in order: remove header lines, line breaks, spaces, tabs, digits and other whitespace; upper-case; `U` to `T`; remove everything except A, C, G, T, counting each removed symbol. The original length is the number of symbols after the first step.
- Folders: files in byte order of their names. By default one taxon per file (records joined in file order; a single-record file keeps its header's name, a multi-record file is named after the file); `--records per-record` gives one taxon per record. A single file is read as one taxon per record. Confirmed by the owner (D13, 2026-10-02; `docs/OPEN_ISSUES.md`, OI-8). With one taxon per record, the whole header line is the name, unsafe characters replaced by `_` (owner decision, OI-7).
- Validation findings: fewer than 4 usable taxa (error); empty after cleaning, duplicate names, identical cleaned sequences, shorter than 100 letters (warnings, each with its choices); names with characters unsafe in Newick (space, `( ) , : ; ' " [ ]`, control characters) are changed to `_` automatically. Duplicates can be renamed by appending `_2`, `_3`, ... in input order, skipping names already in use. Interactive choices are asked by the menus (later milestones); `qmaws inspect` lists the findings with their choices.

## Newick (M2)

`qmaws-core::newick` parses nested groups, unquoted and single-quoted labels, branch lengths, internal labels and bracket comments, and writes trees back with quoting where needed. Underscores in unquoted labels are kept (not turned into spaces), so leaf names compare exactly with taxon names.

## Data folder, downloads and verification (M2)

- The registry `data/manifests/benchmarks.toml` and the reference trees in `data/references/` are compiled into the program, so it works without the repository. Downloads go to the data folder (default `data/`, option `--data-dir`).
- Download: write to `<file>.part`; continue an existing `.part` file with an HTTP `Range` request; if the server sends the whole file instead, start again; up to 5 attempts with waits of 2, 4, 8 and 16 s between them; client errors (4xx except 429) are not retried. A complete file is checked against every given checksum (published MD5, pinned SHA-256); on a mismatch it is deleted and downloaded once more, then reported. Only a verified file is renamed to its final name; an existing file is replaced only by a verified one.
- HTTPS uses rustls with the ring provider and the operating system's certificate store (`docs/DEPENDENCIES.md`). Timeouts: 30 s to connect, 60 s for the response to start.
- Archives are extracted into `<folder>.extracting` and renamed when complete; entries that would leave the folder are refused. A marker file `.qmaws-extracted.json` records the archive's SHA-256 and file count, so a missing or outdated extraction is repeated.
- After each download, the dataset is read: its taxon count must equal the registry's, and its names must equal the reference tree's leaves exactly. Every download is appended to `data/manifests/download_log.json` (URL, UTC date, size, MD5, SHA-256, result).

## NCBI datasets (M2)

- An NCBI download names an accession list (`data/manifests/accessions/<id>.tsv`, compiled in). `qmaws download` requests the pinned record versions from E-utilities `efetch` in batches of 50 with at least 0.4 s between requests, sending the `tool` and `email` of the registry's `[ncbi]` section. Every listed accession must be returned, with exactly the pinned version; no other record may be returned.
- The records are written in list order as `<id>.fasta`, each under the list's name as header, with the sequence lines as NCBI sends them, and a provenance table `<id>.accessions.tsv` (name, accession, version, NCBI title). The file is checked against the pinned SHA-256 before it replaces anything.
- `qmaws inspect --dataset <a> --compare-with <b>` compares cleaned sequences of two datasets by SHA-256 and lists identical sequences under other names; it was used to show that the NCBI datasets equal the files ML-MAWS used.

## To be written

- Optimiser for the conditioned quartet likelihood (M5)
- wQFM-rs algorithm details with references to the wQFM paper (M7)
