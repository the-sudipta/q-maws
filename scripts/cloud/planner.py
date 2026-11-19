"""Plans one wave: which unfinished tasks run, packed into at most `max_shards` jobs.

Rules (Amendment 10):
  * a task runs only after its prerequisites (the tuned runs wait for all tuning runs);
  * in full mode a method runs only after its smoke test has a result tree;
  * a speed run gets a job of its own (nothing else on that runner);
  * an unfinished resumable run is continued first in a job, with its saved state, and a run
    stopped because its job ran out of time starts again first in a job (Amendment 10);
  * long runs (one job or more of estimated work) start early, since they need several jobs;
  * other tasks are packed longest first into jobs of about 5 h 30 min of estimated work;
  * every job that is not a speed or smoke job then gets its own share of the remaining tasks
    as extra work, so that it keeps working until its deadline even when the estimates were
    too high. The shares are disjoint: no task is planned twice. An extra task that the job
    cannot finish is stopped by the deadline and is not a result (see executor.py).
Estimates only order and pack the work; a wrong estimate costs time, never a result.
"""
import os
import statistics
from dataclasses import asdict

from catalog import METHODS, full_tasks, input_group, smoke_tasks
from state import REF_STEPS, bench, chosen_settings, is_done_ok, is_finished, read_json, read_rows

SHARD_SECONDS = 5 * 3600 + 30 * 60
PRIORITY = {'smoke': 0, 'tuning': 1, 'verify': 2, 'speed': 5, 'w2c1000': 4, 'dependence': 4}
FIRST = -1          # continued or stopped runs: first in a job
LONG = 1.5          # one job or more of estimated work: right after tuning


def smoke_of(task):
    """The smoke test that must pass before a task of this method runs (full mode)."""
    if task.kind == 'tool':
        return f"smoke/{task.extra['tool']}"
    return {'qmaws': 'smoke/qmaws', 'w2c': 'smoke/qmaws', 'matrix': 'smoke/qmaws', 'verify': 'smoke/qmaws',
            'mlmaws': 'smoke/ml_maws', 'iqtree': 'smoke/iqtree_true', 'refs': 'smoke/refs'}[task.kind]


def section(task):
    return task.id.split('/', 1)[0]


def stopped_runs(root):
    """Tasks whose last attempt was stopped by the end of its job (outcome 'interrupted')."""
    last = {}
    for r in read_rows(os.path.join(bench(root), 'hosts.tsv')):
        last[r['task']] = r.get('outcome', '')
    return {t for t, o in last.items() if o == 'interrupted'}


class Estimator:
    """Seconds a task is expected to take: the median of finished runs of the same method
    on the same kind of dataset when there are any, otherwise a size-scaled first guess."""

    def __init__(self, root):
        self.inputs = read_json(os.path.join(root, 'cloud', 'inputs.json'), {}).get('datasets', {})
        self.seen = {}
        for r in read_rows(os.path.join(bench(root), 'runs.tsv')):
            if r['seconds'].isdigit():
                self.seen.setdefault((r['method'].split('_')[0], self._kind(r['dataset'])), []).append(int(r['seconds']))

    @staticmethod
    def _kind(ds):
        return ds.rsplit('_r', 1)[0] if ds.startswith(('hgt_', 'bm_', 'scaling_')) else ds

    def __call__(self, task):
        base = task.method.split('_')[0]
        seen = self.seen.get((base, self._kind(task.dataset)))
        if seen:
            return int(statistics.median(seen) * 1.2)
        info = self.inputs.get(task.dataset, {})
        size = max(0.05, info.get('bytes', 30e6) / 30e6)
        taxa = max(4, info.get('taxa', 30))
        kind = {'w2c': 'qmaws', 'matrix': 'qmaws', 'verify': 'qmaws'}.get(task.kind, task.kind)
        method = task.extra.get('tool') if task.kind == 'tool' else {
            'qmaws': 'qmaws', 'mlmaws': 'ml_maws', 'iqtree': 'iqtree_true', 'refs': 'refs'}[kind]
        if kind == 'qmaws':   # Q-MAWS grows with the number of quartets, not with genome length
            guess = METHODS[method].estimate * (taxa / 30) ** 4
            guess *= {'w2c': 8, 'matrix': 0.05, 'verify': 0.1}.get(task.kind, 1)
        else:
            guess = METHODS[method].estimate * size
        return int(max(60, guess))


def plan(root, mode, max_shards):
    """Returns (shards, summary). root: a checkout of the state branch."""
    if mode == 'smoke':
        tasks = smoke_tasks()
    else:   # the smoke tests first; each method's runs start once its own smoke test passed
        tasks = smoke_tasks() + full_tasks(chosen_settings(root))
    by_id = {t.id: t for t in tasks}
    partials = read_json(os.path.join(root, 'cloud', 'partials.json'), {})
    failures = read_json(os.path.join(root, 'cloud', 'failures.json'), {})
    estimate = Estimator(root)
    stopped = stopped_runs(root)

    pending, blocked = [], {}
    for t in tasks:
        if is_finished(root, t):
            continue
        if any(not is_finished(root, by_id[n]) for n in t.needs):
            blocked[t.id] = 'waits for tuning'
            continue
        if mode == 'full' and section(t) != 'smoke':
            smoke = smoke_tasks_by_id().get(smoke_of(t))
            if smoke is None or not is_done_ok(root, smoke):
                blocked[t.id] = f'waits for {smoke_of(t)}'
                continue
        pending.append(t)

    def first(t):           # continued, or started again from the beginning (Amendment 10)
        return t.id in partials or t.id in stopped

    def priority(t):
        if section(t) == 'smoke':
            return PRIORITY['smoke']
        if first(t):
            return FIRST
        base = PRIORITY.get(section(t), 3)
        return min(LONG, base) if estimate(t) >= SHARD_SECONDS else base

    pending.sort(key=lambda t: (priority(t), -estimate(t)))
    shards = []

    def new_shard():
        if len(shards) >= max_shards:
            return None
        s = {'index': len(shards), 'tasks': [], 'seconds': 0}
        shards.append(s)
        return s

    for t in pending:
        e = estimate(t)
        if t.alone or first(t) or section(t) == 'smoke':   # smoke: one job per method, all at once
            s = new_shard()               # alone, or continued first in its job
            if s is None:
                continue
        else:
            s = next((s for s in shards if not s.get('closed') and s['seconds'] + e <= SHARD_SECONDS), None)
            if s is None:
                s = new_shard()
                if s is None:
                    continue
        s['tasks'].append(t.id)
        s['seconds'] += e
        if t.alone or e >= SHARD_SECONDS or section(t) == 'smoke':
            s['closed'] = True

    # extra work: the tasks left over, dealt out in order to the jobs that may take them; a run
    # that must start first in a job, or that needs a whole job or more, waits for its own job
    planned = {i for s in shards for i in s['tasks']}
    takers = [s for s in shards if not any(by_id[i].alone or section(by_id[i]) == 'smoke' for i in s['tasks'])]
    rest = [t for t in pending if t.id not in planned and not t.alone and section(t) != 'smoke'
            and not first(t) and estimate(t) < SHARD_SECONDS]
    for k, t in enumerate(rest if takers else []):
        s = takers[k % len(takers)]
        s['tasks'].append(t.id)
        s['extra'] = s.get('extra', 0) + 1

    for s in shards:
        ts = [by_id[i] for i in s['tasks']]
        s['groups'] = sorted({input_group(d) for t in ts for d in t.inputs()})
        s['partials'] = {i: partials[i] for i in s['tasks'] if i in partials}
        s['partial_artifacts'] = sorted({(p['run'], p['artifact']) for p in s['partials'].values()})
        s['failures'] = {i: failures[i] for i in s['tasks'] if i in failures}
        s['in_state'] = [f'{os.path.dirname(t.id)}/{step}' for t in ts if t.kind == 'refs' for step in REF_STEPS
                         if os.path.isdir(os.path.join(bench(root), os.path.dirname(t.id), step))]
        s['defs'] = {t.id: asdict(t) for t in ts}     # the job runs exactly what was planned
        s.pop('closed', None)
    summary = {'mode': mode, 'tasks': len(tasks), 'finished': len(tasks) - len(pending) - len(blocked),
               'pending': len(pending), 'blocked': len(blocked), 'planned': sum(len(s['tasks']) for s in shards),
               'extra': sum(s.get('extra', 0) for s in shards),
               'blocked_examples': dict(list(blocked.items())[:20])}
    return shards, summary


def smoke_tasks_by_id():
    return {t.id: t for t in smoke_tasks()}


def tuning_complete(root):
    """True when every tuning run has a result (then tuning/chosen.tsv can be made)."""
    return all(is_finished(root, t) for t in full_tasks() if t.id.startswith('tuning/'))
