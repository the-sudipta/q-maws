"""Statistics of the extended benchmark (docs/PREREGISTRATION.md, Amendments 8 and 9),
computed from results/benchmark/summary/accuracy.tsv (scripts/bench_collect.py).

Usage (from the repository folder):
  python scripts/bench_stats.py            H6, effect sizes and benchmark ranks
  python scripts/bench_stats.py --check    reproduce the published H5 test first

Written to results/benchmark/summary/:
  h6.tsv          H6: Q-MAWS minus ML-MAWS on the 30 new datasets (pre-registered test:
                  one-sided exact Wilcoxon signed-rank, zero differences dropped,
                  Holm across nRF and nQD), with the Hodges-Lehmann estimate, the
                  matched-pairs rank-biserial correlation and a bootstrap 95% CI
  medians.tsv     median nRF and nQD of every method per condition, with bootstrap
                  95% CIs and the number of finished datasets
  friedman.tsv    Friedman test and mean ranks on the datasets every method finished
  pairwise.tsv    two-sided Wilcoxon tests of every pair of methods, Holm-adjusted

The exact Wilcoxon distribution uses mid-ranks (doubled to integers) and is
computed by dynamic programming, so ties are handled exactly. Only the Python
standard library is used.
"""
import csv
import math
import os
import random
import sys
from collections import defaultdict

SUMMARY = os.path.join('results', 'benchmark', 'summary')
BOOT = 10000


def mid_ranks(values):
    order = sorted(range(len(values)), key=lambda i: values[i])
    ranks = [0.0] * len(values)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j + 1]] == values[order[i]]:
            j += 1
        for k in range(i, j + 1):
            ranks[order[k]] = (i + j) / 2 + 1
        i = j + 1
    return ranks


def clean(diffs):
    """Differences of values stored with 6 decimals: round away float noise, so equal
    differences tie exactly (as in the Rust implementation of H1 and H5)."""
    return [round(x, 6) for x in diffs]


def signed_rank(diffs):
    """W+ and its exact null distribution (zeros dropped): (n, w_plus, dist over 2*W+)."""
    d = [x for x in clean(diffs) if x != 0]
    r2 = [int(round(2 * r)) for r in mid_ranks([abs(x) for x in d])]
    w2 = sum(r for r, x in zip(r2, d) if x > 0)
    dist = {0: 1}
    for r in r2:
        nxt = defaultdict(int)
        for s, c in dist.items():
            nxt[s] += c
            nxt[s + r] += c
        dist = nxt
    return len(d), w2, dist


def p_less(diffs):
    """One-sided exact p for 'differences tend to be below 0' (small W+)."""
    n, w2, dist = signed_rank(diffs)
    if n == 0:
        return 1.0
    total = 2 ** n
    return sum(c for s, c in dist.items() if s <= w2) / total


def p_two_sided(diffs):
    n, w2, dist = signed_rank(diffs)
    if n == 0:
        return 1.0
    total = 2 ** n
    lo = sum(c for s, c in dist.items() if s <= w2) / total
    hi = sum(c for s, c in dist.items() if s >= w2) / total
    return min(1.0, 2 * min(lo, hi))


def rank_biserial(diffs):
    """Matched-pairs rank-biserial correlation: (W+ - W-) / (W+ + W-); -1 = all below 0."""
    d = [x for x in clean(diffs) if x != 0]
    if not d:
        return 0.0
    r = mid_ranks([abs(x) for x in d])
    wp = sum(ri for ri, x in zip(r, d) if x > 0)
    wm = sum(ri for ri, x in zip(r, d) if x < 0)
    return (wp - wm) / (wp + wm)


def holm(ps):
    order = sorted(range(len(ps)), key=lambda i: ps[i])
    adj, run = [0.0] * len(ps), 0.0
    for k, i in enumerate(order):
        run = max(run, min(1.0, (len(ps) - k) * ps[i]))
        adj[i] = run
    return adj


def median(v):
    s = sorted(v)
    n = len(s)
    if n == 0:
        return float('nan')
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


def hodges_lehmann(d):
    w = [(d[i] + d[j]) / 2 for i in range(len(d)) for j in range(i, len(d))]
    return median(w)


def boot_ci(v, stat=median, seed=1):
    if len(v) < 2:
        return float('nan'), float('nan')
    rng = random.Random(seed)
    b = sorted(stat([rng.choice(v) for _ in v]) for _ in range(BOOT))
    return b[int(0.025 * BOOT)], b[int(0.975 * BOOT) - 1]


def chi2_sf(x, k):
    """Upper tail of the chi-square distribution (regularized gamma Q(k/2, x/2))."""
    a, x = k / 2, x / 2
    if x <= 0:
        return 1.0
    if x < a + 1:  # series for P, then Q = 1 - P
        term = s = 1 / a
        n = a
        while abs(term) > 1e-15 * abs(s):
            n += 1
            term *= x / n
            s += term
        return max(0.0, 1 - s * math.exp(-x + a * math.log(x) - math.lgamma(a)))
    b, c, d = x + 1 - a, 1e300, 1 / (x + 1 - a)  # continued fraction for Q
    h = d
    for i in range(1, 1000):
        an = -i * (i - a)
        b += 2
        d = an * d + b
        d = 1 / (d if abs(d) > 1e-300 else 1e-300)
        c = b + an / c
        c = c if abs(c) > 1e-300 else 1e-300
        h *= d * c
        if abs(d * c - 1) < 1e-15:
            break
    return h * math.exp(-x + a * math.log(x) - math.lgamma(a))


def load():
    acc = {}
    for r in csv.DictReader(open(os.path.join(SUMMARY, 'accuracy.tsv'), encoding='utf-8'), delimiter='\t'):
        name = r['method'] + ('_tuned' if r['setting'] == 'tuned' else '')
        acc[(r['dataset'], name)] = r
    return acc


def fmt(x, nd=4):
    return 'NA' if x is None or (isinstance(x, float) and math.isnan(x)) else f'{x:.{nd}f}'


def paired_test(acc, datasets, a, b, out, label):
    """Pre-registered H-test of a minus b on datasets (both must be done)."""
    rows, ps, diffs_all = [], [], {}
    for m in ('nrf', 'nqd'):
        d = []
        for ds in datasets:
            ra, rb = acc.get((ds, a)), acc.get((ds, b))
            if ra and rb and ra['status'] == rb['status'] == 'done':
                d.append(float(ra[m]) - float(rb[m]))
        diffs_all[m] = d
        ps.append(p_less(d))
    adj = holm(ps)
    for (m, d), p, pa in zip(diffs_all.items(), ps, adj):
        lo, hi = boot_ci(d)
        ok = pa < 0.05 and median(d) < 0
        rows.append([label, m, len(d), sum(1 for x in d if x < 0), sum(1 for x in d if x == 0),
                     sum(1 for x in d if x > 0), fmt(median(d)), fmt(lo), fmt(hi), fmt(hodges_lehmann(d)),
                     fmt(rank_biserial(d), 3), fmt(p), fmt(pa), 'yes' if ok else 'no'])
    with open(out, 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(['comparison', 'measure', 'n', 'lower', 'equal', 'higher', 'median_diff', 'ci_low', 'ci_high',
                    'hodges_lehmann', 'rank_biserial', 'p_one_sided', 'p_holm', 'criterion_met'])
        w.writerows(rows)
    return rows


def check_h5():
    """Reproduces the published H5 test (results/h5/h5_test.tsv) from h5_values.tsv."""
    v = list(csv.DictReader(open(os.path.join('results', 'h5', 'h5_values.tsv'), encoding='utf-8'), delimiter='\t'))
    out = []
    for who in ('v2', 'primary'):
        ps = [p_less([float(r[f'{who}_{m}']) - float(r[f'ml_maws_{m}']) for r in v]) for m in ('nrf', 'nqd')]
        out.append((who, holm(ps)))
    for who, adj in out:
        print(f'  H5 check, {who} minus ML-MAWS: Holm p nRF {adj[0]:.3f}, nQD {adj[1]:.3f}')


def main():
    if '--check' in sys.argv:
        check_h5()
    acc = load()
    new = sorted({ds for ds, _ in acc if ds.startswith('bm_')})
    rows = paired_test(acc, new, 'qmaws', 'ml_maws', os.path.join(SUMMARY, 'h6.tsv'), 'Q-MAWS minus ML-MAWS (H6)')
    for r in rows:
        print('  H6', r[1], 'n', r[2], 'median', r[6], 'CI', r[7], r[8], 'Holm p', r[12], 'met', r[13])

    # medians per condition
    methods = sorted({m for _, m in acc})
    groups = defaultdict(list)
    for (ds, m), r in acc.items():
        groups[(r['group'], m)].append(r)
    with open(os.path.join(SUMMARY, 'medians.tsv'), 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(['group', 'method', 'datasets', 'done', 'failed', 'pending',
                    'median_nrf', 'nrf_ci_low', 'nrf_ci_high', 'median_nqd', 'nqd_ci_low', 'nqd_ci_high'])
        for (g, m), rs in sorted(groups.items()):
            done = [r for r in rs if r['status'] == 'done']
            line = [g, m, len(rs), len(done), sum(r['status'] == 'failed' for r in rs),
                    sum(r['status'] == 'pending' for r in rs)]
            for k in ('nrf', 'nqd'):
                v = [float(r[k]) for r in done if r[k] != '']
                lo, hi = boot_ci(v)
                line += [fmt(median(v)), fmt(lo), fmt(hi)]
            w.writerow(line)

    # Friedman test and pairwise tests on the datasets every compared method finished
    # (alignment-free methods only; the references are reported, not ranked)
    af = [m for m in methods if m not in ('iqtree_true', 'caster_site', 'treeqmc', 'wtreeqmc')]
    datasets = sorted({ds for ds, _ in acc})
    complete = [ds for ds in datasets if all(acc.get((ds, m), {}).get('status') == 'done' for m in af)]
    with open(os.path.join(SUMMARY, 'friedman.tsv'), 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(['measure', 'datasets', 'methods', 'chi2', 'p'] + [f'rank_{m}' for m in af])
        for k in ('nrf', 'nqd'):
            n, kk = len(complete), len(af)
            if n < 2:
                w.writerow([k, n, kk, 'NA', 'NA'] + ['NA'] * kk)
                continue
            sums = [0.0] * kk
            for ds in complete:
                r = mid_ranks([float(acc[(ds, m)][k]) for m in af])
                sums = [s + x for s, x in zip(sums, r)]
            chi2 = 12 / (n * kk * (kk + 1)) * sum(s * s for s in sums) - 3 * n * (kk + 1)
            w.writerow([k, n, kk, fmt(chi2, 3), fmt(chi2_sf(chi2, kk - 1), 6)] + [fmt(s / n, 2) for s in sums])
    pw = []
    for k in ('nrf', 'nqd'):
        for i, a in enumerate(af):
            for b in af[i + 1:]:
                d = [float(acc[(ds, a)][k]) - float(acc[(ds, b)][k]) for ds in complete]
                pw.append([k, a, b, len(d), fmt(median(d)), fmt(rank_biserial(d), 3), p_two_sided(d)])
    adj = holm([r[6] for r in pw]) if pw else []
    with open(os.path.join(SUMMARY, 'pairwise.tsv'), 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(['measure', 'method_a', 'method_b', 'datasets', 'median_diff', 'rank_biserial', 'p', 'p_holm'])
        for r, pa in zip(pw, adj):
            w.writerow(r[:6] + [fmt(r[6]), fmt(pa)])
    print(f'  Friedman: {len(complete)} datasets finished by all {len(af)} alignment-free methods')


if __name__ == '__main__':
    main()
