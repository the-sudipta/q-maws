"""Adds the outputs of one wave's jobs to the state (a checkout of the branch
benchmark-cloud-state), then makes tuning/chosen.tsv once tuning is complete and
seals every finished run with SHA-256 (scripts/bench_hash.py)."""
import json
import os
import shutil
import subprocess
import time

from catalog import REPO
from planner import tuning_complete
from state import HOSTS_HEADER, RUNS_HEADER, append_rows, bench, read_json, read_rows, write_json

MAX_FILE = 95 * 1024 * 1024     # GitHub refuses files of 100 MB; none is expected


def _copy_tree(src, dest, notes):
    """Copies run folders into the state; Q-MAWS work/ folders (checkpoints) are left out,
    as in every committed run of the project."""
    for folder, dirs, files in os.walk(src):
        dirs[:] = [d for d in dirs if d != 'work']
        rel = os.path.relpath(folder, src)
        os.makedirs(os.path.join(dest, rel), exist_ok=True)
        for name in files:
            s = os.path.join(folder, name)
            if os.path.getsize(s) > MAX_FILE:
                notes.append(f'{os.path.join(rel, name)}: {os.path.getsize(s)} bytes, not committed')
                continue
            shutil.copy2(s, os.path.join(dest, rel, name))


def merge(state, outputs, run_id, wave):
    """state: state checkout; outputs: folder holding one sub-folder per job output."""
    partials = read_json(os.path.join(state, 'cloud', 'partials.json'), {})
    failures = read_json(os.path.join(state, 'cloud', 'failures.json'), {})
    notes, counts = [], {}
    for job in sorted(os.listdir(outputs)):
        out = os.path.join(outputs, job)
        if not os.path.isdir(os.path.join(out, 'cloud')):
            continue
        if os.path.isdir(os.path.join(out, 'results', 'benchmark')):
            _copy_tree(os.path.join(out, 'results', 'benchmark'), bench(state), notes)
        append_rows(os.path.join(bench(state), 'runs.tsv'), RUNS_HEADER, read_rows(os.path.join(out, 'cloud', 'runs.tsv')))
        append_rows(os.path.join(bench(state), 'hosts.tsv'), HOSTS_HEADER, read_rows(os.path.join(out, 'cloud', 'hosts.tsv')))
        result = read_json(os.path.join(out, 'cloud', 'outcomes.json'), {'outcomes': {}, 'partials': {}})
        artifact = 'partial-' + job[len('out-'):] if job.startswith('out-') else 'partial-' + job
        for task_id, outcome in result['outcomes'].items():
            counts[outcome] = counts.get(outcome, 0) + 1
            if outcome in ('done', 'failed'):
                partials.pop(task_id, None)
                failures.pop(task_id, None)
            elif outcome == 'retry':
                failures[task_id] = int(failures.get(task_id, 0)) + 1
                partials.pop(task_id, None)
            if task_id in result['partials']:
                partials[task_id] = {'run': run_id, 'artifact': artifact, 'path': result['partials'][task_id]}
    write_json(os.path.join(state, 'cloud', 'partials.json'), partials)
    write_json(os.path.join(state, 'cloud', 'failures.json'), failures)
    log = read_json(os.path.join(state, 'cloud', 'waves.json'), [])
    log.append({'wave': wave, 'workflow_run': run_id, 'utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
                'outcomes': counts, 'notes': notes})
    write_json(os.path.join(state, 'cloud', 'waves.json'), log)

    choose = os.path.join(bench(state), 'tuning', 'chosen.tsv')
    if not os.path.isfile(choose) and os.path.isdir(os.path.dirname(choose)) and tuning_complete(state):
        subprocess.run(['python3', os.path.join(REPO, 'scripts', 'bench_tune_choose.py'),
                        os.path.dirname(choose), choose], check=True, cwd=state)
    # bench_hash.py seals a run only when its marker is older than two minutes (so that a
    # running job is never sealed). Every merged run is finished, but its files were just
    # written here; wait out that rule instead of changing it.
    time.sleep(125)
    subprocess.run(['python3', os.path.join(REPO, 'scripts', 'bench_hash.py'), 'seal'], check=True, cwd=state)
    print(json.dumps(counts, sort_keys=True))
    return counts
