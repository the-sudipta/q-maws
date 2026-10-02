# Contributing

Q-MAWS is distributed under a source-available license (`LICENSE`). Creating derivative works requires a written license from the copyright holder. These rules describe how changes are made in this repository.

## Before every commit

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. The pre-commit guard must pass (installed once with `scripts/install-hooks.sh` or `scripts\install-hooks.bat`).

Broken code is never committed.

## Commits

- One commit per small, complete, coherent change.
- Commit timestamps are the real time of the commit.
- Pushed history is never rewritten.
- Message format:

```
<gitmoji> : <Imperative title, at most 72 characters, no trailing period>

What:
- <What changed.>

Why:
- <The reason for the change.>

How:
- <Key implementation details or decisions.>

Tests:
- <Each test run and its result.>

Milestone: <Mxx (name)>
Limitations: <Known limitations, or "None known.">
```

Gitmoji used: 🎉 initial setup, ✨ feature, 🐛 bug fix, ✅ tests, ⚡️ performance, 📝 documentation, ♻️ refactor, 📊 experiments and results, 🎨 figures and GUI appearance, 🔧 configuration, 🙈 ignore rules, 🔖 release, 👷 CI, 🔒️ security and integrity, 🌐 data download and network, 🧪 experimental or simulation code.

## Documentation

- Every folder has a `README.md` with the sections Purpose, Contents, Relationships and Notes.
- A commit that adds, removes or renames a file also updates the README of that folder.
- All program-facing text is English.

## Never commit

- Files larger than 50 MB, downloaded or generated data (`data/raw/`), run work folders (`work/`), checkpoints, or secrets.
- Attribution lines or trailers naming tools that helped write code or text.

## Numbers

Every number in results, tables, figures or documentation comes from a logged run or a cited published source. Numbers are never estimated or read from figures.
