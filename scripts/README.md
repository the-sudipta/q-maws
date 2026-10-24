# scripts

## Purpose
Development scripts that protect the repository: the pre-commit guard, the installers that put it into place as git hooks, a self-test for the guard, and the README coverage check.

## Contents
| Item | Description |
|---|---|
| `check-readmes.sh` | Checks that every folder has a README listing every item in it |
| `equivalence.sh` | Development check: builds ML-MAWS at a fixed commit (needs g++ with OpenMP), runs it and `qmaws matrix` on Fish mtDNA and Yersinia HGT, and compares the matrices byte for byte and the entropy tables |
| `iqtree_check.sh` | Development check (Linux x86-64): downloads IQ-TREE 3.1.4 and checks its SHA-256, exports 5 quartets of Fish mtDNA with `qmaws iqtree-export`, fits them in IQ-TREE and compares the log-likelihoods with `qmaws iqtree-compare` (tolerance 0.0001) |
| `wqfm_check.sh` | Development check (needs Java): checks out wQFM v1.4 at a fixed commit, writes 23 weighted-quartet inputs with `qmaws wqfm-export` (worksheet, simulated, Fish mtDNA), runs the jar on each and compares with `qmaws wqfm-compare` |
| `metrics_check.py` | Development check, golden test G9 (needs Python and DendroPy): reads `pairs.tsv` from `qmaws metrics-check` and recomputes every pair's Robinson–Foulds distance with DendroPy, reading the trees as unrooted and dividing by 2(n − 3) as ML-MAWS does |
| `mlmaws_h4.sh` | Development experiment for H4 (Linux; needs g++, git, curl): builds ML-MAWS at the inspected commit and IQ-TREE 2.4.0 outside the repository, runs `ml-maws --strand --iqtree` (1,000 UFBoot) on one simulated HGT dataset, and writes the tree with support, the IQ-TREE log (seed), the reports and `provenance.txt` (docs/PREREGISTRATION.md, Amendment 3) |
| `check_hashes.py` | Checks every `.sha256` file under the given folders (default `results/`) against the file it belongs to; Windows, macOS and Linux, Python 3 standard library only |
| `baselines.sh` | Development experiment of Amendment 5 (Linux; needs g++, git, curl, python3 with Biopython; Peafowl also Jellyfish and RAxML): runs Mash 2.3, CD-MAWS (cd-maws-sa) or Peafowl at pinned versions on one dataset (the H5 data made again and checked by SHA-256), and writes the tree, `comparison.tsv` from `qmaws tree-compare` and `provenance.txt` with the status, also when the tool fails |
| `nj_tree.py` | Neighbour-joining tree (Biopython) from the distances of Mash or CD-MAWS, for `baselines.sh` |
| `paper_figures.py` | Draws the result charts in `figures/paper/` from the committed result tables (Python 3 standard library only) |
| `site_data.py` | Writes the data of the website (`site/data/site.json`) and copies its figures from the committed results; run by `pages.yml` |
| `simulate_scaling.sh` | The simulated scaling datasets of H2 (plan 6.5): AliSim (IQ-TREE 2.4.0) genomes of 1,000,000 letters on random Yule–Harding trees, 25/50/100 taxa × 3 replicates and 200 taxa × 1, fixed seeds; writes the manifest and true trees to `data/manifests/simulated/` |
| `simulate_hgt.sh` | The simulated HGT datasets of H5 (Amendment 4): AliSim species trees (30 taxa), gene trees with transfers from `qmaws hgt-trees`, genomes of 1,000 genes from AliSim topology-unlinked partitions, h = 0, 0.1, 0.2, 0.4 × 5 replicates, fixed seeds; writes the manifest, species trees and transfer tables to `data/manifests/simulated/` |
| `simulate_benchmark.sh` | The new simulated datasets of the extended benchmark (Amendment 8), in the H5 design: divergence (mean branch length 0.005, 0.02, 0.05), number of species (15, 60), 6 tuning datasets and fragmented assemblies of the H5 datasets with h = 0.2; fixed seeds from 9000; writes `data/manifests/simulated/benchmark.tsv`, species trees and transfer tables |
| `bench_tools.sh` | Runs one alignment-free method of the extended benchmark inside Ubuntu (WSL) with its default or tuned setting and writes its tree (Amendment 8b) |
| `benchmark_queue.sh` | The restartable job queue of the extended benchmark (Amendments 8 and 8a): verifies the reused runs with release 1.0.0, then runs Q-MAWS, ML-MAWS and the IQ-TREE reference; skips finished jobs, records wall time and peak memory in `results/benchmark/runs.tsv` |
| `benchmark_queue_b.sh` | Phase B of the benchmark queue (Amendment 8b): tuning on the 6 tuning datasets, then the 12 Linux methods on the 59 evaluation datasets (default and tuned) and the 9 speed datasets; restartable; wall time and peak memory by GNU time |
| `bench_tune_choose.py` | Picks each method's tuned setting: lowest median nRF on the tuning datasets, ties keep the default |
| `start_benchmark.bat` | Double-click to start or continue `benchmark_queue.sh` with Git Bash; closing the window pauses the queue |
| `stop_benchmark.bat` | Double-click to pause the benchmark queue (stops the queue and the program it runs; nothing is lost) |
| `benchmark_status.bat` | Double-click to open the live dashboard (`benchmark_dashboard.ps1`) |
| `benchmark_dashboard.ps1` | Live dashboard of the benchmark: state, current job with its expected end, progress of phases A and B, time left and the expected end of everything (local time, 12-hour clock), jobs per day, failures, processor, memory, disk, power and the last steps; redraws in place, Q closes it |
| `mem_peak.ps1` | Samples the summed working set of the named processes every second and writes the peak in MB; used by `benchmark_queue.sh` |
| `fragment_genomes.py` | Cuts each genome of a multi-FASTA file into contigs of 2,000 to 18,000 letters (seeded) and writes one multi-record file per taxon; used by `simulate_benchmark.sh` |
| `install-hooks.bat` | Installs the guard as the `pre-commit` and `commit-msg` hooks (Windows) |
| `install-hooks.sh` | Installs the guard as the `pre-commit` and `commit-msg` hooks (macOS, Linux, Git Bash) |
| `pre-commit` | The guard: blocks unsafe commits (see Notes) |
| `test-guard.sh` | Self-test: runs the guard in a temporary repository against blocked and allowed cases |

## Relationships
The installers copy `pre-commit` into the repository's hooks folder (`git rev-parse --git-path hooks`). `test-guard.sh` and `check-readmes.sh` are run before commits and will run in continuous integration (`.github/workflows/`).

## Notes
- **Install once per clone:** `sh scripts/install-hooks.sh`, or `scripts\install-hooks.bat` on Windows. Git for Windows runs the hooks with its bundled shell. Re-run after `pre-commit` changes.
- **The guard blocks a commit if:** a staged file is larger than 50 MB; a staged path is under `data/raw/` or inside any `work/` folder; a staged path is listed in the local, uncommitted `.git/info/exclude` file; staged content or a path matches a secret pattern (private key header, common access-token formats, a quoted password or key in a config line); staged content, a path, or the commit message contains a forbidden attribution string (attribution trailers and the names of code-generation tools).
- **Commit messages:** the same script runs as the `commit-msg` hook; with the message file as its argument it checks only the message.
- **Fragments:** the forbidden strings and secret markers are assembled from fragments at run time, so the scripts never contain them literally and do not block themselves.
- **Run the self-test:** `sh scripts/test-guard.sh` (on Windows, in Git Bash). It never touches the real repository. It writes a 50 MB file in a temporary folder and takes about 20 seconds.
- **Run the README check:** `sh scripts/check-readmes.sh`. Folders under a crate's `src/` that contain a single file need no README. `.github/` is described by `.github/ABOUT.md` instead, because GitHub shows `.github/README.md` in place of the root README.
- Shell scripts use LF line endings and `.bat` files CRLF (`.gitattributes`). Paths containing a newline character are not supported by the guard.
