# M11 resume notes

Working notes for resuming milestone M11 (full benchmark) after an interruption. Branch `m11-complete` (from `main` after PR #1). Removed when M11 is complete.

## Done
- Published baselines, H1 statistics, `qmaws summary` (merged in PR #1).
- Seed-1 runs: `yersinia_hgt_seed1`, `sim_hgt_0_seed1` (PR #1); Fish mtDNA and E. coli seed 1 are the M10 runs.

## In progress
- Benchmark queue (one run at a time, restartable): finished runs are skipped, unfinished ones resumed.
  Order: seed 1 of the 7 HGT datasets (S2 100) → seeds 2–5 of the HGT datasets (`--bootstrap 0`) → Fish mtDNA and E. coli seeds 2–5 → influenza A, coronavirus, mammal mtDNA, ebolavirus seed 1 (S2 100) and seeds 2–5 → rhinovirus seeds 1–5 (`--bootstrap 0`).
- All runs use one binary, built from commit 7b1de8a (`qmaws 0.0.0`), so every run of M11 has the same code.
- `sim_hgt_250_seed1` was interrupted on 2026-10-04 at S2 replicate 52 of 100; the queue resumes it.

## Next command
Restart the queue if it stopped (the queue script lives in the local scratchpad, not in the repository):

```
bash queue.sh   # runs: qmaws --quiet run --dataset <id> --seed <n> --bootstrap <100|0> --output results/runs/<id>_seed<n>
```

After each finished run: `qmaws verify --quick results/runs/<id>_seed<n>`, add a row to `results/runs/README.md`, commit, push.
After the 9 AFproject seed-1 runs: `qmaws summary --afproject baselines/afproject_submission`.
After all seeds: `qmaws summary --h1-pairs mean --h1-zeros wilcoxon`, then fill `docs/milestones/M11.md`, README results section, CHANGELOG.

## Run status
| Run | Status |
|---|---|
| `ecoli_shigella_hgt_seed1` | running (started 2026-10-04 17:29 UTC) |
| `sim_hgt_250_seed1` | interrupted, queued for resume |
