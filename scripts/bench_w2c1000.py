"""W2c with 1,000 instead of 100 resamples (docs/PREREGISTRATION.md, Amendment 9;
descriptive). No new runs; only the Python standard library, plus the `zstd` program
when Python has no zstd module (Python < 3.14).

Usage (from the repository folder):
  python scripts/bench_w2c1000.py

Compares, on the five H5 datasets with h = 0.2 (seed 1, no S2 bootstrap):
  100 resamples:   results/h5/<dataset>/qmaws              (H5 runs, checked again with
                                                            release 1.0.0, Amendment 8)
  1,000 resamples: results/benchmark/w2c1000/<dataset>     (Amendment 9)
Reported, as registered: nRF and nQD of both runs, the share of quartets whose winning tree
changes, and the change in the mean W2c weight of the winner. The winner and its weight
come from each run's audit/quartet_decisions.bin.zst (docs/DESIGN.md: magic QMAWDEC1, the
number of quartets as u64, then per quartet the winning topology as u8, 3 meaning no
weight, and its weight times 65,535 as u16; little-endian).

Written to results/benchmark/summary/w2c1000.tsv.
"""
import csv
import json
import os
import struct
import subprocess

DATASETS = [f'hgt_h020_r{r}' for r in range(1, 6)]
R100 = os.path.join('results', 'h5', '{}', 'qmaws')
R1000 = os.path.join('results', 'benchmark', 'w2c1000', '{}')
OUT = os.path.join('results', 'benchmark', 'summary', 'w2c1000.tsv')
MAGIC = b'QMAWDEC1'
NO_WEIGHT = 3


def decompress(path):
    try:
        from compression import zstd          # Python 3.14 and later
        with open(path, 'rb') as f:
            return zstd.decompress(f.read())
    except ImportError:
        return subprocess.run(['zstd', '-dc', path], check=True, capture_output=True).stdout


def decisions(run):
    """[(topology, weight)] in quartet rank order; weight in [0, 1]."""
    raw = decompress(os.path.join(run, 'audit', 'quartet_decisions.bin.zst'))
    if raw[:8] != MAGIC:
        raise ValueError(f'{run}: not a quartet decisions file')
    (n,) = struct.unpack_from('<Q', raw, 8)
    if len(raw) != 16 + 3 * n:
        raise ValueError(f'{run}: {len(raw)} bytes for {n} quartets')
    return [(raw[16 + 3 * i], struct.unpack_from('<H', raw, 17 + 3 * i)[0] / 65535)
            for i in range(n)]


def evaluation(run):
    with open(os.path.join(run, 'audit', 'evaluation.json'), encoding='utf-8') as f:
        e = json.load(f)
    return e['nrf'], e['nqd']


def mean(values):
    return sum(values) / len(values) if values else float('nan')


def compare(ds):
    a, b = R100.format(ds), R1000.format(ds)
    if not os.path.isfile(os.path.join(b, 'audit', 'root.txt')):
        return {'dataset': ds, 'status': 'pending'}
    da, db = decisions(a), decisions(b)
    if len(da) != len(db):
        raise ValueError(f'{ds}: {len(da)} and {len(db)} quartets')
    both = [(x, y) for x, y in zip(da, db) if x[0] != NO_WEIGHT and y[0] != NO_WEIGHT]
    changed = sum(x[0] != y[0] for x, y in both)
    w100 = mean([x[1] for x, _ in both])
    w1000 = mean([y[1] for _, y in both])
    (nrf_a, nqd_a), (nrf_b, nqd_b) = evaluation(a), evaluation(b)
    return {'dataset': ds, 'status': 'done',
            'nrf_100': f'{nrf_a:.6f}', 'nrf_1000': f'{nrf_b:.6f}',
            'nqd_100': f'{nqd_a:.6f}', 'nqd_1000': f'{nqd_b:.6f}',
            'quartets': str(len(da)), 'weighted_in_both': str(len(both)),
            'weighted_in_one': str(sum((x[0] == NO_WEIGHT) != (y[0] == NO_WEIGHT) for x, y in zip(da, db))),
            'winner_changed': str(changed), 'share_changed': f'{changed / len(both):.6f}' if both else 'NA',
            'mean_winner_weight_100': f'{w100:.6f}', 'mean_winner_weight_1000': f'{w1000:.6f}',
            'weight_change': f'{w1000 - w100:+.6f}'}


def main():
    rows = [compare(ds) for ds in DATASETS]
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    header = ['dataset', 'status', 'nrf_100', 'nrf_1000', 'nqd_100', 'nqd_1000', 'quartets',
              'weighted_in_both', 'weighted_in_one', 'winner_changed', 'share_changed',
              'mean_winner_weight_100', 'mean_winner_weight_1000', 'weight_change']
    with open(OUT, 'w', encoding='utf-8', newline='') as f:
        w = csv.DictWriter(f, header, delimiter='\t', lineterminator='\n', restval='NA')
        w.writeheader()
        w.writerows(rows)
    print(f"w2c1000: {sum(r['status'] == 'done' for r in rows)} of {len(rows)} datasets written to {OUT}")


if __name__ == '__main__':
    main()
