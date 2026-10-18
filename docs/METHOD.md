# Method

The mathematics of Q-MAWS, step by step. Where each step is in the code: [ARCHITECTURE.md](ARCHITECTURE.md). The pre-registered evaluation: [PREREGISTRATION.md](PREREGISTRATION.md).

## Notation

- *m* taxa (genomes), each a string over {A, C, G, T}.
- A **quartet** is a set of four taxa {a, b, c, d} with a < b < c < d. Its three unrooted topologies are T1 = ab|cd, T2 = ac|bd and T3 = ad|bc. There are C(*m*, 4) quartets.

## 1. Input cleaning

Headers, spaces and digits are removed, letters are made upper case, U becomes T, and only A, C, G and T are kept. Every removal is counted and reported. The SHA-256 of each cleaned sequence is recorded.

## 2. Minimal absent words

A word *w* = *x u y* (with *x* and *y* single letters and *u* possibly empty) is a **minimal absent word** (MAW) of a string *s* if *x u* and *u y* occur in *s* but *w* does not. MAWs of length at least 2 are read off a suffix automaton of *s* (Crochemore, Mignosi and Restivo 1998; Blumer et al. 1985), built in time linear in the length of *s*.

**Strand filter** (as in ML-MAWS): a MAW *w* of a genome is kept only if its reverse complement is also a MAW of the same genome.

## 3. Length selection and the matrix

For each candidate length *L* in a range that depends on the average sequence length and on *m* (as in ML-MAWS's code), the matrix of MAWs of length *L* is formed and the binary Shannon entropy of each column is summed:

H(*L*) = Σ over columns of [ −*p* log₂ *p* − (1 − *p*) log₂ (1 − *p*) ], where *p* is the fraction of taxa with 1 in the column.

The *K* = 3 lengths with the largest H(*L*) are kept. Ties are broken by a fixed rule.

**Matrix `M_full`:** one row per taxon and one column per word that is a MAW of at least one taxon at a selected length. The entry is 1 if the word is a MAW of that taxon, else 0. Rows are stored as bitsets of 64-bit words.

**Matrix `M_ml`:** ML-MAWS's matrix (constant columns removed, at most 50,000 columns kept by its balance rule). It is built only to check equality with ML-MAWS; Q-MAWS uses `M_full`.

## 4. Quartet pattern counts

For a quartet, every column shows one of 16 patterns *x* = (*x_a*, *x_b*, *x_c*, *x_d*) ∈ {0, 1}⁴. Let *n*(S) be the number of columns with 1 for every taxon in a set S: the popcount of the AND of their rows. The number of columns with 1 exactly at the members P of the quartet *q* is

*c*(P) = Σ over S with P ⊆ S ⊆ *q* of (−1)^(|S| − |P|) *n*(S).

*n*(S) is precomputed for all single taxa, pairs and triples, so each quartet needs one new four-way AND-popcount. Quartets are visited in the order of a seeded keyed Feistel permutation of their ranks, so that the quartets finished at any moment are a uniform sample.

## 5. Quartet weights

**W1 (votes):** the split patterns support the topologies. 1100 and 0011 support T1, 1010 and 0101 support T2, and 1001 and 0110 support T3. The W1 weight of a topology is its share of these votes.

**W2 (conditioned likelihood):**
- **Model:** a two-state model on each topology with five branch lengths (four leaf edges and one inner edge, each in [10⁻⁶, 10]). Under the symmetric model (W2-sym; Cavender–Farris–Neyman) a branch of length *t* changes the state with probability *p*(*t*) = (1 − e^(−2*t*)) / 2. *P_T*(*x*) is the probability of pattern *x* on topology *T*, summed over the states of the two inner nodes.
- **Conditioned log-likelihood.** A MAW column exists only because some taxon has that MAW, so inside a quartet the pattern 0000 carries no evidence about the four taxa. The likelihood is therefore conditioned on "not 0000":

  ℓ_T = Σ over *x* ≠ 0000 of *c*(*x*) [ ln *P_T*(*x*) − ln(1 − *P_T*(0000)) ]

- **Fitting.** ℓ_T is maximised over the five branch lengths by projected Newton steps with exact first and second derivatives and Levenberg–Marquardt damping, from three starting points.

**Weights from the three maximised values ℓ*₁, ℓ*₂, ℓ*₃:**
- W2a: the best topology with weight ℓ*_best − ℓ*_second.
- W2b: *w_T* = exp(ℓ*_T − max) / Σ exp(ℓ*_T′ − max).
- **W2c (primary):**
  1. The 15 counts other than 0000 are resampled 100 times (multinomial, same total), and the three topologies are fitted again each time.
  2. The weight of a topology is its share of wins; ties are shared equally.
  3. If all three fits are stars (inner edge at its lower bound), that resample counts as a three-way tie (Amendment 1).
  4. Each quartet's random numbers come from SHA-256 of the run seed and the quartet's rank, so any quartet can be recomputed alone.

## 6. Amalgamation (wQFM-rs)

The weighted quartet topologies are combined into one unrooted tree with wQFM (Mahbub et al. 2021), re-implemented in Rust:
- recursive bipartitioning of the taxa that maximises the weight of satisfied quartets;
- Fiduccia–Mattheyses moves;
- dummy taxa when a subproblem is split.

wQFM-rs gives the same tree as the original wQFM program on all 23 comparison inputs.

## 7. Support and halo values

- **S1** of an internal edge with split A | B: over all quartets with two taxa in A and two in B, the weight on the topology that agrees with the split, divided by their total weight. 1/3 means no signal.
- **S2:** 100 column-bootstrap replicates. In each, every matrix column gets a Poisson(1) weight, the counts become weighted counts, and weighting (W2b, for cost) and amalgamation are repeated. S2 is the fraction of replicate trees containing the split.
- **Halo value** of a taxon: over all quartets containing it, the weight on the topology that agrees with the final tree, divided by their total weight. A low value marks a taxon whose quartet evidence disagrees with its place in the tree.

## 8. Evaluation measures

| Measure | Definition |
|---|---|
| **nRF** | The number of splits in only one of the two trees, divided by 2(*m* − 3) |
| **nQD** | The fraction of quartets that the two trees resolve differently |
| **MSD** | The matching split distance (minimum-cost matching of the splits) |
| **ECE** | With 10 equal-width bins of support *s*: Σ over bins (*n*_bin / *n*) × \|mean *s* − fraction of edges in the true tree\| |
| **Hypothesis tests** | The exact one-sided Wilcoxon signed-rank test over all 2ⁿ sign assignments (zero differences dropped), the Holm correction across metrics, the median and the Hodges–Lehmann estimate |

## 9. Reproducibility

- A run is a sequence of stages. Chunked stages write one file per chunk with its SHA-256; a stopped run resumes at the first unfinished chunk.
- The **root fingerprint** is the SHA-256 over the configuration hash (without file locations) and every stage's content hash. It does not depend on the computer, the number of threads, the chunk size or the program version, only on the inputs, the settings and the seed.
- Mathematical functions come from the pure-Rust `libm` library, so the root fingerprint is the same on Windows, macOS and Linux (golden test G12).
- `qmaws verify` recomputes the input hashes and a seeded sample of chunks (or the whole run with `--full`) and compares them with the record.
