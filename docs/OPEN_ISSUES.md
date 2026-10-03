# Open issues

Problems, ambiguities and discrepancies found during development. Each entry states what the project specification says, what was found, the evidence, and options with trade-offs. Resolved entries stay in the file with their resolution and date.

## OI-1: Contact address placeholder in LICENSE (resolved, 2026-10-02)

- **Specification:** commit the owner-supplied `LICENSE` verbatim and never edit its text.
- **Found:** Section 12 of `LICENSE` contains the placeholder `<CONTACT EMAIL>` instead of an address. Section 3.2(a) requires registration at that address before individual academic research use.
- **Options:** (a) the owner supplies a corrected `LICENSE`, committed as a separate change; (b) keep the placeholder until the first public release, accepting that registration is not possible before then.
- **Resolution:** on the owner's instruction (2026-10-02), the placeholder in Section 12 was replaced with the owner's contact address, sudiptakumar400@gmail.com. No other text of `LICENSE` was changed.

## OI-2: The guard must detect strings that may not appear in the repository (resolved, 2026-10-02)

- **Specification:** the pre-commit guard blocks forbidden attribution strings, and those strings must not appear anywhere in the repository. It also blocks private local files whose names must not appear in the repository.
- **Resolution:** the guard assembles forbidden strings from fragments at run time, and blocks any staged path matched by the local, uncommitted `.git/info/exclude` file instead of naming files. See `scripts/README.md`.

## OI-3: Commit messages are checked by a commit-msg hook (resolved, 2026-10-02)

- **Specification:** the pre-commit guard enforces the attribution rules, which also apply to commit messages.
- **Found:** a pre-commit hook cannot see the commit message.
- **Resolution:** the same script is also installed as the `commit-msg` hook; it checks the message when called with the message file as its argument.

## OI-4: Root fingerprint and device-dependent chunk boundaries (resolved by a design choice, 2026-10-02; owner may revise)

- **Specification:** chunk sizes are chosen per device by a calibration benchmark (target 2 to 5 seconds per chunk) and frozen in `run.json`; the root fingerprint is the SHA-256 over the ordered list of all stage and chunk hashes; the root fingerprint must be identical on Windows, macOS and Linux for the same data and settings.
- **Found:** if per-chunk hashes enter the root fingerprint, two devices that calibrate different chunk sizes produce different fingerprints for identical results, so the cross-platform requirement cannot hold.
- **Options:** (a) the root covers stage content hashes, each computed over the concatenated chunk outputs in order, so it is independent of chunk boundaries; per-chunk hashes stay in `audit/chunks.json` for quick verification. (b) Fix the chunk size per dataset instead of calibrating it; chunk durations then vary by device, and estimates and the live tree cadence become less even. (c) Keep chunk hashes in the root and compare roots only between runs with the same chunk plan; cross-platform comparison then needs the stored plan to be reused.
- **Choice:** (a), implemented in M1 (`docs/DESIGN.md`, "Root fingerprint"). It keeps both requirements and changes no research design.

## OI-5: Layout of the simulated HGT archive (resolved, 2026-10-02)

- **Specification:** the simulated archive contains 5 FASTA files (one per HGT level), each with 33 genomes.
- **Found:** `simulated-sim_hgt.zip` (MD5 `967b15c79b44524a8f3c389e66344279`, matching the published value) contains five folders `simulated-sim_hgt/hgt_0`, `hgt_250`, `hgt_500`, `hgt_750`, `hgt_1000`, each with 33 files `Species1.fasta` to `Species33.fasta` holding one record each (171 entries in total).
- **Resolution:** registered as five datasets `sim_hgt_0` to `sim_hgt_1000` with layout `file_per_taxon`, sharing one download. Nothing else changes.

## OI-6: Influenza A sequence lengths (resolved, 2026-10-02)

- **Specification:** the Influenza A dataset (38 sequences) has an average length of 13 kb (from ML-MAWS Table 2); a later publication describes a 38-sequence Influenza A dataset as neuraminidase segment sequences.
- **Found:** the file ML-MAWS used (`Data/influenza.fasta`, commit `0c38db1`) has 38 records with a mean of 1,407 A/C/G/T letters per record. Headers are strain names with subtypes, for example `A/duck/Hong Kong/319/1978(H2N2)`. This is consistent with single-segment sequences, not with whole genomes of the 13 kb listed in Table 2.
- **Options:** (a) use the file as it is (it is what ML-MAWS ran on) and report the measured lengths next to the Table 2 value; (b) also assemble whole-genome sequences for these strains as a separate, labelled "reconstructed" dataset (decision D5). Recommendation: (a), with the discrepancy stated neutrally in `baselines/README.md` when baselines are collected (M11).

- **Resolution:** the owner decided (D5, 2026-10-02) to build the NCBI datasets from accession lists and to rebuild influenza from NCBI. Li, He, He and Yau (2017), the source of the dataset, state that they used "Segment 6 gene encoding N (neuraminidase)"; their Table S2 lists the 38 accessions with lengths of 1,350 to 1,467 bp, and every NCBI record title names segment 6 or neuraminidase. The rebuilt dataset `influenza_a` (38 neuraminidase segments, pinned record versions) is identical in sequence to the ML-MAWS file. So the data are correct as the neuraminidase segment, and the 13 kb in ML-MAWS Table 2 does not describe them; this discrepancy will be stated neutrally in `baselines/README.md` (M11). A whole-genome influenza dataset was not built; it can be added as a separate, labelled "reconstructed" dataset if the owner wishes.

## OI-7: Taxon names from FASTA headers that contain spaces (resolved, 2026-10-02)

- **Specification:** a FASTA record's taxon name is the first word of its header; duplicate names produce a warning with the choices "rename automatically (append `_2`)" or "abort".
- **Found:** in the ML-MAWS data files, 25 of 38 influenza headers and 18 of 41 mammal mtDNA headers contain spaces. Taking the first word gives duplicate names: `A/American` (3 records), `A/wild` (2) and `Common` (2), so 38 records give 35 distinct names and 41 give 40. ML-MAWS's reader uses the first word in the same way.
- **Options:** (a) keep the rule; renaming then produces names such as `A/American_2` that no longer say which strain is meant; (b) for `record_per_taxon` datasets, use the whole header line with unsafe characters replaced by `_` (for example `A/American_black_duck/NB/2538/2007_H7N3_`), which gives unique, readable names; (c) a name table per dataset. Choosing (b) changes names relative to ML-MAWS, which matters only for the M3 matrix comparison (column and row contents stay the same). Recommendation: (b). **Owner decision needed.**

- **Resolution:** option (b), decided by the owner on 2026-10-02: with one taxon per record, the whole header line is the name, with unsafe characters replaced by `_`. No duplicate names remain in the influenza and mammal mtDNA data.

## OI-8: Several records in one file (decision D13, resolved, 2026-10-02)

- **Specification:** default one taxon per file, records concatenated in file order; the owner decides per D13.
- **Found:** ML-MAWS (`FastaReader.cpp`) treats a folder as one taxon per file, joining the records of a multi-record file and naming the taxon after the file; a single multi-FASTA file given directly is read as one taxon per record. No AFproject file has more than one record; the five ML-MAWS data files are multi-FASTA files with one taxon per record.
- **Implemented now:** folders default to one taxon per file (records joined; a single-record file keeps its header name), with `--records per-record` to choose one taxon per record; a single file is always read as one taxon per record. This matches ML-MAWS. **Owner decision D13 needed** to confirm or change the folder default.
- **Resolution:** the owner confirmed the implemented behaviour (D13, 2026-10-02): folders give one taxon per file (records joined in file order); a single multi-FASTA file gives one taxon per record.

## OI-9: MAW length range in ML-MAWS code differs from the paper's formula (resolved, 2026-10-02)

- **Specification:** l_max = min(floor(log2(average cleaned length)), L_cap), with L_cap = 10 for mitochondrial and 14 for bacterial genomes, the range widened when m > 50; to be determined from `EntropySelector.cpp` and `main.cpp` and reproduced exactly.
- **Found:** ML-MAWS (commit `0c38db1`, `EntropySelector::computeAdaptiveRange`) uses a table on the integer average length (total letters ÷ taxa): below 500 → [2, 6]; below 2,000 → [2, 8]; below 50,000 → [3, 10]; below 500,000 → [4, 12]; otherwise [5, 14]; l_max + 2 (at most 16) if m > 50, and + 2 more (at most 18) if m > 100. No logarithm and no L_cap appear in the code. The table gives the ranges reported in the paper ([3, 10] for Fish mtDNA, [5, 14] for E. coli).
- **Resolution:** the code's table is reproduced (`qmaws_core::matrix::adaptive_range`), as the specification asks ("reproduce exactly" from the code).

## OI-10: Tie rule of the length selection (resolved, 2026-10-02)

- **Specification:** ties in entropy are broken by the shorter length; verify ML-MAWS's tie rule and follow it.
- **Found:** ML-MAWS sorts the candidate lengths by entropy with `std::sort`, which is not stable, so equal entropies have no defined order. Exact ties of floating-point entropy sums are not expected in practice.
- **Resolution:** Q-MAWS uses a stable sort on lengths in ascending order, so ties go to the shorter length (the specification's rule). The selection itself follows ML-MAWS: lengths with at least 5 variable columns and positive entropy; if fewer than 3, lengths with at least one variable column; the 3 with the highest entropy; if none, the single best length.

## OI-11: License statements inside ML-MAWS source files (open, 2026-10-02)

- **Found:** `LICENSE.txt` of the ML-MAWS repository is the Apache License 2.0, while the header of `SuffixAutomaton.h` states "License: MIT" and "Authors: [Your Name]". No ML-MAWS code is copied into Q-MAWS; the behaviour is reimplemented, so the difference does not affect Q-MAWS. It is recorded for the baselines notes (M11).

## OI-12: Golden worksheet wording (resolved, 2026-10-02)

- **Specification:** `qmaws teach --example` must match `tests/golden/worksheet_example.txt` byte for byte, with all values of the worksheet example (Appendix A of the teaching material); golden outputs change only with the owner's approval.
- **Found:** the golden file did not exist; it was written in M4 from the program's output after a test confirmed every value against the teaching material: the 16 candidate words and the constant words CC, CT, GA; the 13-column matrix; the five quartets and the full pattern table; the W1 votes with their words (for example `KL|NP (8: AT, CA, GC, TG, AC, CG, GT, TA)`), the weights 1, 1, 1, 0.875, 0.875 and the vote summary (27 of 65 cells); the contribution and score matrices, both merge rounds and the tree `((K,L),M,(N,P));`; the splits, nRF = 0 and the control nRF = 0.5; the model values 0.0401 and 0.0081.
- **Differences in wording only:** steps are numbered "Step 1" to "Step 9" instead of A.1 to A.9; arithmetic uses `x` and `/` (plain text) instead of × and ÷; every new score is written out in full, for example `{K,L}-N = (0 + 0) / 2 = 0` where the teaching material writes `{K,L}–N = 0`; the model terms are listed in a fixed order of the internal states (u, v) = (0,0), (0,1), (1,0), (1,1), so `P(1100 | ab|cd) = 0.5 x (0.00729 + 0.00001 + 0.06561 + 0.00729)` lists the same four terms as the teaching material in another order.
- **Options:** (a) approve the golden file as it is; (b) ask for specific wording changes, after which the golden file is regenerated and approved again.
- **Resolution:** the owner approved the golden file as it is (option a) on 2026-10-02.

## OI-13: Quartet decisions cannot re-run the amalgamation exactly (resolved, 2026-10-03)

- **Specification:** `audit/quartet_decisions.bin.zst` holds, per quartet in rank order, the winning topology (2 bits) and its weight quantised to 16 bits, and is described as "enough to re-run amalgamation exactly".
- **Found:** the amalgamation uses all three weights of every quartet (W2c gives weight to more than one topology for most quartets), and the weights are quantised. From the winning topology and its quantised weight alone, the amalgamation input cannot be rebuilt, so its tree cannot be guaranteed.
- **Implemented now:** the format as specified (`docs/DESIGN.md`). It supports the single-quartet verification; the full verification recomputes the tree from the data.
- **Options:** (a) keep the format and correct the description; (b) store all three weights quantised to 16 bits (about 6 bytes per quartet, about 43 MB before compression for m = 116), so the amalgamation can be re-run on the quantised weights (the tree may still differ from the run's in rare cases near ties); (c) store all three weights as 64-bit values (24 bytes per quartet), which reproduces the tree exactly, at about 172 MB before compression for m = 116. Recommendation: (a), because the full verification already re-runs the amalgamation exactly from the data. 
- **Resolution:** the owner chose (a) on 2026-10-03. The format stays as implemented; `docs/DESIGN.md` describes it as supporting the single-quartet verification, not as enough to re-run the amalgamation, which the full verification re-runs from the data.

## OI-14: W2c on quartets whose best fit is a star (resolved, 2026-10-03)

- **Specification:** W2 fits each topology with branch lengths bounded in [0.000001, 10]; W2c is the fraction of 100 resamples in which each topology has the highest refitted log-likelihood, with ties (log-likelihoods within 10⁻⁸, `weight::TIE_TOLERANCE`) split equally.
- **Found:** when the data favour no resolution, the internal branch of all three topologies ends at the lower bound 0.000001. The three log-likelihoods then differ only through that tiny branch, by about 10⁻⁵ (more than the tie tolerance), and the same topology wins in most resamples. On Fish mtDNA (run with seed 1, strand filter on, 12,650 quartets): in 9,024 quartets all three fitted internal branches are at the lower bound, and 3,326 of these still give one topology a W2c weight of at least 0.9; the mean of the largest W2c weight over these 9,024 quartets is 0.789 instead of the 1/3 of a tie. An example is in a worksheet (quartet NC_009057, NC_009066, NC_011177, NC_013564: log-likelihoods −30890.494709, −30890.494670, −30890.494689, every internal branch 0.000001, W2c 0.00 / 0.97 / 0.03).
- **Measured effect on the tree (exploratory, Fish mtDNA, same counts and seeds):** nRF to the reference 0.500 as implemented; 0.455 when a resample whose three internal branches are all at the lower bound counts as a three-way tie; 0.773 when log-likelihoods within 0.001 count as ties. So the small differences are not pure noise; they carry the sign of the excess of split patterns, but W2c turns them into near-certain weights.
- **Options:** (a) keep W2c as specified and report this behaviour; (b) count a resample as a three-way tie when all three internal branches are at the lower bound (a star fit); (c) lower the bound of the internal branch to 0, so that a star fit gives exactly equal likelihoods (a change of the specified bounds); (d) another rule chosen by the owner. Options (b) to (d) change the weighting fixed in M5 and the pre-registered primary method's details, so H3 (M6) and the W2c cross-checks would be run again. **Owner decision needed.**
- **Controls (2026-10-02, seed 1, exploratory measurements of the three rules on the same counts and seeds):**

  | Data | Rule | nRF | Mean S1 | Quartets with weights 1/3 each |
  |---|---|---|---|---|
  | Fish mtDNA, each sequence shuffled (negative control) | as implemented | 1.000 | 0.649 | 0 of 12,650 |
  | | star resamples tied | 0.909 | 0.333 | 12,650 |
  | | ties within 0.001 | 0.909 | 0.333 | 12,650 |
  | Fish mtDNA | as implemented | 0.500 | 0.776 | 0 |
  | | star resamples tied | 0.455 | 0.577 | 7,765 |
  | | ties within 0.001 | 0.773 | 0.470 | 9,060 |
  | Simulated, 16 taxa, 20,000 bases, Jukes–Cantor on a random tree (positive control) | as implemented | 0.154 | 0.936 | 0 of 1,820 |
  | | star resamples tied | 0.308 | 0.655 | 1,159 |
  | | ties within 0.001 | 0.308 | 0.586 | 1,184 |

  On shuffled sequences, which share no history, W2c as implemented gives a mean S1 of 0.649; with either tie rule every quartet becomes a tie and S1 is 1/3, the value for no signal. On the simulated tree-like data, most quartets also have star fits (1,159 of 1,820), yet the tiny differences recover the tree better (nRF 0.154 against 0.308). So the two-state model fitted to MAW columns puts the internal branch at zero for most quartets, and the remaining signal sits in differences of about 10⁻⁵; W2c as implemented uses that signal but overstates its certainty.
- **Resolution:** the owner chose (b) on 2026-10-03. Implemented in `weight::resample_winners` (milestone M8): a W2c resample whose three fits all have the internal branch at the lower bound is a three-way tie. The example quartet above now gets W2c 1/3 each. H3 (M6) was run again: W1 and W2 are unchanged, so the verdict stands; only the supplementary W2c column of `results/h3/` changed. The IQ-TREE cross-check compares log-likelihoods, which this rule does not change; it runs again on GitHub Actions because `weight.rs` changed. The pre-registration records the change in a dated amendment.
- **Controls with the rule (2026-10-03, seed 1, committed in `results/controls/`):** shuffled Fish mtDNA nRF 0.909, mean S1 0.333 (every edge), mean S2 0.001; Fish mtDNA nRF 0.455, mean S1 0.577; simulated nRF 0.308, mean S1 0.655. These equal the exploratory measurements of the rule above. The negative control now shows low support; the cost is a worse recovery of the simulated tree (nRF 0.308 against 0.154).
