"""One function per kind of run. Each reproduces the commands of the laptop queues
(scripts/benchmark_queue.sh, benchmark_queue_b.sh, bench_refs.sh, bench_w2c1000.sh)
on a Linux runner, and says how the run ended:

  done         a result (the run folder has its finished marker)
  failed       a result: the method failed or reached its time limit (failed.txt says why)
  interrupted  not a result: the job ran out of time; the run is continued (resumable runs)
               or started again from the beginning in a later job (Amendment 10)

Every function receives the Task (catalog.py) and the Context of the shard (executor.py).
"""
import os
import shutil
import subprocess
import time

from catalog import REPO, input_path, reference_path
from state import REF_STEPS

DONE, FAILED, INTERRUPTED = 'done', 'failed', 'interrupted'


# --- helpers ---------------------------------------------------------------------
def utc(t=None):
    return time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime(t))


def timed(ctx, argv, timeout, time_file, cwd=None, stdout=None, env=None):
    """Runs argv under GNU time (wall time, peak memory) and coreutils timeout.
    Returns (exit code, seconds, peak MB, timed out)."""
    cmd = ['/usr/bin/time', '-v', '-o', time_file,
           'timeout', '-k', '60', str(max(1, int(timeout)))] + argv
    t0 = time.time()
    with open(stdout or os.devnull, 'a', encoding='utf-8') as out:
        rc = subprocess.call(cmd, cwd=cwd, stdout=out, stderr=subprocess.STDOUT,
                             env={**os.environ, **(env or {})})
    seconds = int(round(time.time() - t0))
    peak = 'NA'
    try:
        for line in open(time_file, encoding='utf-8'):
            if 'Maximum resident set size' in line:
                peak = str(int(line.split(':')[1]) // 1024)
    except (OSError, ValueError):
        pass
    # GNU time reports "Command exited with non-zero status 124" for a timeout
    return rc, seconds, peak, rc in (124, 137)


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(text if text.endswith('\n') else text + '\n')


def row(task, start, seconds, peak, rc):
    return {'dataset': task.dataset, 'method': task.method, 'start_utc': utc(start),
            'end_utc': utc(start + seconds), 'seconds': str(seconds), 'peak_mb': peak, 'exit': str(rc)}


def compare(ctx, folder, ds):
    """qmaws tree-compare of folder/tree.nwk with the reference (as benchmark_queue_b.sh)."""
    with open(os.path.join(folder, 'comparison.tsv'), 'w', encoding='utf-8') as out, \
         open(os.path.join(folder, 'comparison_error.txt'), 'w', encoding='utf-8') as err:
        subprocess.call([ctx.qmaws, '--quiet', 'tree-compare', '--tree', os.path.join(folder, 'tree.nwk'),
                         '--reference', os.path.join(REPO, reference_path(ds))], stdout=out, stderr=err)
    return os.path.getsize(os.path.join(folder, 'comparison_error.txt')) == 0


# --- the Linux methods (bench_tools.sh) --------------------------------------------
def run_tool(task, ctx):
    folder = ctx.out(task.id)
    shutil.rmtree(folder, ignore_errors=True)
    os.makedirs(folder)
    budget = ctx.remaining()
    timeout = min(task.limit, budget)
    start = time.time()
    rc, seconds, peak, timed_out = timed(
        ctx, ['bash', os.path.join(REPO, 'scripts', 'bench_tools.sh'), task.extra['tool'],
              os.path.join(REPO, input_path(task.dataset)), folder]
        + ([task.setting] if task.setting else []),
        timeout, os.path.join(folder, 'time.txt'), stdout=os.path.join(folder, 'run.log'),
        env={'BENCH_THREADS': str(ctx.threads), 'BENCH_LIMIT': f'{task.limit // 60}m',
             'BENCH_SCRATCH': ctx.scratch})
    if timed_out and timeout < task.limit:
        shutil.rmtree(folder, ignore_errors=True)       # the job ran out of time: not a result
        return INTERRUPTED, None
    tree = os.path.join(folder, 'tree.nwk')
    if timed_out:
        write(os.path.join(folder, 'failed.txt'),
              f'not run (time): reached the limit of {task.limit // 60} minutes (Amendment 10)')
        return FAILED, row(task, start, seconds, peak, rc)
    if os.path.isfile(tree) and os.path.getsize(tree) and compare(ctx, folder, task.dataset):
        write(os.path.join(folder, 'done.txt'), 'ok')
        return DONE, row(task, start, seconds, peak, rc)
    reason = ('tree not comparable with the reference, see comparison_error.txt'
              if os.path.isfile(tree) and os.path.getsize(tree) else 'see run.log')
    write(os.path.join(folder, 'failed.txt'), f'exit {rc}; {reason}')
    return FAILED, row(task, start, seconds, peak, rc)


# --- Q-MAWS (release 1.0.0) ---------------------------------------------------------
def _qmaws_command(task, ctx, folder):
    if os.path.isfile(os.path.join(folder, 'run.json')):
        # A run continued in another job: its lock belongs to a process on another runner.
        # Mark it closed (crates/qmaws-engine/src/lock.rs). Removing it is not enough: without
        # a lock, Q-MAWS takes a run.json changed in the last 120 s as in use, and the files
        # of a restored run are all new, so every resume stopped at once (Amendment 10c).
        lock = os.path.join(folder, 'work', 'run.lock')
        os.makedirs(os.path.dirname(lock), exist_ok=True)
        with open(lock, 'w', encoding='utf-8') as f:
            f.write('closed\n')
        return [ctx.qmaws, '--quiet', 'resume', '--output', folder]
    cmd = [ctx.qmaws, '--quiet', 'run', '--input', os.path.join(REPO, input_path(task.dataset)),
           '--reference', os.path.join(REPO, reference_path(task.dataset)), '--seed', '1',
           '--bootstrap', '0', '--no-live-tree', '--output', folder]
    if task.kind == 'w2c':        # Amendment 9: 1,000 W2c resamples, all cores (Amendment 10)
        cmd[-2:-2] = ['--replicates', '1000', '--cores', str(ctx.threads)]
    return cmd


def run_qmaws(task, ctx):
    """Q-MAWS run (also the W2c-1000 runs). No method limit; continued with qmaws resume."""
    folder = ctx.out(task.id)
    ctx.restore_partial(task, folder)
    os.makedirs(os.path.dirname(folder), exist_ok=True)
    start = time.time()
    rc, seconds, peak, timed_out = timed(
        ctx, _qmaws_command(task, ctx, folder), ctx.remaining(),
        ctx.scratch_file(task, 'time.txt'), stdout=os.path.join(os.path.dirname(folder), 'qmaws.out.txt'))
    ctx.add_attempt(task, seconds, peak)
    if os.path.isfile(os.path.join(folder, 'audit', 'root.txt')):
        return DONE, ctx.final_row(task, start, rc)
    if timed_out:
        ctx.save_partial(task, folder)
        return INTERRUPTED, None
    return ctx.retry_or_fail(task, folder, f'qmaws exit {rc}; see qmaws.out.txt', start, rc)


# --- ML-MAWS (as job_mlmaws and ml_once of benchmark_queue.sh) -----------------------
_ML_CLEANUP = ['ml_maws_matrix.phy', 'ml_maws_matrix.tsv', 'ml_maws_iqtree.ckp.gz', 'ml_maws_iqtree.mldist',
               'ml_maws_iqtree.model.gz', 'ml_maws_iqtree.bionj', 'ml_maws_iqtree.parstree',
               'ml_maws_iqtree.splits.nex']


def run_mlmaws(task, ctx):
    folder = ctx.out(task.id)
    ctx.restore_partial(task, folder)
    os.makedirs(folder, exist_ok=True)
    t = str(ctx.threads)
    iq = ('iqtree2 -s ml_maws_matrix.phy -st BIN -m MFP+ASC -bb 1000 -nt ' + t +
          ' --prefix ml_maws_iqtree')
    if os.path.isfile(os.path.join(folder, 'ml_maws_iqtree.ckp.gz')):
        # continued: IQ-TREE goes on from its checkpoint (same command, without -redo)
        script = f'cd "{folder}" && {iq} >> iqtree_normal_mode.txt 2>&1'
    else:
        script = (f'"{ctx.mlmaws}" -i "{os.path.join(REPO, input_path(task.dataset))}" -o "{folder}" '
                  f'--strand --iqtree --threads {t} > "{folder}/stdout.txt" 2> "{folder}/stderr.txt"; '
                  f'if [ ! -s "{folder}/ML_MAWS_tree.newick" ] && [ -f "{folder}/ml_maws_matrix.phy" ]; then '
                  f'cd "{folder}" && {iq} -redo > iqtree_normal_mode.txt 2>&1; fi')
    start = time.time()
    rc, seconds, peak, timed_out = timed(ctx, ['bash', '-c', script], ctx.remaining(),
                                         ctx.scratch_file(task, 'time.txt'), env={'PATH': ctx.path})
    ctx.add_attempt(task, seconds, peak)
    tree = next((n for n in ('ML_MAWS_tree.newick', 'ml_maws_iqtree.treefile')
                 if os.path.isfile(os.path.join(folder, n)) and os.path.getsize(os.path.join(folder, n))), None)
    if tree:
        for name in _ML_CLEANUP:
            try:
                os.remove(os.path.join(folder, name))
            except OSError:
                pass
        write(os.path.join(folder, 'done.txt'), f'tree={tree}')
        return DONE, ctx.final_row(task, start, rc)
    if timed_out:
        if os.path.isfile(os.path.join(folder, 'ml_maws_iqtree.ckp.gz')):
            ctx.save_partial(task, folder)
        else:
            shutil.rmtree(folder, ignore_errors=True)    # nothing to continue: start again later
        return INTERRUPTED, None
    return ctx.retry_or_fail(task, folder, f'exit {rc}; see stderr.txt', start, rc)


# --- the IQ-TREE reference on the true alignment (job_iqtree) ------------------------
def run_iqtree(task, ctx):
    folder = ctx.out(task.id)
    ctx.restore_partial(task, folder)
    os.makedirs(folder, exist_ok=True)
    aln = os.path.join(REPO, input_path(task.dataset))
    cmd = ['iqtree2', '-s', aln, '-m', 'MFP', '-nt', str(ctx.threads), '-seed', '1', '--prefix', 'true_aln']
    resumed = os.path.isfile(os.path.join(folder, 'true_aln.ckp.gz'))
    start = time.time()
    rc, seconds, peak, timed_out = timed(ctx, cmd if resumed else cmd + ['-redo'], ctx.remaining(),
                                         ctx.scratch_file(task, 'time.txt'), cwd=folder,
                                         stdout=os.path.join(folder, 'iqtree.out.txt'), env={'PATH': ctx.path})
    ctx.add_attempt(task, seconds, peak)
    if rc == 0 and os.path.isfile(os.path.join(folder, 'true_aln.treefile')):
        for ext in ('ckp.gz', 'mldist', 'model.gz', 'bionj', 'uniqueseq.phy'):
            try:
                os.remove(os.path.join(folder, 'true_aln.' + ext))
            except OSError:
                pass
        write(os.path.join(folder, 'done.txt'), 'tree=true_aln.treefile')
        return DONE, ctx.final_row(task, start, rc)
    if timed_out:
        ctx.save_partial(task, folder)
        return INTERRUPTED, None
    return ctx.retry_or_fail(task, folder, f'iqtree2 exit {rc}; see iqtree.out.txt', start, rc)


# --- the alignment-based references of Amendment 9 (bench_refs.sh) -------------------
def run_refs(task, ctx):
    """FastTree gene trees, CASTER-site and (weighted) TREE-QMC on one dataset. bench_refs.sh
    works in a repository folder; it gets a private one whose data/ and scripts/ are links."""
    root = ctx.private_root(task)
    ds = task.dataset
    base = os.path.dirname(task.id)          # '<dataset>' for evaluation, 'smoke' for the smoke test
    ctx.seed_from_state({f'{ds}/{s}': f'{base}/{s}' for s in REF_STEPS}, root)   # steps finished earlier
    full = ctx.remaining() >= ctx.full_budget
    rc, _, _, timed_out = timed(
        ctx, ['bash', os.path.join(REPO, 'scripts', 'bench_refs.sh')], ctx.remaining(),
        ctx.scratch_file(task, 'time.txt'), stdout=ctx.scratch_file(task, 'refs.log'),
        env={'BENCH_ROOT': root, 'BENCH_THREADS': str(ctx.threads), 'BENCH_SCRATCH': ctx.scratch,
             'BENCH_REFS_DATASETS': ds, 'PATH': ctx.path})
    bench = os.path.join(root, 'results', 'benchmark')
    if timed_out and full:   # could not finish within one whole job: not run (time), Amendment 10
        for s in REF_STEPS:
            f = os.path.join(bench, ds, s)
            if not any(os.path.isfile(os.path.join(f, m)) for m in ('done.txt', 'failed.txt')):
                write(os.path.join(f, 'failed.txt'), 'not run (time): did not finish within one job (Amendment 10)')
    rows = ctx.take_rows(os.path.join(bench, 'runs.tsv'))
    for s in REF_STEPS:     # every finished step is kept, also when the job ran out of time
        f = os.path.join(bench, ds, s)
        if any(os.path.isfile(os.path.join(f, m)) for m in ('done.txt', 'failed.txt')) and not ctx.in_state(f'{base}/{s}'):
            ctx.keep_folder(f, f'{base}/{s}')
    if timed_out and not full:
        return INTERRUPTED, rows
    return DONE, rows   # each step's own marker says whether it succeeded (bench_refs.sh)


# --- checks of the reused runs (job_verify) ---------------------------------------
def run_verify(task, ctx):
    run = os.path.join(REPO, task.extra['run'])
    report = ctx.out(task.id)
    os.makedirs(os.path.dirname(report), exist_ok=True)
    timed_out = timed(ctx, [ctx.qmaws, '--no-color', 'verify', '--quick', '--seed', '1', '--output', run,
                            '--input', os.path.join(REPO, input_path(task.dataset))],
                      ctx.remaining(), ctx.scratch_file(task, 'time.txt'), stdout=report + '.part')[3]
    if timed_out:
        os.remove(report + '.part')
        return INTERRUPTED, None
    os.replace(report + '.part', report)
    return DONE, None


# --- MAW matrices for the dependence analysis (Amendment 10 clarification) -------------
def run_matrix(task, ctx):
    folder = ctx.out(task.id)
    shutil.rmtree(folder, ignore_errors=True)
    start = time.time()
    rc, seconds, peak, timed_out = timed(
        ctx, [ctx.qmaws, '--quiet', 'matrix', '--input', os.path.join(REPO, input_path(task.dataset)),
              '--output', folder], ctx.remaining(), ctx.scratch_file(task, 'time.txt'),
        stdout=ctx.scratch_file(task, 'matrix.log'))
    if timed_out:
        shutil.rmtree(folder, ignore_errors=True)
        return INTERRUPTED, None
    write(os.path.join(folder, 'done.txt' if rc == 0 else 'failed.txt'), 'ok' if rc == 0 else f'exit {rc}')
    return (DONE if rc == 0 else FAILED), row(task, start, seconds, peak, rc)


RUNNERS = {'tool': run_tool, 'qmaws': run_qmaws, 'w2c': run_qmaws, 'mlmaws': run_mlmaws,
           'iqtree': run_iqtree, 'refs': run_refs, 'verify': run_verify, 'matrix': run_matrix}
