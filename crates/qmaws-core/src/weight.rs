//! Quartet weighting: informative-pattern votes (W1) and conditioned quartet
//! maximum likelihood (W2) with the weights W2a, W2b and W2c.
//!
//! Topologies of a quartet with members (a, b, c, d) in index order:
//! T1 = ab|cd (index 0), T2 = ac|bd (index 1), T3 = ad|bc (index 2).
//! Pattern indices are those of `quartet`: 8·x_a + 4·x_b + 2·x_c + x_d.
//!
//! Model: a two-state Markov chain with stationary frequencies (π0, π1) and
//! P_ij(t) = π_j + (δ_ij − π_j)·exp(−βt), β = 1 ÷ (2·π0·π1), so that t is the
//! expected number of changes per character. W2-sym is π0 = π1 = 0.5.
//!
//! Branch lengths of a fit are stored by quartet position: `lengths[p]` is
//! the leaf branch of member p (0 = a, …, 3 = d) and `lengths[4]` is the
//! internal branch, whatever the topology.
//!
//! Optimiser (see `docs/DESIGN.md`): cyclic coordinate ascent. With the
//! other branches fixed, every pattern probability is affine in
//! s = exp(−βt) of the free branch, because the branch's transition matrix is
//! Π + s·(I − Π). The one-dimensional problem in s therefore has exact first
//! and second derivatives and is solved by a safeguarded Newton search for a
//! stationary point in the direction of ascent. A step is kept only if the
//! likelihood does not decrease. The bounds [0.000001, 10] on t are bounds
//! on s, and s is a monotone function of ln t, so the search is a search over
//! log branch lengths.
//!
//! Determinism: every transcendental function comes from `libm`; no
//! `mul_add`; all sums run in a fixed order.

// Index loops mirror the formulas over patterns, positions and branches.
#![allow(clippy::needless_range_loop)]

use crate::quartet::PatternCounts;
use sha2::{Digest, Sha256};

/// Lower bound of a branch length.
pub const MIN_LENGTH: f64 = 0.000_001;

/// Upper bound of a branch length.
pub const MAX_LENGTH: f64 = 10.0;

/// Number of multinomial resamples of W2c.
pub const REPLICATES: u32 = 100;

/// Two log-likelihoods closer than this are a tie (W2a and W2c).
pub const TIE_TOLERANCE: f64 = 1e-8;

/// Smallest pattern probability used inside a logarithm.
const MIN_PROBABILITY: f64 = 1e-300;

/// Maximum number of Newton steps and coordinate sweeps of one fit.
const MAX_ITERATIONS: u32 = 2_000;

/// Convergence: the estimated remaining gain in log-likelihood is at most
/// `TOLERANCE_ABS + TOLERANCE_REL·|ℓ|`.
const TOLERANCE_ABS: f64 = 1e-9;
const TOLERANCE_REL: f64 = 1e-14;

/// Leaf positions of each topology: (first pair, second pair).
pub const PAIRS: [[usize; 4]; 3] = [[0, 1, 2, 3], [0, 2, 1, 3], [0, 3, 1, 2]];

/// Topology names in quartet positions.
pub const TOPOLOGY_NAMES: [&str; 3] = ["ab|cd", "ac|bd", "ad|bc"];

/// Split patterns that support each topology (W1).
pub const SUPPORTING: [[usize; 2]; 3] = [[0b1100, 0b0011], [0b1010, 0b0101], [0b1001, 0b0110]];

/// The two-state substitution model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    /// Stationary frequencies (π0, π1).
    pub pi: [f64; 2],
    /// Rate factor β = 1 ÷ (2·π0·π1).
    pub beta: f64,
}

impl Model {
    /// W2-sym: π0 = π1 = 0.5.
    pub fn symmetric() -> Self {
        Self::with_frequency_of_one(0.5)
    }

    /// W2-emp and others: π1 given, π0 = 1 − π1. `pi1` must be in (0, 1).
    pub fn with_frequency_of_one(pi1: f64) -> Self {
        assert!(pi1 > 0.0 && pi1 < 1.0, "frequency of 1 must be in (0, 1)");
        let pi = [1.0 - pi1, pi1];
        Self {
            pi,
            beta: 1.0 / (2.0 * pi[0] * pi[1]),
        }
    }

    /// Transition matrix of a branch of length t.
    pub fn transition(&self, t: f64) -> [[f64; 2]; 2] {
        // 1 − s without cancellation for short branches.
        let one_minus_s = -libm::expm1(-self.beta * t);
        let change_to = |j: usize| self.pi[j] * one_minus_s;
        [
            [1.0 - change_to(1), change_to(1)],
            [change_to(0), 1.0 - change_to(0)],
        ]
    }

    /// Π: every row is (π0, π1).
    fn stationary(&self) -> [[f64; 2]; 2] {
        [self.pi, self.pi]
    }

    /// I − Π.
    fn identity_minus_stationary(&self) -> [[f64; 2]; 2] {
        [
            [1.0 - self.pi[0], -self.pi[1]],
            [-self.pi[0], 1.0 - self.pi[1]],
        ]
    }

    /// s = exp(−βt) of a branch length t.
    fn s_of(&self, t: f64) -> f64 {
        libm::exp(-self.beta * t)
    }

    /// Branch length t of s = exp(−βt).
    fn t_of(&self, s: f64) -> f64 {
        -libm::log(s) / self.beta
    }
}

/// Which patterns the likelihood conditions away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conditioning {
    /// Exclude 0000 (Q-MAWS: a MAW column needs at least one taxon with 1).
    NotAllZero,
    /// Exclude 0000 and 1111. Only for the cross-check with IQ-TREE's +ASC.
    NotConstant,
}

impl Conditioning {
    fn excludes(self, x: usize) -> bool {
        match self {
            Self::NotAllZero => x == 0,
            Self::NotConstant => x == 0 || x == 15,
        }
    }
}

type Mat = [[f64; 2]; 2];

/// Bit of position p (0 = a) in pattern x.
#[inline]
fn bit(x: usize, p: usize) -> usize {
    (x >> (3 - p)) & 1
}

/// Pattern probabilities for topology `topology` with branch matrices `m`
/// (indexed like branch lengths).
fn probabilities_of(pi: [f64; 2], topology: usize, m: &[Mat; 5]) -> [f64; 16] {
    let [i, j, k, l] = PAIRS[topology];
    let e = &m[4];
    // left[u][xi][xj] and right[v][xk][xl].
    let mut left = [[[0.0; 2]; 2]; 2];
    let mut right = [[[0.0; 2]; 2]; 2];
    for u in 0..2 {
        for a in 0..2 {
            for b in 0..2 {
                left[u][a][b] = m[i][u][a] * m[j][u][b];
                right[u][a][b] = m[k][u][a] * m[l][u][b];
            }
        }
    }
    // g[u][xk][xl] = Σ_v e[u][v]·right[v][xk][xl].
    let mut g = [[[0.0; 2]; 2]; 2];
    for u in 0..2 {
        for c in 0..2 {
            for d in 0..2 {
                g[u][c][d] = e[u][0] * right[0][c][d] + e[u][1] * right[1][c][d];
            }
        }
    }
    let mut out = [0.0; 16];
    for (x, o) in out.iter_mut().enumerate() {
        let (xi, xj, xk, xl) = (bit(x, i), bit(x, j), bit(x, k), bit(x, l));
        *o = pi[0] * left[0][xi][xj] * g[0][xk][xl] + pi[1] * left[1][xi][xj] * g[1][xk][xl];
    }
    out
}

/// The 16 pattern probabilities of a topology with the given branch lengths.
pub fn pattern_probabilities(model: &Model, topology: usize, lengths: &[f64; 5]) -> [f64; 16] {
    let m: [Mat; 5] = std::array::from_fn(|b| model.transition(lengths[b]));
    probabilities_of(model.pi, topology, &m)
}

/// Number of counted (not excluded) columns.
pub fn conditioned_total(counts: &PatternCounts, cond: Conditioning) -> u64 {
    (0..16)
        .filter(|&x| !cond.excludes(x))
        .map(|x| counts[x])
        .sum()
}

/// Conditioned log-likelihood Σ c(x)·[ln P(x) − ln P(counted)] over the
/// counted patterns, where P(counted) is the sum of their probabilities.
pub fn log_likelihood(
    model: &Model,
    cond: Conditioning,
    topology: usize,
    lengths: &[f64; 5],
    counts: &PatternCounts,
) -> f64 {
    let p = pattern_probabilities(model, topology, lengths);
    conditioned(&p, cond, counts)
}

fn conditioned(p: &[f64; 16], cond: Conditioning, counts: &PatternCounts) -> f64 {
    let mut kept = 0.0;
    let mut total = 0u64;
    let mut sum = 0.0;
    for x in 0..16 {
        if cond.excludes(x) {
            continue;
        }
        kept += p[x];
        if counts[x] > 0 {
            sum += counts[x] as f64 * libm::log(p[x].max(MIN_PROBABILITY));
            total += counts[x];
        }
    }
    sum - total as f64 * libm::log(kept.max(MIN_PROBABILITY))
}

/// Result of fitting one topology.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// Maximised conditioned log-likelihood.
    pub log_likelihood: f64,
    /// Fitted branch lengths (leaf of a, b, c, d; internal).
    pub lengths: [f64; 5],
    /// Which start gave this fit (0: all 0.05; 1: all 0.3; 2: from mismatches;
    /// 3: a given start).
    pub start: u8,
    /// Newton steps and coordinate sweeps until convergence.
    pub iterations: u32,
}

/// Branch-length starts for a topology: all 0.05, all 0.3, and one derived
/// from pairwise mismatch proportions of the counted columns.
pub fn starts(
    model: &Model,
    cond: Conditioning,
    topology: usize,
    counts: &PatternCounts,
) -> [[f64; 5]; 3] {
    [
        [0.05; 5],
        [0.3; 5],
        mismatch_start(model, cond, topology, counts),
    ]
}

/// Start from distances: each pair's mismatch proportion p over the counted
/// columns is corrected by d = −ln(1 − p ÷ (2·π0·π1)) ÷ β, and branch lengths
/// follow from the four distances across and two within the pairs.
fn mismatch_start(
    model: &Model,
    cond: Conditioning,
    topology: usize,
    counts: &PatternCounts,
) -> [f64; 5] {
    let total = conditioned_total(counts, cond).max(1) as f64;
    let h = 2.0 * model.pi[0] * model.pi[1];
    let mut d = [[0.0; 4]; 4];
    for p in 0..4 {
        for q in p + 1..4 {
            let differ: u64 = (0..16)
                .filter(|&x| !cond.excludes(x) && bit(x, p) != bit(x, q))
                .map(|x| counts[x])
                .sum();
            let ratio = (differ as f64 / total / h).min(1.0 - 1e-9);
            let dist = -libm::log1p(-ratio) / model.beta;
            d[p][q] = dist;
            d[q][p] = dist;
        }
    }
    let [i, j, k, l] = PAIRS[topology];
    let mut t = [0.0; 5];
    t[i] = (2.0 * d[i][j] + d[i][k] + d[i][l] - d[j][k] - d[j][l]) / 4.0;
    t[j] = (2.0 * d[i][j] + d[j][k] + d[j][l] - d[i][k] - d[i][l]) / 4.0;
    t[k] = (2.0 * d[k][l] + d[i][k] + d[j][k] - d[i][l] - d[j][l]) / 4.0;
    t[l] = (2.0 * d[k][l] + d[i][l] + d[j][l] - d[i][k] - d[j][k]) / 4.0;
    t[4] = (d[i][k] + d[i][l] + d[j][k] + d[j][l]) / 4.0 - (d[i][j] + d[k][l]) / 2.0;
    t.map(|v| v.clamp(MIN_LENGTH, MAX_LENGTH))
}

/// Fits one topology from the three standard starts and keeps the best (the
/// earliest start on equal likelihoods). `None` if no column is counted.
pub fn fit(
    model: &Model,
    cond: Conditioning,
    topology: usize,
    counts: &PatternCounts,
) -> Option<Fit> {
    if conditioned_total(counts, cond) == 0 {
        return None;
    }
    let mut best: Option<Fit> = None;
    for (n, start) in starts(model, cond, topology, counts).iter().enumerate() {
        let mut f = fit_from(model, cond, topology, counts, start);
        f.start = n as u8;
        if best.is_none_or(|b| f.log_likelihood > b.log_likelihood) {
            best = Some(f);
        }
    }
    best
}

/// Branch lengths of a fit in progress, their matrices and log-likelihood.
#[derive(Clone, Copy)]
struct State {
    lengths: [f64; 5],
    mats: [Mat; 5],
    value: f64,
}

/// Fits one topology from one start: projected Newton steps on the exact
/// gradient and Hessian in s = exp(−βt), with coordinate sweeps where a
/// Newton step cannot be taken.
pub fn fit_from(
    model: &Model,
    cond: Conditioning,
    topology: usize,
    counts: &PatternCounts,
    start: &[f64; 5],
) -> Fit {
    let ctx = Context {
        model,
        cond,
        topology,
        counts,
        lo: model.s_of(MAX_LENGTH),
        hi: model.s_of(MIN_LENGTH),
    };
    let lengths = start.map(|t| t.clamp(MIN_LENGTH, MAX_LENGTH));
    let mut st = ctx.state(lengths);
    let mut iterations = 0;
    let mut previous_gain = 0.0;
    while iterations < MAX_ITERATIONS {
        iterations += 1;
        let tol = TOLERANCE_ABS + TOLERANCE_REL * st.value.abs();
        match ctx.newton(&mut st, tol) {
            Step::Converged => break,
            Step::Moved => {
                previous_gain = 0.0;
                continue;
            }
            Step::Failed => {}
        }
        // Coordinate sweep. It converges linearly: with gains Δ_k and ratio
        // r = Δ_k ÷ Δ_(k−1), the remaining gain is about Δ_k·r ÷ (1 − r).
        let gain = ctx.sweep(&mut st);
        let ratio = if previous_gain > 0.0 {
            gain / previous_gain
        } else {
            0.0
        };
        let remaining = if ratio < 1.0 {
            gain * ratio / (1.0 - ratio)
        } else {
            f64::INFINITY
        };
        if gain <= 0.0 || (gain <= tol && remaining <= tol) {
            break;
        }
        previous_gain = gain;
    }
    Fit {
        log_likelihood: st.value,
        lengths: st.lengths,
        start: 3,
        iterations,
    }
}

enum Step {
    /// The predicted gain of a Newton step is below the tolerance.
    Converged,
    /// A Newton step increased the likelihood.
    Moved,
    /// The Hessian is not negative definite or no step length helped.
    Failed,
}

struct Context<'a> {
    model: &'a Model,
    cond: Conditioning,
    topology: usize,
    counts: &'a PatternCounts,
    /// s of the longest and of the shortest branch.
    lo: f64,
    hi: f64,
}

impl Context<'_> {
    /// Branch length of s, exactly at a bound when s is at its end of the
    /// range (so that the bound is recognised as active).
    fn length_of(&self, s: f64) -> f64 {
        if s >= self.hi {
            MIN_LENGTH
        } else if s <= self.lo {
            MAX_LENGTH
        } else {
            self.model.t_of(s).clamp(MIN_LENGTH, MAX_LENGTH)
        }
    }

    fn probabilities(&self, m: &[Mat; 5]) -> [f64; 16] {
        probabilities_of(self.model.pi, self.topology, m)
    }

    fn state(&self, lengths: [f64; 5]) -> State {
        let mats = std::array::from_fn(|b| self.model.transition(lengths[b]));
        let value = conditioned(&self.probabilities(&mats), self.cond, self.counts);
        State {
            lengths,
            mats,
            value,
        }
    }

    /// One pass of exact one-dimensional maximisation over each branch;
    /// returns the gain.
    fn sweep(&self, st: &mut State) -> f64 {
        let before = st.value;
        for b in 0..5 {
            let mut m = st.mats;
            m[b] = self.model.stationary();
            let a = self.probabilities(&m);
            m[b] = self.model.identity_minus_stationary();
            let slope = self.probabilities(&m);
            let line = Line::new(&a, &slope, self.cond, self.counts);
            let s0 = self.model.s_of(st.lengths[b]);
            let s = line.maximise_from(s0, self.lo, self.hi);
            if s == s0 {
                continue;
            }
            let mut lengths = st.lengths;
            lengths[b] = self.length_of(s);
            let trial = self.state(lengths);
            if trial.value >= st.value {
                *st = trial;
            }
        }
        st.value - before
    }

    /// Gradient and Hessian of the log-likelihood in (s_1, …, s_5). Each
    /// pattern probability is multilinear in the s values: its derivative
    /// in s_k replaces branch k's matrix by I − Π, and so on.
    fn derivatives(&self, st: &State) -> ([f64; 5], [[f64; 5]; 5]) {
        let j_mat = self.model.identity_minus_stationary();
        let p = self.probabilities(&st.mats);
        let mut p1 = [[0.0; 16]; 5];
        for (k, out) in p1.iter_mut().enumerate() {
            let mut m = st.mats;
            m[k] = j_mat;
            *out = self.probabilities(&m);
        }
        let mut p2 = [[[0.0; 16]; 5]; 5];
        for j in 0..5 {
            for k in j + 1..5 {
                let mut m = st.mats;
                m[j] = j_mat;
                m[k] = j_mat;
                p2[j][k] = self.probabilities(&m);
            }
        }
        let allowed = |x: usize| !self.cond.excludes(x);
        let sum = |v: &[f64; 16]| (0..16).filter(|&x| allowed(x)).map(|x| v[x]).sum::<f64>();
        let d = sum(&p).max(MIN_PROBABILITY);
        let d1: [f64; 5] = std::array::from_fn(|k| sum(&p1[k]));
        let n = conditioned_total(self.counts, self.cond) as f64;
        let mut g = [0.0; 5];
        let mut h = [[0.0; 5]; 5];
        for x in 0..16 {
            if !allowed(x) || self.counts[x] == 0 {
                continue;
            }
            let c = self.counts[x] as f64;
            let px = p[x].max(MIN_PROBABILITY);
            for j in 0..5 {
                let rj = p1[j][x] / px;
                g[j] += c * rj;
                h[j][j] -= c * rj * rj;
                for k in j + 1..5 {
                    h[j][k] += c * (p2[j][k][x] / px - rj * p1[k][x] / px);
                }
            }
        }
        for j in 0..5 {
            let rj = d1[j] / d;
            g[j] -= n * rj;
            h[j][j] += n * rj * rj;
            for k in j + 1..5 {
                h[j][k] -= n * (sum(&p2[j][k]) / d - rj * d1[k] / d);
                h[k][j] = h[j][k];
            }
        }
        (g, h)
    }

    /// One projected Newton step with backtracking. Branches at a bound
    /// whose gradient points out of the box stay fixed.
    fn newton(&self, st: &mut State, tol: f64) -> Step {
        let (g, h) = self.derivatives(st);
        let s: [f64; 5] = std::array::from_fn(|k| self.model.s_of(st.lengths[k]));
        let free: Vec<usize> = (0..5)
            .filter(|&k| {
                let at_short = st.lengths[k] <= MIN_LENGTH && g[k] >= 0.0;
                let at_long = st.lengths[k] >= MAX_LENGTH && g[k] <= 0.0;
                !(at_short || at_long)
            })
            .collect();
        if free.is_empty() {
            return Step::Converged;
        }
        // Solve (−H_ff + λ·W)·d = g_f by Cholesky factorisation, where W is
        // the diagonal of |H|. λ = 0 is Newton's method; larger λ
        // (Levenberg–Marquardt damping) is used where −H is not positive
        // definite or the Newton step does not help.
        let f = free.len();
        let gf: Vec<f64> = free.iter().map(|&i| g[i]).collect();
        let largest = free.iter().map(|&i| h[i][i].abs()).fold(0.0, f64::max);
        let weight: Vec<f64> = free
            .iter()
            .map(|&i| h[i][i].abs().max(1e-12 * largest).max(f64::MIN_POSITIVE))
            .collect();
        let mut lambda = 0.0;
        while lambda <= 1e8 {
            let mut a = [[0.0; 5]; 5];
            for (r, &i) in free.iter().enumerate() {
                for (c, &j) in free.iter().enumerate() {
                    a[r][c] = -h[i][j];
                }
                a[r][r] += lambda * weight[r];
            }
            if let Some(dir) = cholesky_solve(&mut a, f, &gf) {
                if lambda == 0.0 {
                    let predicted: f64 = 0.5 * (0..f).map(|r| gf[r] * dir[r]).sum::<f64>();
                    if predicted <= tol {
                        return Step::Converged;
                    }
                }
                let mut alpha = 1.0;
                for _ in 0..if lambda == 0.0 { 12 } else { 2 } {
                    let mut lengths = st.lengths;
                    for (r, &k) in free.iter().enumerate() {
                        let sk = (s[k] + alpha * dir[r]).clamp(self.lo, self.hi);
                        lengths[k] = self.length_of(sk);
                    }
                    let trial = self.state(lengths);
                    if trial.value > st.value {
                        *st = trial;
                        return Step::Moved;
                    }
                    alpha *= 0.5;
                }
            }
            lambda = if lambda == 0.0 { 1e-6 } else { lambda * 10.0 };
        }
        Step::Failed
    }
}

/// Solves A·x = b for a symmetric positive definite n × n matrix A (the top
/// left of `a`, overwritten by its Cholesky factor). `None` if A is not
/// positive definite.
fn cholesky_solve(a: &mut [[f64; 5]; 5], n: usize, b: &[f64]) -> Option<[f64; 5]> {
    for j in 0..n {
        let mut d = a[j][j];
        for k in 0..j {
            d -= a[j][k] * a[j][k];
        }
        if d.is_nan() || d <= 0.0 {
            return None;
        }
        let d = d.sqrt();
        a[j][j] = d;
        for i in j + 1..n {
            let mut v = a[i][j];
            for k in 0..j {
                v -= a[i][k] * a[j][k];
            }
            a[i][j] = v / d;
        }
    }
    let mut y = [0.0; 5];
    for i in 0..n {
        let mut v = b[i];
        for k in 0..i {
            v -= a[i][k] * y[k];
        }
        y[i] = v / a[i][i];
    }
    let mut x = [0.0; 5];
    for i in (0..n).rev() {
        let mut v = y[i];
        for k in i + 1..n {
            v -= a[k][i] * x[k];
        }
        x[i] = v / a[i][i];
    }
    Some(x)
}

/// The conditioned log-likelihood along one branch, as a function of s:
/// f(s) = Σ c(x)·ln(A_x + B_x·s) − N·ln(A_D + B_D·s).
struct Line {
    a: [f64; 16],
    b: [f64; 16],
    c: [f64; 16],
    a_d: f64,
    b_d: f64,
    n: f64,
}

impl Line {
    fn new(a: &[f64; 16], b: &[f64; 16], cond: Conditioning, counts: &PatternCounts) -> Self {
        let mut line = Line {
            a: [0.0; 16],
            b: [0.0; 16],
            c: [0.0; 16],
            a_d: 0.0,
            b_d: 0.0,
            n: 0.0,
        };
        for x in 0..16 {
            if cond.excludes(x) {
                continue;
            }
            line.a_d += a[x];
            line.b_d += b[x];
            if counts[x] > 0 {
                line.a[x] = a[x];
                line.b[x] = b[x];
                line.c[x] = counts[x] as f64;
                line.n += counts[x] as f64;
            }
        }
        line
    }

    /// First and second derivative of f at s.
    fn derivatives(&self, s: f64) -> (f64, f64) {
        let mut d1 = 0.0;
        let mut d2 = 0.0;
        for x in 0..16 {
            if self.c[x] > 0.0 {
                let p = (self.a[x] + self.b[x] * s).max(MIN_PROBABILITY);
                let r = self.b[x] / p;
                d1 += self.c[x] * r;
                d2 -= self.c[x] * r * r;
            }
        }
        let den = (self.a_d + self.b_d * s).max(MIN_PROBABILITY);
        let r = self.b_d / den;
        d1 -= self.n * r;
        d2 += self.n * r * r;
        (d1, d2)
    }

    /// A local maximum of f on [lo, hi] reached by moving uphill from s0.
    fn maximise_from(&self, s0: f64, lo: f64, hi: f64) -> f64 {
        let s0 = s0.clamp(lo, hi);
        let (g0, _) = self.derivatives(s0);
        let candidate = if g0 > 0.0 {
            if self.derivatives(hi).0 >= 0.0 {
                hi
            } else {
                self.stationary_point(s0, hi)
            }
        } else if g0 < 0.0 {
            if self.derivatives(lo).0 <= 0.0 {
                lo
            } else {
                self.stationary_point(lo, s0)
            }
        } else {
            s0
        };
        // The caller keeps the step only if the exactly evaluated
        // likelihood does not decrease; the affine form here is less precise.
        candidate
    }

    /// Root of f' in (left, right), where f'(left) > 0 > f'(right): Newton's
    /// method, falling back to bisection whenever a step leaves the bracket.
    fn stationary_point(&self, mut left: f64, mut right: f64) -> f64 {
        let mut s = 0.5 * (left + right);
        for _ in 0..200 {
            let (g, h) = self.derivatives(s);
            if g > 0.0 {
                left = s;
            } else if g < 0.0 {
                right = s;
            } else {
                return s;
            }
            let newton = if h < 0.0 { s - g / h } else { f64::NAN };
            let next = if newton > left && newton < right {
                newton
            } else {
                0.5 * (left + right)
            };
            if (next - s).abs() <= 4.0 * f64::EPSILON * s.abs().max(f64::MIN_POSITIVE)
                || right - left <= 4.0 * f64::EPSILON * right.abs()
            {
                return next;
            }
            s = next;
        }
        s
    }
}

/// W1: score(T) is the sum of the counts of the two split patterns that
/// support T; the weight is score(T) ÷ total. `None` if the total is 0.
pub fn w1(counts: &PatternCounts) -> Option<[f64; 3]> {
    let scores = SUPPORTING.map(|[x, y]| counts[x] + counts[y]);
    let total: u64 = scores.iter().sum();
    if total == 0 {
        return None;
    }
    Some(scores.map(|s| s as f64 / total as f64))
}

/// Fits of the three topologies; `None` if no column is counted.
pub fn fit_all(model: &Model, cond: Conditioning, counts: &PatternCounts) -> Option<[Fit; 3]> {
    let fits = [
        fit(model, cond, 0, counts)?,
        fit(model, cond, 1, counts)?,
        fit(model, cond, 2, counts)?,
    ];
    Some(fits)
}

/// True when the internal branch of all three fits is at the lower bound
/// (a star fit): the data favour no resolution of the quartet.
pub fn is_star(fits: &[Fit; 3]) -> bool {
    fits.iter().all(|f| f.lengths[4] <= MIN_LENGTH)
}

/// Winners of one W2c resample: all three topologies on a star fit
/// (OI-14, option b), otherwise `winners` of the log-likelihoods.
pub fn resample_winners(fits: &[Fit; 3]) -> [bool; 3] {
    if is_star(fits) {
        [true; 3]
    } else {
        winners(&fits.map(|f| f.log_likelihood))
    }
}

/// Topologies whose log-likelihood is within `TIE_TOLERANCE` of the best.
pub fn winners(ll: &[f64; 3]) -> [bool; 3] {
    let best = ll[0].max(ll[1]).max(ll[2]);
    ll.map(|v| best - v <= TIE_TOLERANCE)
}

/// W2a: the best topology (the first on ties) and ℓ*_best − ℓ*_second
/// (0 on a tie).
pub fn w2a(ll: &[f64; 3]) -> (usize, f64) {
    let mut order = [0usize, 1, 2];
    order.sort_by(|&x, &y| ll[y].total_cmp(&ll[x]).then(x.cmp(&y)));
    let gap = ll[order[0]] - ll[order[1]];
    (order[0], if gap <= TIE_TOLERANCE { 0.0 } else { gap })
}

/// W2b: w_T = exp(ℓ*_T − max) ÷ Σ exp(ℓ*_T' − max).
pub fn w2b(ll: &[f64; 3]) -> [f64; 3] {
    let best = ll[0].max(ll[1]).max(ll[2]);
    let e = ll.map(|v| libm::exp(v - best));
    let sum = e[0] + e[1] + e[2];
    e.map(|v| v / sum)
}

/// Per-quartet seed: the first 8 bytes (little-endian) of
/// SHA-256(global seed as 8 bytes little-endian ‖ rank as 8 bytes
/// little-endian).
pub fn quartet_seed(global_seed: u64, rank: u64) -> u64 {
    let mut h = Sha256::new();
    h.update(global_seed.to_le_bytes());
    h.update(rank.to_le_bytes());
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
}

/// SplitMix64 (Steele, Lea and Flood 2014): a small, fully specified
/// generator, so resamples are identical on every platform.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1) with 53 random bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// One draw from Binomial(n, p) by inversion, searching outward from the
/// mode (the probability of the mode from `libm::lgamma`, its neighbours by
/// the ratio of successive probabilities).
pub fn binomial(rng: &mut SplitMix64, n: u64, p: f64) -> u64 {
    if n == 0 || p <= 0.0 {
        return 0;
    }
    if p >= 1.0 {
        return n;
    }
    let q = 1.0 - p;
    let nf = n as f64;
    let mode = (((nf + 1.0) * p) as u64).min(n);
    let mf = mode as f64;
    let ln_mode = libm::lgamma(nf + 1.0) - libm::lgamma(mf + 1.0) - libm::lgamma(nf - mf + 1.0)
        + mf * libm::log(p)
        + (nf - mf) * libm::log1p(-p);
    let f_mode = libm::exp(ln_mode);
    let mut u = rng.next_f64();
    if u < f_mode {
        return mode;
    }
    u -= f_mode;
    let ratio = p / q;
    let (mut down, mut f_down) = (mode, f_mode);
    let (mut up, mut f_up) = (mode, f_mode);
    loop {
        let mut moved = false;
        if down > 0 && f_down > 0.0 {
            // P(k − 1) = P(k)·k ÷ (n − k + 1) ÷ ratio.
            f_down *= down as f64 / ((n - down + 1) as f64 * ratio);
            down -= 1;
            moved = true;
            if u < f_down {
                return down;
            }
            u -= f_down;
        }
        if up < n && f_up > 0.0 {
            // P(k + 1) = P(k)·(n − k) ÷ (k + 1)·ratio.
            f_up *= (n - up) as f64 / (up + 1) as f64 * ratio;
            up += 1;
            moved = true;
            if u < f_up {
                return up;
            }
            u -= f_up;
        }
        if !moved {
            // The remaining probability is below rounding error.
            return mode;
        }
    }
}

/// One multinomial resample of the counted patterns: N = their total,
/// probabilities c ÷ N, drawn as conditional binomials in pattern order.
pub fn resample(rng: &mut SplitMix64, counts: &PatternCounts, cond: Conditioning) -> PatternCounts {
    let mut out = [0u64; 16];
    let mut left_n = conditioned_total(counts, cond);
    let mut left_c = left_n;
    for x in 0..16 {
        if cond.excludes(x) || counts[x] == 0 {
            continue;
        }
        if left_c == counts[x] {
            out[x] = left_n;
            break;
        }
        let k = binomial(rng, left_n, counts[x] as f64 / left_c as f64);
        out[x] = k;
        left_n -= k;
        left_c -= counts[x];
        if left_n == 0 {
            break;
        }
    }
    out
}

/// W2c: fraction of `replicates` multinomial resamples in which each
/// topology has the highest refitted likelihood (ties split equally; a
/// resample whose three fits are stars is a three-way tie). The
/// resamples come from `SplitMix64::new(seed)`. `None` if no column is
/// counted.
pub fn w2c(
    model: &Model,
    cond: Conditioning,
    counts: &PatternCounts,
    seed: u64,
    replicates: u32,
) -> Option<[f64; 3]> {
    if conditioned_total(counts, cond) == 0 || replicates == 0 {
        return None;
    }
    let mut rng = SplitMix64::new(seed);
    let mut wins = [0.0; 3];
    for _ in 0..replicates {
        let sample = resample(&mut rng, counts, cond);
        let fits = fit_all(model, cond, &sample).expect("a resample keeps the total");
        let w = resample_winners(&fits);
        let share = 1.0 / w.iter().filter(|&&v| v).count() as f64;
        for t in 0..3 {
            if w[t] {
                wins[t] += share;
            }
        }
    }
    Some(wins.map(|v| v / replicates as f64))
}

/// All weights of one quartet.
#[derive(Clone, Debug, PartialEq)]
pub struct QuartetWeights {
    /// W1 weights, `None` if no split pattern was seen.
    pub w1: Option<[f64; 3]>,
    /// The three W2 fits, `None` if no column is counted.
    pub fits: Option<[Fit; 3]>,
    /// W2a: winner and likelihood gap.
    pub w2a: Option<(usize, f64)>,
    /// W2b weights.
    pub w2b: Option<[f64; 3]>,
    /// W2c weights.
    pub w2c: Option<[f64; 3]>,
}

/// Every weight of one quartet with the given per-quartet seed.
pub fn weigh(
    model: &Model,
    cond: Conditioning,
    counts: &PatternCounts,
    seed: u64,
    replicates: u32,
) -> QuartetWeights {
    let fits = fit_all(model, cond, counts);
    let ll = fits.map(|f| f.map(|x| x.log_likelihood));
    QuartetWeights {
        w1: w1(counts),
        fits,
        w2a: ll.as_ref().map(w2a),
        w2b: ll.as_ref().map(w2b),
        w2c: w2c(model, cond, counts, seed, replicates),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 11
        }
        fn unit(&mut self) -> f64 {
            self.next() as f64 / (1u64 << 53) as f64
        }
    }

    fn models() -> [Model; 3] {
        [
            Model::symmetric(),
            Model::with_frequency_of_one(0.3),
            Model::with_frequency_of_one(0.82),
        ]
    }

    #[test]
    fn probabilities_sum_to_one() {
        let mut rng = Lcg(5);
        for model in models() {
            for _ in 0..500 {
                let lengths: [f64; 5] = std::array::from_fn(|_| {
                    MIN_LENGTH + rng.unit() * (MAX_LENGTH - MIN_LENGTH) * rng.unit()
                });
                for t in 0..3 {
                    let p = pattern_probabilities(&model, t, &lengths);
                    let sum: f64 = p.iter().sum();
                    assert!((sum - 1.0).abs() <= 1e-12, "sum {sum} for {lengths:?}");
                    assert!(p.iter().all(|&v| v >= 0.0));
                }
            }
        }
    }

    #[test]
    fn golden_g3() {
        // Every branch's change probability is 0.1: (1 − exp(−2t)) ÷ 2 = 0.1.
        let t = -libm::log(0.8) / 2.0;
        assert!((t - 0.1115718).abs() < 1e-7);
        let model = Model::symmetric();
        let p1 = pattern_probabilities(&model, 0, &[t; 5]);
        let p2 = pattern_probabilities(&model, 1, &[t; 5]);
        assert!((p1[0b1100] - 0.0401).abs() < 1e-7, "{}", p1[0b1100]);
        assert!((p2[0b1100] - 0.0081).abs() < 1e-7, "{}", p2[0b1100]);
        let change = model.transition(t)[0][1];
        assert!((change - 0.1).abs() < 1e-15);
    }

    /// Applies a permutation of quartet positions to a pattern index:
    /// position p of the result takes the bit of position perm[p].
    fn permute_pattern(x: usize, perm: [usize; 4]) -> usize {
        (0..4).fold(0, |acc, p| acc | (bit(x, perm[p]) << (3 - p)))
    }

    /// Topology that a split of positions becomes under a permutation.
    fn topology_of_pairs(a: usize, b: usize) -> usize {
        let (x, y) = (a.min(b), a.max(b));
        let partner_of_0 = if x == 0 {
            y
        } else {
            // 0 is in the other pair; its partner is the remaining position.
            (1..4).find(|&p| p != x && p != y).unwrap()
        };
        partner_of_0 - 1
    }

    #[test]
    fn relabelling_leaves_permutes_probabilities() {
        let mut rng = Lcg(17);
        let perms = [
            [1, 0, 2, 3],
            [0, 1, 3, 2],
            [2, 3, 0, 1],
            [0, 2, 1, 3],
            [3, 1, 2, 0],
            [1, 2, 3, 0],
        ];
        for model in models() {
            for _ in 0..50 {
                let lengths: [f64; 5] = std::array::from_fn(|_| 0.01 + 2.0 * rng.unit());
                for t in 0..3 {
                    let p = pattern_probabilities(&model, t, &lengths);
                    for perm in perms {
                        // New member p is old member perm[p].
                        let [i, j, _, _] = PAIRS[t];
                        let inv = |old: usize| (0..4).find(|&p| perm[p] == old).unwrap();
                        let nt = topology_of_pairs(inv(i), inv(j));
                        let mut nl = [0.0; 5];
                        for p in 0..4 {
                            nl[p] = lengths[perm[p]];
                        }
                        nl[4] = lengths[4];
                        let q = pattern_probabilities(&model, nt, &nl);
                        for x in 0..16 {
                            let y = permute_pattern(x, perm);
                            assert!((q[y] - p[x]).abs() < 1e-15, "perm {perm:?} t {t} x {x}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn symmetric_model_is_symmetric_in_zero_and_one() {
        let mut rng = Lcg(23);
        let model = Model::symmetric();
        for _ in 0..100 {
            let lengths: [f64; 5] = std::array::from_fn(|_| 0.01 + 3.0 * rng.unit());
            for t in 0..3 {
                let p = pattern_probabilities(&model, t, &lengths);
                for x in 0..16 {
                    assert!((p[x] - p[15 - x]).abs() < 1e-15);
                }
            }
        }
    }

    /// Expected counts of a topology, rounded.
    fn expected_counts(model: &Model, t: usize, lengths: &[f64; 5], n: f64) -> PatternCounts {
        pattern_probabilities(model, t, lengths).map(|p| libm::round(p * n) as u64)
    }

    #[test]
    fn fit_reaches_a_stationary_point_and_beats_the_starts() {
        let mut rng = Lcg(31);
        for model in models() {
            for case in 0..30 {
                let truth: [f64; 5] = std::array::from_fn(|_| 0.02 + 0.8 * rng.unit());
                let counts = expected_counts(&model, case % 3, &truth, 20_000.0);
                for cond in [Conditioning::NotAllZero, Conditioning::NotConstant] {
                    for t in 0..3 {
                        let f = fit(&model, cond, t, &counts).unwrap();
                        for s in starts(&model, cond, t, &counts) {
                            assert!(
                                f.log_likelihood >= log_likelihood(&model, cond, t, &s, &counts)
                            );
                        }
                        // No single branch can still be improved noticeably.
                        for b in 0..5 {
                            for factor in [0.999, 1.001] {
                                let mut l = f.lengths;
                                l[b] = (l[b] * factor).clamp(MIN_LENGTH, MAX_LENGTH);
                                let v = log_likelihood(&model, cond, t, &l, &counts);
                                assert!(
                                    v <= f.log_likelihood + 1e-7,
                                    "case {case} t {t} b {b}: {v} > {}",
                                    f.log_likelihood
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_true_topology_wins_with_its_expected_counts() {
        let mut rng = Lcg(37);
        let model = Model::symmetric();
        for case in 0..30 {
            let mut truth: [f64; 5] = std::array::from_fn(|_| 0.05 + 0.3 * rng.unit());
            truth[4] = 0.2;
            let t = case % 3;
            let counts = expected_counts(&model, t, &truth, 50_000.0);
            let fits = fit_all(&model, Conditioning::NotAllZero, &counts).unwrap();
            let ll = fits.map(|f| f.log_likelihood);
            assert_eq!(w2a(&ll).0, t, "{ll:?}");
            // Fitted lengths close to the truth.
            for b in 0..5 {
                assert!(
                    (fits[t].lengths[b] - truth[b]).abs() < 0.02,
                    "case {case} branch {b}: {} vs {}",
                    fits[t].lengths[b],
                    truth[b]
                );
            }
        }
    }

    #[test]
    fn relabelling_the_counts_relabels_the_fit() {
        let model = Model::symmetric();
        let counts: PatternCounts = [0, 40, 38, 3, 4, 14, 5, 2, 3, 5, 14, 1, 10, 2, 1, 6];
        let ll = fit_all(&model, Conditioning::NotAllZero, &counts)
            .unwrap()
            .map(|f| f.log_likelihood);
        // Swap members b and c: ab|cd ↔ ac|bd, ad|bc stays.
        let perm = [0, 2, 1, 3];
        let mut swapped = [0u64; 16];
        for x in 0..16 {
            swapped[permute_pattern(x, perm)] = counts[x];
        }
        let ll2 = fit_all(&model, Conditioning::NotAllZero, &swapped)
            .unwrap()
            .map(|f| f.log_likelihood);
        assert!((ll[0] - ll2[1]).abs() < 1e-8);
        assert!((ll[1] - ll2[0]).abs() < 1e-8);
        assert!((ll[2] - ll2[2]).abs() < 1e-8);
    }

    #[test]
    fn w1_follows_the_split_patterns() {
        let mut c = [0u64; 16];
        c[0b1100] = 3;
        c[0b0011] = 1;
        c[0b1010] = 4;
        c[0b0110] = 2;
        c[0b1000] = 50;
        assert_eq!(w1(&c), Some([0.4, 0.4, 0.2]));
        let mut none = [0u64; 16];
        none[0b1000] = 5;
        none[0b1111] = 5;
        assert_eq!(w1(&none), None);
    }

    #[test]
    fn w2a_and_w2b() {
        let ll = [-100.0, -97.5, -99.0];
        assert_eq!(w2a(&ll), (1, 1.5));
        let w = w2b(&ll);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        assert!(w[1] > w[2] && w[2] > w[0]);
        assert_eq!(w2a(&[-5.0, -5.0, -6.0]), (0, 0.0));
        assert_eq!(winners(&[-5.0, -5.0 - 1e-10, -6.0]), [true, true, false]);
    }

    #[test]
    fn quartet_seeds_are_fixed() {
        // First 8 bytes of SHA-256 of 16 zero bytes, little-endian.
        let zero = quartet_seed(0, 0);
        let digest = Sha256::digest([0u8; 16]);
        assert_eq!(zero, u64::from_le_bytes(digest[..8].try_into().unwrap()));
        assert_eq!(quartet_seed(7, 12), quartet_seed(7, 12));
        assert_ne!(quartet_seed(7, 12), quartet_seed(7, 13));
        assert_ne!(quartet_seed(7, 12), quartet_seed(8, 12));
    }

    #[test]
    fn splitmix_reference_values() {
        // Reference output of SplitMix64 seeded with 1234567.
        let mut r = SplitMix64::new(1_234_567);
        let v: Vec<u64> = (0..3).map(|_| r.next_u64()).collect();
        assert_eq!(
            v,
            [
                6_457_827_717_110_365_317,
                3_203_168_211_198_807_973,
                9_817_491_932_198_370_423
            ]
        );
    }

    #[test]
    fn binomial_matches_its_distribution() {
        let mut rng = SplitMix64::new(99);
        for (n, p) in [
            (10u64, 0.3),
            (37_827, 0.02),
            (5_000, 0.5),
            (200, 0.97),
            (3, 0.0001),
        ] {
            let draws = 40_000;
            let mut sum = 0.0;
            let mut sq = 0.0;
            for _ in 0..draws {
                let k = binomial(&mut rng, n, p);
                assert!(k <= n);
                sum += k as f64;
                sq += (k as f64) * (k as f64);
            }
            let mean = sum / draws as f64;
            let var = sq / draws as f64 - mean * mean;
            let (em, ev) = (n as f64 * p, n as f64 * p * (1.0 - p));
            assert!(
                (mean - em).abs() < 5.0 * (ev / draws as f64).sqrt() + 1e-9,
                "n {n} p {p} mean {mean}"
            );
            assert!((var - ev).abs() < 0.05 * ev + 1e-3, "n {n} p {p} var {var}");
        }
        // Exact frequencies for a small case: Binomial(4, 0.25).
        let pmf = [
            81.0 / 256.0,
            108.0 / 256.0,
            54.0 / 256.0,
            12.0 / 256.0,
            1.0 / 256.0,
        ];
        let mut hist = [0u32; 5];
        let draws = 200_000;
        for _ in 0..draws {
            hist[binomial(&mut rng, 4, 0.25) as usize] += 1;
        }
        for k in 0..5 {
            let f = hist[k] as f64 / draws as f64;
            let sd = (pmf[k] * (1.0 - pmf[k]) / draws as f64).sqrt();
            assert!((f - pmf[k]).abs() < 5.0 * sd, "k {k}: {f} vs {}", pmf[k]);
        }
    }

    #[test]
    fn resample_keeps_the_total_and_the_excluded_patterns() {
        let counts: PatternCounts = [500, 40, 38, 3, 4, 14, 5, 2, 3, 5, 14, 1, 10, 2, 1, 6];
        let mut rng = SplitMix64::new(3);
        for cond in [Conditioning::NotAllZero, Conditioning::NotConstant] {
            for _ in 0..200 {
                let r = resample(&mut rng, &counts, cond);
                assert_eq!(
                    conditioned_total(&r, cond),
                    conditioned_total(&counts, cond)
                );
                for x in 0..16 {
                    if cond.excludes(x) || counts[x] == 0 {
                        assert_eq!(r[x], 0);
                    }
                }
            }
        }
    }

    #[test]
    fn w2c_is_reproducible_and_sums_to_one() {
        let model = Model::symmetric();
        let counts: PatternCounts = [0, 40, 38, 3, 4, 14, 5, 2, 3, 5, 14, 1, 10, 2, 1, 6];
        let seed = quartet_seed(42, 1234);
        let a = w2c(&model, Conditioning::NotAllZero, &counts, seed, 30).unwrap();
        let b = w2c(&model, Conditioning::NotAllZero, &counts, seed, 30).unwrap();
        assert_eq!(a, b);
        assert!((a.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let empty: PatternCounts = [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            w2c(&model, Conditioning::NotAllZero, &empty, seed, 30),
            None
        );
    }

    /// Quartet NC_009057, NC_009066, NC_011177, NC_013564 of Fish mtDNA
    /// (seed 1, strand filter on), the example of OI-14.
    const FISH_STAR: PatternCounts = [
        24744, 2356, 2094, 215, 2973, 311, 246, 54, 2136, 255, 366, 56, 236, 61, 80, 34,
    ];

    #[test]
    fn star_resamples_are_three_way_ties() {
        let model = Model::symmetric();
        let cond = Conditioning::NotAllZero;
        let fits = fit_all(&model, cond, &FISH_STAR).unwrap();
        assert!(is_star(&fits));
        // The log-likelihoods differ by about 1e-5, more than TIE_TOLERANCE,
        // but a star fit favours no topology.
        assert_eq!(
            winners(&fits.map(|f| f.log_likelihood)),
            [false, true, false]
        );
        assert_eq!(resample_winners(&fits), [true; 3]);
        let w = w2c(&model, cond, &FISH_STAR, 5031545449894749448, 100).unwrap();
        assert!(w.iter().all(|&v| (v - 1.0 / 3.0).abs() < 1e-12), "{w:?}");
    }

    #[test]
    fn resolved_fits_keep_the_likelihood_winner() {
        let model = Model::symmetric();
        let counts: PatternCounts = [0, 40, 38, 3, 4, 14, 5, 2, 3, 5, 14, 1, 10, 2, 1, 6];
        let fits = fit_all(&model, Conditioning::NotAllZero, &counts).unwrap();
        assert!(!is_star(&fits));
        assert_eq!(
            resample_winners(&fits),
            winners(&fits.map(|f| f.log_likelihood))
        );
    }

    #[test]
    fn weigh_reports_every_variant() {
        let model = Model::symmetric();
        let counts: PatternCounts = [0, 4, 38, 3, 4, 2, 5, 2, 40, 5, 14, 1, 10, 2, 1, 6];
        let w = weigh(&model, Conditioning::NotAllZero, &counts, 1, 10);
        assert!(w.w1.is_some() && w.fits.is_some() && w.w2a.is_some());
        assert!(w.w2b.is_some() && w.w2c.is_some());
    }

    #[test]
    fn gradient_and_hessian_match_finite_differences() {
        let counts: PatternCounts = [
            5525, 2045, 2038, 907, 1765, 693, 692, 335, 1612, 727, 731, 415, 1093, 541, 545, 336,
        ];
        for model in models() {
            for cond in [Conditioning::NotAllZero, Conditioning::NotConstant] {
                for topology in 0..3 {
                    let ctx = Context {
                        model: &model,
                        cond,
                        topology,
                        counts: &counts,
                        lo: model.s_of(MAX_LENGTH),
                        hi: model.s_of(MIN_LENGTH),
                    };
                    let lengths = [0.2, 0.5, 0.9, 0.3, 0.05];
                    let (g, h) = ctx.derivatives(&ctx.state(lengths));
                    let s0 = lengths.map(|t| model.s_of(t));
                    let at = |s: [f64; 5]| ctx.state(s.map(|v| model.t_of(v)));
                    let e = 1e-5;
                    for k in 0..5 {
                        let mut up = s0;
                        up[k] += e;
                        let mut down = s0;
                        down[k] -= e;
                        let num = (at(up).value - at(down).value) / (2.0 * e);
                        assert!((num - g[k]).abs() <= 1e-5 * (1.0 + num.abs()), "g {k}");
                        let (gu, _) = ctx.derivatives(&at(up));
                        let (gd, _) = ctx.derivatives(&at(down));
                        for j in 0..5 {
                            let num = (gu[j] - gd[j]) / (2.0 * e);
                            assert!(
                                (num - h[k][j]).abs() <= 1e-4 * (1.0 + num.abs()),
                                "h {k} {j}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_start_reaches_the_same_optimum_of_the_true_topology() {
        let mut rng = Lcg(41);
        for model in models() {
            for case in 0..20 {
                let mut truth: [f64; 5] = std::array::from_fn(|_| 0.05 + 0.5 * rng.unit());
                truth[4] = 0.1 + 0.3 * rng.unit();
                let t = case % 3;
                let counts = expected_counts(&model, t, &truth, 10_000.0);
                let cond = Conditioning::NotAllZero;
                let values: Vec<f64> = starts(&model, cond, t, &counts)
                    .iter()
                    .map(|s| fit_from(&model, cond, t, &counts, s).log_likelihood)
                    .collect();
                for v in &values {
                    assert!((v - values[0]).abs() < 1e-6, "case {case}: {values:?}");
                }
            }
        }
    }

    /// Throughput of the weighting (run with `--ignored --nocapture` in a
    /// release build): counts resampled from random quartet trees.
    #[test]
    #[ignore]
    fn benchmark_weighting() {
        let mut rng = Lcg(2026);
        let model = Model::symmetric();
        let cond = Conditioning::NotAllZero;
        for columns in [1_000.0, 10_000.0, 50_000.0] {
            let quartets: Vec<PatternCounts> = (0..200)
                .map(|i| {
                    let truth: [f64; 5] = std::array::from_fn(|_| 0.02 + 0.6 * rng.unit());
                    let exact = expected_counts(&model, i % 3, &truth, columns);
                    resample(
                        &mut SplitMix64::new(i as u64),
                        &exact,
                        Conditioning::NotAllZero,
                    )
                })
                .collect();
            let t0 = std::time::Instant::now();
            let mut sink = 0.0;
            for c in &quartets {
                sink += fit_all(&model, cond, c).unwrap()[0].log_likelihood;
            }
            let fits = t0.elapsed().as_secs_f64();
            let t1 = std::time::Instant::now();
            for (i, c) in quartets.iter().enumerate() {
                sink += w2c(&model, cond, c, quartet_seed(1, i as u64), REPLICATES).unwrap()[0];
            }
            let resampled = t1.elapsed().as_secs_f64();
            let n = quartets.len() as f64;
            eprintln!(
                "columns {columns}: three fits {:.1} µs per quartet ({:.0} quartets/s); W2c with {REPLICATES} resamples {:.2} ms per quartet ({:.1} quartets/s) [{sink:.1}]",
                fits / n * 1e6,
                n / fits,
                resampled / n * 1e3,
                n / resampled
            );
        }
    }
}
