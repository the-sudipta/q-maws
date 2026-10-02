# Dependencies

Every crate used by the workspace is listed here before it is added, with its license and the reason for using it.

## License policy

| Allowed | Forbidden |
|---|---|
| MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode, MPL-2.0 (file-level, unmodified) | GPL, LGPL, AGPL, SSPL, and any license that would require the Q-MAWS source to be released under its terms |

Pure-Rust crates are preferred to keep cross-compilation simple. The policy will be enforced in continuous integration with `cargo deny` (from M1).

## Crates in use

None. As of milestone M0 the workspace uses only the Rust standard library.

| Crate | Version | License | Used by | Reason | Maintenance checked |
|---|---|---|---|---|---|
