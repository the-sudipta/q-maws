"""The speed runs of the extended benchmark (docs/PREREGISTRATION.md, Amendments 8 and 10;
descriptive). No new runs; only the Python standard library.

Usage (from the repository folder):
  python scripts/bench_speed.py

Every method ran alone on each of the 9 speed datasets of M13 (25, 50 and 100 taxa, three
replicates), each run in a job of its own on a GitHub-hosted runner. Runners differ in
processor, so times are compared directly only between runs on the same processor model
(Amendment 10). The M13 times measured on the development laptop stay the result of H2 and
are not used here.

Written to results/benchmark/summary/:
  speed.tsv         one row per run: status, wall seconds (all parts of a continued run
                    added), Q-MAWS engine seconds (run.json, covering every part), peak
                    memory, the processor model(s) and calibration of its job(s)
  speed_by_cpu.tsv  per processor model, number of taxa and method: finished runs, median
                    seconds (engine seconds for Q-MAWS, as in H2; wall seconds otherwise)
                    and median peak memory; runs whose parts ran on different processor
                    models are left out
"""
import csv
import json
import os
import statistics

BENCH = os.path.join('results', 'benchmark')
SPEED = os.path.join(BENCH, 'speed')
SUMMARY = os.path.join(BENCH, 'summary')
DATASETS = [f'scaling_m{m}_r{r}' for m in (25, 50, 100) for r in (1, 2, 3)]


def read_rows(path):
    if not os.path.isfile(path):
        return []
    with open(path, encoding='utf-8', newline='') as f:
        return list(csv.DictReader(f, delimiter='\t'))


def first_line(path):
    with open(path, encoding='utf-8', errors='replace') as f:
        return f.readline().strip()


def status_of(folder):
    """done, failed or pending, and the reason of a failure (the run's own markers)."""
    if os.path.isfile(os.path.join(folder, 'failed.txt')):
        return 'failed', first_line(os.path.join(folder, 'failed.txt'))
    if os.path.isfile(os.path.join(folder, 'done.txt')) or \
            os.path.isfile(os.path.join(folder, 'audit', 'root.txt')):
        return 'done', ''
    return 'pending', ''


def engine_seconds(folder):
    """Q-MAWS's own working time over every part of the run (run.json), else ''."""
    try:
        with open(os.path.join(folder, 'run.json'), encoding='utf-8') as f:
            value = json.load(f).get('elapsed_seconds')
    except (OSError, ValueError):
        return ''
    return f'{value:.1f}' if isinstance(value, (int, float)) else ''


def collect():
    runs = {}
    for r in read_rows(os.path.join(BENCH, 'runs.tsv')):
        if r['dataset'] in DATASETS:
            runs[(r['dataset'], r['method'])] = r          # the last row of a run counts
    hosts = {}
    for h in read_rows(os.path.join(BENCH, 'hosts.tsv')):
        if h['task'].startswith('speed/'):
            hosts.setdefault(h['task'], []).append(h)
    out = []
    for ds in DATASETS:
        folder_ds = os.path.join(SPEED, ds)
        methods = sorted(os.listdir(folder_ds)) if os.path.isdir(folder_ds) else []
        for m in methods:
            folder = os.path.join(folder_ds, m)
            if not os.path.isdir(folder):
                continue
            status, reason = status_of(folder)
            run = runs.get((ds, m), {})
            parts = hosts.get(f'speed/{ds}/{m}', [])
            cpus = sorted({p['cpu_model'] for p in parts})
            out.append({
                'dataset': ds, 'taxa': ds.split('_')[1][1:], 'replicate': ds.rsplit('_r', 1)[1],
                'method': m, 'status': status,
                'seconds_wall': run.get('seconds', ''),
                'seconds_engine': engine_seconds(folder) if m == 'qmaws' else '',
                'peak_mb': run.get('peak_mb', ''),
                'cpu_model': '; '.join(cpus),
                'calibration_kbps': '; '.join(p['calibration_sha256_16k_kbps'] for p in parts),
                'jobs': str(len(parts)), 'note': reason})
    return out


def by_cpu(rows):
    groups = {}
    for r in rows:
        if r['status'] != 'done' or ';' in r['cpu_model'] or not r['cpu_model']:
            continue
        seconds = r['seconds_engine'] or r['seconds_wall']
        if not seconds:
            continue
        g = groups.setdefault((r['cpu_model'], int(r['taxa']), r['method']), {'s': [], 'mb': []})
        g['s'].append(float(seconds))
        if r['peak_mb'].isdigit():
            g['mb'].append(int(r['peak_mb']))
    return [{'cpu_model': cpu, 'taxa': str(taxa), 'method': m, 'runs': str(len(g['s'])),
             'median_seconds': f"{statistics.median(g['s']):.1f}",
             'median_peak_mb': str(int(statistics.median(g['mb']))) if g['mb'] else 'NA'}
            for (cpu, taxa, m), g in sorted(groups.items())]


def write(path, rows, header):
    with open(path, 'w', encoding='utf-8', newline='') as f:
        w = csv.DictWriter(f, header, delimiter='\t', lineterminator='\n')
        w.writeheader()
        w.writerows(rows)


def main():
    os.makedirs(SUMMARY, exist_ok=True)
    rows = collect()
    write(os.path.join(SUMMARY, 'speed.tsv'), rows,
          ['dataset', 'taxa', 'replicate', 'method', 'status', 'seconds_wall', 'seconds_engine',
           'peak_mb', 'cpu_model', 'calibration_kbps', 'jobs', 'note'])
    write(os.path.join(SUMMARY, 'speed_by_cpu.tsv'), by_cpu(rows),
          ['cpu_model', 'taxa', 'method', 'runs', 'median_seconds', 'median_peak_mb'])
    done = sum(r['status'] == 'done' for r in rows)
    print(f'speed: {len(rows)} runs ({done} done) written to {SUMMARY}')


if __name__ == '__main__':
    main()
