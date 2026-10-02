# Teaching

`qmaws teach` prints a worksheet that students can work through by hand: every step of a simplified Q-MAWS analysis with every arithmetic operation written out.

## Commands

| Command | What it prints |
|---|---|
| `qmaws teach --example` | The worksheet of the built-in example: five sequences of six letters (taxa K, L, M, N, P), compared with the reference tree `((K,L),M,(N,P));`, plus a control tree and a small model calculation |
| `qmaws teach --input "<folder>" [--reference "<tree.nwk>"]` | The worksheet for your own sequences: at most 8 taxa and at most 200 letters per sequence (larger inputs are refused, because the worksheet is meant to be done by hand) |

Redirect the output to a file to print it, for example `qmaws teach --example > worksheet.txt`.

## Steps of the worksheet

1. **Input**: the sequences.
2. **Neighbouring pairs**: every pair of adjacent letters (a sequence of length z has z − 1 pairs).
3. **Two-letter minimal absent words**: xy is a MAW of s if x and y occur in s but xy is not a neighbouring pair; words that are a MAW of every sequence or of none give constant columns and are removed.
4. **Matrix**: 1 if the word is a MAW of the sequence, 0 otherwise.
5. **Quartets**: Q = m(m − 1)(m − 2)(m − 3) / 24 groups of four taxa.
6. **Pattern table**: for each word and quartet, the four matrix values in name order.
7. **Votes and weights (W1)**: ab|cd is supported by 1100 and 0011, ac|bd by 1010 and 0101, ad|bc by 1001 and 0110; the winner's weight is its votes divided by all votes. Other patterns (one taxon differs, all equal) cast no vote.
8. **Classroom amalgamation**: each winning topology adds its weight to its two pairs; the pair with the highest score is merged (ties: alphabetical order); the new group's score with another group is the average of the two merged scores; this stops at three groups, which give the tree.
9. **Evaluation**: the splits of the tree and the normalised Robinson–Foulds distance to the reference.

## Notes for teachers

- The worksheet uses only two-letter words, no strand filter and no length selection, so it can be checked by hand. The program checks (golden test G2) that its full method, with the same settings, gives the same matrix and pattern counts.
- Votes ignore the one-taxon-differs patterns; the teaching example of long branches shows why the full method uses a likelihood model instead (the model calculation at the end of the example worksheet is its first step).
- Use in teaching or courses is Institutional Use under the Q-MAWS license (`LICENSE`, Section 1 and 4.1).
