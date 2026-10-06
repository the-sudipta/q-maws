# M11 resume notes

Working notes for resuming milestone M11 (full benchmark) after an interruption. Branch `m11-complete` (from `main` after PR #1). Removed when M11 is complete.

## Done
- Published baselines, H1 statistics, `qmaws summary` (merged in PR #1).
- Seed-1 runs: `yersinia_hgt_seed1`, `sim_hgt_0_seed1` (PR #1); Fish mtDNA and E. coli seed 1 are the M10 runs.

## In progress
- 2026-10-05: milestones M0 to M10 were audited against the plan and the gaps found were completed (commits 097e83f to 0a02de7; `docs/OPEN_ISSUES.md` OI-19, CHANGELOG). The queue was restarted at 17:12 UTC with the same binary (commit 7b1de8a), whose computation the fixes do not change. After each finished run, `qmaws figures` of the current program adds the comparison records (OI-19).
- Benchmark queue (one run at a time, restartable): finished runs are skipped, unfinished ones resumed.
  Order: seed 1 of the 7 HGT datasets (S2 100) → seeds 2–5 of the HGT datasets (`--bootstrap 0`) → Fish mtDNA and E. coli seeds 2–5 → influenza A, coronavirus, mammal mtDNA, ebolavirus seed 1 (S2 100) and seeds 2–5 → rhinovirus seeds 1–5 (`--bootstrap 0`).
- All runs use one binary, built from commit 7b1de8a (`qmaws 0.0.0`), so every run of M11 has the same code.

## Next command
The queues run on the owner's laptop, outside the repository, in `D:\_external\queues`: `m11_queue.sh`, then `m12_queue.sh`, `h5_queue.sh` and `m13_queue.sh`. Each one waits for "all done" in the log of the one before it. The logs (`*_queue.log`) are in the same folder. The script `start_all.ps1` starts every queue that is not already running, each in a hidden window (`launch_hidden.ps1`, WMI `Win32_Process Create`), so a queue never stops when the shell that launched it ends. The scheduled task `QMAWS-Queues` runs that script at every logon. To restart by hand after a sleep or a reboot:

```
powershell -File D:\_external\queues\start_all.ps1
```

The M11 queue runs: `qmaws --quiet run --dataset <id> --seed <n> --bootstrap <100|0> --output results/runs/<id>_seed<n>`. Unfinished runs are resumed.

After each finished run: `qmaws verify --quick results/runs/<id>_seed<n>`, add a row to `results/runs/README.md`, commit, push.
After the 9 AFproject seed-1 runs: `qmaws summary --afproject baselines/afproject_submission`.
After all seeds: `qmaws summary --h1-pairs mean --h1-zeros wilcoxon`, then fill `docs/milestones/M11.md`, README results section, CHANGELOG.

## Run status
| Run | Status |
|---|---|
| `ecoli_shigella_hgt_seed1` | done, committed (5,026 s) |
| `sim_hgt_250_seed1` | done, committed (finished 2026-10-05 03:51 UTC after two interruptions) |
| `sim_hgt_500_seed1` | done, committed (15,202 s; finished 2026-10-05 08:05 UTC) |
| `sim_hgt_750_seed1` | done, committed (stopped twice, at 09:33 UTC when the launching session ended and at 18:40 UTC when its console window closed; resumed from its checkpoints each time; finished 2026-10-05 20:32 UTC) |
| `sim_hgt_1000_seed1` | done, committed (stopped when the laptop slept and when it restarted; resumed from its checkpoints at 2026-10-06 03:31 and 03:40 UTC; finished 2026-10-06 04:44 UTC; nRF 0.567, nQD 0.423) |
| `yersinia_hgt_seed2` | done, committed (51 s; seeds 2–5 use `--bootstrap 0`) |
| `ecoli_shigella_hgt_seed2` | done, committed (14,329 s: laptop on battery in its "Silent" power mode, throughput 2.6–7 times lower than seed 1; results unaffected) |
| `sim_hgt_0_seed2` | running (started 2026-10-06 08:44 UTC) |
