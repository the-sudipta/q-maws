"""Draws the result charts of the paper as plain SVG from the committed result
tables (results/), into figures/paper/. Python 3 standard library only.

Usage: python scripts/paper_figures.py   (from anywhere inside the repository)
"""
import csv
import os
import statistics
import shutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, 'figures', 'paper')
COLORS = ['#4C6EF5', '#F08C00', '#2B8A3E', '#C2255C', '#7048E8', '#868E96']
NAMES = {
    'fish_mito': 'Fish mtDNA', 'ecoli': 'E. coli/Shigella', 'ecoli_shigella_hgt': 'E. coli/Sh. HGT',
    'yersinia_hgt': 'Yersinia HGT', 'sim_hgt_0': 'Sim. HGT 0', 'sim_hgt_250': 'Sim. HGT 250',
    'sim_hgt_500': 'Sim. HGT 500', 'sim_hgt_750': 'Sim. HGT 750', 'sim_hgt_1000': 'Sim. HGT 1000',
}


def tsv(path):
    with open(f'{ROOT}/{path}', encoding='utf-8') as f:
        return list(csv.DictReader((l for l in f if not l.startswith('#')), delimiter='\t'))


def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;')


def bars(file, title, groups, series, values, ymax=1.0, ylabel='', errors=None, note=''):
    """groups: x labels; series: legend names; values[g][s] (None = no value)."""
    W, H, L, R, T, B = 900, 460, 70, 20, 60, 110
    pw, ph = W - L - R, H - T - B
    gw = pw / len(groups)
    bw = gw * 0.8 / len(series)
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" font-family="Arial, sans-serif" font-size="12">',
         f'<rect width="{W}" height="{H}" fill="white"/>',
         f'<text x="{W/2}" y="24" text-anchor="middle" font-size="16" font-weight="bold">{esc(title)}</text>']
    for i in range(6):
        v = ymax * i / 5
        y = T + ph - ph * v / ymax
        o.append(f'<line x1="{L}" y1="{y:.1f}" x2="{W-R}" y2="{y:.1f}" stroke="#ddd"/>')
        o.append(f'<text x="{L-6}" y="{y+4:.1f}" text-anchor="end">{v:.2f}</text>')
    o.append(f'<text x="18" y="{T+ph/2}" text-anchor="middle" transform="rotate(-90 18 {T+ph/2})">{esc(ylabel)}</text>')
    for g, name in enumerate(groups):
        x0 = L + g * gw + gw * 0.1
        for s in range(len(series)):
            v = values[g][s]
            x = x0 + s * bw
            if v is None:
                o.append(f'<text x="{x+bw/2:.1f}" y="{T+ph-4}" text-anchor="middle" font-size="9" fill="#888">n/a</text>')
                continue
            h = ph * v / ymax
            o.append(f'<rect x="{x:.1f}" y="{T+ph-h:.1f}" width="{bw-1:.1f}" height="{max(h,2):.1f}" fill="{COLORS[s]}"><title>{esc(series[s])}: {v:.3f}</title></rect>')
            if errors and errors[g][s]:
                e = ph * errors[g][s] / ymax
                cx, cy = x + bw / 2, T + ph - h
                o.append(f'<line x1="{cx:.1f}" y1="{cy-e:.1f}" x2="{cx:.1f}" y2="{cy+e:.1f}" stroke="black"/>')
        lx = L + g * gw + gw / 2
        o.append(f'<text x="{lx:.1f}" y="{T+ph+14}" text-anchor="end" transform="rotate(-30 {lx:.1f} {T+ph+14})">{esc(name)}</text>')
    o.append(f'<line x1="{L}" y1="{T+ph}" x2="{W-R}" y2="{T+ph}" stroke="black"/>')
    lx = L
    for s, name in enumerate(series):
        o.append(f'<rect x="{lx}" y="{H-22}" width="12" height="12" fill="{COLORS[s]}"/>')
        o.append(f'<text x="{lx+16}" y="{H-12}">{esc(name)}</text>')
        lx += 34 + 7.2 * len(name)
    if note:
        o.append(f'<text x="{W-R}" y="44" text-anchor="end" font-size="11" fill="#555">{esc(note)}</text>')
    o.append('</svg>')
    open(f'{OUT}/{file}', 'w', encoding='utf-8').write('\n'.join(o))


def num(s):
    return None if s in ('-', '', 'NA') else float(s)


# 1-2. Accuracy against the references
comp = [r for r in tsv('results/summary/comparison.tsv') if r['dataset'] in NAMES]
for metric in ('nRF', 'nQD'):
    rows = [r for r in comp if r['metric'] == metric]
    bars(f'fig_accuracy_{metric.lower()}.svg', f'{metric} against the reference tree (lower is better)',
         [NAMES[r['dataset']] for r in rows],
         ['Q-MAWS (mean of 5 seeds)', 'ML-MAWS strand (reported)', 'Best AFproject tool (reported)'],
         [[num(r['qmaws_mean']), num(r['ml_maws_strand_reported']), num(r['afproject_best_reported'])] for r in rows],
         ylabel=metric, errors=[[num(r['qmaws_sd']), None, None] for r in rows])

# 3. H1 pairs
pairs = tsv('results/summary/h1_pairs.tsv')
for metric in ('nRF', 'nQD'):
    rows = [r for r in pairs if r['metric'] == metric]
    bars(f'fig_h1_pairs_{metric.lower()}.svg', f'H1 pairs: {metric} on the seven HGT datasets (lower is better)',
         [NAMES[r['dataset']] for r in rows], ['Q-MAWS (mean of 5 seeds)', 'ML-MAWS strand (reported)'],
         [[float(r['qmaws']), float(r['ml_maws_strand_reported'])] for r in rows], ylabel=metric)

# 4. Seed variability
q = [r for r in tsv('results/summary/qmaws.tsv') if r['dataset'] in NAMES]
bars('fig_seed_variability.svg', 'Seed variability: mean and SD over 5 seeds (bars = mean, whisker = 1 SD)',
     [NAMES[r['dataset']] for r in q], ['nRF', 'nQD'],
     [[float(r['nrf_mean']), float(r['nqd_mean'])] for r in q], ylabel='distance',
     errors=[[float(r['nrf_sd']), float(r['nqd_sd'])] for r in q])

# 5. Sensitivity (nRF), primary + weights + settings
sens = {}
for r in tsv('results/sensitivity/settings.tsv'):
    sens[(r['dataset'], r['setting'])] = float(r['nrf'])
weights = tsv('results/sensitivity/weights.tsv')
for r in weights:
    key = r.get('weights') or r.get('variant') or r.get('setting')
    sens[(r['dataset'], 'weights_' + key.lower())] = float(r['nrf'])
dsets = ['fish_mito', 'ecoli', 'sim_hgt_0', 'sim_hgt_500']
settings = [('primary', 'Primary (W2c)'), ('weights_w1', 'W1'), ('weights_w2a', 'W2a'), ('weights_w2b', 'W2b'),
            ('lengths_k1', 'K = 1'), ('lengths_k2', 'K = 2'), ('lengths_k4', 'K = 4'),
            ('strand_off', 'Strand off'), ('matrix_ml', 'M_ml'), ('model_w2emp', 'W2-emp')]
bars('fig_sensitivity_nrf.svg', 'Sensitivity (seed 1): nRF of each setting (lower is better)',
     [lab for _, lab in settings], [NAMES[d] for d in dsets],
     [[sens.get((d, k)) for d in dsets] for k, _ in settings], ylabel='nRF')

# 6. Reliability diagram (H4)
W, H, L, T, S = 520, 520, 70, 50, 400
o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" font-family="Arial, sans-serif" font-size="12">',
     f'<rect width="{W}" height="{H}" fill="white"/>',
     f'<text x="{W/2}" y="26" text-anchor="middle" font-size="16" font-weight="bold">H4 reliability diagram (150 edges)</text>']
for i in range(6):
    v = i / 5
    o.append(f'<line x1="{L}" y1="{T+S-S*v}" x2="{L+S}" y2="{T+S-S*v}" stroke="#eee"/>')
    o.append(f'<text x="{L-6}" y="{T+S-S*v+4}" text-anchor="end">{v:.1f}</text>')
    o.append(f'<text x="{L+S*v}" y="{T+S+16}" text-anchor="middle">{v:.1f}</text>')
o.append(f'<line x1="{L}" y1="{T+S}" x2="{L+S}" y2="{T}" stroke="#999" stroke-dasharray="5,4"/>')
o.append(f'<text x="{L+S/2}" y="{T+S+36}" text-anchor="middle">mean support in the bin</text>')
o.append(f'<text x="20" y="{T+S/2}" text-anchor="middle" transform="rotate(-90 20 {T+S/2})">fraction of edges in the true tree</text>')
for k, (f, name) in enumerate([('calibration_s1.tsv', 'Q-MAWS S1 (ECE 0.2002)'), ('calibration_ufboot.tsv', 'ML-MAWS UFBoot (ECE 0.2002)'),
                               ('calibration_s2.tsv', 'Q-MAWS S2 (ECE 0.0861)')]):
    pts = [(float(r['mean_support']), float(r['fraction_in_true_tree']), int(r['edges'])) for r in tsv(f'results/h4/{f}') if r['mean_support'] != 'NA']
    path = ' '.join(f'{L+S*x:.1f},{T+S-S*y:.1f}' for x, y, _ in pts)
    o.append(f'<polyline points="{path}" fill="none" stroke="{COLORS[k]}" stroke-width="2"/>')
    for x, y, n in pts:
        o.append(f'<circle cx="{L+S*x:.1f}" cy="{T+S-S*y:.1f}" r="{3+min(n,85)**0.5/1.5:.1f}" fill="{COLORS[k]}" fill-opacity="0.6"><title>{n} edges</title></circle>')
    o.append(f'<rect x="{L+10}" y="{T+10+k*18}" width="12" height="12" fill="{COLORS[k]}"/><text x="{L+28}" y="{T+21+k*18}">{name}</text>')
o.append(f'<text x="{L+S}" y="{T+S-8}" text-anchor="end" fill="#777" font-size="11">dashed line = perfect calibration; dot size = number of edges</text>')
o.append('</svg>')
open(f'{OUT}/fig_h4_reliability.svg', 'w', encoding='utf-8').write('\n'.join(o))

# 7. Comparison with peer-reviewed tools and H5 per transfer level
v = {(r['dataset'], r['tool']): r for r in tsv('results/baselines/values.tsv')}
q = {r['dataset']: r for r in tsv('results/summary/qmaws.tsv')}
af = ['fish_mito','ecoli','ecoli_shigella_hgt','yersinia_hgt','sim_hgt_0','sim_hgt_250','sim_hgt_500','sim_hgt_750','sim_hgt_1000']
for m in ('nrf','nqd'):
    bars(f'fig_tools_afproject_{m}.svg', f'{m.replace("n","n",1).upper().replace("NRF","nRF").replace("NQD","nQD")} on the AFproject datasets: Q-MAWS and peer-reviewed tools (lower is better)',
         [NAMES[d] for d in af], ['Q-MAWS (mean of 5 seeds)', 'Mash', 'CD-MAWS', 'Peafowl'],
         [[float(q[d][f'{m}_mean'])] + [float(v[(d,t)][m]) for t in ('mash','cdmaws','peafowl')] for d in af], ylabel=m)
h = tsv('results/h5/h5_values.tsv')
lv = [('h000','h = 0'),('h010','h = 0.1'),('h020','h = 0.2'),('h040','h = 0.4')]
for m in ('nrf','nqd'):
    vals = []
    for code, _ in lv:
        rows = [r for r in h if code in r['dataset']]
        med = statistics.median
        vals.append([med([float(r[f'primary_{m}']) for r in rows]), med([float(r[f'v2_{m}']) for r in rows]), med([float(r[f'ml_maws_{m}']) for r in rows])]
                    + [med([float(v[(r['dataset'],t)][m]) for r in rows]) for t in ('mash','cdmaws','peafowl')])
    bars(f'fig_h5_levels_{m}.svg', f'H5 data: median {"nRF" if m=="nrf" else "nQD"} per transfer level (5 datasets each; lower is better)',
         [l for _, l in lv], ['Q-MAWS primary', 'Q-MAWS v2', 'ML-MAWS', 'Mash', 'CD-MAWS', 'Peafowl'], vals, ymax=0.5, ylabel=m)

# 8. Figures made by the program itself (H3 recovery, H2 scaling)
for src, dst in [('results/h3/recovery.svg', 'h3_recovery.svg'), ('results/scaling/scaling.svg', 'h2_scaling.svg')]:
    shutil.copy(os.path.join(ROOT, src), os.path.join(OUT, dst))
print('figures written to', OUT)
