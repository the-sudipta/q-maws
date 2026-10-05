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
