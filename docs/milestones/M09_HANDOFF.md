# M9 handoff (work in progress)

Status on 2026-10-03: **M9 in progress on the branch `m9`** (pushed to `origin/m9`). This note lets the work continue in a new session. It is not a milestone report; it is replaced by `M09.md` when M9 is complete, and should then be deleted.

## M9 from the plan (`Q-MAWS_PLANNING_PHASE.md`)

**Tasks** (Part 11, M9; sections 4.7, 4.10, 5.2, 5.5): complete GUI (4.10) implementing the full menu (5.2), live worksheet panel, pause, stop, resume, interface switching; provisional live tree pipeline (4.7) with frames; G10.

**Acceptance:** G10 passes; a run started in the terminal resumes in the GUI and vice versa with identical root fingerprint; live provisional tree updates within the overhead budget.

## Owner decisions for M9 (2026-10-03, decision cards in the project thread)

1. **GUI license policy:** use `eframe`/`egui` with its built-in fonts turned off (crate `epaint_default_fonts`, licensed OFL-1.1 and Ubuntu-font-1.0, is not used); load a system font at runtime, so no font files ship with Q-MAWS. Add **BSL-1.0** to the allowed licenses in `deny.toml` (needed by `clipboard-win` and `error-code`, which `eframe` always pulls in through its clipboard support). Do **not** allow OFL-1.1 or Ubuntu-font-1.0.
2. **G10 input:** the 16-taxon simulated control input in `results/controls/inputs/` (the worksheet example's 6-letter sequences are below the 100-letter minimum of analysis runs).
3. **Live figure scope:** a basic provisional figure in M9 (circular cladogram, halo ring, PROVISIONAL watermark); group bands, support colours and other polish in M10.

## Done (commits on `m9`, oldest first)

| Commit | What |
|---|---|
| `73a41a7` | `qmaws-viz`: `tree.rs` (circular cladogram layout with midpoint display root, changed edges between trees, basic provisional Halo Tree SVG with halo ring, legend and watermark) and `render.rs` (PNG with `resvg` 0.45.1, PDF with `svg2pdf` 0.13.0, system fonts). Dependencies and notices updated |
| `6c92fa7` | Engine: `ProgressSink::pause_requested` (pause between units of work, not counted as working time; stop works during a pause); events `Quartet` (live worksheet) and `Provisional`; `provisional.rs` (live provisional tree every 5% or 3 min, 10% overhead budget doubling both intervals, `figures/live/halo_tree_latest.svg/.png/.pdf`, frames in `work/provisional/frames/`, state in `work/provisional/state.json`, `report/convergence.csv`); `qmaws run --no-live-tree` |
| `e676d87` | Engine: `launch.rs` (settings, new run request, run scan with percentage and last interface, matching unfinished run, user config file with extra results folders and the resume queue, `QMAWS_CONFIG_DIR` for tests); `analysis::config_sha256`, `analysis::percent_complete`; `AnalysisOptions.cores` (session thread pool) |
| `85cd0ae` | `qmaws-tui`: `menu.rs`, the terminal main menu of plan 5.2 behind a `Prompter` trait (`dialoguer` in the terminal, a script in tests) returning a `MenuAction`; `dialoguer` 0.12 added |

Tests at the last full run: 236 pass, 4 ignored (with the menu commit).

## Next

1. **CLI wiring** (`crates/qmaws-cli/src/main.rs`): commands `menu` (loop: `qmaws_tui::menu::main_menu`, then carry out the `MenuAction`; after a single resumed run, `menu::ask_next_run`), `gui`, flags `--gui`/`--terminal` on `run` and `resume` (default terminal), `resume --all` (queue saved in `UserConfig.queue`, removed entry by entry as runs finish), `verify --quick` (alias of the default). Download callback for the menu: `data_cmd::download`. Remember results folders with `UserConfig::remember_run`.
2. **GUI** (`crates/qmaws-gui`):
   - `Cargo.toml`: `eframe = { version = "0.36.2", default-features = false, features = ["accesskit", "glow", "wayland", "x11"] }` plus `qmaws-core`, `qmaws-engine`, `qmaws-viz`. Then `deny.toml`: add `"BSL-1.0"` to `allow`, with a comment naming `clipboard-win` and `error-code`. Update `docs/DEPENDENCIES.md` (policy table: BSL-1.0 allowed; the decision; crate rows) and regenerate `THIRD_PARTY_NOTICES` (the generator used so far reads `cargo tree -e normal,build --target <each release target>` and `cargo metadata`, keeps existing entries verbatim and sorts names with `_` read as `-`; it reproduced the committed file exactly).
   - eframe 0.36 API: `App::ui(&mut self, ui: &mut egui::Ui, frame)` (and optional `logic`); panels are `egui::Panel::left/right/top/bottom(id).show_inside(ui, ...)`; `egui::Scene` for zoom and pan; `eframe::run_native(name, NativeOptions, Box::new(|cc| Ok(Box::new(app))))`.
   - Fonts: with default fonts off egui has none, so load one at startup: `resvg::usvg::fontdb::Database::load_system_fonts`, query `Family::SansSerif` and `Family::Monospace`, copy the face data into `egui::FontData` (keep the face index for .ttc files) and set both families. No emoji or icon glyphs are available: use plain text on buttons.
   - Headless `controller.rs` (no egui): runs the engine in a worker thread with a sink that sends `Event`s through an `mpsc` channel and answers `pause_requested` from an `Arc<AtomicBool>`; stop via the cancel flag; runs a list of new runs or a resume queue; reports the `Outcome`. G10 and the interface-switching tests use it.
   - `app.rs`: left the steps Data → Reference → Output → Settings → Review → Run (same questions as the terminal menu, using `launch`); centre the live Halo Tree drawn from `qmaws_viz::tree::TreeLayout` with changed edges highlighted (`changed_edges`), the final tree afterwards and buttons to open the figures folder; right the live worksheet (taxa, 16 counts, W1, W2 log-likelihoods, weights) and the stage log; bottom overall and stage progress, elapsed, remaining, Pause/Continue, Stop. On launch, list unfinished runs and offer resume. Verify screen with the four types.
3. **G10** (`crates/qmaws-cli/tests/`): the 16-taxon control input from `results/controls/inputs/`: a run through `qmaws run --terminal` and a run through the GUI controller give the same root; a run started in the terminal (stopped) and resumed through the controller, and the reverse, give the same root. Check the folder name of the control input first.
4. **Measure** the live tree overhead on Fish mtDNA (release build): log lines "Provisional tree n: x% of quartets, in s" and any doubling; record in the report.
5. **Docs:** README ("Graphical interface", menu), `docs/USER_GUIDE.md` (menu, GUI, pause, live tree, `--no-live-tree`, `--gui`/`--terminal`, `resume --all`), `docs/DESIGN.md` (provisional tree, pause, launch, controller), crate READMEs, `CHANGELOG.md` (Unreleased), `docs/milestones/M09.md` (report in the format of `M08.md`), then delete this note.

## Design notes and gotchas

- Provisional files are outside every stage's outputs, so the root fingerprint is unchanged (tested); they depend on timing.
- With tiny chunks every update exceeds the 10% budget, so intervals double quickly; tests assert the number of doublings from the log, not a fixed number of frames.
- `Event::Quartet` is boxed (clippy `large_enum_variant`); JSON progress includes the new events; the terminal display ignores them.
- Pause is polled every 100 ms; the paused time is subtracted from `elapsed_seconds`.
- `cores` and `memory_limit` apply to one session and are not stored in `run.json`; a resumed run uses all cores.
- The engine refuses inputs with warnings, so the menu shows each warning and offers only "Choose another folder" or back. Reference trees are checked for matching names but not used yet (comparison comes with M11); the Open Tree of Life download is not implemented (the menu says so).
- The repository's commit hook rejects attribution trailers in commit messages; keep the CONTRIBUTING.md message format.
- `git` warns that LF will become CRLF in the working copy: harmless (`core.autocrlf`).
- A command chain like `cargo test ... | grep` hides the test exit code; check `test result` lines for failures.

## M8 approval and push: done

The owner approved M8 on 2026-10-03. `5bd0032` (`📝 : Record approval of milestone M8`) is on `main`, tag `v0.8-m08-support` points to `b75c2da`; both are pushed. `main` was merged into `m9`, and `m9` is pushed to `origin/m9`.
