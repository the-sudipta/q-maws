"""Collects the accuracy, time and memory of every method on every dataset of the
extended benchmark (docs/PREREGISTRATION.md, Amendments 8 to 9) into one table.

Usage (from the repository folder):
  python scripts/bench_collect.py

Writes results/benchmark/summary/accuracy.tsv with one row per dataset and
method: dataset, group, method, setting (default or tuned), status
(done, failed or pending), nRF, nQD, seconds and peak memory in MB.
A run that is not finished yet is "pending"; a run with failed.txt is
"failed" (its reason is in its run.log and stderr.txt). Trees that have no
comparison yet (ML-MAWS, the IQ-TREE reference) are compared here with
`qmaws tree-compare`; the comparison is stored in results/benchmark/summary/compare/
(never inside a run folder, so the SHA-256 seals of the runs stay valid).
"""
import csv
import os
import subprocess
import sys

OUT = os.path.join('results', 'benchmark')
QMAWS = os.path.join('target', 'release', 'qmaws.exe' if os.name == 'nt' else 'qmaws')

AF = ['fish_mito', 'ecoli', 'ecoli_shigella_hgt', 'yersinia_hgt',
      'sim_hgt_0', 'sim_hgt_250', 'sim_hgt_500', 'sim_hgt_750', 'sim_hgt_1000']
H5 = [f'hgt_h{h}_r{r}' for h in ('000', '010', '020', '040') for r in range(1, 6)]
NEW = [f'bm_{c}_r{r}' for c in ('div005', 'div020', 'div050', 'm15', 'm60', 'frag') for r in range(1, 6)]
LINUX = ['mash', 'cdmaws', 'peafowl', 'andi', 'phylonium', 'fswm', 'multispam',
         'sans', 'cvtree', 'jolytree', 'skmer', 'mashtree']
# alignment-based references (Amendments 8a and 9); only where the true alignment is known
REFS = ['iqtree_true', 'caster_site', 'treeqmc', 'wtreeqmc']
AF_SEED1 = {'ecoli': 'ecoli_2026-10-03_080534', 'fish_mito': 'fish_mito_2026-10-03_092551'}


def group_of(ds):
    if ds in AF:
        return 'afproject'
    if ds in H5:
        return 'hgt_' + ds[5:8]
    return ds[3:].rsplit('_', 1)[0]


def reference_of(ds):
    if ds.startswith('sim_hgt_'):
        return os.path.join('data', 'references', 'sim_hgt.nwk')
    if ds in AF:
        return os.path.join('data', 'references', ds + '.nwk')
    return os.path.join('data', 'manifests', 'simulated', 'trees', ds + '.nwk')


def read_comparison(path):
    vals = {}
    for row in csv.reader(open(path, encoding='utf-8'), delimiter='\t'):
        if len(row) >= 2 and row[0] in ('nRF', 'nQD'):
            vals[row[0]] = row[1]
    return vals.get('nRF', ''), vals.get('nQD', '')


def compare(tree, ds, dest):
    """Compares tree with the reference of ds once and stores the result in dest."""
    dest = os.path.join(OUT, 'summary', 'compare', dest)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    if not os.path.exists(dest):
        with open(dest, 'w', encoding='utf-8') as f:
            subprocess.run([QMAWS, '--quiet', 'tree-compare', '--tree', tree,
                            '--reference', reference_of(ds)], stdout=f, check=True)
    return read_comparison(dest)


def times():
    t = {}
    path = os.path.join(OUT, 'runs.tsv')
    for r in csv.DictReader(open(path, encoding='utf-8'), delimiter='\t'):
        t[(r['dataset'], r['method'])] = (r['seconds'], r['peak_mb'])  # last row wins
    return t


def tree_in(folder):
    """The tree named by done.txt (tree=<file>) or tree.nwk."""
    done = os.path.join(folder, 'done.txt')
    name = 'tree.nwk'
    if os.path.exists(done):
        line = open(done, encoding='utf-8').read().strip()
        if line.startswith('tree='):
            name = line[5:]
    return os.path.join(folder, name)


def status_of(folder):
    if os.path.exists(os.path.join(folder, 'done.txt')):
        return 'done'
    if os.path.exists(os.path.join(folder, 'failed.txt')):
        return 'failed'
    return 'pending'


def main():
    chosen = {}
    cpath = os.path.join(OUT, 'tuning', 'chosen.tsv')
    if os.path.exists(cpath):
        for r in csv.DictReader(open(cpath, encoding='utf-8'), delimiter='\t'):
            chosen[r['method']] = (r['default'], r['chosen'])
    t = times()
    rows = []

    def add(ds, method, setting, status, nrf='', nqd='', key=None):
        sec, mb = t.get((ds, key or method), ('', ''))
        rows.append([ds, group_of(ds), method, setting, status, nrf, nqd, sec, mb])

    for ds in AF + H5 + NEW:
        # Q-MAWS 1.0.0 (primary configuration, seed 1)
        if ds in AF:
            run = os.path.join('results', 'runs', AF_SEED1.get(ds, ds + '_seed1'))
        elif ds in H5:
            run = os.path.join('results', 'h5', ds, 'qmaws')
        else:
            run = os.path.join(OUT, ds, 'qmaws')
        rc = os.path.join(run, 'report', 'reference_comparison.tsv')
        if os.path.exists(rc):
            add(ds, 'qmaws', 'default', 'done', *read_comparison(rc))
        else:
            add(ds, 'qmaws', 'default', 'pending')
        # ML-MAWS (laptop runs; H5 runs of Amendment 4)
        folder = os.path.join('results', 'h5', ds, 'ml_maws') if ds in H5 else os.path.join(OUT, ds, 'ml_maws')
        tree = os.path.join(folder, 'ML_MAWS_tree.newick') if ds in H5 else tree_in(folder)
        if os.path.exists(tree) and (ds in H5 or status_of(folder) == 'done'):
            add(ds, 'ml_maws', 'default', 'done', *compare(tree, ds, f'{ds}_ml_maws.tsv'))
        else:
            add(ds, 'ml_maws', 'default', 'pending')
        # the Linux methods of phase B, default and tuned
        for m in LINUX:
            for sub, setting, key in [(m, 'default', m)] + (
                    [(m + '_tuned', 'tuned', f'{m}_{chosen[m][1]}')]
                    if m in chosen and chosen[m][0] != chosen[m][1] else []):
                folder = os.path.join(OUT, ds, sub)
                st = status_of(folder)
                cmp_ = os.path.join(folder, 'comparison.tsv')
                if st == 'done' and os.path.exists(cmp_):
                    add(ds, m, setting, st, *read_comparison(cmp_), key=key)
                else:
                    add(ds, m, setting, st, key=key)
        # alignment-based references, where the true alignment is known
        if ds in H5 or (ds in NEW and not ds.startswith('bm_frag')):
            for m in REFS:
                folder = os.path.join(OUT, ds, m)
                st = status_of(folder)
                tree = tree_in(folder)
                if st == 'done' and os.path.exists(tree):
                    add(ds, m, 'reference', st, *compare(tree, ds, f'{ds}_{m}.tsv'))
                else:
                    add(ds, m, 'reference', st)

    os.makedirs(os.path.join(OUT, 'summary'), exist_ok=True)
    path = os.path.join(OUT, 'summary', 'accuracy.tsv')
    with open(path, 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t', lineterminator='\n')
        w.writerow(['dataset', 'group', 'method', 'setting', 'status', 'nrf', 'nqd', 'seconds', 'peak_mb'])
        w.writerows(rows)
    n = {s: sum(1 for r in rows if r[4] == s) for s in ('done', 'failed', 'pending')}
    print(f'{len(rows)} rows: {n["done"]} done, {n["failed"]} failed, {n["pending"]} pending -> {path}')


if __name__ == '__main__':
    sys.exit(main())
