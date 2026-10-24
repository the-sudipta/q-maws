"""Chooses the tuned setting of each method (docs/PREREGISTRATION.md, Amendment 8b).

Usage: python scripts/bench_tune_choose.py <tuning folder> <output tsv>

Reads <tuning folder>/<method>_<value>/<dataset>/comparison.tsv (written by
`qmaws tree-compare`), takes the median nRF of each value over the tuning
datasets, and picks the value with the lowest median. Ties keep the default,
or else the tied value closest to the default. A value with any failed run
counts its failed runs as nRF 1. Writes one row per method: method, default,
chosen, and the median nRF of every value.
"""
import os
import statistics
import sys

DEFAULTS = {'mash': 21, 'mashtree': 21, 'sans': 31, 'skmer': 31, 'fswm': 12, 'multispam': 10, 'cvtree': 7}


def nrf(path):
    try:
        for line in open(path, encoding='utf-8'):
            f = line.rstrip('\n').split('\t')
            if f[0] == 'nRF':
                return float(f[1])
    except OSError:
        pass
    return 1.0  # failed run: no tree, or no comparison


def main():
    root, out = sys.argv[1], sys.argv[2]
    medians = {}
    for d in sorted(os.listdir(root)):
        if '_' not in d or not os.path.isdir(os.path.join(root, d)):
            continue
        method, value = d.rsplit('_', 1)
        if method not in DEFAULTS:
            continue
        vals = [nrf(os.path.join(root, d, ds, 'comparison.tsv')) for ds in sorted(os.listdir(os.path.join(root, d)))]
        if vals:
            medians.setdefault(method, {})[int(value)] = statistics.median(vals)
    with open(out, 'w', encoding='utf-8', newline='\n') as f:
        f.write('method\tdefault\tchosen\tmedian_nrf_by_value\n')
        for method, by in sorted(medians.items()):
            default = DEFAULTS[method]
            best = min(by.values())
            tied = [v for v, m in by.items() if m == best]
            chosen = default if default in tied else min(tied, key=lambda v: (abs(v - default), v))
            f.write(f"{method}\t{default}\t{chosen}\t{'; '.join(f'{v}: {by[v]:.4f}' for v in sorted(by))}\n")
    print(open(out, encoding='utf-8').read())


if __name__ == '__main__':
    main()
