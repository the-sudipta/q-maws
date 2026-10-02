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

## To be written

- Optimiser for the conditioned quartet likelihood (M5)
- wQFM-rs algorithm details with references to the wQFM paper (M7)
