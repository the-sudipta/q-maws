"""Runs one shard (a list of tasks) on one GitHub runner and writes everything the
merge step needs into an output folder:

  <out>/results/benchmark/<task id>   finished run folders and reports
  <out>/cloud/runs.tsv                one row per finished run (the runs.tsv of the laptop queues)
  <out>/cloud/hosts.tsv               one row per attempt with the computer it ran on
  <out>/cloud/outcomes.json           {task id: done | failed | interrupted | retry | not started}
  <partial>/<key>/                    unfinished resumable runs, continued in a later job

The job has a fixed deadline; a task is never started with less than a minute left.
"""
import json
import os
import shutil
import subprocess
import time

import hostinfo
from catalog import REPO
from runners import DONE, FAILED, INTERRUPTED, RUNNERS, utc, write
from state import HOSTS_HEADER, RUNS_HEADER, append_rows, read_rows

MAX_FAILURES = 3          # a run that fails this often (not by time) is a result: failed


def key_of(task_id):
    """A file-name-safe form of a task id."""
    return task_id.replace('/', '__')


class Context:
    def __init__(self, shard, out, partial, scratch, tools, deadline, full_budget, state_ref):
        self.shard = shard
        self.out_root = out
        self.partial_root = partial
        self.scratch = scratch
        self.tools = tools
        self.deadline = deadline
        self.full_budget = full_budget
        self.state_ref = state_ref
        self.threads = os.cpu_count() or 1
        self.qmaws = os.path.join(tools, 'bin', 'qmaws')
        self.mlmaws = os.path.join(tools, 'ML-MAWS', 'ml-maws')
        self.path = os.path.join(tools, 'bin') + os.pathsep + os.environ.get('PATH', '')
        os.environ['PATH'] = self.path
        self.attempts = {}          # task id -> list of {start, seconds, peak}
        self.saved_partials = {}

    # paths
    def out(self, task_id):
        return os.path.join(self.out_root, 'results', 'benchmark', task_id)

    def scratch_file(self, task, name):
        d = os.path.join(self.scratch, 'files', key_of(task.id))
        os.makedirs(d, exist_ok=True)
        return os.path.join(d, name)

    def remaining(self):
        return int(self.deadline - time.time())

    # resumable runs: attempts and partial state
    def restore_partial(self, task, folder):
        info = self.shard['partials'].get(task.id)
        self.attempts[task.id] = []
        if not info:
            return
        src = os.path.join(self.partial_root, 'in', info['path'])
        meta = os.path.join(src, 'attempts.json')
        if os.path.isfile(meta):
            self.attempts[task.id] = json.load(open(meta, encoding='utf-8'))
        if os.path.isdir(os.path.join(src, 'run')):
            shutil.rmtree(folder, ignore_errors=True)
            shutil.copytree(os.path.join(src, 'run'), folder, symlinks=True)

    def add_attempt(self, task, seconds, peak):
        self.attempts.setdefault(task.id, []).append(
            {'start': time.time() - seconds, 'seconds': seconds, 'peak': peak})

    def save_partial(self, task, folder):
        dest = os.path.join(self.partial_root, 'out', key_of(task.id))
        shutil.rmtree(dest, ignore_errors=True)
        shutil.copytree(folder, os.path.join(dest, 'run'), symlinks=True)
        with open(os.path.join(dest, 'attempts.json'), 'w', encoding='utf-8') as f:
            json.dump(self.attempts.get(task.id, []), f)
        self.saved_partials[task.id] = key_of(task.id)
        shutil.rmtree(folder, ignore_errors=True)     # an unfinished run is not merged

    def final_row(self, task, start, rc):
        """runs.tsv row of a run made of one or more attempts: the sum of their times and
        the largest peak memory (Amendment 10)."""
        parts = self.attempts.get(task.id) or [{'start': start, 'seconds': 0, 'peak': 'NA'}]
        peaks = [int(p['peak']) for p in parts if str(p['peak']).isdigit()]
        seconds = sum(int(p['seconds']) for p in parts)
        return {'dataset': task.dataset, 'method': task.method, 'start_utc': utc(parts[0]['start']),
                'end_utc': utc(time.time()), 'seconds': str(seconds),
                'peak_mb': str(max(peaks)) if peaks else 'NA', 'exit': str(rc)}

    def retry_or_fail(self, task, folder, reason, start, rc):
        failures = int(self.shard['failures'].get(task.id, 0)) + 1
        if failures >= MAX_FAILURES:
            write(os.path.join(folder, 'failed.txt'), f'{reason} (attempt {failures} of {MAX_FAILURES})')
            return FAILED, self.final_row(task, start, rc)
        shutil.rmtree(folder, ignore_errors=True)
        return 'retry', None

    # the references (bench_refs.sh) work inside a private repository folder
    def private_root(self, task):
        root = os.path.join(self.scratch, 'roots', key_of(task.id))
        shutil.rmtree(root, ignore_errors=True)
        os.makedirs(os.path.join(root, 'results', 'benchmark'))
        for name in ('data', 'scripts'):
            os.symlink(os.path.join(REPO, name), os.path.join(root, name))
        return root

    def in_state(self, task_id):
        return task_id in self.shard['in_state']

    def seed_from_state(self, places, root):
        """Copies run folders finished in earlier jobs from the state branch into root.
        places: {path under root's results/benchmark/: task id in the state}."""
        for local, state_id in places.items():
            if not self.in_state(state_id):
                continue
            archive = subprocess.run(['git', '-C', REPO, 'archive', self.state_ref, f'results/benchmark/{state_id}'],
                                     capture_output=True, check=True)
            tmp = os.path.join(self.scratch, 'seed')
            shutil.rmtree(tmp, ignore_errors=True)
            os.makedirs(tmp)
            subprocess.run(['tar', '-x', '-C', tmp], input=archive.stdout, check=True)
            dest = os.path.join(root, 'results', 'benchmark', local)
            shutil.rmtree(dest, ignore_errors=True)
            shutil.copytree(os.path.join(tmp, 'results', 'benchmark', state_id), dest)

    def keep_folder(self, src, task_id):
        dest = self.out(task_id)
        shutil.rmtree(dest, ignore_errors=True)
        shutil.copytree(src, dest, symlinks=True)

    @staticmethod
    def take_rows(path):
        return read_rows(path)


def run_shard(shard, tasks_by_id, out, partial, scratch, tools, deadline, full_budget, wave, job, state_ref):
    for folder in (os.path.join(out, 'cloud'), scratch, partial):
        os.makedirs(folder, exist_ok=True)
    ctx = Context(shard, out, partial, scratch, tools, deadline, full_budget, state_ref)
    host = hostinfo.describe(scratch)
    outcomes = {}
    for task_id in shard['tasks']:
        task = tasks_by_id[task_id]
        if ctx.remaining() < 60:
            outcomes[task_id] = 'not started'
            continue
        start = time.time()
        print(f'[{utc()}] {task_id}: start ({ctx.remaining() // 60} min left in this job)', flush=True)
        outcome, rows = RUNNERS[task.kind](task, ctx)
        outcomes[task_id] = outcome
        print(f'[{utc()}] {task_id}: {outcome}', flush=True)
        if rows:
            append_rows(os.path.join(out, 'cloud', 'runs.tsv'), RUNS_HEADER, rows if isinstance(rows, list) else [rows])
        append_rows(os.path.join(out, 'cloud', 'hosts.tsv'), HOSTS_HEADER, [{
            **host, 'task': task_id, 'attempt': len(ctx.attempts.get(task_id, [])) or 1, 'wave': wave,
            'workflow_run': os.environ.get('GITHUB_RUN_ID', 'local'), 'job': job,
            'start_utc': utc(start), 'end_utc': utc(), 'outcome': outcome}])
    with open(os.path.join(out, 'cloud', 'outcomes.json'), 'w', encoding='utf-8') as f:
        json.dump({'outcomes': outcomes, 'partials': ctx.saved_partials}, f, indent=1, sort_keys=True)
    done = sum(o in (DONE, FAILED) for o in outcomes.values())
    print(f'shard finished: {done} of {len(outcomes)} tasks have a result; '
          f'{sum(o == INTERRUPTED for o in outcomes.values())} continue later')
