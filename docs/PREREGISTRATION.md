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
