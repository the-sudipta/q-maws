"""Seals and verifies the runs of the extended benchmark with SHA-256 (tamper check).

Usage (from the repository folder):
  python scripts/bench_hash.py seal     seal every finished run that has no seal yet,
                                        then rebuild results/benchmark/benchmark.sha256
  python scripts/bench_hash.py verify   check every seal and the overall manifest

A run is a folder of results/benchmark/ with a finished marker: done.txt or
failed.txt (ML-MAWS, IQ-TREE reference, the Linux methods of phase B, tuning
and speed runs), audit/root.txt (Q-MAWS runs), or a folder of smoke/. Its
seal is <run>/files.sha256: one line "<sha256>  <path>" per file of the run
(Q-MAWS's work/ folder is left out; Q-MAWS has its own audit record). A run
is sealed only when its marker is older than two minutes, so the job that is
running is never sealed half-way. A seal is never overwritten: if the files
of a sealed run change, verify reports it.

results/benchmark/benchmark.sha256 lists the SHA-256 of every seal and of
every report in verify/ (phase A's checks of the reused runs). Its last line
is the root fingerprint: the SHA-256 of all lines above it.
"""
import hashlib
import os
import sys
import time

ROOT = os.path.join('results', 'benchmark')
SEAL = 'files.sha256'
MANIFEST = os.path.join(ROOT, 'benchmark.sha256')
SKIP_NAMES = {SEAL, 'run.lock'}
MIN_AGE = 120  # seconds since the finished marker was written


def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def run_files(run):
    """Every file of a run, as forward-slash paths relative to the run folder."""
    out = []
    for d, dirs, files in os.walk(run):
        rel = os.path.relpath(d, run).replace('\\', '/')
        if rel == 'work' or rel.startswith('work/'):
            dirs[:] = []
            continue
        dirs.sort()
        for name in sorted(files):
            if name in SKIP_NAMES or name.endswith('.part'):
                continue
            out.append((rel + '/' + name).lstrip('./') if rel != '.' else name)
    return sorted(out)


def finished_runs():
    """(run folder, marker file) for every finished run folder."""
    runs = []
    for d, dirs, files in os.walk(ROOT):
        dirs.sort()
        rel = os.path.relpath(d, ROOT).replace('\\', '/')
        if rel.startswith('verify'):
            continue
        parts = rel.split('/')
        if 'done.txt' in files or 'failed.txt' in files:
            runs.append((d, os.path.join(d, 'done.txt' if 'done.txt' in files else 'failed.txt')))
            dirs[:] = []
        elif os.path.isfile(os.path.join(d, 'audit', 'root.txt')) and parts[-1] == 'qmaws':
            runs.append((d, os.path.join(d, 'audit', 'root.txt')))
            dirs[:] = []
        elif len(parts) == 2 and parts[0] == 'smoke' and os.path.isfile(os.path.join(d, 'version.txt')):
            runs.append((d, os.path.join(d, 'version.txt')))
            dirs[:] = []
    return runs


def seal():
    now = time.time()
    new = old = young = 0
    for run, marker in finished_runs():
        if os.path.exists(os.path.join(run, SEAL)):
            old += 1
            continue
        if now - os.path.getmtime(marker) < MIN_AGE:
            young += 1
            continue
        lines = [f'{sha(os.path.join(run, p))}  {p}\n' for p in run_files(run)]
        with open(os.path.join(run, SEAL), 'w', encoding='utf-8', newline='\n') as f:
            f.writelines(lines)
        new += 1
    write_manifest()
    print(f'sealed {new} new runs; {old} already sealed; {young} finished less than two minutes ago (next time)')
    print(f'root fingerprint: {root_line()}')


def manifest_lines():
    lines = []
    for run, _ in finished_runs():
        s = os.path.join(run, SEAL)
        if os.path.exists(s):
            lines.append(f'{sha(s)}  {os.path.relpath(s, ROOT).replace(os.sep, "/")}\n')
    vdir = os.path.join(ROOT, 'verify')
    if os.path.isdir(vdir):
        for name in sorted(os.listdir(vdir)):
            if name.endswith('.txt'):
                lines.append(f'{sha(os.path.join(vdir, name))}  verify/{name}\n')
    return sorted(lines, key=lambda l: l.split('  ', 1)[1])


def write_manifest():
    lines = manifest_lines()
    root = hashlib.sha256(''.join(lines).encode('utf-8')).hexdigest()
    with open(MANIFEST, 'w', encoding='utf-8', newline='\n') as f:
        f.writelines(lines)
        f.write(f'{root}  ROOT FINGERPRINT\n')


def root_line():
    with open(MANIFEST, encoding='utf-8') as f:
        return f.read().splitlines()[-1].split()[0]


def verify():
    problems = []
    checked = files = 0
    for run, _ in finished_runs():
        s = os.path.join(run, SEAL)
        if not os.path.exists(s):
            continue
        checked += 1
        want = {}
        for line in open(s, encoding='utf-8'):
            h, p = line.rstrip('\n').split('  ', 1)
            want[p] = h
        have = run_files(run)
        rel = os.path.relpath(run, ROOT).replace(os.sep, '/')
        for p in sorted(set(want) - set(have)):
            problems.append(f'MISSING  {rel}/{p}')
        for p in sorted(set(have) - set(want)):
            problems.append(f'ADDED    {rel}/{p}')
        for p in sorted(set(want) & set(have)):
            files += 1
            if sha(os.path.join(run, p)) != want[p]:
                problems.append(f'CHANGED  {rel}/{p}')
    if os.path.exists(MANIFEST):
        stored = open(MANIFEST, encoding='utf-8').read().splitlines()
        body, last = stored[:-1], stored[-1].split()[0]
        if hashlib.sha256(''.join(l + '\n' for l in body).encode('utf-8')).hexdigest() != last:
            problems.append('CHANGED  benchmark.sha256 (its root fingerprint does not match its lines)')
        current = [l.rstrip('\n') for l in manifest_lines()]
        for l in sorted(set(body) - set(current)):
            problems.append(f'MANIFEST entry no longer matches: {l.split("  ", 1)[1]}')
        for l in sorted(set(current) - set(body)):
            problems.append(f'MANIFEST lacks (run "seal" to add): {l.split("  ", 1)[1]}')
    else:
        problems.append('no benchmark.sha256 yet (run "seal")')
    for p in problems:
        print(p)
    print(f'checked {checked} sealed runs, {files} files')
    tamper = [p for p in problems if not p.startswith('MANIFEST lacks') and not p.startswith('no benchmark')]
    print('Verdict: ' + ('PASS (every sealed file is unchanged)' if not tamper else f'FAIL ({len(tamper)} problems)'))
    return 0 if not tamper else 1


if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else ''
    if cmd == 'seal':
        seal()
    elif cmd == 'verify':
        sys.exit(verify())
    else:
        print(__doc__)
        sys.exit(2)
