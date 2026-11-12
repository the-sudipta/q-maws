"""Dependence between MAW columns and the share of star quartets
(docs/PREREGISTRATION.md, Amendment 9; descriptive). Only the Python standard library.

Usage (from the repository folder; the matrices are made by `qmaws matrix`):
  python scripts/bench_dependence.py <name>=<matrix folder> [...]

For each matrix folder (m_ml.phy and m_ml_columns.txt of `qmaws matrix`: the matrix of
ML-MAWS, at most 50,000 non-constant columns; on Fish mtDNA it equals M_full):
  1. Overlap correlation: the phi coefficient between the 0/1 columns of two words that
     overlap by all but one letter (w = x u, v = u y, same length), against random pairs.
  2. Star quartets: 100 random quartets (seed 1). Each is fitted under the conditioned
     two-state (CFN) likelihood of Q-MAWS (pattern 0000 left out, five branch lengths in
     [1e-6, 10]); a quartet is a star when the internal branch of all three fits is at its
     lower bound (Q-MAWS's rule). Then, for every quartet, 10 pattern-count sets with the same
     number of columns are simulated from its best fit (parametric bootstrap, seed 1) and
     fitted again. If the columns were independent draws of the model, the two star shares
     would agree; a gap shows how far real MAW data are from the model.
Writes results/benchmark/summary/dependence.tsv.
"""
import csv
import math
import os
import random
import sys

MIN_T, MAX_T = 1e-6, 10.0
TOPOS = [((0, 1), (2, 3)), ((0, 2), (1, 3)), ((0, 3), (1, 2))]
PATTERNS = [tuple((x >> (3 - i)) & 1 for i in range(4)) for x in range(16)]


def read_matrix(folder):
    rows = open(os.path.join(folder, 'm_ml.phy'), encoding='utf-8').read().split('\n')
    n, m = map(int, rows[0].split())
    names, bits = [], []
    for line in rows[1:1 + n]:
        name, seq = line.split()
        names.append(name)
        bits.append(seq)
    words = open(os.path.join(folder, 'm_ml_columns.txt'), encoding='utf-8').read().split()
    if len(words) != m:
        raise SystemExit(f'{folder}: {len(words)} words for {m} columns')
    return names, bits, words


def column_ints(bits):
    """One integer per column; bit i = taxon i."""
    n, m = len(bits), len(bits[0])
    cols = [0] * m
    for i, row in enumerate(bits):
        for j, ch in enumerate(row):
            if ch == '1':
                cols[j] |= 1 << i
    return cols


def phi(a, b, n):
    n11 = bin(a & b).count('1')
    n1_, n_1 = bin(a).count('1'), bin(b).count('1')
    den = n1_ * (n - n1_) * n_1 * (n - n_1)
    return (n * n11 - n1_ * n_1) / math.sqrt(den) if den else None


def overlap(cols, words, n, rng):
    index = {w: j for j, w in enumerate(words)}
    pairs = []
    for j, w in enumerate(words):
        for y in 'ACGT':
            k = index.get(w[1:] + y)
            if k is not None and k != j:
                pairs.append((j, k))
    ov = [p for p in (phi(cols[a], cols[b], n) for a, b in pairs) if p is not None]
    rnd = []
    while len(rnd) < max(1000, len(ov)) and len(rnd) < 200000:
        a, b = rng.randrange(len(cols)), rng.randrange(len(cols))
        if a != b and len(words[a]) == len(words[b]):
            p = phi(cols[a], cols[b], n)
            if p is not None:
                rnd.append(p)
    return ov, rnd


# --- conditioned CFN quartet likelihood -------------------------------------
def pchange(t):
    return (1 - math.exp(-2 * t)) / 2


def pattern_probs(lengths, topo):
    (a, b), (c, d) = topo
    p = [pchange(t) for t in lengths]  # leaf edges of taxa 0..3, then internal
    out = []
    for x in PATTERNS:
        s = 0.0
        for su in (0, 1):
            for sv in (0, 1):
                q = 0.5
                q *= p[a] if x[a] != su else 1 - p[a]
                q *= p[b] if x[b] != su else 1 - p[b]
                q *= p[4] if sv != su else 1 - p[4]
                q *= p[c] if x[c] != sv else 1 - p[c]
                q *= p[d] if x[d] != sv else 1 - p[d]
                s += q
        out.append(s)
    return out


def loglik(lengths, topo, counts):
    pr = pattern_probs(lengths, topo)
    z = 1 - pr[0]
    return sum(c * (math.log(max(pr[k], 1e-300)) - math.log(z)) for k, c in enumerate(counts) if c and k)


def fit(topo, counts):
    """Coordinate ascent on log branch lengths (golden-section per coordinate)."""
    lo, hi = math.log(MIN_T), math.log(MAX_T)
    x = [math.log(0.1)] * 5
    best = loglik([math.exp(v) for v in x], topo, counts)
    for _ in range(8):
        old = best
        for i in range(5):
            def f(v):
                y = list(x)
                y[i] = v
                return loglik([math.exp(u) for u in y], topo, counts)
            a, b = lo, hi
            g = (math.sqrt(5) - 1) / 2
            c1, c2 = b - g * (b - a), a + g * (b - a)
            f1, f2 = f(c1), f(c2)
            for _ in range(25):
                if f1 < f2:
                    a, c1, f1 = c1, c2, f2
                    c2 = a + g * (b - a)
                    f2 = f(c2)
                else:
                    b, c2, f2 = c2, c1, f1
                    c1 = b - g * (b - a)
                    f1 = f(c1)
            cand = [(f(lo), lo), (f1, c1), (f2, c2)]
            fv, v = max(cand)
            if fv > best:
                best, x[i] = fv, v
        if best - old < 1e-6:
            break
    return [math.exp(v) for v in x], best


def is_star(fits):
    return all(f[0][4] <= MIN_T * 1.0001 for f in fits)


def counts_of(cols, q):
    c = [0] * 16
    for col in cols:
        k = 0
        for t in q:
            k = (k << 1) | ((col >> t) & 1)
        c[k] += 1
    c[0] = 0
    return c


def simulate(lengths, topo, total, rng):
    pr = pattern_probs(lengths, topo)
    z = 1 - pr[0]
    cum, s = [], 0.0
    for k in range(1, 16):
        s += pr[k] / z
        cum.append(s)
    c = [0] * 16
    for _ in range(total):
        u = rng.random() * s
        lo, hi = 0, 14
        while lo < hi:
            mid = (lo + hi) // 2
            if cum[mid] < u:
                lo = mid + 1
            else:
                hi = mid
        c[lo + 1] += 1
    return c


def main():
    out = []
    for arg in sys.argv[1:]:
        name, folder = arg.split('=', 1)
        rng = random.Random(1)
        names, bits, words = read_matrix(folder)
        n = len(names)
        cols = column_ints(bits)
        ov, rnd = overlap(cols, words, n, rng)
        mean = lambda v: sum(v) / len(v) if v else float('nan')
        quartets = [tuple(sorted(rng.sample(range(n), 4))) for _ in range(100)]
        real_star, sim_star, sim_total = 0, 0, 0
        for q in quartets:
            c = counts_of(cols, q)
            fits = [fit(t, c) for t in TOPOS]
            real_star += is_star(fits)
            best = max(range(3), key=lambda i: fits[i][1])
            for _ in range(10):
                sc = simulate(fits[best][0], TOPOS[best], sum(c), rng)
                sim_star += is_star([fit(t, sc) for t in TOPOS])
                sim_total += 1
        row = [name, n, len(cols), len(ov), f'{mean(ov):.4f}', f'{mean([abs(x) for x in ov]):.4f}',
               len(rnd), f'{mean(rnd):.4f}', f'{mean([abs(x) for x in rnd]):.4f}',
               len(quartets), f'{real_star / len(quartets):.3f}', f'{sim_star / sim_total:.3f}']
        out.append(row)
        print('  ', row, flush=True)
    path = os.path.join('results', 'benchmark', 'summary', 'dependence.tsv')
    with open(path, 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(['dataset', 'taxa', 'columns', 'overlap_pairs', 'overlap_phi', 'overlap_abs_phi',
                    'random_pairs', 'random_phi', 'random_abs_phi', 'quartets', 'star_share_real',
                    'star_share_simulated'])
        w.writerows(out)


if __name__ == '__main__':
    main()
