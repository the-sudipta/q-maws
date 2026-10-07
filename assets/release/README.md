# release

## Purpose
Files that the release workflow puts into the ready-to-run bundles of Q-MAWS.

## Contents
| Item | Description |
|---|---|
| `BUNDLE_README.md` | The `README.md` of every bundle: how to start the window and the terminal mode, where results go, and the first start on each system |
| `Info.plist` | The macOS app description of `Q-MAWS.app`; `VERSION` is replaced by the release version |
| `q-maws.desktop` | Linux desktop entry, for users who install the program in their menu |

## Relationships
Used by `.github/workflows/release.yml` with `run.bat`, `run.sh`, `LICENSE`, `THIRD_PARTY_NOTICES`, `CITATION.cff` and the icons in `assets/icon/`.

## Notes
The bundles are not signed: the bundle README explains how to open them the first time on Windows and macOS.
