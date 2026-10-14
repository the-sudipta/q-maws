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
