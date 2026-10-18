# docs

## Purpose
Documentation for readers, reviewers and users of Q-MAWS.

## Where to start

| If you want to… | Read |
|---|---|
| Find the method, the GUI, the CLI or the CI in the code | `ARCHITECTURE.md` |
| Understand the mathematics of the method | `METHOD.md` |
| See what was planned before the experiments, and every dated change | `PREREGISTRATION.md` |
| Follow the project step by step | `milestones/` |
| Use the program | `USER_GUIDE.md` |

## Contents

The table lists every item in this folder, grouped by purpose.

| Item | Description |
|---|---|
| **For readers and reviewers** | |
| `ARCHITECTURE.md` | Where everything is: each step of the method mapped to its file and functions, the program parts (science, engine, data, figures, GUI, terminal menu, CLI), the CI, and the evidence |
| `METHOD.md` | The mathematics of the method: MAWs, matrix, quartet counts, conditioned likelihood and weights, amalgamation, support, evaluation measures, reproducibility |
| **The scientific record** | |
| `PREREGISTRATION.md` | Hypotheses, tests and success criteria fixed before any experiment, with seven dated amendments |
| `milestones/` | One report per milestone, M0 to M14: what was built, how it was checked, decisions ([README](milestones/README.md)) |
| `OPEN_ISSUES.md` | Problems, ambiguities and discrepancies found during the project, with the options and the decisions |
| **For users** | |
| `USER_GUIDE.md` | Starting the program, the window, the terminal menu, every command, figures, verifying a run, the benchmark datasets |
| `TEACHING.md` | How to use `qmaws teach` in a classroom |
| `images/` | Screenshots used in the user guide |
| **For developers** | |
| `DESIGN.md` | Software design decisions and their reasons |
| `DEPENDENCIES.md` | Every Rust crate used, with its licence and the reason |
| `EXTERNAL_TOOLS.md` | External tools, services and repositories, with what was checked about each |

## Relationships
Describes the code in `crates/`, the data in `data/` and the results in `results/`. Milestone reports cite measurements from logged runs.

## Notes
Every number comes from a logged run or a cited source.
