"""Builds the data of the project website (site/) from the committed results:
site/data/site.json with every number the page shows, and site/figures/
with the figures. Run by the workflow `pages.yml` before each deployment, so
the website always shows the current results. Python 3 standard library only.

Usage: python scripts/site_data.py
"""
import csv
import datetime
import json
import os
import re
import shutil
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SITE = os.path.join(ROOT, 'site')
FIG = os.path.join(SITE, 'figures')

NAMES = {
    'fish_mito': 'Fish mtDNA', 'ecoli': 'E. coli/Shigella', 'ecoli_shigella_hgt': 'E. coli/Shigella HGT',
    'yersinia_hgt': 'Yersinia HGT', 'sim_hgt_0': 'Sim. HGT 0', 'sim_hgt_250': 'Sim. HGT 250',
    'sim_hgt_500': 'Sim. HGT 500', 'sim_hgt_750': 'Sim. HGT 750', 'sim_hgt_1000': 'Sim. HGT 1000',
    'influenza_a': 'Influenza A', 'coronavirus': 'Coronavirus', 'mammal_mtdna': 'Mammal mtDNA',
    'ebolavirus': 'Ebolavirus', 'rhinovirus': 'Rhinovirus',
}
AFPROJECT = ['fish_mito', 'ecoli', 'ecoli_shigella_hgt', 'yersinia_hgt', 'sim_hgt_0', 'sim_hgt_250',
             'sim_hgt_500', 'sim_hgt_750', 'sim_hgt_1000']


def path(*p):
    return os.path.join(ROOT, *p)


def read(*p):
    with open(path(*p), encoding='utf-8') as f:
        return f.read()


def table(*p, delim='\t'):
    lines = [l for l in read(*p).splitlines() if l.strip() and not l.startswith('#')]
    return list(csv.DictReader(lines, delimiter=delim))


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def ece_of(*p):
    m = re.search(r'ECE ([0-9.]+)', read(*p))
    return float(m.group(1)) if m else None


def count_files(folder, suffix):
    n = 0
    for _, _, files in os.walk(path(folder)):
        n += sum(1 for f in files if f.endswith(suffix))
    return n


def main():
    os.makedirs(os.path.join(SITE, 'data'), exist_ok=True)
    os.makedirs(FIG, exist_ok=True)
    d = {}

    # --- meta -------------------------------------------------------------
    commit = os.environ.get('GITHUB_SHA', '')
    if not commit:
        try:
            commit = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, capture_output=True, text=True).stdout.strip()
        except OSError:
            commit = ''
    version = re.search(r'^version = "([^"]+)"', read('Cargo.toml'), re.M).group(1)
    verify_reports = 0
    for top in ('results/runs', 'results/h5', 'results/scaling', 'results/sensitivity', 'results/controls'):
        for root, _, files in os.walk(path(top)):
            if any(f.startswith('verify_') and f.endswith('.txt') for f in files):
                verify_reports += 1
    d['meta'] = {
        'version': version,
        'built_utc': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%d %H:%M UTC'),
        'commit': commit[:7],
        'hashed_files': count_files('results', '.sha256'),
        'verified_runs': verify_reports,
        'repo': 'https://github.com/the-sudipta/q-maws',
    }

    # --- milestones and amendments -----------------------------------------
    ms = []
    for line in read('docs/milestones/README.md').splitlines():
        m = re.match(r'^\| `(M\d\d)\.md` \| (M\d+): (.*) \|$', line)
        if m:
            ms.append({'id': m.group(2), 'text': m.group(3)})
    d['milestones'] = ms
    am = []
    pre = read('docs/PREREGISTRATION.md')
    for m in re.finditer(r'^## Amendment (\d+) \((\d{4}-\d\d-\d\d)\): (.*)$', pre, re.M):
        am.append({'n': int(m.group(1)), 'date': m.group(2), 'title': m.group(3)})
    d['amendments'] = am

    # --- accuracy (M11) and H1 --------------------------------------------
    comp = table('results/summary/comparison.tsv')
    acc = []
    for ds in AFPROJECT:
        row = {'id': ds, 'name': NAMES[ds]}
        for r in comp:
            if r['dataset'] == ds:
                k = r['metric'].lower()
                row[k] = {'qmaws': num(r['qmaws_mean']), 'sd': num(r['qmaws_sd']),
                          'mlmaws': num(r['ml_maws_strand_reported']), 'best': num(r['afproject_best_reported'])}
        acc.append(row)
    d['accuracy'] = acc
    h1 = {r['metric']: r for r in table('results/summary/h1.tsv')}
    d['h1'] = {
        'test': {k: {'p_holm': num(v['p_holm']), 'median': num(v['median_difference']),
                     'w_plus': num(v['w_plus']), 'w_minus': num(v['w_minus']), 'met': v['criterion_met']}
                 for k, v in h1.items()},
        'pairs': [{'id': r['dataset'], 'name': NAMES[r['dataset']], 'metric': r['metric'],
                   'qmaws': num(r['qmaws']), 'mlmaws': num(r['ml_maws_strand_reported'])}
                  for r in table('results/summary/h1_pairs.tsv')],
    }

    # --- H3 ------------------------------------------------------------------
    d['h3'] = [{'t_long': num(r['t_long']), 'n': int(float(r['characters'])),
                'w1': num(r['w1_recovery']), 'w2': num(r['w2_recovery'])}
               for r in table('results/h3/recovery.csv')]

    # --- H4 ------------------------------------------------------------------
    h4 = {}
    for key, f in [('s1', 'calibration_s1.tsv'), ('ufboot', 'calibration_ufboot.tsv'), ('s2', 'calibration_s2.tsv')]:
        h4[key] = {'ece': ece_of('results', 'h4', f),
                   'bins': [{'lo': num(r['bin_lower']), 'hi': num(r['bin_upper']), 'n': int(r['edges']),
                             'support': num(r['mean_support']), 'true': num(r['fraction_in_true_tree'])}
                            for r in table('results', 'h4', f)]}
    d['h4'] = h4

    # --- H5 ------------------------------------------------------------------
    h5v = table('results/h5/h5_values.tsv')
    d['h5'] = {'values': [{'id': r['dataset'], **{k: num(v) for k, v in r.items() if k != 'dataset'}} for r in h5v]}
    tests, block = {}, None
    for line in read('results/h5/h5_test.tsv').splitlines():
        if line.startswith('# H5 (Amendment'):
            block = 'v2'
        elif line.startswith('# Descriptive'):
            block = 'primary'
        elif line.startswith('# H5 '):
            tests['verdict'] = line[5:].strip()
        elif block and line and not line.startswith(('#', 'metric')):
            f = line.split('\t')
            tests.setdefault(block, {})[f[0]] = {'n': int(f[1]), 'p_holm': num(f[6]), 'median': num(f[7]),
                                                 'hl': num(f[8]), 'met': f[9]}
    d['h5']['test'] = tests

    # --- comparison with peer-reviewed tools -------------------------------
    vals = table('results/baselines/values.tsv')
    q = {r['dataset']: r for r in table('results/summary/qmaws.tsv')}
    tools = []
    for ds in AFPROJECT:
        row = {'id': ds, 'name': NAMES[ds], 'qmaws_nrf': num(q[ds]['nrf_mean']), 'qmaws_nqd': num(q[ds]['nqd_mean'])}
        for r in vals:
            if r['dataset'] == ds:
                row[r['tool'] + '_nrf'] = num(r['nrf'])
                row[r['tool'] + '_nqd'] = num(r['nqd'])
        tools.append(row)
    d['tools'] = {'afproject': tools,
                  'h5': {f"{r['dataset']}|{r['tool']}": {'nrf': num(r['nrf']), 'nqd': num(r['nqd'])}
                         for r in vals if r['dataset'].startswith('hgt_')},
                  'paired': table('results/baselines/paired.tsv')}

    # --- H2 ------------------------------------------------------------------
    d['h2'] = {'values': [{k: (num(v) if k not in ('dataset', 'qmaws_faster') else v) for k, v in r.items()}
                          for r in table('results/scaling/h2_values.tsv')],
               'verdict': next((l[3:] for l in read('results/scaling/h2_test.txt').splitlines()
                                 if l.startswith('H2 ') and l.endswith('SUPPORTED')), '')}

    # --- sensitivity and seeds ---------------------------------------------
    sens = {}
    for r in table('results/sensitivity/weights.tsv'):
        if r['weights'] not in ('w1', 'w2a', 'w2b', 'w2c'):
            continue
        key = 'primary' if r['weights'] == 'w2c' else 'weights_' + r['weights']
        sens.setdefault(key, {})[r['dataset']] = num(r['nrf'])
    for r in table('results/sensitivity/settings.tsv'):
        sens.setdefault(r['setting'], {})[r['dataset']] = num(r['nrf'])
    d['sensitivity'] = sens
    d['seeds'] = [{'id': ds, 'name': NAMES[ds], 'nrf': num(q[ds]['nrf_mean']), 'nrf_sd': num(q[ds]['nrf_sd']),
                   'nqd': num(q[ds]['nqd_mean']), 'nqd_sd': num(q[ds]['nqd_sd'])} for ds in AFPROJECT]
    d['ncbi'] = [{'id': ds, 'name': NAMES[ds], 'taxa': int(q[ds]['taxa']), 's1': num(q[ds]['s1_mean']),
                  's2': num(q[ds]['s2_seed1'])} for ds in ('influenza_a', 'coronavirus', 'mammal_mtdna', 'ebolavirus', 'rhinovirus')]

    # --- proof: a real run --------------------------------------------------
    run = 'results/runs/fish_mito_seed2'
    log = read(run, 'run.log').splitlines()
    verify = sorted(f for f in os.listdir(path(run, 'report')) if f.startswith('verify_') and f.endswith('.txt'))[-1]
    vtext = read(run, 'report', verify).splitlines()
    d['proof'] = {
        'run': run,
        'log_head': log[:8], 'log_tail': log[-4:],
        'root': read(run, 'audit', 'root.txt').strip(),
        'hashes': [{'file': f, 'sha256': read(run, f + '.sha256').strip()}
                   for f in ('report/tree.nwk', 'audit/inputs.json', 'audit/stages.json', 'run.json')],
        'verify_head': vtext[:9], 'verify_tail': [l for l in vtext[-6:] if l.strip()],
        'worksheet': read(run, 'audit', 'sample_worksheets.txt').split('-' * 64)[1].strip().splitlines()[:40],
    }

    # --- figures ------------------------------------------------------------
    figs = [
        ('figures/paper/fig_accuracy_nrf.svg', 'accuracy', 'nRF against the reference tree on the nine AFproject datasets: Q-MAWS (mean of 5 seeds), ML-MAWS and the best AFproject tool (reported values).'),
        ('figures/paper/fig_h1_pairs_nrf.svg', 'h1', 'H1: nRF of Q-MAWS and ML-MAWS on the seven datasets with horizontal gene transfer.'),
        ('figures/paper/h3_recovery.svg', 'h3', 'H3: how often simple voting (W1) and the conditioned likelihood (W2) recover the true quartet tree when two taxa evolve fast.'),
        ('figures/paper/fig_h4_reliability.svg', 'h4', 'H4: reliability diagram. Each dot is a group of branches with similar support; on the dashed line, support equals the true fraction.'),
        ('figures/paper/fig_h5_levels_nrf.svg', 'h5', 'H5 data: median nRF per transfer level for both Q-MAWS versions, ML-MAWS, Mash, CD-MAWS and Peafowl.'),
        ('figures/paper/fig_tools_afproject_nrf.svg', 'tools', 'Q-MAWS against Mash, CD-MAWS and Peafowl on the AFproject datasets (nRF).'),
        ('figures/paper/h2_scaling.svg', 'h2', 'H2: wall-clock time against the number of taxa on the same laptop.'),
        ('figures/paper/fig_sensitivity_nrf.svg', 'sensitivity', 'Sensitivity analysis: nRF of each setting on four datasets.'),
        ('figures/paper/fig_seed_variability.svg', 'seeds', 'Seed variability: mean and standard deviation over five seeds.'),
        ('results/runs/fish_mito_2026-10-03_092551/figures/halo_tree.png', 'halo', 'Quartet Halo Tree of Fish mtDNA: branch colour shows S1 support; the ring shows how well each species agrees with its place in the tree; outer bands are fish families.'),
        ('results/runs/fish_mito_2026-10-03_092551/figures/tanglegram.png', 'halo', 'Tanglegram: the Fish mtDNA tree (left) against the AFproject reference (right); dashed lines mark species placed differently.'),
        ('results/runs/sim_hgt_500_seed1/figures/halo_tree.png', 'halo', 'Quartet Halo Tree of a simulated dataset with gene transfer (Sim. HGT 500).'),
        ('results/runs/ecoli_2026-10-03_080534/figures/halo_tree.png', 'halo', 'Quartet Halo Tree of E. coli/Shigella (29 strains).'),
        ('results/runs/fish_mito_2026-10-03_092551/figures/convergence.png', 'halo', 'Provisional trees during a run: how far each one is from the final tree as more quartets are weighed.'),
    ]
    out = []
    for src, group, caption in figs:
        if not os.path.exists(path(src)):
            continue
        name = src.replace('/', '_').replace('results_runs_', '').replace('figures_paper_', '')
        shutil.copy(path(src), os.path.join(FIG, name))
        out.append({'file': 'figures/' + name, 'group': group, 'caption': caption, 'source': src})
    d['figures'] = out

    with open(os.path.join(SITE, 'data', 'site.json'), 'w', encoding='utf-8', newline='\n') as f:
        json.dump(d, f, indent=1, ensure_ascii=False)
    print(f"site/data/site.json written ({len(out)} figures, {d['meta']['hashed_files']} hashed files)")


if __name__ == '__main__':
    main()
