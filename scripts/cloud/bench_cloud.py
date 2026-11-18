"""Command line of the cloud benchmark (docs/PREREGISTRATION.md, Amendment 10).
Called by .github/workflows/benchmark_cloud.yml and benchmark_wave.yml; only the
Python standard library is used.

  plan     --state DIR --mode smoke|full --max-shards N --out FILE   plan one wave
  run      --shard FILE --plan FILE --out DIR --partial DIR --scratch DIR --tools DIR
           --deadline EPOCH --wave N --job NAME --state-ref REF          run one job
  merge    --state DIR --outputs DIR --run ID --wave N                  add a wave to the state
  inputs   --data DIR --out FILE                                         record input sizes
  status   --state DIR --mode smoke|full                                 print progress
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import catalog  # noqa: E402
import planner  # noqa: E402
from state import write_json  # noqa: E402

FULL_BUDGET = catalog.TOOL_LIMIT     # a task that starts with this much time left had a whole job


def cmd_plan(a):
    shards, summary = planner.plan(a.state, a.mode, a.max_shards)
    write_json(a.out, {'shards': shards, 'summary': summary})
    print(json.dumps(summary, indent=1))
    gh_out = os.environ.get('GITHUB_OUTPUT')
    if gh_out:
        with open(gh_out, 'a', encoding='utf-8') as f:
            f.write(f"matrix={json.dumps({'shard': [s['index'] for s in shards]})}\n")
            f.write(f'count={len(shards)}\n')
            f.write(f"remaining={summary['pending'] + summary['blocked']}\n")


def cmd_run(a):
    import executor  # imported here: it needs the tools of a runner
    plan = json.load(open(a.plan, encoding='utf-8'))
    shard = plan['shards'][a.shard]
    tasks = {i: catalog.Task(**d) for i, d in shard['defs'].items()}
    executor.run_shard(shard, tasks, a.out, a.partial, a.scratch, a.tools,
                       a.deadline, FULL_BUDGET, a.wave, a.job, a.state_ref)


def cmd_merge(a):
    import merge
    merge.merge(a.state, a.outputs, a.run, a.wave)


def cmd_inputs(a):
    """Records the size (letters of all files) and taxon count of every input."""
    datasets = {}
    every = sorted(set(catalog.EVALUATION + catalog.TUNE + catalog.SPEED + catalog.NCBI))
    for ds in every:
        path = os.path.join(a.data, catalog.input_path(ds))
        files = ([os.path.join(path, f) for f in sorted(os.listdir(path))] if os.path.isdir(path)
                 else [path] if os.path.isfile(path) else [])
        if not files:
            continue
        taxa = len(files) if os.path.isdir(path) else sum(
            1 for line in open(path, encoding='ascii', errors='replace') if line.startswith('>'))
        datasets[ds] = {'bytes': sum(os.path.getsize(f) for f in files), 'taxa': taxa,
                        'group': catalog.input_group(ds)}
    write_json(a.out, {'datasets': datasets})
    print(f'{len(datasets)} datasets recorded')


def cmd_status(a):
    _, summary = planner.plan(a.state, a.mode, 10 ** 6)
    print(json.dumps(summary, indent=1))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='command', required=True)
    s = sub.add_parser('plan')
    s.add_argument('--state', required=True)
    s.add_argument('--mode', choices=['smoke', 'full'], required=True)
    s.add_argument('--max-shards', type=int, default=20)
    s.add_argument('--out', required=True)
    s.set_defaults(func=cmd_plan)
    s = sub.add_parser('run')
    s.add_argument('--plan', required=True)
    s.add_argument('--shard', type=int, required=True)
    s.add_argument('--out', required=True)
    s.add_argument('--partial', required=True)
    s.add_argument('--scratch', required=True)
    s.add_argument('--tools', required=True)
    s.add_argument('--deadline', type=float, required=True)
    s.add_argument('--wave', required=True)
    s.add_argument('--job', required=True)
    s.add_argument('--state-ref', default='FETCH_HEAD')
    s.set_defaults(func=cmd_run)
    s = sub.add_parser('merge')
    s.add_argument('--state', required=True)
    s.add_argument('--outputs', required=True)
    s.add_argument('--run', required=True)
    s.add_argument('--wave', required=True)
    s.set_defaults(func=cmd_merge)
    s = sub.add_parser('inputs')
    s.add_argument('--data', required=True)
    s.add_argument('--out', required=True)
    s.set_defaults(func=cmd_inputs)
    s = sub.add_parser('status')
    s.add_argument('--state', required=True)
    s.add_argument('--mode', choices=['smoke', 'full'], required=True)
    s.set_defaults(func=cmd_status)
    a = p.parse_args()
    a.func(a)


if __name__ == '__main__':
    main()
