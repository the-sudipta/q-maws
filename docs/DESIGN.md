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

## MAW extraction (M3)

- `qmaws-core::maw` builds a suffix automaton (at most 2n states of 24 bytes: four `u32` transitions, suffix link, length) and enumerates MAWs as ML-MAWS does: a depth-first walk over every path from the initial state, that is, every distinct factor x with |x| < l_max. At the state p of x, for each letter b without a transition from p, x·b is a MAW when the suffix-link state q of p has a transition on b and len(q) + 1 ≥ |x|. Because |x| − 1 ≥ len(q) always holds for a state reached by x, the condition means that x without its first letter lies in q, which extends by b; so the test matches the definition exactly. Only lengths in [l_min, l_max] and at least 2 are produced. The walk uses an explicit stack, so deep recursion cannot occur.
- Words are stored 2 bits per letter (A=0, C=1, G=2, T=3) in one sorted list per length: `u64` up to 32 letters, `u128` from 33 to 64 letters. Within one length, numeric order is lexicographic order; across lengths, words are compared after left-aligning the codes, then by length, which gives the same order as comparing the letter strings. Lists release spare capacity after sorting.
- Strand filter: the MAWs of the sequence intersected with the MAWs of its reverse complement (two automata, built one after the other).
- Tests: the brute-force oracle enumerates every word over A, C, G, T up to length 8 and tests the definition with a table of factors; the automaton output equals the oracle on 2,000 random strings (length 1 to 60, 2 to 4 letters), golden test G4.

## Length selection and matrices (M3)

- Range: ML-MAWS's table on the integer average length (`docs/OPEN_ISSUES.md`, OI-9). Entropy per length: sum over variable columns of −(p0 log2 p0 + p1 log2 p1), with `libm::log2`, columns in lexicographic order. Selection: ML-MAWS's rule with ties to the shorter length (OI-10).
- The columns of a length come from a k-way merge (binary heap) of the taxa's sorted lists, giving each word with the taxa that have it; the selected lengths are merged in lexicographic word order. A first pass counts the columns, so the memory needed by `M_full` is estimated (rows: taxa × columns ÷ 8 bytes, padded to 4 × 64 bits; per column 13 bytes for word length, code and count) and checked against the memory limit before anything is allocated (decision D3 if it does not fit).
- `M_full`: one bitset row per taxon over all columns; `M_ml`: column indices of `M_full` (constant columns removed, at most 50,000 kept by min(n_j, m − n_j), ties by index), exported in ML-MAWS's PHYLIP layout.
- Memory: the limit is 70% of the available memory when the build starts. The number of parallel extractions is chosen so that the input sequences, the MAW lists of all taxa (estimated at 3 bytes per letter) and one extraction per worker (64 bytes per letter) fit; sequences are released once extracted and lists of lengths that were not selected are released after the selection. Measured on E. coli (29 genomes): peak working set 662 MB with a limit of 1,082 MB.

## Quartets and pattern counts (M4)

- Ranks: a quartet a < b < c < d has rank C(a, 1) + C(b, 2) + C(c, 3) + C(d, 4) (combinatorial number system); unranking takes the largest x with C(x, k) ≤ r for k = 4, 3, 2, 1. Golden test G6: round trip for every quartet with m ≤ 30.
- Processing order: a keyed Feistel network with 4 rounds on 2 × ⌈bits(Q)/2⌉ bits (round function: SplitMix64 finaliser of the right half XOR a round key derived from the seed), with cycle-walking until the value is below Q. It is a bijection of [0, Q) with O(1) memory; golden test G7 checks bijectivity for 11 sizes up to 1,000,000. Position i of the order is the quartet with rank `perm.apply(i)`, so the finished quartets are a seeded random sample at any time.
- Kernel: n(S) is precomputed for single taxa, pairs and triples (flat tables indexed by combinatorial rank; for m = 116 they take 2.1 MB); each quartet needs one 4-way AND-popcount over the bitset rows; the 16 counts follow by the superset Möbius transform (4 × 16 subtractions). Golden test G5: equal to a column scan on 200 random matrices (m from 4 to 12, 1 to 3,000 columns), and the counts of each quartet sum to the number of columns.
- Popcount: `PopcountPath::detect()` chooses the x86-64 `popcnt` instruction when the CPU reports it at run time (`is_x86_feature_detected!`), through a `#[target_feature(enable = "popcnt")]` function; otherwise portable `count_ones` (native on ARM64). Release binaries are not built with `target-cpu=native`. A test checks that both paths agree. AVX2 was not added: the measured throughput (below) is limited by memory traffic for wide matrices, and the optional triple-cache optimisation does not apply to the permuted processing order.

## Analysis run (M4)

- `qmaws run` starts a run of kind `analysis` with the stages `ingest`, `maw_extract` (one checkpointed unit per taxon; the strand filter is applied inside this stage), `length_select`, `matrix_build`, `quartet_count` (chunked), `quartet_weight` (chunked, from M5) and `finalize`. Amalgamation, support, evaluation and figures are added as stages in later milestones.
- Files: `audit/inputs.json` (per taxon: name, source file, original and cleaned length, removed characters, cleaned SHA-256; the input files' fingerprints are also in `run.json`), `work/maws/taxon_<n>.bin`, `work/matrix/selection.json`, `work/matrix/m_full.bin`, `report/m_ml.phy`, `work/chunks/quartet_count_<n>.bin` (one 72-byte record per quartet: rank and 16 counts), `audit/stages.json`, `audit/chunks.json`, `audit/root.txt`. Binary formats: `crates/qmaws-engine/src/store.rs`.
- Chunks: a calibration counts a sample of quartets the same way the chunks are counted; the chunk size for the target duration (default 3 s), capped at 64 MB of records (932,067 quartets), is fixed in `run.json`. Within a chunk, blocks of 4,096 positions are counted in parallel and joined in order, so the bytes do not depend on the number of threads.
- Resume: inputs are verified; stale temporary files are removed; damaged whole-stage outputs reset that stage and the stages after it; damaged MAW files are recomputed and reset everything after extraction; damaged or missing chunks are recomputed. Root fingerprint: SHA-256 over the content hash of every stage (for MAWs and counts over all units in order), so it does not depend on chunk boundaries.

## Teaching worksheet (M4)

`qmaws teach --example` prints the worksheet of the teaching example; `qmaws teach --input <folder> [--reference <tree>]` prints it for a student's own data (at most 8 taxa and 200 letters per sequence). The worksheet computes its steps independently of the production code (two-letter MAW rule, W1 votes, classroom amalgamation, nRF), and golden test G2 checks that the production MAW extraction, matrix and counting kernel give the same matrix and pattern counts. Wording rules: arithmetic is written with `x` and `/`; numbers are printed with up to 10 decimals without trailing zeros; ties in W1 go to the first topology (T1, T2, T3), ties in the amalgamation to the alphabetically first pair. The output is fixed by the golden file `tests/golden/worksheet_example.txt` (golden test G1).

## Quartet weighting (M5)

Code: `crates/qmaws-core/src/weight.rs`. Topologies of a quartet (a, b, c, d) in index order: T1 = ab|cd, T2 = ac|bd, T3 = ad|bc.

- **W1:** score(T) = the counts of T's two split patterns (1100 and 0011 for T1, 1010 and 0101 for T2, 1001 and 0110 for T3); weight = score ÷ total; no weight if the total is 0.
- **Model:** two states, P_ij(t) = π_j + (δ_ij − π_j)·exp(−βt), β = 1 ÷ (2π0π1). W2-sym: π0 = π1 = 0.5. W2-emp: π1 = the fraction of 1s in `M_full` (all taxa, all columns). Off-diagonal entries are computed as π_j·(−expm1(−βt)), so short branches lose no precision. Pattern probabilities: P(x) = Σ_u π_u L_u(x_i, x_j)·G_u(x_k, x_l) with L the product of the two leaf transitions of the first pair and G the internal transition applied to the second pair (32 products per topology).
- **Conditioned log-likelihood:** ℓ_T = Σ over counted x of c(x)·[ln P(x) − ln P(counted)], where P(counted) is the sum of the counted patterns' probabilities (not 1 − P(0000), which would lose precision when P(0000) is close to 1). Counted patterns: all except 0000; for the IQ-TREE cross-check only, a second mode also excludes 1111. Probabilities are clamped at 10⁻³⁰⁰ inside logarithms; patterns with count 0 are skipped.
- **Branch lengths:** each in [0.000001, 10]. Fits store them by quartet position (leaf of a, b, c, d; internal), whatever the topology.
- **Starts:** all 0.05; all 0.3; and lengths derived from distances: each pair's mismatch proportion p over the counted columns gives d = −ln(1 − p ÷ (2π0π1)) ÷ β, and the five lengths follow from the six distances by the four-point formulas (leaf i of pair (i, j) | (k, l): (2d_ij + d_ik + d_il − d_jk − d_jl) ÷ 4; internal: (d_ik + d_il + d_jk + d_jl) ÷ 4 − (d_ij + d_kl) ÷ 2), clamped to the bounds. The best of the three fits is kept; on equal likelihoods the earliest start.
- **Optimiser (the choice asked for in plan 2.7.3):** projected Newton steps with Levenberg–Marquardt damping, falling back to coordinate sweeps, all in the variables s_k = exp(−βt_k).
  - Why s: the transition matrix of a branch is Π + s·(I − Π), so every pattern probability is multilinear in (s_1, …, s_5). The exact gradient and Hessian therefore cost 16 evaluations of the pattern probabilities (branch k's matrix replaced by I − Π for ∂/∂s_k, two matrices replaced for the mixed second derivatives); no finite differences are needed. A test compares them with finite differences. s is a monotone function of ln t, so the box [0.000001, 10] on t is a box on s and the search is a search over log branch lengths, as the plan asks.
  - Newton step: branches at a bound whose gradient points out of the box are held fixed; for the others (−H + λW)·d = g is solved by Cholesky factorisation (W the diagonal of |H|). λ = 0 first (Newton's method, quadratic convergence); if −H is not positive definite or no step length up to 1/2¹¹ increases the likelihood, λ = 10⁻⁶, 10⁻⁵, …, 10⁸. Steps are projected onto the box, and a step whose s reaches the end of the range sets the length exactly to the bound, so the bound is recognised as active.
  - Convergence: the predicted Newton gain ½·gᵀd is at most 10⁻⁹ + 10⁻¹⁴·|ℓ|. If no damped step helps, one coordinate sweep is made: on each branch the exact one-dimensional problem (a sum of logarithms of affine functions of s) is solved by a safeguarded Newton search for the stationary point in the uphill direction. A sweep is kept only if the exactly evaluated likelihood does not decrease. Sweeps stop when the estimated remaining gain of the geometric tail is below the same tolerance. At most 2,000 iterations.
  - Rejected alternatives. Coordinate ascent alone (the first version) converges linearly. On ridges, where a leaf branch and the internal branch trade off, it reached its cap of 5,000 sweeps and stopped up to 0.0000003 log-likelihood units short of the optimum; with Newton steps the six fits examined converged in 11 to 33 iterations. L-BFGS-B would need a library or a long implementation, and its quasi-Newton Hessian brings nothing here, because the exact Hessian is cheap. Brent's method is one-dimensional and serves at most for the sweeps.
  - Tests: a fit is at least as good as every start, and no branch can be changed by ±0.1% to gain more than 10⁻⁷. On expected counts of a known tree the true topology wins and its lengths are recovered within 0.02. All three starts reach the same optimum for the true topology. Relabelling the counts relabels the fits.
- **W2a:** the best topology and ℓ*_best − ℓ*_second. **W2b:** exp(ℓ*_T − max) ÷ Σ exp(ℓ*_T′ − max).
- **W2c:**
  - B = 100 resamples (`--replicates`, default 100 as pre-registered; 0 skips W2c).
  - Each resample draws the counted pattern counts from Multinomial(N, c ÷ N) as conditional binomials in pattern order, then refits all three topologies from the three starts. The weight of T is the fraction of resamples it wins.
  - Ties: log-likelihoods within 10⁻⁸ of the best are tied and share the resample equally. This tolerance is far above the optimiser's precision, so ties are not decided by rounding.
  - Binomial draws: exact inversion searching outward from the mode, with the mode's probability from `libm::lgamma` and its neighbours by the ratio of successive probabilities. This is simple, exact up to rounding, and costs O(√(npq)) steps.
  - Generator: SplitMix64 (Steele, Lea and Flood 2014), written out in the code (a few lines, no crate), seeded per quartet with the first 8 bytes, little-endian, of SHA-256(global seed as 8 bytes little-endian ‖ rank as 8 bytes little-endian). The global seed is the run's `--seed`.
  - The draws therefore depend only on the seed, the rank and the counts, not on the thread or the chunk. Reference values of SplitMix64 and of the seeds are fixed in tests.
- **Determinism:** every exp, ln, log1p, expm1 and lgamma comes from the `libm` crate; sqrt is the IEEE operation (correctly rounded); no `mul_add`; all sums run in a fixed order; each quartet is computed sequentially on one thread.
- **Analysis stage `quartet_weight`:**
  - The stage comes after `quartet_count` and is chunked with its own calibrated plan (its rate is about 10,000 times lower). Each chunk recomputes its pattern counts from `M_full` (microseconds per quartet, against milliseconds for the weights) instead of reading the count chunks.
  - Records: 60 bytes per quartet (rank, flags, three log-likelihoods, three W2c weights; `store.rs`). W1 and W2b follow from the counts and the log-likelihoods.
  - Hashing: the stage's content hash is taken over a text form of the records with every floating-point value rounded to 9 significant digits (`{:.8e}`). `audit/chunks.json` lists each chunk's raw and rounded SHA-256.
  - `--weighting w2-sym` (default), `w2-emp` or `none` (no weighting stage).

## To be written

- wQFM-rs algorithm details with references to the wQFM paper (M7)
