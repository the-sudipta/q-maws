# src (qmaws-viz)

## Purpose
Source code of the figure crate. Modules receive data and return figure text (SVG) or bytes (PNG, PDF); none reads or writes files.

## Contents
| Item | Description |
|---|---|
| `lib.rs` | Crate root: module list |
| `anim.rs` | The growth animation: PNG frames to a looping GIF (`gif` crate, NeuQuant colours) |
| `chart.rs` | Small-multiple line charts as SVG: logarithmic x axis, reference lines, colour-blind safe colours with line styles and markers, legend; the convergence chart of the provisional trees |
| `colour.rs` | Colour scales of the figures (support, halo values, groups) and the colour-blind check: protanopia, deuteranopia and tritanopia simulated (Machado et al. 2009), CIEDE2000 differences |
| `groups.rs` | Group bands from a group file, and automatic groups by cutting the tree into clades |
| `html.rs` | The self-contained interactive tree page: data as JSON and inline JavaScript; zoom, pan, search, tooltips, circular or rectangular layout |
| `icon.rs` | The program icon: the embedded logo SVG, PNG and RGBA renderings, and the Windows `.ico` and macOS `.icns` containers |
| `rect.rs` | Rectangular tree with support values, halo squares and groups; the tanglegram with greedy untangling and marked taxa |
| `render.rs` | SVG to PNG (`resvg`) and SVG to PDF (`svg2pdf`), with the fonts installed on the computer; `system_font` gives the GUI an installed font |
| `tree.rs` | Circular cladogram layout (midpoint display root, leaves equally spaced, internal angles as means), changed edges between two trees, and the Quartet Halo Tree as SVG: branches (coloured and widened by support when given), label ring, halo ring on a diverging colour-blind safe scale with a symbol for values below 0.6, group bands, title block, legends, watermark for provisional trees |

## Relationships
Used by `qmaws-cli` (the H3 figure), `qmaws-engine` (the live provisional tree) and `qmaws-gui` (the tree layout of the live panel). The final tree figures of M10 are added here.

## Notes
Group bands, support colours, the tanglegram, the interactive HTML tree and the growth animation come with M10.
