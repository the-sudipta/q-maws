# site

## Purpose
The project website, published on GitHub Pages at https://the-sudipta.github.io/q-maws/ by the workflow `.github/workflows/pages.yml`. Plain HTML, CSS and JavaScript; it explains the project simply and shows the results, comparisons, figures and the proof of a real run.

## Contents
| Item | Description |
|---|---|
| `index.html` | The page: overview, the plan, milestones and amendments, the workflow diagram, results H1–H5, comparisons, figures, proof, downloads |
| `assets/` | Stylesheet and scripts of the page ([README](assets/README.md)) |

## Relationships
Every number comes from `data/site.json`, and the figures from `figures/`; both are written by `scripts/site_data.py` from `results/` and `figures/paper/` when the workflow builds the website, and are not committed. To view the page locally: `python scripts/site_data.py`, then `python -m http.server --directory site` and open http://localhost:8000.

## Notes
Libraries are loaded at fixed versions from jsDelivr (three.js, GSAP, Chart.js, medium-zoom) and fonts from Google Fonts. Icons by Icons8 (https://icons8.com), credited on the page.
