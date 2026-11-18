"""The finished state of the benchmark: which tasks are done, from the run folders
themselves (the same markers the laptop queues used), and the run tables.

The state lives on the branch benchmark-cloud-state, laid out as
results/benchmark/... plus cloud/ (bookkeeping of this workflow only).
"""
import csv
import json
import os

RUNS_HEADER = ['dataset', 'method', 'start_utc', 'end_utc', 'seconds', 'peak_mb', 'exit']
HOSTS_HEADER = ['task', 'attempt', 'wave', 'workflow_run', 'job', 'cpu_model', 'cores', 'memory_gb',
                'disk_free_gb', 'image', 'calibration_sha256_16k_kbps', 'start_utc', 'end_utc', 'outcome']
REF_STEPS = ['fasttree_genes', 'caster_site', 'treeqmc', 'wtreeqmc']


def bench(root):
    return os.path.join(root, 'results', 'benchmark')


def _marked(folder):
    return any(os.path.isfile(os.path.join(folder, m)) for m in ('done.txt', 'failed.txt'))


def is_finished(root, task):
    """True when the task has its finished marker (a result: done or failed)."""
    path = os.path.join(bench(root), task.id)
    if task.kind == 'verify':
        return os.path.isfile(path)
    if task.kind in ('qmaws', 'w2c'):
        return (os.path.isfile(os.path.join(path, 'audit', 'root.txt'))
                or os.path.isfile(os.path.join(path, 'failed.txt')))
    if task.kind == 'refs':
        base = os.path.dirname(path)
        steps = REF_STEPS if os.path.isfile(os.path.join(base, 'fasttree_genes', 'done.txt')) else REF_STEPS[:2]
        return all(_marked(os.path.join(base, s)) for s in steps)
    return _marked(path)


def is_done_ok(root, task):
    """True when the task finished with a result (not failed)."""
    path = os.path.join(bench(root), task.id)
    if task.kind in ('qmaws', 'w2c'):
        return os.path.isfile(os.path.join(path, 'audit', 'root.txt'))
    if task.kind == 'refs':   # a references task is good when every step has a result tree
        return all(os.path.isfile(os.path.join(os.path.dirname(path), s, 'done.txt')) for s in REF_STEPS)
    return os.path.isfile(os.path.join(path, 'done.txt'))


def read_json(path, default):
    try:
        with open(path, encoding='utf-8') as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def write_json(path, value):
    if os.path.dirname(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        json.dump(value, f, indent=1, sort_keys=True)
        f.write('\n')


def read_rows(path):
    if not os.path.isfile(path):
        return []
    with open(path, encoding='utf-8', newline='') as f:
        return list(csv.DictReader(f, delimiter='\t'))


def append_rows(path, header, rows):
    """Appends rows (dicts) to a TSV file, writing the header first if the file is new."""
    new = not os.path.isfile(path)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'a', encoding='utf-8', newline='') as f:
        w = csv.DictWriter(f, fieldnames=header, delimiter='\t', lineterminator='\n', extrasaction='ignore')
        if new:
            w.writeheader()
        for r in rows:
            w.writerow(r)


def chosen_settings(root):
    """{method: (default, chosen)} from tuning/chosen.tsv (bench_tune_choose.py), or None."""
    path = os.path.join(bench(root), 'tuning', 'chosen.tsv')
    rows = read_rows(path)
    if not rows:
        return None
    return {r['method']: (r['default'], r['chosen']) for r in rows}
