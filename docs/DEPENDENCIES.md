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
| `toml` | 1.1.6 | MIT OR Apache-2.0 | `qmaws-data` | Reading the benchmark registry `benchmarks.toml` | 2026-10-02 |
| `ureq` (features `rustls-no-provider`, `_ring`, `platform-verifier`; no default features) | 3.4.2 | MIT OR Apache-2.0 | `qmaws-data` | HTTPS downloads with range requests (resume), pure-Rust TLS (rustls with ring) verified against the operating system's certificate store | 2026-10-02 |
| `md-5` | 0.11.0 | MIT OR Apache-2.0 | `qmaws-data` | MD5 checksums, as published by AFproject | 2026-10-02 |
| `zip` (feature `deflate` only) | 8.6.0 | MIT | `qmaws-data` | Extracting dataset archives (newest stable release; 9.0.0 is a pre-release) | 2026-10-02 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 | `qmaws-data` | Reading `.gz` sequence files | 2026-10-02 |
| `qmaws-core` (workspace) | – | Q-MAWS license | `qmaws-data` | Input cleaning and Newick parsing | – |

`ureq` is used without its default features, because the default adds `webpki-roots`, whose certificate data is licensed CDLA-Permissive-2.0, a license not on the allowed list. Certificates are instead verified by the operating system (`rustls-platform-verifier`).

## Indirect dependencies

The direct dependencies bring in further crates; 100 crates in total are used to build Q-MAWS for the release platforms. Each is listed with its license, authors and source in `THIRD_PARTY_NOTICES`. Declared licenses (2026-10-02): MIT, Apache-2.0, ISC, BSD-3-Clause, Zlib and Unicode-3.0, alone or as alternatives; `ring` is "Apache-2.0 AND ISC"; `memchr` is "Unlicense OR MIT" and is used under MIT; `adler2` is "0BSD OR MIT OR Apache-2.0" and is used under MIT. `cargo deny check licenses bans sources` passes.

## Considered and not used

| Crate | Reason |
|---|---|
| `chrono`, `time` | Only a UTC date and time are needed (run identifiers, log lines); a short tested function in `qmaws-engine` does this without a dependency |
