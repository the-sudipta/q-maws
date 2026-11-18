# config

## Purpose
The two inputs of the cloud benchmark that are data, not code.

## Contents
| Item | Description |
|---|---|
| `methods.tsv` | Every method: kind of run, default setting, tuning grid (Amendment 8b) and a first time estimate used only to pack jobs |
| `run.json` | The run request: `mode` (`smoke` or `full`), `waves` (1 to 12) and `request` (a counter: increase it to start again). A push that changes it starts `benchmark_cloud.yml` |

## Relationships
Read by `scripts/cloud/catalog.py` and `.github/workflows/benchmark_cloud.yml`.

## Notes
The settings and grids in `methods.tsv` are those of Amendment 8b and must not change; the estimates may, since they only order and pack the work.
