# Dependencies

Every crate used by the workspace is listed here before it is added, with its license and the reason for using it.

## License policy

| Allowed | Forbidden |
|---|---|
| MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0 (file-level, unmodified) | GPL, LGPL, AGPL, SSPL, and any license that would require the Q-MAWS source to be released under its terms |

Pure-Rust crates are preferred to keep cross-compilation simple. The policy is enforced in continuous integration by `cargo deny` with `deny.toml` (licenses, banned and duplicate crates, allowed sources). A crate offered under a choice of licenses is accepted when one of the choices is allowed.

## Direct dependencies

Versions are those in `Cargo.lock`. "Maintenance checked" means: the version used is the newest release on crates.io on the stated date, and the crate has a public repository.

| Crate | Version | License | Used by | Reason | Maintenance checked |
|---|---|---|---|---|---|
| `serde` (feature `derive`) | 1.0.229 | MIT OR Apache-2.0 | `qmaws-engine` | Serialisation of `run.json` and other records | 2026-10-02 |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | `qmaws-engine`, `qmaws-tui`; tests of `qmaws-cli` | JSON for `run.json`, audit files and `--json-progress` output | 2026-10-02 |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 | `qmaws-engine` | SHA-256 of every output file and of the root fingerprint (pure Rust, RustCrypto) | 2026-10-02 |
| `indicatif` | 0.18.6 | MIT | `qmaws-tui` | Terminal progress bars (overall and stage bars, log lines above the bars) | 2026-10-02 |
| `clap` (feature `derive`) | 4.6.7 | MIT OR Apache-2.0 | `qmaws-cli` | Command-line parsing, help and version output | 2026-10-02 |
| `ctrlc` | 3.5.2 | MIT/Apache-2.0 | `qmaws-cli` | Ctrl+C handling on Windows, macOS and Linux for clean stopping | 2026-10-02 |

## Indirect dependencies

The direct dependencies bring in 46 further crates for the release platforms (52 in total). Each of them is listed with its license, authors and source in `THIRD_PARTY_NOTICES`. Their declared licenses are MIT, Apache-2.0, Zlib or Unicode-3.0, alone or as alternatives; `memchr` is offered as "Unlicense OR MIT" and is used under MIT.

## Considered and not used

| Crate | Reason |
|---|---|
| `chrono`, `time` | Only a UTC date and time are needed (run identifiers, log lines); a short tested function in `qmaws-engine` does this without a dependency |
