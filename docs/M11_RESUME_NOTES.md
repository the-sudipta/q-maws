# M11 resume notes

Working notes for resuming milestone M11 (full benchmark) after an interruption. Branch `m11-complete` (from `main` after PR #1). Removed when M11 is complete.

## Done
- Published baselines, H1 statistics, `qmaws summary` (merged in PR #1).
- Seed-1 runs: `yersinia_hgt_seed1`, `sim_hgt_0_seed1` (PR #1); Fish mtDNA and E. coli seed 1 are the M10 runs.

## In progress
- 2026-10-05: milestones M0 to M10 were audited against the plan and the gaps found were completed (commits 097e83f to 0a02de7; `docs/OPEN_ISSUES.md` OI-19, CHANGELOG). The queue was restarted at 17:12 UTC with the same binary (commit 7b1de8a), whose computation the fixes do not change. After each finished run, `qmaws figures` of the current program adds the comparison records (OI-19).
- Benchmark queue (one run at a time, restartable): finished runs are skipped, unfinished ones resumed.
  Order: seed 1 of the 7 HGT datasets (S2 100) → seeds 2–5 of the HGT datasets (`--bootstrap 0`) → Fish mtDNA and E. coli seeds 2–5 → influenza A, coronavirus, mammal mtDNA, ebolavirus seed 1 (S2 100) and seeds 2–5 → rhinovirus seeds 1–5 (`--bootstrap 0`).
- Until 2026-10-07 11:51 UTC every run used one binary, built from commit 7b1de8a (`qmaws 0.0.0`). From then on, with the owner's approval, the queues use a build of commit 5f0cd87 (branch `m11-perf`): 7b1de8a plus one change to how a chunk of quartets is shared between threads (see "Binary change" below). The computation is the same, so the results are the same.

## Next command
The queues run on the owner's laptop, outside the repository, in `D:\_external\queues`: `m11_queue.sh`, then `m12_queue.sh`, `h5_queue.sh` and `m13_queue.sh`. Each one waits for "all done" in the log of the one before it. The logs (`*_queue.log`) are in the same folder. The script `start_all.ps1` starts every queue that is not already running, each in a hidden window (`launch_hidden.ps1`, WMI `Win32_Process Create`), so a queue never stops when the shell that launched it ends. The scheduled task `QMAWS-Queues` runs that script at every logon. To restart by hand after a sleep or a reboot:

```
powershell -File D:\_external\queues\start_all.ps1
```

The M11 queue runs: `qmaws --quiet run --dataset <id> --seed <n> --bootstrap <100|0> --output results/runs/<id>_seed<n>`. Unfinished runs are resumed.

After each finished run: `qmaws verify --quick results/runs/<id>_seed<n>`, add a row to `results/runs/README.md`, commit, push.
After the 9 AFproject seed-1 runs: `qmaws summary --afproject baselines/afproject_submission`.
After all seeds: `qmaws summary --h1-pairs mean --h1-zeros wilcoxon`, then fill `docs/milestones/M11.md`, README results section, CHANGELOG.

## AFproject files
The upload files of the nine AFproject datasets (seed-1 trees) are in `baselines/afproject_submission/` (2026-10-07, `qmaws summary --afproject`; each tree is byte-identical to its run's `report/tree.nwk`, and each root in the README agrees with the run's `audit/root.txt`). Waiting for the owner's manual upload; the scores AFproject returns are then recorded there with the access date.

## Second worker
From 2026-10-06 16:22 UTC, at the owner's request to use the idle cores, a second worker (`m11b_queue.sh`, same binary) runs the NCBI seeds 2–5 and rhinovirus seeds 1–5 (`--bootstrap 0`) in the reverse of the main order, at the same time as the main queue. It never runs the four NCBI seed-1 runs and starts nothing once the main queue has reached the NCBI runs, so the two never work on the same run. Results do not depend on this; the working times of runs made while both workers run are not comparable with single runs (no timing claim is made from M11).

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
| `sim_hgt_0_seed2` | done, committed (2,635 s, on battery in Silent mode) |
| `sim_hgt_250_seed2` | done, committed (2,693 s, on battery in Silent mode) |
| `sim_hgt_500_seed2` | done, committed (resumed 10:38 UTC after a restart of the laptop; finished 11:14 UTC) |
| `sim_hgt_750_seed2` | done, committed (stopped by a shutdown of the laptop at about 11:20 UTC; resumed 12:35 UTC; finished 12:44 UTC) |
| `sim_hgt_1000_seed2` | done, committed (on charger; seed 2 of all 7 HGT datasets done) |
| `yersinia_hgt_seed3` | done, committed (22 s) |
| `ecoli_shigella_hgt_seed3` | done, committed (22,978 s, while the second worker ran beside it) |
| `sim_hgt_0_seed3` | done, committed (6,289 s, beside the second worker) |
| `sim_hgt_250_seed3` | done, committed (6,229 s after its resume, beside the second worker) |
| `sim_hgt_500_seed3` | done, committed (10,444 s, beside the second worker) |
| `sim_hgt_750_seed3` | done, committed (resumed after a laptop shutdown; 1,888 s after the resume, beside the second worker) |
| `sim_hgt_1000_seed3` | done, committed (resumed with the 5f0cd87 build at 11:51 UTC; 3,029 s after that resume; seed 3 of all 7 HGT datasets done) |
| `yersinia_hgt_seed4` | done, committed (159 s, 5f0cd87 build) |
| `ecoli_shigella_hgt_seed4` | done, committed (resumed after a laptop shutdown; 11,825 s after the resume, 5f0cd87 build) |
| `sim_hgt_0_seed4` | done, committed (8,829 s, 5f0cd87 build) |
| `sim_hgt_250_seed4` | done, committed (resumed after a laptop shutdown; 1,301 s after the resume, 5f0cd87 build) |
| `sim_hgt_500_seed4` | done, committed (6,701 s, 5f0cd87 build) |
| `sim_hgt_750_seed4` | done, committed (resumed after a sudden laptop stop; 4,149 s after the resume, 5f0cd87 build) |
| `sim_hgt_1000_seed4` | done, committed (8,016 s, 5f0cd87 build; seed 4 of all 7 HGT datasets done) |
| `yersinia_hgt_seed5` | done, committed (201 s, 5f0cd87 build) |
| `ecoli_shigella_hgt_seed5` | done, committed (resumed after a laptop shutdown; 960 s after the last resume, 5f0cd87 build) |
| `sim_hgt_0_seed5` | done, committed (4,191 s, 5f0cd87 build) |
| `sim_hgt_250_seed5` | done, committed (7,837 s, 5f0cd87 build) |
| `sim_hgt_500_seed5` | done, committed (6,906 s, 5f0cd87 build) |
| `sim_hgt_750_seed5` | done, committed (6,439 s, 5f0cd87 build) |
| `rhinovirus_seed5` | running in the second worker (started 2026-10-06 16:22 UTC) |

## Binary change (2026-10-07)
The weighing stage splits each chunk of quartets into blocks of 16 that run in parallel. The calibrated chunks of M11 are small (21 to 25 quartets for the simulated HGT datasets, 82 for rhinovirus), so only 2 to 6 of the 12 threads had work and the laptop ran at about 35% CPU. Commit 5f0cd87 (on 7b1de8a, branch `m11-perf`) changes only `run_blocks` in `crates/qmaws-engine/src/analysis.rs`: a chunk is cut into at least one block per thread. Records are still joined in position order, so their bytes do not depend on the blocks.

Checks before the switch, all on this laptop:
- Root fingerprints of the 7b1de8a build and the 5f0cd87 build are identical: Yersinia HGT seed 3 without S2 (`8502148d...`), Fish mtDNA seed 3 without S2 (`9f7445aa...`), and Fish mtDNA with 21-quartet chunks (`9f7445aa...`).
- A Fish mtDNA run killed under the 7b1de8a build (452 weighing chunks done) and resumed with the 5f0cd87 build gave the same root (`9f7445aa...`).
- The engine tests of 5f0cd87 pass (77).
- Speed: Fish mtDNA with 21-quartet chunks took 698 s with the 7b1de8a build and 125 s with the 5f0cd87 build (both while the M11 queues were running).

The switch: at 11:51 UTC both queues and their runs were stopped, `qmaws_m11.exe` was renamed `qmaws_m11_7b1de8a.exe` (kept), the 5f0cd87 build was copied to `qmaws_m11.exe`, and both queues were started again; `sim_hgt_1000_seed3` and `rhinovirus_seed5` resumed from their saved chunks. Each run records the build in `audit/environment.json` (`git_commit`): runs finished before the switch show 7b1de8a; `sim_hgt_1000_seed3`, `rhinovirus_seed5` and every later run show 5f0cd87.

## Windows power throttling (2026-10-08)
Windows 11 runs a program without a visible window (as the queues are started, hidden) in "efficiency mode": on the slow cores at a low clock speed. Measured on this laptop with the same build (5f0cd87 with timing output) and the same Fish mtDNA weighing in chunks of 82 quartets: 0.42 s per chunk from a console, 4.92 s launched hidden, 0.35 s launched hidden with a fix that asks Windows not to throttle the program (`SetProcessInformation` with `ProcessPowerThrottling`, in `crates/qmaws-cli/src/main.rs`; no change to the computation). On the rhinovirus data the hidden run needed 3.22 s per chunk against 0.33 s from a console. The M11 runs so far ran throttled; their results do not depend on it.

Builds with the fix, each its earlier commit plus only this change, and each giving the same root fingerprints as the build it replaces (Yersinia HGT seed 3 without S2, `8502148d...`; Fish mtDNA seed 3 without S2, `9f7445aa...` for M11):
- M11: commit 5943bcc (branch `m11-perf`, on 5f0cd87), `qmaws_m11_nothrottle.exe`; switching the running M11 queues waits for the owner's approval.
- M12 and M13: commit b8782e2 (branch `m12-run`, on 07b3a8a), now `qmaws_m12.exe` (the earlier build kept as `qmaws_m12_07b3a8a.exe`).
- H5: commit 826812e (branch `h5-run`, on 9c42cb9), now `qmaws_h5.exe` (kept: `qmaws_h5_9c42cb9.exe`).
The M12, H5 and M13 queues had not started any run when their binaries were replaced. ML-MAWS and IQ-TREE (H5, M13) are not Q-MAWS programs and stay throttled when run hidden; M13 (H2, timing) needs a setup without throttling for both programs before it starts.
