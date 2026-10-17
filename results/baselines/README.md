# Peer-reviewed alignment-free tools (Amendment 5)

Mash 2.3, CD-MAWS (suffix-automaton implementation `cd-maws-sa`, commit `8bfe7d5`) and Peafowl (commit `89ed477`), run by this project with `scripts/baselines.sh` on GitHub's Linux runners, on the nine AFproject datasets with a reference tree and the 20 H5 datasets (`docs/PREREGISTRATION.md`, Amendment 5). Descriptive comparison only; no hypothesis. Run times measured on the runners are recorded in each `provenance.txt` and not compared with anything.

| Path | Content |
|---|---|
| `<tool>/<dataset>/` | `tree.nwk`, `comparison.tsv` (`qmaws tree-compare` against the reference or species tree), `provenance.txt` (status, versions and settings, Peafowl's chosen k, runner), the tool's messages; Mash and CD-MAWS also their distance matrix |
| `values.tsv` | nRF and nQD of every tool on every dataset, with its status (made by `qmaws baselines`) |
| `paired.tsv` | Q-MAWS minus each tool: counts of lower/equal/higher, median difference, Hodges–Lehmann estimate and the exact one-sided Wilcoxon p-values in both directions, as descriptive numbers (made by `qmaws baselines`; the H5 rows appear when H5 is complete) |
| `mash/` | Mash 2.3: one folder per dataset |
| `cdmaws/` | CD-MAWS (suffix-automaton implementation, `cd-maws-sa`): one folder per dataset |
| `peafowl/` | Peafowl: one folder per dataset |
| `values.tsv.sha256` | SHA-256 of `values.tsv` |
| `paired.tsv.sha256` | SHA-256 of `paired.tsv` |

Where each folder comes from (workflow `baselines.yml`, results branches `baselines-results-<run id>`):

- Mash and Peafowl on the AFproject datasets: run 38546099791.
- CD-MAWS on the AFproject datasets: run 38564176984 (the first two runs could not build it: its source asks for AVX2 through `#pragma GCC target`, which g++ 13 accepts only with the same options on the command line).
- All three tools on the 20 H5 datasets: run 38570100252, with the laptop's H5 data from the branch `h5-data`, each file checked against its SHA-256 in `data/manifests/simulated/hgt.tsv` (AliSim on Linux does not give the same data from the same seeds).

All 87 tool–dataset pairs finished with status `ok`. Peafowl reproduces the values reported in its paper (for example Fish mtDNA nRF 0.045, reported 0.05), and CD-MAWS reproduces the E. coli/Shigella value that the ML-MAWS paper reports for it (0.577).
