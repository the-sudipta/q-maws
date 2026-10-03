# M10 handoff (work in progress)

Status on 2026-10-03: **M10 in progress on the branch `m10`**. This note lets the work continue in a new session. It is not a milestone report; it is replaced by `M10.md` when M10 is complete, and should then be deleted.

## M10 from the plan

**Tasks** (Part 11, M10; sections 4.7, 4.8): Quartet Halo Tree, tanglegram, rectangular tree, SVG, PDF, PNG, interactive HTML, growth animation, convergence report; group bands from all three sources.

**Acceptance:** all figure types produced for Fish mtDNA and E. coli–Shigella; text readable at 116 taxa (Rhinovirus); HTML works offline in a browser; colour-blind safe palettes verified with a simulation check.

## Done (in the working tree of `m10`; see `git log` for what is committed)

- `qmaws-core::metrics`: nQD (four-point condition, unresolved reference quartets counted apart) and raw MSD (Hungarian); tests against brute force.
- `qmaws-viz`: `colour.rs` (palettes and the colour-blind check: all pass, smallest group difference 10.9 under tritanopia), extended `tree.rs` (support colours and widths, group bands, title block), `rect.rs` (rectangular tree, tanglegram, untangling with all child rotations), `html.rs` (interactive page), `anim.rs` (GIF, `gif` 0.14.2 added), `groups.rs` (group file, automatic clades), convergence chart in `chart.rs`.
- `qmaws-data::otl`: Open Tree of Life match_names and taxon_info (API verified live, see `docs/EXTERNAL_TOOLS.md`).
- `qmaws-engine::figures`: `auto_options` (benchmark reference tree and name table, `groups.tsv` in the input folder) and `render` (all figures, `figures/groups.tsv`, `report/reference_comparison.tsv`, `audit/otl_taxonomy.json` with `--otl`); run READMEs describe the new files; engine test `figures_of_a_finished_run_leave_the_root_unchanged`.
- Figures are drawn after every finished run (CLI `run`/`resume`, menu, GUI controller); `qmaws figures` (`--reference`, `--groups`, `--otl`, `--s2`); hidden `qmaws figure-check --dataset rhinovirus --output <dir>` (random tree, readability check).
- GUI: buttons "Open the Halo Tree" and "Open the interactive tree" after a run; `Controller::spawn` takes the data folder.
- Docs: README "Figures", USER_GUIDE "Figures", DESIGN "Figures (M10)", DEPENDENCIES (gif, 380 crates), THIRD_PARTY_NOTICES regenerated, EXTERNAL_TOOLS (Open Tree of Life), OPEN_ISSUES OI-15 (MSD normalisation), OI-16 (which taxa the tanglegram marks), OI-17 (when the Open Tree of Life is asked), CHANGELOG, crate READMEs.

## Checks so far

- Fish mtDNA figures (a finished Fish run in a scratch folder, `qmaws figures --otl`): Open Tree of Life 3.7, 23 of 25 matched, family rank, 5 groups; nRF 0.455 (20 of 44), nQD 0.371 (4,690 of 12,650), MSD 51; tanglegram crossings 51 before the rotation search, 22 after.
- 116 taxa (Rhinovirus names, random tree): labels stay at 11 px in a 1,296-pixel figure.
- HTML: opened in a browser (Fish and the 116-taxon page): drawing, search (10 and 3 matches), zoom, layout switch, S2 colours and tooltips work; no outside resources were requested.

## Next

1. E. coli–Shigella run `results/runs/ecoli_2026-10-03_080534` (default settings, started 08:05 UTC with the M9 binary): when finished, `qmaws figures --output results/runs/ecoli_2026-10-03_080534` (automatic groups; strain names do not match the Open Tree of Life).
2. A new Fish mtDNA run with the M10 binary (`qmaws run --dataset fish_mito`, live tree on), then `qmaws figures --otl` on it.
3. Commit both run folders (without `work/`), list them in `results/runs/README.md`.
4. Full `cargo test --workspace -j 3`, clippy, fmt, deny, README check; commit in the CONTRIBUTING.md format; push `m10`.
5. Write `docs/milestones/M10.md` (format of `M09.md`), delete this note, ask the owner about OI-15, OI-16 and OI-17, and stop for approval.
EOF
sed -i 's/| `M09.md` | M9: GUI, terminal menu, live provisional tree, interface switching |/| `M09.md` | M9: GUI, terminal menu, live provisional tree, interface switching |\n| `M10_HANDOFF.md` | M10 work in progress: what is done and what is next (replaced by `M10.md`) |/' docs/milestones/README.md && grep -n M10 docs/milestones/README.md