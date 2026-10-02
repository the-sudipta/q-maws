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

## OI-6: Influenza A sequence lengths (open, 2026-10-02)

- **Specification:** the Influenza A dataset (38 sequences) has an average length of 13 kb (from ML-MAWS Table 2); a later publication describes a 38-sequence Influenza A dataset as neuraminidase segment sequences.
- **Found:** the file ML-MAWS used (`Data/influenza.fasta`, commit `0c38db1`) has 38 records with a mean of 1,407 A/C/G/T letters per record. Headers are strain names with subtypes, for example `A/duck/Hong Kong/319/1978(H2N2)`. This is consistent with single-segment sequences, not with whole genomes of the 13 kb listed in Table 2.
- **Options:** (a) use the file as it is (it is what ML-MAWS ran on) and report the measured lengths next to the Table 2 value; (b) also assemble whole-genome sequences for these strains as a separate, labelled "reconstructed" dataset (decision D5). Recommendation: (a), with the discrepancy stated neutrally in `baselines/README.md` when baselines are collected (M11).

## OI-7: Taxon names from FASTA headers that contain spaces (open, 2026-10-02)

- **Specification:** a FASTA record's taxon name is the first word of its header; duplicate names produce a warning with the choices "rename automatically (append `_2`)" or "abort".
- **Found:** in the ML-MAWS data files, 25 of 38 influenza headers and 18 of 41 mammal mtDNA headers contain spaces. Taking the first word gives duplicate names: `A/American` (3 records), `A/wild` (2) and `Common` (2), so 38 records give 35 distinct names and 41 give 40. ML-MAWS's reader uses the first word in the same way.
- **Options:** (a) keep the rule; renaming then produces names such as `A/American_2` that no longer say which strain is meant; (b) for `record_per_taxon` datasets, use the whole header line with unsafe characters replaced by `_` (for example `A/American_black_duck/NB/2538/2007_H7N3_`), which gives unique, readable names; (c) a name table per dataset. Choosing (b) changes names relative to ML-MAWS, which matters only for the M3 matrix comparison (column and row contents stay the same). Recommendation: (b). **Owner decision needed.**

## OI-8: Several records in one file (decision D13, open, 2026-10-02)

- **Specification:** default one taxon per file, records concatenated in file order; the owner decides per D13.
- **Found:** ML-MAWS (`FastaReader.cpp`) treats a folder as one taxon per file, joining the records of a multi-record file and naming the taxon after the file; a single multi-FASTA file given directly is read as one taxon per record. No AFproject file has more than one record; the five ML-MAWS data files are multi-FASTA files with one taxon per record.
- **Implemented now:** folders default to one taxon per file (records joined; a single-record file keeps its header name), with `--records per-record` to choose one taxon per record; a single file is always read as one taxon per record. This matches ML-MAWS. **Owner decision D13 needed** to confirm or change the folder default.
