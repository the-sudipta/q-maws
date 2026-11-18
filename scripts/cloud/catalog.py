"""What the extended benchmark runs: datasets, methods and the list of tasks
(docs/PREREGISTRATION.md, Amendments 8 to 10).

A task is one run. Its id is the path of its run folder (or report file) under
results/benchmark/, exactly the layout of the laptop queues, so the analysis
scripts (bench_collect.py, bench_stats.py, bench_extra.py) read cloud results
unchanged. Nothing here runs anything; see runners.py and executor.py.
"""
import csv
import os
from dataclasses import dataclass, field

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))

# --- datasets ------------------------------------------------------------------
AFPROJECT = ['fish_mito', 'ecoli', 'ecoli_shigella_hgt', 'yersinia_hgt',
             'sim_hgt_0', 'sim_hgt_250', 'sim_hgt_500', 'sim_hgt_750', 'sim_hgt_1000']
NCBI = ['coronavirus', 'ebolavirus', 'influenza_a', 'mammal_mtdna', 'rhinovirus']  # reused M11 runs only
H5 = [f'hgt_h{h}_r{r}' for h in ('000', '010', '020', '040') for r in range(1, 6)]
NEW = [f'bm_{c}_r{r}' for c in ('div005', 'div020', 'div050', 'm15', 'm60', 'frag') for r in range(1, 6)]
TUNE = [f'bm_tune_r{r}' for r in range(1, 7)]
SPEED = [f'scaling_m{m}_r{r}' for m in (25, 50, 100) for r in (1, 2, 3)]
EVALUATION = AFPROJECT + H5 + NEW                      # the 59 evaluation datasets
TRUE_ALIGNMENT = H5 + [d for d in NEW if not d.startswith('bm_frag')]  # the 45 with a known alignment
SMOKE_DATASET = 'bm_tune_r1'                            # Amendments 8b and 10
DEPENDENCE = {'fish_mito': 'fish_mito', **{f'hgt_h{h}_r1': f'hgt_h{h}_r1' for h in ('000', '010', '020', '040')}}
W2C1000 = [f'hgt_h020_r{r}' for r in range(1, 6)]       # Amendment 9


def input_group(ds):
    """The artifact that carries a dataset's input files (see inputs_linux.sh)."""
    if ds in AFPROJECT:
        return 'afproject'
    if ds in NCBI:
        return 'ncbi'
    if ds in H5:
        return 'h5'
    if ds.startswith('scaling_'):
        return 'scaling'
    return 'benchmark'


def input_path(ds):
    """Input of a dataset, relative to the repository: a folder or one multi-FASTA file."""
    fixed = {'fish_mito': 'data/raw/fish_mito/assembled-fish_mito',
             'ecoli': 'data/raw/ecoli/assembled-ecoli',
             'ecoli_shigella_hgt': 'data/raw/ecoli_shigella_hgt/unsimulated-ecoli_shigella',
             'yersinia_hgt': 'data/raw/yersinia_hgt/unsimulated-yersinia'}
    if ds in fixed:
        return fixed[ds]
    if ds.startswith('sim_hgt_'):
        return f'data/raw/sim_hgt/simulated-sim_hgt/hgt_{ds[len("sim_hgt_"):]}'
    if ds in NCBI:
        return f'data/raw/{ds}/{ds}.fasta'
    if ds.startswith('bm_frag_'):
        return f'data/raw/simulated/{ds}'
    return f'data/raw/simulated/{ds}/sequences.fasta'


def reference_path(ds):
    """Reference or true species tree of a dataset, relative to the repository."""
    if ds.startswith('sim_hgt_'):
        return 'data/references/sim_hgt.nwk'
    if ds in AFPROJECT:
        return f'data/references/{ds}.nwk'
    return f'data/manifests/simulated/trees/{ds}.nwk'


# --- methods -------------------------------------------------------------------
@dataclass(frozen=True)
class Method:
    name: str
    kind: str              # tool | qmaws | mlmaws | iqtree | refs
    default: str           # default setting ('-' when the method has none)
    grid: tuple            # tuning grid (empty when not tuned)
    estimate: int          # seconds on a 30-taxon, 30-Mbp dataset (first plan only)


def load_methods(path=os.path.join(HERE, 'config', 'methods.tsv')):
    methods = {}
    with open(path, encoding='utf-8') as f:
        for row in csv.DictReader(f, delimiter='\t'):
            grid = tuple(row['tuning_grid'].split()) if row['tuning_grid'] != '-' else ()
            methods[row['method']] = Method(row['method'], row['kind'], row['default'], grid,
                                            int(row['estimate_seconds']))
    return methods


METHODS = load_methods()
TOOLS = [m.name for m in METHODS.values() if m.kind == 'tool']   # run through bench_tools.sh


# --- tasks ---------------------------------------------------------------------
@dataclass
class Task:
    id: str                     # run folder (or report file) under results/benchmark/
    kind: str                   # tool | qmaws | mlmaws | iqtree | refs | verify | w2c | matrix
    method: str                 # label in runs.tsv
    dataset: str
    setting: str = ''           # tuned value for tool runs ('' = default)
    alone: bool = False         # speed runs: nothing else on the runner
    resumable: bool = False     # continued in a later job instead of started again
    limit: int = 0              # method time limit in seconds (0 = none, Amendments 8a and 10)
    needs: list = field(default_factory=list)   # task ids that must be finished first
    extra: dict = field(default_factory=dict)   # kind-specific values

    def inputs(self):
        """Datasets whose input files the task reads."""
        return [self.dataset] if self.dataset else []


TOOL_LIMIT = 5 * 3600 + 30 * 60    # 5 h 30 min (Amendment 10)


def tool_task(tid, method, ds, setting='', alone=False):
    """A run of scripts/bench_tools.sh. Its label in runs.tsv is <method>_<value> when a
    setting is given, as in benchmark_queue_b.sh."""
    return Task(tid, 'tool', f'{method}_{setting}' if setting else method, ds,
                setting=setting, alone=alone, limit=TOOL_LIMIT, extra={'tool': method})


def smoke_tasks():
    ds = SMOKE_DATASET
    tasks = [tool_task(f'smoke/{m}', m, ds) for m in TOOLS]
    tasks += [Task('smoke/qmaws', 'qmaws', 'qmaws', ds, resumable=True),
              Task('smoke/ml_maws', 'mlmaws', 'ml_maws', ds, resumable=True),
              Task('smoke/iqtree_true', 'iqtree', 'iqtree_true', ds, resumable=True),
              Task('smoke/refs', 'refs', 'refs', ds)]
    return tasks


def reused_runs():
    """The finished Q-MAWS runs of M11, H5 and M13 that the benchmark reuses (Amendment 8)."""
    runs = []
    for base, pattern in (('results/runs', None), ('results/h5', 'qmaws'), ('results/scaling', 'qmaws')):
        root = os.path.join(REPO, base)
        for name in sorted(os.listdir(root)):
            folder = f'{base}/{name}/{pattern}' if pattern else f'{base}/{name}'
            if os.path.isfile(os.path.join(REPO, folder, 'run.json')):
                runs.append(folder)
    return runs


def dataset_of_run(folder):
    """Dataset id of a reused run folder, e.g. results/runs/fish_mito_seed2 -> fish_mito."""
    parts = folder.split('/')
    if parts[1] in ('h5', 'scaling'):
        return parts[2]
    name = parts[2]
    for ds in AFPROJECT + NCBI:
        if name == ds or name.startswith(ds + '_seed') or name.startswith(ds + '_20'):
            return ds
    raise ValueError(f'unknown dataset of {folder}')


def tuned_values(chosen):
    """{method: value} for the methods whose tuned value differs from the default."""
    return {m: v for m, (d, v) in chosen.items() if v and v != d}


def full_tasks(chosen=None):
    """Every run of the benchmark. chosen: {method: (default, chosen value)} once tuning is
    finished (then the tuned evaluation runs are added); None before."""
    tasks = []
    # 1. checks of the reused runs (qmaws verify --quick, release 1.0.0)
    for run in reused_runs():
        tasks.append(Task('verify/' + run.replace('/', '_') + '.txt', 'verify', 'verify',
                          dataset_of_run(run), extra={'run': run}))
    # 2. tuning: every grid value on the six tuning datasets
    for m in TOOLS:
        for v in METHODS[m].grid:
            for ds in TUNE:
                tasks.append(tool_task(f'tuning/{m}_{v}/{ds}', m, ds, setting=v))
    # 3. evaluation with defaults, and tuned where the tuned value differs
    tuned = tuned_values(chosen or {})
    tuning_ids = {m: [f'tuning/{m}_{v}/{ds}' for v in METHODS[m].grid for ds in TUNE] for m in TOOLS}
    for ds in EVALUATION:
        for m in TOOLS:
            tasks.append(tool_task(f'{ds}/{m}', m, ds))
            if m in tuned:
                t = tool_task(f'{ds}/{m}_tuned', m, ds, setting=tuned[m])
                t.needs = tuning_ids[m]
                tasks.append(t)
    # 4. the Windows-native methods of Amendment 8a (now on Linux, Amendment 10)
    for ds in NEW:
        tasks.append(Task(f'{ds}/qmaws', 'qmaws', 'qmaws', ds, resumable=True))
    for ds in NEW + AFPROJECT:
        tasks.append(Task(f'{ds}/ml_maws', 'mlmaws', 'ml_maws', ds, resumable=True))
    for ds in TRUE_ALIGNMENT:
        tasks.append(Task(f'{ds}/iqtree_true', 'iqtree', 'iqtree_true', ds, resumable=True))
        tasks.append(Task(f'{ds}/refs', 'refs', 'refs', ds))   # FastTree, CASTER-site, TREE-QMC
    # 5. Amendment 9 checks of Q-MAWS
    for ds in W2C1000:
        tasks.append(Task(f'w2c1000/{ds}', 'w2c', 'qmaws_w2c1000', ds, resumable=True))
    for name, ds in DEPENDENCE.items():
        tasks.append(Task(f'dependence/{name}', 'matrix', 'qmaws_matrix', ds))
    # 6. speed: every method alone on each speed dataset
    for ds in SPEED:
        for m in TOOLS:
            tasks.append(tool_task(f'speed/{ds}/{m}', m, ds, alone=True))
        tasks.append(Task(f'speed/{ds}/qmaws', 'qmaws', 'qmaws', ds, alone=True, resumable=True))
        tasks.append(Task(f'speed/{ds}/ml_maws', 'mlmaws', 'ml_maws', ds, alone=True, resumable=True))
    return tasks
