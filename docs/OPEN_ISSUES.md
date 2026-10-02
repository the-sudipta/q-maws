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
