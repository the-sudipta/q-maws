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
