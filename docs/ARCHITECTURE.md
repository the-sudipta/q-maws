# Architecture: where everything is

A guide for readers and reviewers who want to find the method, the program parts and the evidence in this repository. File paths are relative to the repository root.

## 1. The method, step by step, in the code

All of the science is in one crate, `crates/qmaws-core/`. It reads no files and draws no windows: it receives data and returns data, so every step can be tested on its own.

| Step of the method | File | Main functions |
|---|---|---|
| Read and clean the genomes (keep A, C, G, T; report every removal) | `crates/qmaws-core/src/input.rs` | reading and validation functions |
| Find minimal absent words with a suffix automaton; strand filter | `crates/qmaws-core/src/maw.rs` | `extract`, `extract_strand_aware`, `reverse_complement` |
| Choose the MAW lengths by entropy; build the 0/1 matrix (`M_full`, and `M_ml` as in ML-MAWS) | `crates/qmaws-core/src/matrix.rs` | `column_entropy`, `select_lengths`, `build_full`, `ml_columns` |
| Number the quartets, visit them in a seeded order, count the 16 patterns by inclusion–exclusion | `crates/qmaws-core/src/quartet.rs` | `rank`, `unrank`, `pattern_counts` (checked against `pattern_counts_brute`) |
| Weigh the three quartet trees: votes (W1) and the conditioned two-state likelihood (W2a, W2b, W2c; star rule) | `crates/qmaws-core/src/weight.rs` | `w1`, `pattern_probabilities`, `log_likelihood`, `fit`, `is_star`, `w2a`, `w2b`, `w2c` |
| Combine the weighted quartets into one tree (wQFM-rs) | `crates/qmaws-core/src/amalgamate.rs` | `wqfm` |
| Support S1 and halo values | `crates/qmaws-core/src/support.rs` | — |
| Support S2 (column bootstrap with Poisson(1) weights) | `crates/qmaws-core/src/bootstrap.rs` | `replicate_seed` and the weighted counts |
| Tree comparison: nRF, nQD, MSD | `crates/qmaws-core/src/metrics.rs`, `newick.rs` | — |
| Calibration of support (ECE) | `crates/qmaws-core/src/calibration.rs` | — |
| Statistics of the hypotheses (exact Wilcoxon, Holm, Hodges–Lehmann) | `crates/qmaws-core/src/stats.rs` | `signed_rank_less`, `holm`, `median`, `hodges_lehmann` |
| Simulation for H3 and gene trees with transfer for H5 | `crates/qmaws-core/src/sim.rs`, `hgt.rs` | — |

The mathematics behind these steps is written out in [METHOD.md](METHOD.md).

## 2. The program parts

The program is one Rust workspace (`Cargo.toml`) with seven crates in `crates/`:

| Part | Folder | What it does |
|---|---|---|
| **Science** | `crates/qmaws-core/` | The method (section 1) |
| **Run engine** | `crates/qmaws-engine/` | Runs the method as stages with checkpoints, resume, the audit record, the root fingerprint and verification (`analysis.rs`, `runner.rs`, `verify.rs`, `store.rs`, `hash.rs`) |
| **Data** | `crates/qmaws-data/` | Benchmark registry, downloads with checksums, NCBI and Open Tree of Life clients, reference trees |
| **Figures** | `crates/qmaws-viz/` | Quartet Halo Tree, rectangular tree, tanglegram, interactive HTML tree, animation, charts |
| **GUI (window)** | `crates/qmaws-gui/` | The desktop window: menu, new-run steps, live Halo Tree, live worksheet, results library (`app.rs`, `controller.rs`, `figure_view.rs`) |
| **Terminal menu** | `crates/qmaws-tui/` | The interactive menu and progress display in the terminal (`menu.rs`) |
| **CLI (command-line program)** | `crates/qmaws-cli/` | The `qmaws` program: every command (`main.rs`; one `*_cmd.rs` file per command), started from a terminal, the menu or the window |

How they connect: the CLI calls the engine; the engine calls the science, the data and the figure crates; the GUI and the terminal menu are two front ends to the same engine, so a run started in one can be resumed in the other with the same result.

## 3. Checks that run automatically (CI)

| What | Where |
|---|---|
| Continuous integration on Windows, macOS and Linux: formatting, lints, all tests, the cross-platform fingerprint test G12, dependency licences, folder READMEs | `.github/workflows/ci.yml` |
| Building and publishing the release bundles | `.github/workflows/release.yml` |
| Development experiments that need other programs (ML-MAWS, IQ-TREE, wQFM, Mash, CD-MAWS, Peafowl, DendroPy) | the other files in `.github/workflows/` with their scripts in `scripts/` |
| Unit tests | next to the code, in each `crates/*/src/*.rs` (`#[cfg(test)]` modules) |
| Golden tests and test inputs | `tests/` |

## 4. The evidence

| What | Where |
|---|---|
| Hypotheses, tests and criteria, fixed before the experiments, with seven dated amendments | `docs/PREREGISTRATION.md` |
| One report per milestone (what was built, how it was checked, decisions) | `docs/milestones/` |
| Every run with its complete record (inputs, settings, hashes, root fingerprint, verification report) | `results/runs/`, `results/h5/`, `results/scaling/`, `results/sensitivity/`, `results/controls/` |
| Summary tables and hypothesis tests | `results/summary/`, `results/h3/`, `results/h4/`, `results/h5/`, `results/scaling/`, `results/baselines/` |
| Published values of other methods, with sources | `baselines/` |
| Figures of the results | `figures/paper/` |
| Dataset manifests, accession lists, reference trees (sequences are downloaded, not stored) | `data/` |

## 5. Commands for users and for development

Commands for users: `qmaws gui`, `qmaws menu`, `qmaws run`, `qmaws resume`, `qmaws verify`, `qmaws figures`, `qmaws datasets`, `qmaws download`, `qmaws inspect`, `qmaws matrix`, `qmaws teach`, `qmaws simulate-h3` (details in [USER_GUIDE.md](USER_GUIDE.md)).

Hidden development commands (they rebuild the evaluation from the run folders and are listed in `crates/qmaws-cli/src/README.md`): `summary` (M11 tables and H1), `variant` and `calibration` (M12, H4), `h5`, `h2`, `baselines`, `tree-compare`, `controls`, `monitor`, and the checks against other programs (`wqfm-export` and `wqfm-compare`, `iqtree-export` and `iqtree-compare`, `metrics-check`). `simulate-h3` (the H3 experiment) and `matrix` are visible commands.
