# milestones

## Purpose
One report per completed milestone: what was built, how it was verified, measured numbers, and known limitations.

## Contents
| Item | Description |
|---|---|
| `M00.md` | M0: foundation and pre-registration |
| `M01.md` | M1: engine skeleton — checkpoints, progress, terminal display, basic CI |
| `M02.md` | M2: input and data acquisition |
| `M03.md` | M3: MAW extraction, strand filter, length selection, matrices |
| `M04.md` | M4: quartet pattern counting and teaching mode |
| `M05.md` | M5: quartet weighting |
| `M06.md` | M6: long-branch simulation (H3) |
| `M07.md` | M7: amalgamation (wQFM-rs) |
| `M08.md` | M8: support, audit, verification, controls |
| `M09.md` | M9: GUI, terminal menu, live provisional tree, interface switching |
| `M10.md` | M10: figures, group bands, nQD and MSD, colour-blind check |
| `M11.md` | M11: full benchmark (14 datasets × 5 seeds), comparison with published results, H1 (not supported); approved |
| `M12.md` | M12: calibration (H4, not supported), sensitivity (exploratory) and seed variability; approved |
| `M13.md` | M13: scaling (H2, not supported: faster up to 50 taxa, slower at 100; 200 taxa left out); approved |
| `M14.md` | M14: release readiness (window, batch view, run lock, full CI, release workflow, user guide, version 1.0.0); v1.0.0 published |

## Relationships
Reports refer to code in `crates/`, scripts in `scripts/`, and run records in `results/`. Each report corresponds to a milestone tag.

## Notes
File names are `M00.md` to `M14.md`. Every number in a report comes from a logged run on the stated machine.
