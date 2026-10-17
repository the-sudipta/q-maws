# M13: scaling and H2

Hypothesis H2 (`docs/PREREGISTRATION.md`, Amendments 3, 6 and 7): on the same computer, Q-MAWS (primary configuration, seed 1, S1 only) is faster than ML-MAWS (IQ-TREE with 1,000 UFBoot) in every replicate with m ≥ 100. Data: AliSim genomes of 1,000,000 letters on random Yule–Harding trees (`scripts/simulate_scaling.sh`, manifest `data/manifests/simulated/scaling.tsv`). Computer: the development laptop (Intel Core 5 210H, 12 logical cores, 7.6 GB RAM, Windows 11), one run at a time, both programs kept out of Windows' efficiency mode (Q-MAWS by its own call, ML-MAWS and IQ-TREE by `unthrottle.ps1`), 2026-10-14 to 2026-10-15.

| Path | Content |
|---|---|
| `<dataset>/qmaws/` | Q-MAWS run (the large `work/` folder is not committed) |
| `<dataset>/ml_maws/` | ML-MAWS tree, IQ-TREE log and report, ML-MAWS messages (its matrix and IQ-TREE work files are not kept) |
| `times.tsv` | Every attempt of either program, as written by the M13 queue and the ML-MAWS completion script of Amendment 6 |
| `memory.tsv` | Peak working set of each program (descriptive), from the memory recorders beside the queue |
| `h2_values.tsv`, `h2_test.txt`, `scaling.svg` | The evaluation, made by `qmaws h2` |
| `scaling_m100_r1/` | Dataset scaling_m100_r1: Q-MAWS run and ML-MAWS run |
| `scaling_m100_r2/` | Dataset scaling_m100_r2: Q-MAWS run and ML-MAWS run |
| `scaling_m100_r3/` | Dataset scaling_m100_r3: Q-MAWS run and ML-MAWS run |
| `scaling_m25_r1/` | Dataset scaling_m25_r1: Q-MAWS run and ML-MAWS run |
| `scaling_m25_r2/` | Dataset scaling_m25_r2: Q-MAWS run and ML-MAWS run |
| `scaling_m25_r3/` | Dataset scaling_m25_r3: Q-MAWS run and ML-MAWS run |
| `scaling_m50_r1/` | Dataset scaling_m50_r1: Q-MAWS run and ML-MAWS run |
| `scaling_m50_r2/` | Dataset scaling_m50_r2: Q-MAWS run and ML-MAWS run |
| `scaling_m50_r3/` | Dataset scaling_m50_r3: Q-MAWS run and ML-MAWS run |
| `h2_values.tsv.sha256` | SHA-256 of `h2_values.tsv` |
| `h2_test.txt.sha256` | SHA-256 of `h2_test.txt` |
| `scaling.svg.sha256` | SHA-256 of `scaling.svg` |

## Result

| m | Q-MAWS (s) | ML-MAWS (s) | Faster |
|---|---|---|---|
| 25 | 43, 45, 44 | 1,383, 1,210, 1,322 | Q-MAWS, about 30× |
| 50 | 587, 586, 597 | 7,038, 3,143, 3,027 | Q-MAWS, about 5–12× |
| 100 | 11,616, 10,133, 13,218 | 1,751, 5,150, 1,513 | **ML-MAWS, about 2–9×** |

**H2 is not supported:** ML-MAWS was faster in all three replicates with m = 100. No crossover in favour of Q-MAWS exists: Q-MAWS is faster at m = 25 and 50, but slower at m = 100. Q-MAWS's time grows roughly with the number of quartets (about m⁴: ×13 from 25 to 50 taxa and ×20 from 50 to 100), while ML-MAWS works on at most 50,000 matrix columns whatever the number of taxa.

Peak memory at m = 100: Q-MAWS 752–772 MB; ML-MAWS about 2.2–3.0 GB (its MAW steps; IQ-TREE itself estimated 599 MB in its logs).

## Notes on the runs

- **ML-MAWS above 50 taxa (Amendment 6):** as distributed, ML-MAWS switches to a fast mode above 50 species that adds `-fast` to `-bb 1000`; IQ-TREE 2.4.0 refuses this, and ML-MAWS stops without a tree while exiting with status 0. The queue's rows for m = 100 with notes `new` (185–212 s) are these failed attempts and are not used. The times used are those of the rows `amendment 6`: the ML-MAWS run plus the IQ-TREE command of its normal mode on the matrix it made (`D:\_external\queues\m13_mlmaws_complete.sh`).
- **200 taxa (Amendment 7):** not run. The amalgamation of about 192 million weighted quartets would need about 12 GB of memory, more than the laptop has. The input was set aside, so both programs stopped at once on it; those rows (exit 1) are not used.
- **Resumed runs:** two Q-MAWS runs with m = 100 (r1, r3) were stopped by laptop shutdowns and resumed from their saved chunks. An attempt ended by a shutdown writes no row, so their time is the engine's working time in `run.json`, which covers every attempt (`qmaws h2` takes the larger of this and the summed rows).
- **Timing variability:** ML-MAWS's times vary between replicates of the same size (for example 7,038 s against about 3,100 s at m = 50; 1,513 to 5,150 s at m = 100); the IQ-TREE logs show different search lengths. During the ML-MAWS run of `scaling_m25_r2` light single-thread work (the H5 analysis and a git commit) ran on the laptop for about 25 minutes. Neither changes the H2 outcome, which is decided at m = 100.
- **Memory recorder:** the first recorder missed the Q-MAWS rows whose end coincided with the next start; the corrected recorder ran from 2026-10-14 23:15 UTC, so Q-MAWS peaks for m = 25 and three of the m = 50 runs are missing (`NA`). The ML-MAWS peaks for m = 100 in `memory.tsv` were recorded during its failed fast-mode attempts (steps 1 to 4, which make the matrix; IQ-TREE is not included).
