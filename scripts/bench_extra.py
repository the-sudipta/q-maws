"""Descriptive analyses of finished results (docs/PREREGISTRATION.md, Amendment 9).
No new runs; only the Python standard library.

Usage (from the repository folder):
  python scripts/bench_extra.py

Written to results/benchmark/summary/:
  calibration.tsv       ECE (10 equal-width bins, as H4) of Q-MAWS S1, Q-MAWS S2 and ML-MAWS
                        UFBoot, with bootstrap 95% CIs over branches, on the H4 branches
                        (reproduces H4) and on the branches of the 20 H5 and 30 new datasets
  calibration_bins.tsv  reliability-diagram bins of the same sets
  calibration_power.tsv the ECE difference S1 minus UFBoot, its bootstrap SE, and the
                        smallest difference the set detects with 80% power (2.8 SE)
  halo.tsv              per taxon on the simulated datasets: halo value and the share of
                        quartets containing the taxon that the Q-MAWS tree resolves wrongly
  halo_summary.tsv      Spearman correlation and AUC (halo predicting a taxon in a wrong
                        quartet) per dataset group
  wrong_splits.tsv      for Q-MAWS and ML-MAWS: wrong branches and the size of their smaller
                        side (deep errors cost many quartets: the nRF/nQD relation)
"""
import csv
import itertools
import math
import os
import random
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
from bench_stats import boot_ci, median, mid_ranks  # noqa: E402

SUM = os.path.join('results', 'benchmark', 'summary')
H5 = [f'hgt_h{h}_r{r}' for h in ('000', '010', '020', '040') for r in range(1, 6)]
NEW = [f'bm_{c}_r{r}' for c in ('div005', 'div020', 'div050', 'm15', 'm60', 'frag') for r in range(1, 6)]
AF_SIM = ['sim_hgt_0', 'sim_hgt_250', 'sim_hgt_500', 'sim_hgt_750', 'sim_hgt_1000']
BOOT = 10000


# --- Newick ------------------------------------------------------------------
def parse(text):
    """Returns (children dict, labels dict, root id, leaf names by id)."""
    text = text.strip().rstrip(';')
    children, label, names = {}, {}, {}
    stack, cur, nid, i = [], None, 0, 0
    root = nid
    children[root] = []
    cur = root
    tok = re.compile(r'[^(),:;]+')
    while i < len(text):
        c = text[i]
        if c == '(':
            nid += 1
            children[nid] = []
            children[cur].append(nid)
            stack.append(cur)
            cur = nid
            i += 1
        elif c == ',':
            i += 1
        elif c == ')':
            i += 1
            m = tok.match(text, i)
            if m:
                label[cur] = m.group().strip()
                i = m.end()
            if i < len(text) and text[i] == ':':
                m = re.compile(r':[^(),;]*').match(text, i)
                i = m.end()
            cur = stack.pop() if stack else cur
        else:
            m = tok.match(text, i)
            nid += 1
            children[nid] = []
            names[nid] = m.group().strip()
            children[cur].append(nid)
            i = m.end()
            if i < len(text) and text[i] == ':':
                m = re.compile(r':[^(),;]*').match(text, i)
                i = m.end()
    if text.startswith('(') and stack == [] and root in children and len(children[root]) == 1:
        root = children[root][0]
    return children, label, root, names


def splits(text):
    """Non-trivial splits {frozenset(side without the first taxon): support or None}, and taxa."""
    ch, lab, root, names = parse(text)
    below = {}

    def walk(n):
        if n in names:
            below[n] = frozenset([names[n]])
        else:
            s = frozenset()
            for c in ch[n]:
                s |= walk(c)
            below[n] = s
        return below[n]

    taxa = walk(root)
    first = min(taxa)
    out = {}
    for n, s in below.items():
        if n == root or n in names or len(s) < 2 or len(taxa) - len(s) < 2:
            continue
        side = s if first not in s else taxa - s
        sup = lab.get(n)
        try:
            sup = float(sup) if sup not in (None, '') else None
        except ValueError:
            sup = None
        out[side] = sup
    return out, taxa


def read(path):
    return open(path, encoding='utf-8').read()


# --- calibration -------------------------------------------------------------
def edges(est, truth, scale=1.0):
    e, _ = splits(read(est))
    t, _ = splits(read(truth))
    return [(s / scale, side in t) for side, s in e.items() if s is not None]


def ece(edges_, bins=10):
    n = len(edges_)
    tot = 0.0
    for b in range(bins):
        lo, hi = b / bins, (b + 1) / bins
        sel = [(s, ok) for s, ok in edges_ if lo <= s < hi or (b == bins - 1 and s == 1.0)]
        if sel:
            tot += len(sel) / n * abs(sum(s for s, _ in sel) / len(sel) - sum(ok for _, ok in sel) / len(sel))
    return tot


def bin_rows(name, edges_, bins=10):
    rows = []
    for b in range(bins):
        lo, hi = b / bins, (b + 1) / bins
        sel = [(s, ok) for s, ok in edges_ if lo <= s < hi or (b == bins - 1 and s == 1.0)]
        rows.append([name, f'{lo:.1f}', f'{hi:.1f}', len(sel),
                     f'{sum(s for s, _ in sel) / len(sel):.4f}' if sel else 'NA',
                     f'{sum(ok for _, ok in sel) / len(sel):.4f}' if sel else 'NA'])
    return rows


def boot_ece(edges_, seed=1):
    rng = random.Random(seed)
    v = sorted(ece([rng.choice(edges_) for _ in edges_]) for _ in range(BOOT))
    return v[int(0.025 * BOOT)], v[int(0.975 * BOOT) - 1], v


def calibration():
    sets = {}
    h4 = os.path.join('results', 'h4')
    ref = os.path.join('data', 'references', 'sim_hgt.nwk')
    sets['H4 (AFproject simulated, 150 branches)'] = {
        'qmaws_s1': sum((edges(os.path.join('results', 'runs', f'{d}_seed1', 'trees', 'tree_s1.nwk'), ref) for d in AF_SIM), []),
        'qmaws_s2': sum((edges(os.path.join('results', 'runs', f'{d}_seed1', 'trees', 'tree_s2.nwk'), ref) for d in AF_SIM), []),
        'ml_maws_ufboot': sum((edges(os.path.join(h4, 'mlmaws', d, 'ML_MAWS_tree.newick'), ref, 100) for d in AF_SIM), []),
    }
    s1, uf = [], []
    for ds in H5 + NEW:
        truth = os.path.join('data', 'manifests', 'simulated', 'trees', ds + '.nwk')
        run = os.path.join('results', 'h5', ds, 'qmaws') if ds in H5 else os.path.join('results', 'benchmark', ds, 'qmaws')
        ml = os.path.join('results', 'h5', ds, 'ml_maws', 'ML_MAWS_tree.newick') if ds in H5 else \
            os.path.join('results', 'benchmark', ds, 'ml_maws', 'ML_MAWS_tree.newick')
        if not os.path.exists(ml):
            ml = os.path.join('results', 'benchmark', ds, 'ml_maws', 'ml_maws_iqtree.treefile')
        s1 += edges(os.path.join(run, 'trees', 'tree_s1.nwk'), truth)
        uf += edges(ml, truth, 100)
    sets['H5 and new data (50 datasets)'] = {'qmaws_s1': s1, 'ml_maws_ufboot': uf}

    rows, bins, power = [], [], []
    for name, d in sets.items():
        boots = {}
        for m, e in d.items():
            lo, hi, boots[m] = boot_ece(e)
            rows.append([name, m, len(e), sum(ok for _, ok in e), f'{ece(e):.6f}', f'{lo:.4f}', f'{hi:.4f}'])
            bins += bin_rows(f'{name} | {m}', e)
        diff = ece(d['qmaws_s1']) - ece(d['ml_maws_ufboot'])
        rng1, rng2 = random.Random(2), random.Random(3)
        dv = [ece([rng1.choice(d['qmaws_s1']) for _ in d['qmaws_s1']]) -
              ece([rng2.choice(d['ml_maws_ufboot']) for _ in d['ml_maws_ufboot']]) for _ in range(2000)]
        mu = sum(dv) / len(dv)
        se = math.sqrt(sum((x - mu) ** 2 for x in dv) / (len(dv) - 1))
        dv.sort()
        power.append([name, f'{diff:.6f}', f'{dv[50]:.4f}', f'{dv[1949]:.4f}', f'{se:.4f}', f'{2.8 * se:.4f}'])
    write('calibration.tsv', ['set', 'support', 'branches', 'in_true_tree', 'ece', 'ci_low', 'ci_high'], rows)
    write('calibration_bins.tsv', ['set_support', 'bin_lower', 'bin_upper', 'branches', 'mean_support', 'fraction_true'], bins)
    write('calibration_power.tsv', ['set', 'ece_s1_minus_ufboot', 'ci_low', 'ci_high', 'bootstrap_se', 'detectable_80pct'], power)
    for r in rows:
        print('  calibration', r)


# --- quartets ----------------------------------------------------------------
def leaf_dist(text):
    """Unweighted path lengths between all leaves (the tree is unrooted)."""
    ch, _, root, names = parse(text)
    adj = {n: [] for n in ch}
    for p, cs in ch.items():
        for c in cs:
            adj[p].append(c)
            adj[c].append(p)
    leaves = {names[n]: n for n in names}
    dist = {}
    for a, na in leaves.items():
        seen = {na: 0}
        todo = [na]
        while todo:
            x = todo.pop()
            for y in adj[x]:
                if y not in seen:
                    seen[y] = seen[x] + 1
                    todo.append(y)
        dist[a] = {b: seen[nb] for b, nb in leaves.items()}
    return dist


def topo(d, a, b, c, e):
    s = [d[a][b] + d[c][e], d[a][c] + d[b][e], d[a][e] + d[b][c]]
    m = min(s)
    return s.index(m) if s.count(m) == 1 else -1


def halo():
    rows, summ = [], []
    for ds in AF_SIM + H5 + NEW:
        if ds in AF_SIM:
            run, truth = os.path.join('results', 'runs', f'{ds}_seed1'), os.path.join('data', 'references', 'sim_hgt.nwk')
        else:
            run = os.path.join('results', 'h5', ds, 'qmaws') if ds in H5 else os.path.join('results', 'benchmark', ds, 'qmaws')
            truth = os.path.join('data', 'manifests', 'simulated', 'trees', ds + '.nwk')
        de, dt = leaf_dist(read(os.path.join(run, 'report', 'tree.nwk'))), leaf_dist(read(truth))
        taxa = sorted(dt)
        wrong = {t: 0 for t in taxa}
        total = {t: 0 for t in taxa}
        for q in itertools.combinations(taxa, 4):
            bad = topo(de, *q) != topo(dt, *q)
            for t in q:
                total[t] += 1
                wrong[t] += bad
        hv = {r['taxon']: float(r['halo']) for r in csv.DictReader(open(os.path.join(run, 'report', 'halo.tsv'), encoding='utf-8'), delimiter='\t')}
        for t in taxa:
            rows.append([ds, group(ds), t, f'{hv[t]:.6f}', f'{wrong[t] / total[t]:.6f}'])
    write('halo.tsv', ['dataset', 'group', 'taxon', 'halo', 'wrong_quartet_share'], rows)
    by = {}
    for r in rows:
        by.setdefault(r[1], []).append(r)
        by.setdefault('all', []).append(r)
    for g, rs in sorted(by.items()):
        h = [float(r[3]) for r in rs]
        w = [float(r[4]) for r in rs]
        rho = spearman(h, w)
        lo, hi = boot_ci(list(zip(h, w)), stat=lambda p: spearman([x for x, _ in p], [y for _, y in p]))
        # AUC: does a low halo value pick out the taxa with the most wrong quartets (top quarter)?
        cut = sorted(w)[int(0.75 * len(w))]
        pos = [x for x, y in zip(h, w) if y >= cut and y > 0]
        neg = [x for x, y in zip(h, w) if not (y >= cut and y > 0)]
        auc = sum((p < n) + 0.5 * (p == n) for p in pos for n in neg) / (len(pos) * len(neg)) if pos and neg else float('nan')
        summ.append([g, len(rs), f'{rho:.3f}', f'{lo:.3f}', f'{hi:.3f}', len(pos), f'{auc:.3f}'])
        print('  halo', summ[-1])
    write('halo_summary.tsv', ['group', 'taxa', 'spearman_halo_vs_wrong', 'ci_low', 'ci_high', 'taxa_top_quarter_wrong', 'auc_low_halo'], summ)


def spearman(x, y):
    rx, ry = mid_ranks(x), mid_ranks(y)
    mx, my = sum(rx) / len(rx), sum(ry) / len(ry)
    cov = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    sx = math.sqrt(sum((a - mx) ** 2 for a in rx))
    sy = math.sqrt(sum((b - my) ** 2 for b in ry))
    return cov / (sx * sy) if sx and sy else float('nan')


def group(ds):
    if ds in AF_SIM:
        return 'afproject_sim'
    return 'hgt_' + ds[5:8] if ds in H5 else ds[3:].rsplit('_', 1)[0]


# --- where the wrong branches are --------------------------------------------
def wrong_splits():
    rows = []
    for ds in AF_SIM + H5 + NEW:
        if ds in AF_SIM:
            truth = os.path.join('data', 'references', 'sim_hgt.nwk')
            trees = {'qmaws': os.path.join('results', 'runs', f'{ds}_seed1', 'report', 'tree.nwk'),
                     'ml_maws': os.path.join('results', 'benchmark', ds, 'ml_maws', 'ML_MAWS_tree.newick')}
        else:
            truth = os.path.join('data', 'manifests', 'simulated', 'trees', ds + '.nwk')
            base = os.path.join('results', 'h5', ds) if ds in H5 else os.path.join('results', 'benchmark', ds)
            trees = {'qmaws': os.path.join(base, 'qmaws', 'report', 'tree.nwk'),
                     'ml_maws': os.path.join(base, 'ml_maws', 'ML_MAWS_tree.newick')}
        t, taxa = splits(read(truth))
        for m, p in trees.items():
            if not os.path.exists(p):
                p = p.replace('ML_MAWS_tree.newick', 'ml_maws_iqtree.treefile')
            if not os.path.exists(p):
                continue
            e, _ = splits(read(p))
            bad = [min(len(s), len(taxa) - len(s)) for s in e if s not in t]
            rows.append([ds, group(ds), m, len(taxa), len(e), len(bad),
                         f'{median(bad):.1f}' if bad else 'NA', f'{sum(bad) / len(bad):.2f}' if bad else 'NA'])
    write('wrong_splits.tsv', ['dataset', 'group', 'method', 'taxa', 'branches', 'wrong', 'median_small_side', 'mean_small_side'], rows)
    for m in ('qmaws', 'ml_maws'):
        sides = [float(r[7]) for r in rows if r[2] == m and r[7] != 'NA']
        print(f'  wrong splits {m}: datasets {len(sides)}, median of mean smaller side {median(sides):.2f}')


def write(name, header, rows):
    os.makedirs(SUM, exist_ok=True)
    with open(os.path.join(SUM, name), 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(header)
        w.writerows(rows)


if __name__ == '__main__':
    calibration()
    wrong_splits()
    halo()
