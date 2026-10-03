# src (qmaws-viz)

## Purpose
Source code of the figure crate. Modules receive data and return figure text (SVG) or bytes (PNG, PDF); none reads or writes files.

## Contents
| Item | Description |
|---|---|
| `lib.rs` | Crate root: module list |
| `chart.rs` | Small-multiple line charts as SVG: logarithmic x axis, reference lines, colour-blind safe colours with line styles and markers, legend |
| `render.rs` | SVG to PNG (`resvg`) and SVG to PDF (`svg2pdf`), with the fonts installed on the computer; `system_font` gives the GUI an installed font |
| `tree.rs` | Circular cladogram layout (midpoint display root, leaves equally spaced, internal angles as means), changed edges between two trees, and the basic provisional Halo Tree as SVG: branches, label ring, halo ring on a diverging colour-blind safe scale with a symbol for values below 0.6, legend, watermark |

## Relationships
Used by `qmaws-cli` (the H3 figure), `qmaws-engine` (the live provisional tree) and `qmaws-gui` (the tree layout of the live panel). The final tree figures of M10 are added here.

## Notes
Group bands, support colours, the tanglegram, the interactive HTML tree and the growth animation come with M10.
