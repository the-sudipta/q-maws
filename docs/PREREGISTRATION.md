# Pre-registration

This document fixes the hypotheses, data, statistical tests and success criteria of the Q-MAWS study **before any experiment is run**. It is committed to the repository before the first experiment; its commit date is the registration date. Changes after that date are made only in new, dated sections and never alter the original text.

## Primary method configuration

Confirmed by the project owner on 2026-10-02 (decision D1).

| Component | Choice |
|---|---|
| Matrix | `M_full` (all columns of the MAW union; nothing removed) |
| Quartet weighting | W2c: conditioned quartet maximum likelihood, 100 multinomial resamples of the 15 non-0000 pattern counts, weight = fraction of replicates in which the topology wins (ties split equally) |
| Substitution model | W2-sym (two-state symmetric model, stationary frequencies 0.5 and 0.5); W2-emp is an ablation |
| Amalgamation | `wQFM-rs` |
| Support | S1 (quartet support per internal edge); S2 (column bootstrap) only if feasible (decision D7) |

## Hypotheses

| ID | Hypothesis | Data | Test | Success criterion |
|---|---|---|---|---|
| **H1** | Under horizontal gene transfer (HGT), Q-MAWS has lower topological error than ML-MAWS | 7 HGT datasets: simulated HGT = 0, 250, 500, 750, 1000; E. coli/Shigella HGT; Yersinia HGT | One-sided exact Wilcoxon signed-rank test on paired differences (Q-MAWS minus ML-MAWS strand-aware, reported values), for nRF and nQD; Holm correction across the 2 metrics | Adjusted p < 0.05 for at least one metric, and median difference < 0. The Hodges–Lehmann estimate and every per-dataset value are also reported |
| **H2** | Q-MAWS is faster than ML-MAWS for many taxa | Simulated scaling sets, m = 25, 50, 100, 200, 3 replicates each | Same-device wall-clock comparison (requires decision D8) | Q-MAWS total time lower than ML-MAWS (IQ-TREE with 1,000 UFBoot) in all replicates for m ≥ 100. The crossover m is reported |
| **H3** | Naive informative-pattern voting is misled under long-branch conditions; conditioned quartet maximum likelihood is not | Built-in 4-taxon simulation: true tree ab\|cd, two-state symmetric model, leaf branches of a and c t_long ∈ {0.5, 1.0, 1.5}, other branches 0.05; N ∈ {100, 1,000, 10,000, 100,000} characters; 0000 columns removed; 200 replicates per setting | Recovery rate of the true quartet over 200 replicates per setting | In every setting where W1 recovery is below 50% at N = 100,000 characters, W2 recovery is at least 95% at N = 100,000 |
| **H4** | Q-MAWS support values are at least as well calibrated as ML-MAWS bootstrap values | Simulated HGT datasets (known trees) | Expected Calibration Error, 10 equal-width bins | ECE(Q-MAWS) ≤ ECE(ML-MAWS UFBoot) (requires decision D8) |

If decision D8 is declined, H2 and H4 are reported as "not tested" with the reason; they are never replaced with cross-device comparisons.

## Metrics

- **nRF:** |σ(T) Δ σ(T*)| ÷ (2 × (m − 3)), σ the set of non-trivial splits; trees compared unrooted on identical taxon sets.
- **nQD:** fraction of all quartets whose induced topologies differ, computed exactly; quartets unresolved in the reference are counted separately and excluded from the denominator.
- **ECE:** Σ over bins of (n_bin ÷ n_total) × |mean support in bin − fraction of supported splits that are true in bin|.

## Statistical analysis rules

- All pre-registered tests are reported, whatever their outcome. Any additional analysis is labelled "exploratory".
- Effect sizes (median paired difference, Hodges–Lehmann estimate) and every per-dataset value are reported.
- With 7 datasets for H1, statistical power is limited; this is stated in the report, and significance is never claimed when the criterion is not met.
- Every stochastic configuration is run with 5 seeds; mean and standard deviation are reported.
- Baseline values are taken only from published tables or text, never read off figures, and labelled "reported".

## Amendment 1 (2026-10-03): W2c tie rule for star fits

- **Change:** in W2c, a resample in which all three fitted topologies have the internal branch at its lower bound (0.000001) counts as a three-way tie, in addition to the ties of log-likelihoods within 10⁻⁸. The rest of the primary configuration is unchanged.
- **Reason:** found in milestone M8 (`docs/OPEN_ISSUES.md`, OI-14). Such fits favour no resolution, but their log-likelihoods still differed by about 10⁻⁵, so W2c gave near-certain weights to quartets without signal, and the negative control (shuffled sequences) had a mean S1 of 0.649 instead of a value near 1/3.
- **Decided by:** the owner, 2026-10-03, before any experiment of H1, H2 or H4 was run. H3 (milestone M6) was run again with the rule; its pre-registered criterion uses W2, which the rule does not change.

## Amendment 2 (2026-10-03): Pairing value and zero differences in the H1 test

- **Change:** in the H1 test, the Q-MAWS value of each dataset is the mean over the 5 seeds, and zero paired differences are dropped before ranking (Wilcoxon's original procedure). The test, metrics, datasets, Holm correction and success criterion are unchanged.
- **Reason:** the original text did not say which Q-MAWS value enters a pair when there are 5 seeds, or how a zero difference is treated (`docs/OPEN_ISSUES.md`, OI-18). Both change the p-value, and a zero difference is likely on Yersinia HGT, where ML-MAWS reports nRF 1.000.
- **Decided by:** the owner, 2026-10-03, before any H1 number was computed. At that time, seed-1 runs of Fish mtDNA, E. coli/Shigella and Yersinia HGT were finished; no H1 pair or test statistic had been computed.

## Amendment 3 (2026-10-06): Decision D8, the designs of M12 and M13, and improved variants

- **D8 (same-device ML-MAWS): yes.** ML-MAWS (repository commit `0c38db1`) is built and run by this project with IQ-TREE 2.4.0, the program its code calls (`iqtree2`), with its documented options `--strand --iqtree` (IQ-TREE `-st BIN -m MFP+ASC -bb 1000`). IQ-TREE's random seed is read from its log and recorded.
  - **H4** runs ML-MAWS on GitHub's Linux runners: support values do not depend on the device, only on the program versions, settings and seed, which are recorded. ECE is computed over the internal edges of the five simulated HGT datasets pooled (true trees known); Q-MAWS enters with S1 of its seed-1 runs (the primary support); S2 is reported beside it as a secondary result.
  - **H2** runs both programs on the same computer (the development laptop, both built natively for Windows). The Q-MAWS time is a complete run of the primary configuration with S1 support (`--bootstrap 0`); S2 is not part of the comparison. The ML-MAWS time is a complete run including IQ-TREE with 1,000 UFBoot replicates.
- **H2 data, reduced:** m = 25, 50 and 100 with 3 replicates each, and m = 200 with 1 replicate (the plan: 3). Reason: one Q-MAWS run at m = 200 (64,684,950 quartets) is expected to take more than two days on this computer at the throughput measured in M5; three would take about a week. The success criterion is unchanged and is applied to every replicate run: Q-MAWS faster in all replicates with m ≥ 100 (3 at m = 100, 1 at m = 200). The crossover m is reported.
- **M12 sensitivity design (plan Part 8, item 6):** one factor at a time from the primary configuration, on Fish mtDNA, E. coli–Shigella, simulated HGT 0 and simulated HGT 500: number of MAW lengths K = 1, 2, 4 (primary 3); strand filter off; weights W1, W2a, W2b (primary W2c); matrix `M_ml` (primary `M_full`); model W2-emp (primary W2-sym). Variant runs use seed 1 and S1 only; seed variability is reported for the primary configuration (5 seeds, M11). Reason: the full grid (128 combinations × 4 datasets) would take weeks.
- **Improved variants:** results of M12 and any other analysis after this date are exploratory. If a variant appears better than the primary configuration, it is written down here as a new amendment (method, settings, criterion) before it is tested, and it is tested only on simulated datasets generated after that amendment, which no part of the method has seen. The primary configuration and H1 to H4 are reported as pre-registered, whatever the variant shows.
- **Decided by:** the owner, 2026-10-06. At that time no experiment of H2, H4 or M12 had been run; for H1, seed-1 runs of 5 of the 7 HGT datasets were finished, and no H1 pair or test statistic had been computed.

## Amendment 4 (2026-10-06): Q-MAWS v2 and its confirmatory test (H5)

- **Origin (exploratory, M12):** on the four datasets of Amendment 3, the tree made with W1 weights had a lower nRF than the primary configuration (W2c) on all four and a lower nQD on three (results/sensitivity/weights.tsv); on the other HGT datasets W1 was also ahead (results/sensitivity/hgt_w1/). These results only suggest the variant; they do not test it.
- **Q-MAWS v2 (fixed now):** the primary pipeline (strand filter on, K = 3 MAW lengths, `M_full`, inclusion–exclusion counts, wQFM-rs with its default settings) with W1 quartet weights (votes of the split patterns 1100/0011, 1010/0101, 1001/0110, normalised per quartet) for the tree and for S1. Made as `qmaws run --seed 1 --bootstrap 0` followed by `qmaws variant --weights w1` on that run (the W1 tree uses no W2 result).
- **New data (generated after this amendment, seen by no part of the method):** 20 simulated genomes sets, 30 taxa each, made by `scripts/simulate_hgt.sh` (committed before the data are made):
  - species tree: AliSim random Yule–Harding tree with 30 taxa, branch lengths `-rlen 0.001 0.01 0.1` (minimum, mean, maximum substitutions per site; mean 0.01 so that genomes differ by a few percent, as within a bacterial genus or family; AliSim's default mean 0.1 makes data so divergent that both methods fail, as seen in the H2 smoke test);
  - genome: 1,000 genes of 1,000 sites, Jukes–Cantor, AliSim topology-unlinked partitions (`-Q`, one tree per gene);
  - horizontal transfer: in a fraction h of the genes (exactly round(1000 h), chosen at random), the gene tree is the species tree after one random subtree prune-and-regraft (a random subtree, not the whole tree, is cut and attached at the midpoint of a random edge outside it; the cut stem keeps its length); all other genes use the species tree;
  - levels h = 0, 0.1, 0.2, 0.4; 5 replicates each; seed of dataset = 7000 + 100 × level index + replicate, from which the species-tree, transfer and AliSim seeds are derived; manifest with commands, seeds and SHA-256 values in `data/manifests/simulated/`.
- **Methods compared:** Q-MAWS v2, and ML-MAWS (commit `0c38db1`, `--strand --iqtree`, IQ-TREE 2.4.0, all cores, seed recorded from its log) run by this project on the same data and computer. The primary configuration (the W2c tree of the same Q-MAWS run) is reported beside them as a secondary result.
- **H5:** on the new data, Q-MAWS v2 has lower topological error than ML-MAWS. Error = nRF and nQD against the species tree. Test: one-sided exact Wilcoxon signed-rank test on the 20 paired differences (v2 minus ML-MAWS), zero differences dropped (as in Amendment 2), Holm correction across the 2 metrics. Success: adjusted p < 0.05 for at least one metric and median difference < 0. Reported: Hodges–Lehmann estimate, every dataset's values, the medians per transfer level, and the same comparison for the primary configuration (descriptive).
- **Order:** the data are made and analysed only after this text is committed; the results are reported whatever they show. If H5 fails, the thesis reports W1 as an exploratory observation only.
- **Decided by:** the owner, 2026-10-06, after the exploratory M12 weight variants and before any of the new data existed.

## Amendment 5 (2026-10-14): comparison with peer-reviewed alignment-free tools

- **Why:** ML-MAWS, the main comparison method, is a preprint. To place Q-MAWS against peer-reviewed tools, three of them are run by this project on the same data. These comparisons are **descriptive**: no new hypothesis and no success criterion are added, and H1 to H5 are unchanged.
- **Tools, each at a fixed version and with the settings its authors publish as default or use in their own scripts:**
  - **Mash** 2.3 (Ondov et al., Genome Biology 2016): `mash sketch` with the default k = 21 and sketch size 1,000, `mash dist`; the tree is made by neighbour joining from the distance matrix.
  - **CD-MAWS** (Anjum et al., IEEE/ACM TCBB 2023), in the suffix-automaton implementation of Ehsan, Mosaddek and Rahman (WALCOM 2025; repository `TamimEhsan/cd-maws-sa`, commit `8bfe7d5`), which computes the same cosine distance on MAW sets: `maw-gen` with MAW lengths 4 to 14 (the values in the authors' own script), one sequence per taxon (contigs of a genome joined, as the authors' README asks), `dist-calc`; neighbour joining from the distance matrix.
  - **Peafowl** (Zahin et al., BMC Bioinformatics 2025; repository `hasin-abrar/Peafowl-repo`, commit `89ed477`): `peafowl.sh -i <folder> -n <cores> -r` (k chosen by entropy from 9 to 31, k-mers of both strands, RAxML `-m BINGAMMA -p 12345`); Jellyfish and RAxML from the Ubuntu packages of the runner, versions recorded.
  - **Neighbour joining** for Mash and CD-MAWS: Biopython `DistanceTreeConstructor("nj")` at a pinned version, on the full symmetric matrix.
- **Data:** the nine AFproject datasets with a reference tree, and the 20 H5 datasets. The H5 data are made again on the runner with `scripts/simulate_hgt.sh` (same commands and seeds) and used only if the SHA-256 of `sequences.fasta` equals the value in `data/manifests/simulated/hgt.tsv`; otherwise that dataset is reported as "not run (data differ)".
- **Where and limits:** GitHub's Linux runners (the development laptop runs M13). Accuracy does not depend on the computer; run times measured there are not compared with anything. A tool that does not finish one dataset within the runner's 6-hour job limit, or fails, is reported as "not finished" or "failed" with its message, never left out silently.
- **Reported:** nRF and nQD against the reference or species tree for every tool and dataset (computed by `qmaws tree-compare`); medians per H5 transfer level; for each tool, the paired differences of Q-MAWS (primary and v2) against it, with the same Wilcoxon statistics as H1 and H5 shown as descriptive numbers only.
- **Order:** written and committed before any of these tools is run on any dataset. At this time 7 of the 20 H5 datasets were finished and no H5 value or statistic had been looked at.
- **Decided by:** the owner, 2026-10-14.
- **Note (2026-10-14, same day, before any tool had produced an H5 tree):** the H5 data made again on GitHub's Linux runners are not the laptop's data; even the AliSim species tree from the same seed differs, so every H5 job stopped with "not run (data differ)", as designed. The H5 data are therefore taken from the laptop: the 20 `sequences.fasta` files, each checked against its SHA-256 in `data/manifests/simulated/hgt.tsv`, are stored gzip-compressed on the branch `h5-data`, and `scripts/baselines.sh` checks the SHA-256 again on the runner. The decision to run the three tools on the 20 H5 datasets was confirmed by the owner before the H5 test (which needs all 20 datasets) was computed. Tools, settings and reporting are unchanged.

## Amendment 6 (2026-10-15): ML-MAWS on more than 50 taxa (H2)

- **Finding:** ML-MAWS (commit `0c38db1`, `main.cpp` lines 223–227 and 427–433) switches itself to a "fast mode" for more than 50 species, which adds `-fast` to its IQ-TREE command next to `-bb 1000`. IQ-TREE 2.4.0 refuses this combination ("Ultrafast bootstrap (-bb) does not work with -fast option"), so ML-MAWS as distributed stops at its step 5 without a tree for every dataset with more than 50 taxa, and still exits with status 0. The M13 queue recorded such a run for `scaling_m100_r1` (185 s, no tree); its time is not used.
- **What H2 uses for m ≥ 100 (the definition of H2 is unchanged: a complete ML-MAWS run including IQ-TREE with 1,000 UFBoot):** ML-MAWS is run as distributed (`--strand --iqtree`, all cores), which makes the MAW matrix in its steps 1 to 4; then the IQ-TREE command of its normal mode (`iqtree2 -s ml_maws_matrix.phy -st BIN -m MFP+ASC -bb 1000 -nt <cores> --prefix ml_maws_iqtree -redo`, the command ML-MAWS itself prints after the failure, and the one it uses for 50 or fewer taxa) is run on that matrix. ML-MAWS's time = the wall time of the ML-MAWS run + the wall time of this IQ-TREE run. The ML-MAWS source is not changed.
- **When:** these runs are made after the M13 queue has finished, one at a time, with no other work on the laptop (`D:\_external\queues\m13_mlmaws_complete.sh`, outside the repository).
- **Order:** written before any valid ML-MAWS time for m ≥ 100 existed. One Q-MAWS time with m = 100 (`scaling_m100_r1`) had been recorded by the queue; the H2 evaluation had not been computed. The m ≤ 50 runs are not affected: there ML-MAWS used its normal mode and produced its tree.
- **Decided by:** the project on 2026-10-15, under the owner's instruction of 2026-10-14 to proceed as best for the study; reported to the owner the same day.

## Amendment 7 (2026-10-15): H2 without the 200-taxa dataset

- **Finding (measured):** the amalgamation stage holds every weighted quartet in memory. Rhinovirus (116 taxa) had 21,334,911 weighted quartets and needed 1,321 MB (M7); `scaling_m100_r1` had 11,645,968 weighted quartets for 3,921,225 quartets (2.97 per quartet). At 200 taxa (64,684,950 quartets) that gives about 192 million weighted quartets and about 12 GB, while the development laptop has 7.6 GB of memory. The 200-taxa Q-MAWS run would work for about two days and then very likely fail in the amalgamation stage.
- **Decision (D4, the owner, 2026-10-15):** `scaling_m200_r1` is not run. H2 is evaluated as pre-registered on the remaining datasets: Q-MAWS faster than ML-MAWS in every replicate with m ≥ 100, which are now the three replicates with m = 100; the crossover m is reported over m = 25, 50 and 100. The memory requirement at 200 taxa is reported as a limitation, and 200 taxa on a computer with more memory is future work. No quartet subsampling is used.
- **How:** its input folder is renamed to `data/raw/simulated/scaling_m200_r1.skipped` so that the running M13 queue and the ML-MAWS completion script (Amendment 6) stop on it at once; their rows for this dataset are not used.
- **Order:** decided before any run of `scaling_m200_r1` had started and before the H2 evaluation was computed. At this time Q-MAWS times for m = 25, 50 and one replicate of m = 100 had been recorded, and no valid ML-MAWS time for m ≥ 100 existed.

## Amendment 8 (2026-10-23): extended benchmark on one computer, and H6

- **Why:** H1 to H5 and the comparison of Amendment 5 are finished and reported (release 1.0.0, preprint). This amendment extends the study into a benchmark of alignment-free phylogeny under horizontal gene transfer: more methods, more datasets and more conditions. H1 to H5, their data and their results are not changed by anything below.
- **One computer (replaces the GitHub runs):** every run of this benchmark is made on the development laptop (Intel Core 5 210H, 12 threads, 8 GB of memory, Windows 11; Linux tools in Ubuntu under WSL2 on the same laptop). The runs of Amendment 5 (Mash, CD-MAWS, Peafowl) and of H4 (ML-MAWS) were made on GitHub's Linux runners; they are run again on the laptop, and only the laptop runs enter the benchmark. The old and new accuracy values are compared and every difference is reported.
- **Runs reused (already made on the laptop):** the Q-MAWS runs of M11 (14 datasets, 5 seeds), of H5 (20 datasets) and of M13 (9 datasets), and the ML-MAWS runs of H5 and H2. They were made with builds before 1.0.0; release 1.0.0 re-checks them with identical values (`qmaws verify`, PASS, tested 2026-10-22 on two runs), and every reused run is verified with 1.0.0 again before it enters the benchmark.
- **Methods (candidates):** Q-MAWS 1.0.0 (primary configuration, seed 1, `--bootstrap 0`), ML-MAWS (commit `0c38db1`, as in Amendments 3 and 6), CD-MAWS (`cd-maws-sa`, commit `8bfe7d5`), Mash 2.3 with neighbour joining, Peafowl (commit `89ed477`), and further published alignment-free tools: SANS serif, andi, phylonium, FSWM, Multi-SpaM, Skmer, JolyTree, mashtree, AAF and CVTree. A candidate enters the benchmark only if it installs on the laptop and finishes the smoke test (one tuning dataset, below) within 6 hours. On the simulated datasets, IQ-TREE on the true alignment (`sequences.fasta` of AliSim is already aligned) is reported as an alignment-based reference, not as a competitor. The final list, with the exact version or commit, the install command and the run command of every method, is committed as Amendment 8b before the first benchmark run.
- **Settings:** every method runs with its published default settings. Methods with a word length or similar setting also run tuned: the value with the lowest median nRF on the tuning datasets (below) is chosen from a grid fixed in Amendment 8b; ties keep the default. Tuning never uses an evaluation dataset.
- **Datasets (59 for evaluation, 6 for tuning; all with a known reference or species tree):**
  - the 9 AFproject datasets with a reference tree, and the 20 H5 datasets (Amendment 4);
  - **divergence** (15 new): as H5 at h = 0.2, 30 taxa, with mean branch length 0.005, 0.02 or 0.05 (H5 used 0.01), 5 replicates each;
  - **number of species** (10 new): as H5 at h = 0.2, mean branch length 0.01, with 15 or 60 taxa, 5 replicates each;
  - **fragmented assemblies** (5 new): the five H5 datasets with h = 0.2, each genome cut into contigs of random length (mean 10,000 letters, fixed seed) and written as one multi-record file per taxon;
  - **tuning** (6 new, never evaluated): as H5 at h = 0.2, 30 taxa, mean branch length 0.01, with seeds outside every evaluation range;
  - all new data are made by `scripts/simulate_benchmark.sh` with seeds 9000 and above, committed before the data are made; commands, seeds and SHA-256 values go to `data/manifests/simulated/benchmark.tsv`.
- **Memory limit:** the laptop has 8 GB. A run that would exceed it, or fails for memory, is reported as "not run (memory)" with the estimate, never left out silently. Q-MAWS is limited to about 150 taxa on this computer (Amendment 7); this is reported as a limitation.
- **Measures:** nRF and nQD against the reference or species tree (`qmaws tree-compare`); wall time and peak memory of every run. Times are compared between methods only on the speed datasets of M13 (25, 50 and 100 taxa), run one at a time on mains power, Performance power mode and no other work on the laptop; elsewhere times are reported for information only.
- **H6 (confirmatory, new):** on the 30 new evaluation datasets (divergence, number of species, fragmented assemblies), Q-MAWS (primary configuration) has lower topological error than ML-MAWS. Test as in H1 and H5: one-sided exact Wilcoxon signed-rank test on the paired differences (Q-MAWS minus ML-MAWS), zero differences dropped, Holm correction across nRF and nQD. Success: adjusted p < 0.05 for at least one measure and median difference < 0. H6 repeats the secondary result of H5 on data that did not exist when that result was seen.
- **Benchmark analysis (descriptive):** for each measure, the Friedman test across all methods on the datasets that every method finished, followed by pairwise Wilcoxon signed-rank tests with Holm correction, and the mean rank of every method; medians per condition (transfer level, divergence, number of species, fragmentation); failures and memory limits per method. These numbers describe the field; they add no further hypothesis.
- **Order:** this text is committed before any new dataset is made and before any method is installed or run for the benchmark. Amendment 8b (final methods, versions, commands and tuning grids) is committed after the installs and smoke tests, which use only a tuning dataset, and before the first evaluation run. All results are reported, whatever they show.
- **Decided by:** the owner, 2026-10-23.

## Amendment 8a (2026-10-23): the methods that run natively on Windows, fixed now

- **Why:** the Linux tools need WSL2, which does not run on the laptop yet (virtualization is off in its firmware). The methods below run natively on Windows. They are fixed now, so their runs can start before the other tools are installed. Amendment 8b still fixes every other method before its first evaluation run. Nothing else in Amendment 8 changes.
- **Q-MAWS 1.0.0** (built on the laptop from the commit of release 1.0.0): `qmaws run --input <data> --reference <true tree> --seed 1 --bootstrap 0 --no-live-tree --output <run folder>`; an interrupted run is continued with `qmaws resume`.
- **ML-MAWS** (commit `0c38db1`, the Windows build of Amendment 3, IQ-TREE 2.4.0 for Windows): `ml-maws -i <data> -o <folder> --strand --iqtree --threads 12`. Above 50 taxa it stops without a tree (Amendment 6); its normal-mode IQ-TREE command `iqtree2 -s ml_maws_matrix.phy -st BIN -m MFP+ASC -bb 1000 -nt 12 --prefix ml_maws_iqtree -redo` is then run on its own matrix, and the time is the sum of both.
- **Alignment-based reference:** IQ-TREE 2.4.0 on the true alignment of each simulated dataset whose alignment is known (the 20 H5 and the 25 new non-fragmented evaluation datasets): `iqtree2 -s sequences.fasta -m MFP -nt 12 -seed 1`.
- **Runs of these methods:** Q-MAWS on the 30 new evaluation datasets; ML-MAWS on the 30 new evaluation datasets and on the 9 AFproject datasets (H1 used the values reported by ML-MAWS; the benchmark uses laptop runs); the reference on the 45 datasets above; `qmaws verify --quick` with release 1.0.0 on every reused run. Results go to `results/benchmark/`. Wall time and peak memory (sum of the working sets of the method's processes, sampled every second) are recorded for every run.
- **Order:** committed before any run of these methods on any dataset of the benchmark.

## Amendment 8b (2026-10-24): final methods, commands and tuning grids

- **Where:** the Linux tools run in Ubuntu 24.04 under WSL 1 on the development laptop (WSL 2 would need virtualization in the firmware, which the owner keeps off). Tools are installed in `/opt/bench`; every run goes through `scripts/bench_tools.sh` (default settings unless a tuned value is given) with 12 threads and a time limit of 6 hours. Methods that give distances get a neighbour-joining tree from `scripts/nj_tree.py` (Biopython 1.84), as in Amendment 5.
- **Smoke test** (tuning dataset `bm_tune_r1`, results in `results/benchmark/smoke/`): 12 of the 13 Linux candidates wrote a tree with all 30 taxa. **AAF** (commit `40fdd2d`) is **excluded**: its per-species k-mer tables stayed empty (also with 4 threads, the memory limit of 4 GB, and sequences wrapped at 80 letters), so no distance matrix and no tree were made. Peafowl needed 30,898 s of wall time, which included the laptop sleeping and a shared processor; its 6-hour limit (which does not count sleep) was not reached.
- **Final methods** (versions and commands; the run command is the one in `scripts/bench_tools.sh`):
  - Q-MAWS 1.0.0, ML-MAWS `0c38db1` and the IQ-TREE 2.4.0 reference, as in Amendment 8a (Windows).
  - Mash 2.3 (official binary `mash-Linux64-v2.3`): `mash sketch -k 21 -s 1000`, `mash dist`, NJ.
  - CD-MAWS (`cd-maws-sa` `8bfe7d5`): `maw-gen` with lengths 4 to 14 per taxon, `dist-calc`, NJ.
  - Peafowl (`89ed477`, Jellyfish 2.3.1, RAxML 8.2.12): `peafowl.sh -i input -n 12 -r`.
  - andi 0.14 (Ubuntu): `andi -j -t 12`, NJ.
  - phylonium 1.7 (Ubuntu): `phylonium -t 12`, NJ.
  - FSWM (`burkhard-morgenstern/FSWM` `59efdc6`, compiled with its own command plus `-include cstdint`): `fswm -k 12 -t 12` on one record per genome, NJ.
  - Multi-SpaM (`tdencker/multi-SpaM` `3230d3f`, built with `-fcommon` for its PHYLIP part and `-include cstdint`): `multispam.py -t 12 -k 10` (default 100 don't-care positions, 1,000,000 quartet blocks).
  - SANS (`gi-bielefeld/sans` `3d3cdd9`, "SANS ambages", the successor of SANS serif, which moved from GitLab to GitHub): `SANS -k 31 -f strict -N <tree>`.
  - CVTree (`ghzuo/cvtree` `575ba9b`): `cvtree -g fna -m Hao -k 7` (the largest value of its default list 5 6 7).
  - JolyTree 2.1 (`ffbff7b`, with FastME `08e2150`, REQ `a528040`, Mash 2.3): `JolyTree.sh -i <genomes> -b jt -t 12 -x` (k and sketch size estimated by JolyTree).
  - Skmer (`ea948d3`): `skmer reference -k 31 -p 12` (default sketch size 100,000), NJ.
  - mashtree 1.4.6 (`c0853a8`): `mashtree --numcpus 12 --kmerlength 21`.
- **Tuning grids** (value with the lowest median nRF on the six tuning datasets `bm_tune_r1` to `bm_tune_r6`; ties keep the default, or else the tied value closest to it): Mash k ∈ {15, 17, 19, 21, 23, 25, 31}; mashtree k ∈ {15, 17, 19, 21, 23, 25, 31}; SANS k ∈ {15, 19, 23, 27, 31}; Skmer k ∈ {15, 19, 23, 27, 31}; FSWM weight ∈ {8, 10, 12, 14, 16}; Multi-SpaM weight ∈ {8, 10, 12}; CVTree K ∈ {7, 9, 11, 13}. No tuning (no such setting, or the method chooses it itself): Q-MAWS, ML-MAWS, CD-MAWS, Peafowl, andi, phylonium, JolyTree.
- **Runs:** every final method with default settings on all 59 evaluation datasets (the 9 AFproject, 20 H5 and 30 new datasets), and a second time with the tuned value where it differs from the default; for the speed comparison, every method on the 9 speed datasets of M13, one run at a time on mains power with no other work (the Q-MAWS and ML-MAWS times of M13 were measured this way on this laptop and are reused). These runs form phase B of the queue (`scripts/benchmark_queue_b.sh`), which starts after phase A.
- **Order:** committed before any tuning run and before any evaluation run of the methods above; only the smoke test on `bm_tune_r1` had been run.
