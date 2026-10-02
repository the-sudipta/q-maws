# External tools

External tools, services and repositories used during development or by the program. For each one: what it is used for, the version, and what was verified (documentation read, license, options), with the date.

## Development toolchain

| Tool | Version | Used for | Verified |
|---|---|---|---|
| Rust (`rustc`, `cargo`) | 1.98.1 | Building and testing the workspace | Version from `rustc --version` and `cargo --version` on 2026-10-02; standard library license from `share/doc/rust/COPYRIGHT-library.html` in the toolchain |
| rustfmt, clippy | shipped with Rust 1.98.1 | Formatting and lint checks | Run on 2026-10-02 |
| Git | 2.56.0 (Git for Windows) | Version control; hooks run with its bundled shell on Windows | Version from `git --version` on 2026-10-02 |

## Continuous integration (GitHub Actions)

| Tool | Version | Used for | Verified |
|---|---|---|---|
| `actions/checkout` | `v7` (newest major tag) | Checking out the repository in each CI job | Tags listed with `git ls-remote --tags https://github.com/actions/checkout` on 2026-10-02; usage `uses: actions/checkout@v7` from its README |
| `EmbarkStudios/cargo-deny-action` | `v2` (newest major tag) | Running `cargo deny check licenses bans sources` with `deny.toml` | Tags listed with `git ls-remote` on 2026-10-02; inputs `rust-version`, `command`, `command-arguments` read from its `README.md` and `action.yml` at tag `v2`. It is a Docker action, so it runs on the Linux runner only; its built-in Rust is older, so `rust-version` is set to the pinned 1.98.1 |
| `deny.toml` format | cargo-deny configuration | License, ban and source policy | Section and key names taken from `deny.template.toml` in the cargo-deny repository (branch `main`) on 2026-10-02. The first CI run failed with `error[wildcard]` on the path dependencies between workspace crates; reproduced locally with cargo-deny 0.20.2 (installed with `cargo install cargo-deny --locked`) and fixed with `allow-wildcard-paths = true`, which then gave `bans ok, licenses ok, sources ok` |
| `rustup toolchain install` | rustup 1.29.1 | Installing the toolchain pinned in `rust-toolchain.toml` on each runner | `rustup toolchain install --help` (rustup 1.29.1): with no argument it installs "the active toolchain", which is the one named in `rust-toolchain.toml` |

## Repositories, services and data sources

### AFproject (https://afproject.org)

- **Used for:** five benchmark archives (nine datasets) and their reference trees.
- **Verified on 2026-10-02:** each dataset page (`/app/benchmark/genome/.../dataset/`) links the archive as `/media/genome/.../dataset/<file>.zip` and states its size and MD5; the links in `data/manifests/benchmarks.toml` were taken from these pages, and every download matched the published MD5. The E. coli (29) and E. coli/Shigella HGT archives have published MD5 values (`de88729e76a47c1de7f06a6c59298cb8`, `e4282d59f4dae2fd6e4914cb747e5566`). The archives contain no reference trees; each results page (`/app/benchmark/genome/.../results/`) embeds the reference tree as a `data:text/plain;charset=utf-8,` link (see `data/references/SOURCE.md`). The server supports HTTP range requests (`Accept-Ranges: bytes`), used to resume downloads; an interrupted 114,629,704-byte download resumed at byte 12,304,384 and verified.
- **Reference package:** `AF-reference_datasets190511.zip`, 4,894,058,545 bytes, last modified 2019-08-09 (HTTP headers); not downloaded. The results archive `AF-results190511.zip` is 12,422,678,494 bytes; not downloaded.
- **Citation:** A. Zielezinski et al., Genome Biology 20:144 (2019).

### ML-MAWS repository (https://github.com/PapriSaha/ML-MAWS)

- **License:** Apache License 2.0 (`LICENSE.txt`, read before any file was used).
- **Cloned** on 2026-10-02 to `../_external/ML-MAWS` (outside this repository) at commit `0c38db12d9ad271aafcb4940d7558dfcd00925c1` (2026-05-09).
- **Used for:** the five NCBI dataset files in `Data/` (downloaded by `qmaws download` from `raw.githubusercontent.com` at that commit; each download matched the SHA-256 of the file at that commit), and the name tables in `Data/*/reference/dataset.json` (see `data/references/SOURCE.md`). No code is used.
- **Read:** `README.md`; `FastaReader.cpp` (input reading: keeps A, C, G, T after upper-casing, with no conversion of U to T; reads `.fasta`, `.fa`, `.fna`, `.fas` from a folder in sorted order; a single-record file takes the header's first word as name, a multi-record file is joined and named after the file; a single multi-FASTA file is read as one taxon per record); `main.cpp` (input may be a folder or a file); `run_all_benchmarks.sh` (the five NCBI datasets are single multi-FASTA files). In M3 also read: `SuffixAutomaton.h` and `.cpp` (online construction; skips letters other than A, C, G, T), `MAWExtractor.cpp` (depth-first enumeration over factors, the suffix-link test, lexicographic sort, strand filter by intersecting with the MAWs of the reverse complement), `EntropySelector.cpp` (entropy per length, top-3 selection, adaptive range table; see OI-9, OI-10), `MatrixBuilder.cpp` (union, constant-column removal, 50,000-column cap by min(n, m − n), PHYLIP layout) and `main.cpp` (order of steps; strand filter only with `--strand`; matrix capped before PHYLIP export). The repository has no Makefile, although the README says `make`; with current GCC it compiles only if `<cstdint>` is supplied (`-include cstdint`), because `EntropySelector.h` uses `uint8_t` without including it (equivalence runs of 2026-10-02).

### NCBI E-utilities (https://eutils.ncbi.nlm.nih.gov/entrez/eutils/)

- **Used for:** building the five NCBI datasets with `efetch.fcgi?db=nuccore&rettype=fasta&retmode=text&id=<list>&tool=qmaws&email=<contact>`; 50 accessions per request, at least 0.4 s between requests.
- **Usage rules verified on 2026-10-02:** without an API key, at most 3 requests per second (NCBI Insights, "New API keys for the E-utilities", 2017-11-02, and NLM Support article KA-05317); an API key raises the limit to 10 per second and is not needed for our few requests. The E-utilities book page (https://www.ncbi.nlm.nih.gov/books/NBK25497/) answered automated and browser requests with a reCAPTCHA check; it was not bypassed, and the rules were read on the two pages above. The `tool` and `email` parameters follow the project specification; the email is the owner's contact address (decision D10).
- **Behaviour observed:** a request with unversioned accessions returns the current versions; requests with versioned accessions return exactly those versions. Each fetch of the five datasets took 8 requests.

### Li, He, He and Yau (2017), Scientific Reports 7:12226

- **Used for:** the accession lists of the NCBI datasets (Supplementary Tables S1 to S5). Article and supplement are open access under CC BY 4.0 (license stated in the article page metadata).
- **Read:** the article text (dataset descriptions; influenza A data are segment 6, neuraminidase) and the supplementary PDF (1,808,034 bytes, SHA-256 `d4c5b48e...945b`), converted with `pdftotext -raw` (Xpdf 4.06, installed in the MSYS2 UCRT64 environment on the development laptop). Details: `data/manifests/accessions/README.md`.

### GitHub raw file service

- `https://raw.githubusercontent.com/<owner>/<repository>/<commit>/<path>` serves a file at a fixed commit. Verified on 2026-10-02 by downloading the five ML-MAWS data files and comparing their SHA-256 with `git show <commit>:<path>` in the clone: all identical.

### cargo-deny

- Version 0.20.2, installed locally with `cargo install cargo-deny --locked` on 2026-10-02 (development tool only). Used to reproduce the CI dependency check.

### CD-MAWS suffix automaton implementation (https://github.com/TamimEhsan/cd-maws-sa)

- License: MIT (GitHub repository metadata and `LICENSE`, checked on 2026-10-02, HEAD `8bfe7d5`). Its code was not read: Q-MAWS reproduces ML-MAWS, whose extractor states it is based on this work, and the published description.

### IQ-TREE (https://github.com/iqtree/iqtree3)

- **Used for:** the development-only cross-check of the conditioned quartet log-likelihood (plan 2.7.7, item 4), run by `.github/workflows/iqtree.yml` and `scripts/iqtree_check.sh` on a GitHub Linux runner. The owner chose GitHub Actions on 2026-10-02; nothing is installed on the development laptop, and IQ-TREE is not part of Q-MAWS or its releases.
- **Version:** 3.1.4 (latest release, published 2026-09-10), asset `iqtree-3.1.4-Linux-intel.tar.gz`, 7,479,748 bytes. The script checks the SHA-256 `d422cb2b8f04825faea753afda25c60de6611537d07ec8fcd0033e70cb042839`, which is the digest the GitHub releases API lists for that asset (read on 2026-10-02).
- **License:** GNU General Public License v2.0 (repository license, read through the GitHub API on 2026-10-02). The program is only run; no code is used or distributed.
- **Documentation read** (2026-10-02; www.iqtree.org had an expired certificate, so the same pages were read at iqtree.github.io/doc):
  - Substitution Models: binary models JC2 (Jukes–Cantor type) and GTR2 (unequal state frequencies); frequency options +F, +FQ, +FO and user-defined +F{…}; +ASC "will correct the likelihood conditioned on variable sites" and is for alignments without constant sites.
  - Command Reference: `-te` (fixed user tree; no tree search); `-st BIN`; `-blmin` (default: the smaller of 0.000001 and 0.1 ÷ alignment length); `-blmax` (default 10); `-blfix` (fix the branch lengths of the tree given with `-te`); `-me` (log-likelihood epsilon of the final estimation, default 0.01); `-seed`; `-nt`; `-redo`; `-pre`.
- **Matching settings:**
  - Models: W2-sym ↔ `JC2+ASC`; W2-emp ↔ `GTR2+F{π0,π1}+ASC`. With two states, GTR2 has a single exchangeability; normalised to one expected change per unit length, it is the Q-MAWS model.
  - Conditioning: +ASC conditions on "not 0000 and not 1111", so Q-MAWS uses its cross-check mode, and the exported alignments contain only the non-constant columns.
  - Bounds: `-blmin 0.000001 -blmax 10`, as in Q-MAWS. `-me 0.000001` for the optimised fits.
  - Topology: each topology is given as an unrooted Newick tree with `-te`.
- **Comparisons:** for 5 quartets of Fish mtDNA drawn with seed 1, both models and all three topologies (30 rows):
  - (A) IQ-TREE's log-likelihood at Q-MAWS's fitted lengths (`-blfix`) against Q-MAWS's maximum;
  - (B) IQ-TREE's maximised log-likelihood against Q-MAWS's maximum, and Q-MAWS evaluated at IQ-TREE's fitted lengths against IQ-TREE's maximum;
  - (C, added after the first run) IQ-TREE's optimisation started from Q-MAWS's fitted lengths (`-te` with lengths, without `-blfix`) against Q-MAWS's maximum.
  - The script passes when A, B and C agree within the tolerance and IQ-TREE never finds a higher maximum than Q-MAWS. Rows where IQ-TREE's own maximum (from its default starting lengths) is lower are counted and reported.
  - Tolerance 0.0001. IQ-TREE prints log-likelihoods with 4 decimals, so rounding alone accounts for up to 0.00005.
- **First run** (workflow run on commit `6f35d7b`, 2026-10-02):
  - (A) and the second part of (B) agree in all 30 rows within 0.00005, i.e. within IQ-TREE's print precision: the two likelihood functions are the same.
  - IQ-TREE's own maximum was never higher than Q-MAWS's (at most 0.000048 above, within rounding). In 12 of 30 rows it was lower by more than 0.0001, by up to 0.270115 (quartet 4, W2-emp, ab|cd). IQ-TREE confirmed with `-blfix` that Q-MAWS's lengths give the higher value, so its optimisation from the default starting lengths stopped below the optimum in these rows. 11 of the 12 are fits of a topology that is not the best for its quartet; the other is quartet 4, W2-emp, ac|bd (0.000196 lower).
  - Comparison C was added after this run to check that IQ-TREE, started from Q-MAWS's lengths, stays at Q-MAWS's maximum.

### Not used yet

wQFM and Open Tree of Life are added when each is first consulted, including its license.
