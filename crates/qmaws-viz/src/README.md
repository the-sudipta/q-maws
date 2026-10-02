# src (qmaws-viz)

## Purpose
Source code of the figure crate. Modules receive data and return figure text (SVG); none reads files.

## Contents
| Item | Description |
|---|---|
| `lib.rs` | Crate root: module list |
| `chart.rs` | Small-multiple line charts as SVG: logarithmic x axis, reference lines, colour-blind safe colours with line styles and markers, legend |

## Relationships
Used by `qmaws-cli` (the H3 figure). The tree figures of M10 are added here.

## Notes
PDF and PNG output (via `svg2pdf` and `resvg`) come with M10.
