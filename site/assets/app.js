/* Q-MAWS project page: renders everything from data/site.json, which the
   workflow rebuilds from the repository's results on every push. */
(function () {
  'use strict';
  const C = { navy: '#1f4e79', orange: '#d9731a', teal: '#23877a', red: '#b8443b', gold: '#a07a12', grey: '#8a94a6', purple: '#6b4fa0' };
  const ICON = n => `https://img.icons8.com/ios/100/1f4e79/${n}.png`;
  const ICON_NEW = n => `https://img.icons8.com/ios/100/d9731a/${n}.png`;
  const $ = s => document.querySelector(s);
  const $$ = s => Array.from(document.querySelectorAll(s));
  const fmt = (v, d = 3) => (v === null || v === undefined || isNaN(v)) ? '–' : Number(v).toFixed(d);
  const esc = s => String(s).replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
  const charts = {};
  const lazy = {};   // charts of hidden tabs are drawn when the tab is first shown
  function whenShown(panel, fn) {
    if ($('#' + panel).classList.contains('on')) fn(); else lazy[panel] = fn;
  }

  if (window.Chart) {
    Chart.defaults.font.family = '"Inter", system-ui, sans-serif';
    Chart.defaults.color = '#3d4a60';
    Chart.defaults.plugins.legend.labels.usePointStyle = true;
    Chart.defaults.plugins.tooltip.backgroundColor = '#15233b';
    Chart.defaults.plugins.tooltip.padding = 10;
  }

  // ---------------------------------------------------------------- navigation
  $('.nav-toggle').addEventListener('click', () => $('.nav-links').classList.toggle('open'));
  $$('.nav-links a').forEach(a => a.addEventListener('click', () => $('.nav-links').classList.remove('open')));
  const sections = $$('section[id]');
  window.addEventListener('scroll', () => {
    let cur = '';
    sections.forEach(s => { if (window.scrollY >= s.offsetTop - 120) cur = s.id; });
    $$('.nav-links a').forEach(a => a.classList.toggle('active', a.getAttribute('href') === '#' + cur));
  }, { passive: true });

  // ---------------------------------------------------------------- tabs
  $$('#res-tabs .tab').forEach(t => t.addEventListener('click', () => {
    $$('#res-tabs .tab').forEach(x => x.classList.remove('on'));
    $$('#results .panel').forEach(x => x.classList.remove('on'));
    t.classList.add('on');
    $('#' + t.dataset.p).classList.add('on');
    const fn = lazy[t.dataset.p];
    if (fn) { delete lazy[t.dataset.p]; setTimeout(fn, 0); }
  }));

  fetch('data/site.json').then(r => r.json()).then(render).catch(err => {
    console.error(err);
    document.body.insertAdjacentHTML('afterbegin', '<p style="padding:1rem;background:#f8e6e4">The data of this page could not be loaded.</p>');
  });

  function render(d) {
    meta(d); hypotheses(d); timelines(d); workflow(); h1(d); h2(d); h3(d); h4(d); h5(d);
    tools(d); paired(d); sensitivity(d); seeds(d); gallery(d); proof(d); animate();
  }

  // ---------------------------------------------------------------- meta
  function meta(d) {
    const m = d.meta;
    $('#b-version').textContent = 'Version ' + m.version;
    $$('.m-version').forEach(e => e.textContent = m.version);
    $('#m-commit').textContent = m.commit || 'local';
    $('#m-built').textContent = m.built_utc;
    $('#s-runs').dataset.count = m.verified_runs;
    $('#s-hashes').dataset.count = m.hashed_files;
  }

  const V = {
    yes: '<span class="verdict v-yes">Supported</span>',
    no: '<span class="verdict v-no">Not supported</span>',
    near: '<span class="verdict v-near">Practically equal · formally not supported</span>',
  };

  // ---------------------------------------------------------------- hypotheses
  function hypotheses(d) {
    const ece = d.h4;
    const nearH4 = ece.s1.ece - ece.ufboot.ece;
    const h3big = d.h3.filter(r => r.n === 100000);
    const rows = [
      ['H1', 'Under gene transfer, does Q-MAWS make fewer tree errors than ML-MAWS?', '7 datasets with transfer; exact one-sided Wilcoxon test, Holm correction over nRF and nQD', 'Adjusted p < 0.05 for at least one measure, and median difference < 0',
        V.no + `<div class="small muted">Comparable nRF; worse nQD. p<sub>Holm</sub> = ${fmt(d.h1.test.nRF.p_holm)} (nRF)</div>`],
      ['H2', 'Is Q-MAWS faster than ML-MAWS when there are many species?', 'Same laptop; 25, 50 and 100 species, 3 datasets each', 'Q-MAWS faster in every run with ≥ 100 species',
        V.no + `<div class="small muted">Faster at 25 and 50 species, slower at 100</div>`],
      ['H3', 'Is the likelihood safe from long-branch attraction, where simple voting fails?', 'Four-species simulation, 200 replicates per setting', 'Where voting is below 50 % at 100,000 characters, the likelihood reaches ≥ 95 %',
        V.yes + `<div class="small muted">Likelihood ${h3big.map(r => Math.round(r.w2 * 1000) / 10 + ' %').join(', ')}; voting ${h3big.map(r => Math.round(r.w1 * 100) + ' %').join(', ')}</div>`],
      ['H4', 'Are Q-MAWS support values at least as well calibrated as ML-MAWS bootstrap values?', '150 branches with known truth; expected calibration error (ECE)', 'ECE(S1) ≤ ECE(UFBoot)',
        V.near + `<div class="small muted">ECE ${fmt(ece.s1.ece, 6)} vs ${fmt(ece.ufboot.ece, 6)} (gap ${fmt(nearH4, 5)}); S2: ${fmt(ece.s2.ece, 3)}</div>`],
      ['H5', 'On new data, does the vote-weight version (v2) beat ML-MAWS?', '20 new simulated datasets made after the test was written; test as in H1', 'As in H1',
        V.no + `<div class="small muted">But the original method beat ML-MAWS: median nRF difference ${fmt(d.h5.test.primary.nRF.median)}, p<sub>Holm</sub> = ${fmt(d.h5.test.primary.nRF.p_holm)}</div>`],
    ];
    $('#hyp-table tbody').innerHTML = rows.map(r => `<tr><td><b>${r[0]}</b></td><td>${r[1]}</td><td class="small">${r[2]}</td><td class="small">${r[3]}</td><td>${r[4]}</td></tr>`).join('');
  }

  // ---------------------------------------------------------------- timelines
  function timelines(d) {
    $('#tl-milestones').innerHTML = d.milestones.map(m =>
      `<div class="tl-item reveal"><div class="tl-dot">${esc(m.id.replace('M', ''))}</div><b>${esc(m.id)}</b> · ${esc(m.text)}</div>`).join('');
    $('#tl-amendments').innerHTML = d.amendments.map(a =>
      `<div class="tl-item amend reveal"><div class="tl-dot">A${a.n}</div><b>Amendment ${a.n}</b> <span class="muted small">${a.date}</span><br>${esc(a.title)}</div>`).join('') +
      `<p class="small muted">Each amendment was committed before the experiment it changed. Full text: <code>docs/PREREGISTRATION.md</code>.</p>`;
  }

  // ---------------------------------------------------------------- workflow
  const STEPS = [
    { lane: 0, icon: 'document', t: 'Genomes in', s: 'FASTA files, one per species',
      d: 'Headers, spaces and digits are removed, letters become upper case and only A, C, G and T are kept. Every removal is counted, and the SHA-256 of each cleaned sequence is recorded.',
      f: 'crates/qmaws-core/src/input.rs' },
    { lane: 0, icon: 'dna-helix', t: 'Minimal absent words', s: 'suffix automaton',
      d: 'A word is a minimal absent word if it does not occur in the genome, although the word without its first letter and the word without its last letter both do. They are read off a suffix automaton, built in linear time.',
      eq: 'w = x·u·y is a MAW of s  ⇔  x·u ∈ s, u·y ∈ s, x·u·y ∉ s', f: 'crates/qmaws-core/src/maw.rs → extract_strand_aware' },
    { lane: 0, icon: 'filter', t: 'Strand filter & lengths', s: 'entropy picks 3 word lengths',
      d: 'A word is kept only if its reverse complement is also absent-minimal in the same genome. The three word lengths whose matrix columns carry the most information (binary Shannon entropy) are kept.',
      eq: 'H(L) = Σ columns [ −p log₂ p − (1 − p) log₂ (1 − p) ]', f: 'crates/qmaws-core/src/matrix.rs → select_lengths' },
    { lane: 0, icon: 'grid', t: '0/1 matrix', s: 'one row per species, one column per word',
      d: 'Entry 1 if the word is a minimal absent word of that species. Q-MAWS keeps every column (M_full). Its version of ML-MAWS\'s matrix was checked to be byte-identical to ML-MAWS\'s own.',
      f: 'crates/qmaws-core/src/matrix.rs → build_full' },
    { lane: 1, icon: 'calculator', t: 'Count 16 patterns', s: 'for every group of four species',
      d: 'For four species, every column shows one of 16 patterns of 0s and 1s. All counts follow exactly from a few bit operations (inclusion–exclusion), so millions of quartets are counted in seconds.',
      eq: 'c(P) = Σ_{P ⊆ S ⊆ q} (−1)^{|S|−|P|} n(S)', f: 'crates/qmaws-core/src/quartet.rs → pattern_counts' },
    { lane: 1, icon: 'scales', t: 'Weigh the 3 trees', s: 'conditioned two-state likelihood',
      d: 'Each of the three possible four-species trees is fitted with a small two-state model. The pattern 0000 says nothing about these four species, so the likelihood is conditioned on "not 0000". The weight of a tree is how often it wins when the counts are resampled 100 times (W2c).',
      eq: 'ℓ_T = Σ_{x ≠ 0000} c(x) [ ln P_T(x) − ln(1 − P_T(0000)) ]', f: 'crates/qmaws-core/src/weight.rs → log_likelihood, w2c' },
    { lane: 1, icon: 'tree-structure', t: 'Join into one tree', s: 'wQFM-rs amalgamation',
      d: 'All weighted four-species trees are combined into one tree by splitting the species recursively so that as much quartet weight as possible is satisfied. The Rust version gives the same tree as the original wQFM program on all 23 test inputs.',
      f: 'crates/qmaws-core/src/amalgamate.rs → wqfm' },
    { lane: 1, icon: 'combo-chart', t: 'Support & halo', s: 'S1, S2 and a value per species',
      d: 'S1: the share of quartet weight that agrees with each branch. S2: how often a branch returns when the columns are resampled (bootstrap). Halo: how well each species agrees with its place in the tree; a low value flags a possible transfer or misplacement.',
      f: 'crates/qmaws-core/src/support.rs, bootstrap.rs' },
    { lane: 2, icon: 'bar-chart', t: 'Tree, figures, tables', s: 'Halo Tree, tanglegram, nRF, nQD', new: false,
      d: 'Each run ends with its tree, the Quartet Halo Tree, a tanglegram against the reference tree, an interactive tree and the error measures nRF, nQD and MSD.',
      f: 'crates/qmaws-viz/, crates/qmaws-core/src/metrics.rs' },
    { lane: 2, icon: 'fingerprint', t: 'Audit record', s: 'SHA-256 of every file, root fingerprint',
      d: 'Every finished stage and chunk is written with its SHA-256. One root fingerprint summarises the whole run. It is identical on Windows, macOS and Linux and after any stop and resume.',
      f: 'crates/qmaws-engine/src/analysis.rs, store.rs, hash.rs' },
    { lane: 2, icon: 'checked', t: 'Verify', s: 'anyone can recompute and compare',
      d: 'qmaws verify recomputes the inputs\' hashes and a random sample of the run (or the whole run) and compares with the record. PASS means everything agrees. All committed runs passed.',
      f: 'crates/qmaws-engine/src/verify.rs' },
  ];
  function workflow() {
    const lanes = ['Same as ML-MAWS · verified byte-identical', 'New in Q-MAWS', 'Output & proof'];
    const host = $('#flow');
    lanes.forEach((name, li) => {
      const lane = document.createElement('div');
      lane.className = 'lane reveal' + (li === 1 ? ' new' : '');
      lane.innerHTML = `<div class="lane-title"><span>${name}</span><span>${['①', '②', '③'][li]}</span></div>`;
      STEPS.forEach((s, i) => {
        if (s.lane !== li) return;
        const el = document.createElement('div');
        el.className = 'step';
        el.innerHTML = `<div class="n">${i + 1}</div><img src="${li === 1 ? ICON_NEW(s.icon) : ICON(s.icon)}" alt=""><div><b>${s.t}</b><span>${s.s}</span></div>`;
        el.addEventListener('click', () => showStep(i, el));
        lane.appendChild(el);
      });
      host.appendChild(lane);
    });
    showStep(4, host.querySelectorAll('.step')[4]);
  }
  function showStep(i, el) {
    const s = STEPS[i];
    $$('.step').forEach(x => x.classList.remove('on'));
    if (el) el.classList.add('on');
    $('#flow-detail').innerHTML = `<div class="kicker">Step ${i + 1}</div><h3>${s.t}</h3><p>${s.d}</p>${s.eq ? `<div class="eq">${s.eq}</div>` : ''}<div class="file">${s.f}</div>`;
  }

  // ---------------------------------------------------------------- chart helpers
  function bar(id, labels, sets, opts = {}) {
    if (charts[id]) charts[id].destroy();
    charts[id] = new Chart($('#' + id), {
      type: 'bar',
      data: { labels, datasets: sets.map(s => ({ borderRadius: 4, maxBarThickness: 26, ...s })) },
      options: {
        responsive: true, maintainAspectRatio: false,
        scales: { y: { beginAtZero: true, max: opts.max, title: { display: !!opts.y, text: opts.y } }, x: { ticks: { autoSkip: false, maxRotation: 40 } } },
        plugins: { tooltip: { callbacks: { label: c => `${c.dataset.label}: ${fmt(c.raw)}` } } },
      },
    });
  }
  function toggle(name, fn) {
    const t = document.querySelector(`.toggle[data-chart="${name}"]`);
    if (!t) return;
    t.querySelectorAll('button').forEach(b => b.addEventListener('click', () => {
      t.querySelectorAll('button').forEach(x => x.classList.remove('on'));
      b.classList.add('on'); fn(b.dataset.m);
    }));
  }

  // ---------------------------------------------------------------- H1
  function h1(d) {
    const t = d.h1.test;
    $('#v-h1').outerHTML = V.no;
    $('#o-h1').innerHTML = `<b>Formal outcome (as planned):</b> not supported. nRF: median difference ${fmt(t.nRF.median)}, Holm-adjusted p = ${fmt(t.nRF.p_holm)} (needed < 0.05). nQD: median difference +${fmt(t.nQD.median)}.`;
    const draw = m => {
      const rows = d.h1.pairs.filter(p => p.metric === m);
      bar('c-h1', rows.map(r => r.name), [
        { label: 'Q-MAWS (mean of 5 seeds)', data: rows.map(r => r.qmaws), backgroundColor: C.navy },
        { label: 'ML-MAWS (reported)', data: rows.map(r => r.mlmaws), backgroundColor: C.orange },
      ], { y: m + ' (lower is better)', max: 1 });
    };
    draw('nRF'); toggle('h1', draw);
  }

  // ---------------------------------------------------------------- H2
  function h2(d) {
    const v = d.h2.values;
    $('#v-h2').outerHTML = V.no;
    const by = m => v.filter(r => r.taxa === m);
    const mean = (a, k) => a.reduce((s, r) => s + r[k], 0) / a.length;
    $('#kv-h2').innerHTML = [25, 50, 100].map(m => {
      const a = by(m), q = mean(a, 'qmaws_seconds'), l = mean(a, 'ml_maws_seconds');
      const ratio = q < l ? `Q-MAWS ${Math.round(l / q)}× faster` : `ML-MAWS ${Math.round(q / l)}× faster`;
      return `<div><b>${m} species</b><span>${ratio}</span></div>`;
    }).join('');
    $('#o-h2').innerHTML = `<b>Formal outcome:</b> not supported. Q-MAWS was faster in ${v.filter(r => r.taxa >= 100 && r.qmaws_faster === 'yes').length} of ${v.filter(r => r.taxa >= 100).length} runs with 100 species.`;
    whenShown('p-h2', () => { charts['c-h2'] = new Chart($('#c-h2'), {
      type: 'scatter',
      data: { datasets: [
        { label: 'Q-MAWS', data: v.map(r => ({ x: r.taxa, y: r.qmaws_seconds })), backgroundColor: C.navy, pointRadius: 7 },
        { label: 'ML-MAWS (IQ-TREE, 1,000 UFBoot)', data: v.map(r => ({ x: r.taxa, y: r.ml_maws_seconds })), backgroundColor: C.orange, pointRadius: 7, pointStyle: 'rectRot' },
      ] },
      options: {
        responsive: true, maintainAspectRatio: false,
        scales: {
          x: { type: 'logarithmic', min: 20, max: 125, title: { display: true, text: 'number of species' }, ticks: { callback: v => [25, 50, 100].includes(v) ? v : '' } },
          y: { type: 'logarithmic', title: { display: true, text: 'wall-clock time (seconds)' } },
        },
        plugins: { tooltip: { callbacks: { label: c => `${c.dataset.label}: ${Math.round(c.raw.y).toLocaleString()} s (${c.raw.x} species)` } } },
      },
    }); });
  }

  // ---------------------------------------------------------------- H3
  function h3(d) {
    $('#v-h3').outerHTML = V.yes;
    const big = d.h3.filter(r => r.n === 100000);
    $('#o-h3').innerHTML = `<b>Formal outcome:</b> supported. At 100,000 characters the likelihood recovered the true tree in ${big.map(r => Math.round(r.w2 * 1000) / 10 + ' %').join(', ')} of replicates (criterion ≥ 95 %); voting in ${big.map(r => Math.round(r.w1 * 100) + ' %').join(', ')}.`;
    const lens = [...new Set(d.h3.map(r => r.t_long))];
    const Ns = [...new Set(d.h3.map(r => r.n))];
    const cols = [C.navy, C.orange, C.teal];
    const sets = [];
    lens.forEach((t, i) => {
      sets.push({ label: `W2, long branch ${t}`, data: Ns.map(n => d.h3.find(r => r.t_long === t && r.n === n).w2), borderColor: cols[i], backgroundColor: cols[i], tension: .25 });
      sets.push({ label: `W1, long branch ${t}`, data: Ns.map(n => d.h3.find(r => r.t_long === t && r.n === n).w1), borderColor: cols[i], backgroundColor: cols[i], borderDash: [6, 4], tension: .25 });
    });
    whenShown('p-h3', () => { charts['c-h3'] = new Chart($('#c-h3'), {
      type: 'line', data: { labels: Ns.map(n => n.toLocaleString()), datasets: sets },
      options: { responsive: true, maintainAspectRatio: false,
        scales: { y: { min: 0, max: 1, title: { display: true, text: 'fraction of true trees recovered' } }, x: { title: { display: true, text: 'number of characters' } } },
        plugins: { legend: { labels: { boxWidth: 10, font: { size: 11 } } }, tooltip: { callbacks: { label: c => `${c.dataset.label}: ${fmt(c.raw)}` } } } },
    }); });
  }

  // ---------------------------------------------------------------- H4
  function h4(d) {
    const e = d.h4;
    $('#v-h4').outerHTML = V.near;
    $('#kv-h4').innerHTML = `<div><b>${fmt(e.s1.ece, 4)}</b><span>Q-MAWS S1 (tested)</span></div><div><b>${fmt(e.ufboot.ece, 4)}</b><span>ML-MAWS UFBoot</span></div><div><b>${fmt(e.s2.ece, 4)}</b><span>Q-MAWS S2 (bootstrap)</span></div>`;
    $('#o-h4').innerHTML = `<b>Formal outcome:</b> not supported, by ${fmt(e.s1.ece - e.ufboot.ece, 5)} (criterion ECE(S1) ≤ ECE(UFBoot)). In practice the two are equal.`;
    const pts = (k, col, label, style) => ({ label, data: e[k].bins.filter(b => b.support !== null).map(b => ({ x: b.support, y: b.true, n: b.n })), borderColor: col, backgroundColor: col + 'aa', showLine: true, pointStyle: style, pointRadius: c => 4 + Math.sqrt(c.raw ? c.raw.n : 1) * 0.9 });
    whenShown('p-h4', () => { charts['c-h4'] = new Chart($('#c-h4'), {
      type: 'scatter',
      data: { datasets: [
        pts('s1', C.navy, `Q-MAWS S1 (ECE ${fmt(e.s1.ece, 3)})`, 'circle'),
        pts('ufboot', C.orange, `ML-MAWS UFBoot (ECE ${fmt(e.ufboot.ece, 3)})`, 'rectRot'),
        pts('s2', C.teal, `Q-MAWS S2 (ECE ${fmt(e.s2.ece, 3)})`, 'triangle'),
        { label: 'perfect calibration', data: [{ x: 0, y: 0 }, { x: 1, y: 1 }], showLine: true, borderColor: '#9aa3b2', borderDash: [5, 5], pointRadius: 0 },
      ] },
      options: { responsive: true, maintainAspectRatio: false,
        scales: { x: { min: 0, max: 1, title: { display: true, text: 'mean support in the group' } }, y: { min: 0, max: 1, title: { display: true, text: 'fraction of branches in the true tree' } } },
        plugins: { tooltip: { callbacks: { label: c => c.raw.n ? `${c.dataset.label.split(' (')[0]}: support ${fmt(c.raw.x, 2)}, true ${fmt(c.raw.y, 2)} (${c.raw.n} branches)` : '' } } } },
    }); });
  }

  // ---------------------------------------------------------------- H5
  function h5(d) {
    const t = d.h5.test;
    $('#v-h5').outerHTML = V.no;
    $('#kv-h5').innerHTML = `<div><b>p = ${fmt(t.v2.nRF.p_holm)}</b><span>v2 vs ML-MAWS, nRF (Holm)</span></div><div><b>${fmt(t.primary.nRF.median)}</b><span>original vs ML-MAWS, median nRF difference</span></div><div><b>p = ${fmt(t.primary.nRF.p_holm)}</b><span>original vs ML-MAWS (Holm, descriptive)</span></div>`;
    $('#o-h5').innerHTML = `<b>Formal outcome:</b> H5 not supported (v2: Holm p = ${fmt(t.v2.nRF.p_holm)} for nRF, ${fmt(t.v2.nQD.p_holm)} for nQD). The original configuration, compared in the same way as a pre-registered secondary result, had lower error than ML-MAWS (Holm p = ${fmt(t.primary.nRF.p_holm)} for nRF).`;
    const levels = [['h000', '0 %'], ['h010', '10 %'], ['h020', '20 %'], ['h040', '40 %']];
    const med = a => { const s = a.slice().sort((x, y) => x - y); const n = s.length; return n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2; };
    const draw = m => {
      const get = (lv, key) => d.h5.values.filter(r => r.id.includes(lv)).map(r => r[key]);
      const tool = (lv, tl) => d.h5.values.filter(r => r.id.includes(lv)).map(r => (d.tools.h5[r.id + '|' + tl] || {})[m]);
      bar('c-h5', levels.map(l => 'transfer ' + l[1]), [
        { label: 'Q-MAWS (original)', data: levels.map(l => med(get(l[0], 'primary_' + m))), backgroundColor: C.navy },
        { label: 'Q-MAWS v2', data: levels.map(l => med(get(l[0], 'v2_' + m))), backgroundColor: '#7fa3c7' },
        { label: 'ML-MAWS', data: levels.map(l => med(get(l[0], 'ml_maws_' + m))), backgroundColor: C.orange },
        { label: 'Mash', data: levels.map(l => med(tool(l[0], 'mash'))), backgroundColor: C.teal },
        { label: 'CD-MAWS', data: levels.map(l => med(tool(l[0], 'cdmaws'))), backgroundColor: C.purple },
        { label: 'Peafowl', data: levels.map(l => med(tool(l[0], 'peafowl'))), backgroundColor: C.gold },
      ], { y: 'median ' + m.toUpperCase().replace('NRF', 'nRF').replace('NQD', 'nQD') + ' (lower is better)' });
    };
    whenShown('p-h5', () => draw('nrf')); toggle('h5', draw);
  }

  // ---------------------------------------------------------------- tools
  function tools(d) {
    const rows = d.tools.afproject;
    const draw = m => bar('c-tools', rows.map(r => r.name), [
      { label: 'Q-MAWS (mean of 5 seeds)', data: rows.map(r => r['qmaws_' + m]), backgroundColor: C.navy },
      { label: 'Mash', data: rows.map(r => r['mash_' + m]), backgroundColor: C.teal },
      { label: 'CD-MAWS', data: rows.map(r => r['cdmaws_' + m]), backgroundColor: C.purple },
      { label: 'Peafowl', data: rows.map(r => r['peafowl_' + m]), backgroundColor: C.gold },
    ], { y: (m === 'nrf' ? 'nRF' : 'nQD') + ' (lower is better)', max: 1 });
    draw('nrf'); toggle('tools', draw);
  }
  function paired(d) {
    $('#paired-table tbody').innerHTML = d.tools.paired.map(r => {
      const med = parseFloat(r.median_difference);
      return `<tr><td>${esc(r.group)}</td><td>${esc(r.qmaws)}</td><td><b>${esc(r.tool === 'mash' ? 'Mash' : r.tool === 'cdmaws' ? 'CD-MAWS' : r.tool === 'peafowl' ? 'Peafowl' : r.tool)}</b></td><td>${esc(r.metric)}</td>` +
        `<td class="num ${+r.qmaws_lower > +r.qmaws_higher ? 'best' : ''}">${r.qmaws_lower}</td><td class="num">${r.equal}</td><td class="num">${r.qmaws_higher}</td><td class="num">${(med > 0 ? '+' : '') + fmt(med)}</td></tr>`;
    }).join('');
  }

  // ---------------------------------------------------------------- sensitivity and seeds
  function sensitivity(d) {
    const order = [['primary', 'Primary (W2c)'], ['weights_w1', 'W1 votes'], ['weights_w2a', 'W2a'], ['weights_w2b', 'W2b'], ['lengths_k1', 'K = 1'], ['lengths_k2', 'K = 2'], ['lengths_k4', 'K = 4'], ['strand_off', 'Strand off'], ['matrix_ml', 'M_ml'], ['model_w2emp', 'W2-emp']];
    const ds = [['fish_mito', 'Fish mtDNA', C.navy], ['ecoli', 'E. coli/Shigella', C.orange], ['sim_hgt_0', 'Sim. HGT 0', C.teal], ['sim_hgt_500', 'Sim. HGT 500', C.purple]];
    bar('c-sens', order.map(o => o[1]), ds.map(x => ({ label: x[1], data: order.map(o => (d.sensitivity[o[0]] || {})[x[0]]), backgroundColor: x[2], maxBarThickness: 12 })), { y: 'nRF', max: 1 });
  }
  function seeds(d) {
    const rows = d.seeds;
    const errorBars = {
      id: 'errorBars',
      afterDatasetsDraw(chart) {
        const ctx = chart.ctx;
        chart.data.datasets.forEach((ds, i) => {
          const meta = chart.getDatasetMeta(i);
          meta.data.forEach((b, j) => {
            const sd = ds.sd[j]; if (!sd) return;
            const y = chart.scales.y, top = y.getPixelForValue(ds.data[j] + sd), bot = y.getPixelForValue(ds.data[j] - sd);
            ctx.save(); ctx.strokeStyle = '#15233b'; ctx.lineWidth = 1.2;
            ctx.beginPath(); ctx.moveTo(b.x, top); ctx.lineTo(b.x, bot); ctx.moveTo(b.x - 4, top); ctx.lineTo(b.x + 4, top); ctx.moveTo(b.x - 4, bot); ctx.lineTo(b.x + 4, bot); ctx.stroke(); ctx.restore();
          });
        });
      },
    };
    charts['c-seeds'] = new Chart($('#c-seeds'), {
      type: 'bar',
      data: { labels: rows.map(r => r.name), datasets: [
        { label: 'nRF', data: rows.map(r => r.nrf), sd: rows.map(r => r.nrf_sd), backgroundColor: C.navy, borderRadius: 4, maxBarThickness: 18 },
        { label: 'nQD', data: rows.map(r => r.nqd), sd: rows.map(r => r.nqd_sd), backgroundColor: C.orange, borderRadius: 4, maxBarThickness: 18 },
      ] },
      options: { responsive: true, maintainAspectRatio: false, scales: { y: { beginAtZero: true, max: 1 }, x: { ticks: { autoSkip: false, maxRotation: 40 } } },
        plugins: { tooltip: { callbacks: { label: c => `${c.dataset.label}: ${fmt(c.raw)} ± ${fmt(c.dataset.sd[c.dataIndex])}` } } } },
      plugins: [errorBars],
    });
  }

  // ---------------------------------------------------------------- gallery
  function gallery(d) {
    const groups = [['all', 'All'], ['halo', 'Trees and Halo Trees'], ['accuracy', 'Accuracy'], ['h1', 'H1'], ['h2', 'H2'], ['h3', 'H3'], ['h4', 'H4'], ['h5', 'H5'], ['tools', 'Tools'], ['sensitivity', 'Sensitivity'], ['seeds', 'Seeds']];
    const present = new Set(d.figures.map(f => f.group));
    $('#fig-chips').innerHTML = groups.filter(g => g[0] === 'all' || present.has(g[0])).map((g, i) => `<button class="chip${i ? '' : ' on'}" data-g="${g[0]}">${g[1]}</button>`).join('');
    const draw = g => {
      $('#gallery').innerHTML = d.figures.filter(f => g === 'all' || f.group === g).map(f =>
        `<figure class="fig reveal-now"><div class="img"><img src="${f.file}" alt="${esc(f.caption)}" loading="lazy" data-zoomable></div><figcaption class="cap">${esc(f.caption)}<div class="src"><code>${esc(f.source)}</code></div></figcaption></figure>`).join('');
      if (window.mediumZoom) mediumZoom('[data-zoomable]', { margin: 24, background: 'rgba(246,247,251,.96)' });
    };
    $$('#fig-chips .chip').forEach(c => c.addEventListener('click', () => {
      $$('#fig-chips .chip').forEach(x => x.classList.remove('on')); c.classList.add('on'); draw(c.dataset.g);
    }));
    draw('all');
  }

  // ---------------------------------------------------------------- proof
  function proof(d) {
    const p = d.proof;
    $('#proof-run').innerHTML = `<code>${esc(p.run)}</code>`;
    const tint = l => esc(l).replace(/(\[PASS\])/g, '<span class="ok">$1</span>').replace(/(Verdict: PASS[^<]*)/, '<span class="ok">$1</span>').replace(/(Root fingerprint: [0-9a-f]+)/, '<span class="hl">$1</span>');
    $('#term-log').innerHTML = p.log_head.map(tint).join('\n') + '\n<span class="dim">…</span>\n' + p.log_tail.map(tint).join('\n');
    $('#term-verify').innerHTML = p.verify_head.map(tint).join('\n') + '\n<span class="dim">…</span>\n' + p.verify_tail.map(tint).join('\n');
    $('#proof-root').textContent = p.root;
    $('#proof-hashes').innerHTML = p.hashes.map(h => `<div class="small" style="margin-bottom:.45rem"><code>${esc(h.file)}</code><div class="hash">${esc(h.sha256)}</div></div>`).join('');
    $('#proof-expect').textContent = `${d.meta.hashed_files} files checked, ${d.meta.hashed_files} match, 0 differ or are missing`;
    $('#term-worksheet').textContent = p.worksheet.join('\n');
    $$('.copy').forEach(b => b.addEventListener('click', () => {
      navigator.clipboard && navigator.clipboard.writeText(b.dataset.copy);
      b.textContent = 'copied'; setTimeout(() => (b.textContent = 'copy'), 1200);
    }));
  }

  // ---------------------------------------------------------------- animation
  function animate() {
    const counters = $$('[data-count]');
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    counters.forEach(el => {
      const target = +el.dataset.count;
      if (reduce || !window.gsap) { el.textContent = target.toLocaleString(); return; }
      const o = { v: 0 };
      gsap.to(o, { v: target, duration: 1.6, ease: 'power2.out', delay: .3, onUpdate: () => (el.textContent = Math.round(o.v).toLocaleString()) });
    });
    if (window.gsap && !reduce) {
      gsap.from('.hero h1, .hero .lead, .hero .badges, .hero .btns', { y: 24, opacity: 0, duration: .9, stagger: .12, ease: 'power3.out' });
      gsap.from('.stat', { y: 16, opacity: 0, duration: .7, stagger: .08, delay: .5 });
    }
    const items = $$('.reveal');
    if (reduce || !('IntersectionObserver' in window)) { items.forEach(e => e.classList.add('in')); return; }
    const io = new IntersectionObserver(entries => entries.forEach(en => {
      if (en.isIntersecting) { en.target.classList.add('in'); io.unobserve(en.target); }
    }), { rootMargin: '0px 0px -8% 0px' });
    items.forEach(e => io.observe(e));
  }
})();
