//! Quartets: ranks, a seeded processing order, and pattern counts.
//!
//! - Rank of a quartet a < b < c < d (combinatorial number system):
//!   C(a, 1) + C(b, 2) + C(c, 3) + C(d, 4), a number in [0, C(m, 4)).
//! - Processing order: a keyed Feistel network on the next even power of two
//!   with cycle-walking, a bijection of [0, Q) that needs O(1) memory.
//! - Pattern counts: for a quartet q and the set P of its taxa with value 1,
//!   c(P) = Σ over P ⊆ S ⊆ q of (−1)^(|S| − |P|) n(S), where n(S) is the
//!   number of columns where every taxon of S has value 1. n is precomputed
//!   for single taxa, pairs and triples; each quartet needs one 4-way
//!   AND-popcount.
//!
//! Pattern indices: the pattern x = (x_a, x_b, x_c, x_d) of a quartet whose
//! members are in index order has index 8·x_a + 4·x_b + 2·x_c + x_d, so index
//! 12 is the pattern 1100.

/// Binomial coefficient C(n, k) for the small k used here.
pub fn binom(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    let mut r: u128 = 1;
    for i in 0..k {
        r = r * (n - i) as u128 / (i + 1) as u128;
    }
    r as u64
}

/// Number of quartets of `m` taxa.
pub fn quartet_count(m: usize) -> u64 {
    binom(m as u64, 4)
}

/// Rank of the quartet `[a, b, c, d]` with a < b < c < d.
pub fn rank(q: [usize; 4]) -> u64 {
    debug_assert!(q[0] < q[1] && q[1] < q[2] && q[2] < q[3]);
    binom(q[0] as u64, 1) + binom(q[1] as u64, 2) + binom(q[2] as u64, 3) + binom(q[3] as u64, 4)
}

/// The largest x with C(x, k) <= r.
fn largest_with(r: u64, k: u64) -> u64 {
    // Start from a real-valued estimate and correct it.
    let mut x = ((r as f64) * (1..=k).product::<u64>() as f64).powf(1.0 / k as f64) as u64 + k;
    while binom(x, k) > r {
        x -= 1;
    }
    while binom(x + 1, k) <= r {
        x += 1;
    }
    x
}

/// The quartet with rank `r`.
pub fn unrank(mut r: u64) -> [usize; 4] {
    let mut q = [0usize; 4];
    for k in (1..=4u64).rev() {
        let x = largest_with(r, k);
        r -= binom(x, k);
        q[(k - 1) as usize] = x as usize;
    }
    q
}

/// A bijection of [0, n), keyed by a seed (keyed Feistel network on the next
/// even power of two, with cycle-walking).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permutation {
    n: u64,
    half_bits: u32,
    keys: [u64; 4],
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Permutation {
    pub fn new(n: u64, seed: u64) -> Self {
        let bits = if n <= 1 {
            2
        } else {
            64 - (n - 1).leading_zeros()
        };
        let half_bits = bits.div_ceil(2).max(1);
        let mut keys = [0u64; 4];
        for (i, k) in keys.iter_mut().enumerate() {
            *k = mix(seed ^ mix(0x9E37_79B9_7F4A_7C15 ^ i as u64));
        }
        Permutation { n, half_bits, keys }
    }

    pub fn len(&self) -> u64 {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    fn feistel(&self, x: u64) -> u64 {
        let mask = (1u64 << self.half_bits) - 1;
        let mut left = x >> self.half_bits;
        let mut right = x & mask;
        for &k in &self.keys {
            let f = mix(right ^ k) & mask;
            let new_right = left ^ f;
            left = right;
            right = new_right;
        }
        (left << self.half_bits) | right
    }

    /// The element at position `i` of the permutation (`i` < n).
    pub fn apply(&self, i: u64) -> u64 {
        debug_assert!(i < self.n);
        let mut x = self.feistel(i);
        while x >= self.n {
            x = self.feistel(x);
        }
        x
    }
}

/// Number of set bits of `words`, portable.
fn popcount_portable(words: &[u64]) -> u64 {
    words.iter().map(|w| w.count_ones() as u64).sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn and4_popcnt(a: &[u64], b: &[u64], c: &[u64], d: &[u64]) -> u64 {
    let mut s = 0u64;
    for i in 0..a.len() {
        s += (a[i] & b[i] & c[i] & d[i]).count_ones() as u64;
    }
    s
}

fn and4_portable(a: &[u64], b: &[u64], c: &[u64], d: &[u64]) -> u64 {
    let mut s = 0u64;
    for i in 0..a.len() {
        s += (a[i] & b[i] & c[i] & d[i]).count_ones() as u64;
    }
    s
}

/// Which popcount code path is used on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopcountPath {
    /// The `popcnt` instruction (x86-64, detected at run time).
    X86Popcnt,
    /// Plain Rust `count_ones` (on ARM64 this compiles to a native instruction).
    Portable,
}

impl PopcountPath {
    /// The fastest path this computer supports.
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("popcnt") {
                return PopcountPath::X86Popcnt;
            }
        }
        PopcountPath::Portable
    }

    /// Set bits of a AND b AND c AND d (equal lengths).
    pub fn and4(self, a: &[u64], b: &[u64], c: &[u64], d: &[u64]) -> u64 {
        assert!(a.len() == b.len() && b.len() == c.len() && c.len() == d.len());
        match self {
            #[cfg(target_arch = "x86_64")]
            // SAFETY: this variant is only chosen when the CPU supports popcnt.
            PopcountPath::X86Popcnt => unsafe { and4_popcnt(a, b, c, d) },
            _ => and4_portable(a, b, c, d),
        }
    }
}

/// Precomputed co-occurrence counts n(S) for every set S of at most three
/// taxa.
pub struct CoCounts {
    pub m: usize,
    pub columns: u64,
    singles: Vec<u64>,
    pairs: Vec<u64>,
    triples: Vec<u64>,
}

/// Rank of the pair i < j: C(j, 2) + i.
#[inline]
fn rank2(i: usize, j: usize) -> usize {
    j * (j - 1) / 2 + i
}

/// Rank of the triple i < j < k: C(k, 3) + C(j, 2) + i.
#[inline]
fn rank3(i: usize, j: usize, k: usize) -> usize {
    k * (k - 1) * (k - 2) / 6 + j * (j - 1) / 2 + i
}

impl CoCounts {
    /// Counts for bitset rows (all of equal length) and the number of columns.
    pub fn new(rows: &[Vec<u64>], columns: u64) -> Self {
        let m = rows.len();
        let pc2 = |a: &[u64], b: &[u64]| -> u64 {
            a.iter()
                .zip(b)
                .map(|(x, y)| (x & y).count_ones() as u64)
                .sum()
        };
        let singles = rows.iter().map(|r| popcount_portable(r)).collect();
        let mut pairs = vec![0u64; binom(m as u64, 2) as usize];
        for j in 0..m {
            for i in 0..j {
                pairs[rank2(i, j)] = pc2(&rows[i], &rows[j]);
            }
        }
        let mut triples = vec![0u64; binom(m as u64, 3) as usize];
        let mut buf = vec![0u64; rows.first().map_or(0, |r| r.len())];
        for k in 0..m {
            for j in 0..k {
                for (w, (x, y)) in buf.iter_mut().zip(rows[j].iter().zip(&rows[k])) {
                    *w = x & y;
                }
                for i in 0..j {
                    triples[rank3(i, j, k)] = pc2(&rows[i], &buf);
                }
            }
        }
        CoCounts {
            m,
            columns,
            singles,
            pairs,
            triples,
        }
    }

    pub fn single(&self, i: usize) -> u64 {
        self.singles[i]
    }

    pub fn pair(&self, i: usize, j: usize) -> u64 {
        let (i, j) = if i < j { (i, j) } else { (j, i) };
        self.pairs[rank2(i, j)]
    }

    pub fn triple(&self, i: usize, j: usize, k: usize) -> u64 {
        let mut t = [i, j, k];
        t.sort_unstable();
        self.triples[rank3(t[0], t[1], t[2])]
    }

    /// Memory of the tables in bytes.
    pub fn bytes(&self) -> usize {
        (self.singles.len() + self.pairs.len() + self.triples.len()) * 8
    }
}

/// The 16 pattern counts of one quartet, indexed as described in the module
/// documentation.
pub type PatternCounts = [u64; 16];

/// Pattern counts of quartet `q` (a < b < c < d) from the co-occurrence
/// tables and n of the whole quartet (`n4`).
#[inline]
pub fn pattern_counts(cc: &CoCounts, q: [usize; 4], n4: u64) -> PatternCounts {
    let [a, b, c, d] = q;
    // n[S] for each subset S of the quartet, S as a 4-bit mask in pattern
    // order (bit 3 = a, bit 2 = b, bit 1 = c, bit 0 = d).
    let mut n = [0i64; 16];
    n[0b0000] = cc.columns as i64;
    n[0b1000] = cc.singles[a] as i64;
    n[0b0100] = cc.singles[b] as i64;
    n[0b0010] = cc.singles[c] as i64;
    n[0b0001] = cc.singles[d] as i64;
    n[0b1100] = cc.pairs[rank2(a, b)] as i64;
    n[0b1010] = cc.pairs[rank2(a, c)] as i64;
    n[0b1001] = cc.pairs[rank2(a, d)] as i64;
    n[0b0110] = cc.pairs[rank2(b, c)] as i64;
    n[0b0101] = cc.pairs[rank2(b, d)] as i64;
    n[0b0011] = cc.pairs[rank2(c, d)] as i64;
    n[0b1110] = cc.triples[rank3(a, b, c)] as i64;
    n[0b1101] = cc.triples[rank3(a, b, d)] as i64;
    n[0b1011] = cc.triples[rank3(a, c, d)] as i64;
    n[0b0111] = cc.triples[rank3(b, c, d)] as i64;
    n[0b1111] = n4 as i64;
    // Superset Möbius transform: afterwards n[P] = Σ over S ⊇ P of
    // (−1)^(|S| − |P|) n(S), the number of columns with exactly P set.
    for bit in [8usize, 4, 2, 1] {
        for s in 0..16 {
            if s & bit == 0 {
                n[s] -= n[s | bit];
            }
        }
    }
    let mut out = [0u64; 16];
    for (o, &v) in out.iter_mut().zip(&n) {
        debug_assert!(v >= 0);
        *o = v as u64;
    }
    out
}

/// Pattern counts of one quartet by scanning every column (the oracle).
pub fn pattern_counts_brute(rows: &[Vec<u64>], columns: usize, q: [usize; 4]) -> PatternCounts {
    let bit = |t: usize, j: usize| (rows[t][j / 64] >> (j % 64)) & 1;
    let mut out = [0u64; 16];
    for j in 0..columns {
        let p = (bit(q[0], j) << 3) | (bit(q[1], j) << 2) | (bit(q[2], j) << 1) | bit(q[3], j);
        out[p as usize] += 1;
    }
    out
}

/// Pattern name, for example `1100` for index 12.
pub fn pattern_name(index: usize) -> String {
    format!("{index:04b}")
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
            self.0 >> 33
        }
    }

    #[test]
    fn g6_rank_and_unrank_round_trip_for_every_quartet_up_to_30_taxa() {
        for m in 4..=30usize {
            let mut expected = 0u64;
            // Colex order: d is the most significant member.
            for d in 3..m {
                for c in 2..d {
                    for b in 1..c {
                        for a in 0..b {
                            let q = [a, b, c, d];
                            let r = rank(q);
                            assert_eq!(r, expected, "m {m} {q:?}");
                            assert_eq!(unrank(r), q);
                            expected += 1;
                        }
                    }
                }
            }
            assert_eq!(expected, quartet_count(m));
        }
        assert_eq!(quartet_count(116), 7_160_245);
        assert_eq!(unrank(quartet_count(116) - 1), [112, 113, 114, 115]);
    }

    #[test]
    fn g7_feistel_permutation_is_a_bijection_up_to_one_million() {
        for &n in &[1u64, 2, 3, 5, 15, 16, 17, 1000, 65_536, 999_999, 1_000_000] {
            let p = Permutation::new(n, 42);
            let mut seen = vec![false; n as usize];
            for i in 0..n {
                let x = p.apply(i);
                assert!(x < n);
                assert!(!seen[x as usize], "n {n}: {x} twice");
                seen[x as usize] = true;
            }
        }
    }

    #[test]
    fn permutations_depend_on_the_seed_and_are_reproducible() {
        let a = Permutation::new(1000, 1);
        let b = Permutation::new(1000, 2);
        let a2 = Permutation::new(1000, 1);
        let first_a: Vec<u64> = (0..20).map(|i| a.apply(i)).collect();
        let first_b: Vec<u64> = (0..20).map(|i| b.apply(i)).collect();
        assert_ne!(first_a, first_b);
        assert_eq!(first_a, (0..20).map(|i| a2.apply(i)).collect::<Vec<_>>());
        // Not the identity.
        assert_ne!(first_a, (0..20).collect::<Vec<u64>>());
    }

    fn random_rows(rng: &mut Lcg, m: usize, columns: usize) -> Vec<Vec<u64>> {
        let words = columns.div_ceil(64).div_ceil(4) * 4;
        (0..m)
            .map(|_| {
                let mut row = vec![0u64; words];
                for j in 0..columns {
                    if rng.next().is_multiple_of(3) {
                        row[j / 64] |= 1 << (j % 64);
                    }
                }
                row
            })
            .collect()
    }

    #[test]
    fn g5_inclusion_exclusion_equals_brute_force_on_200_random_matrices() {
        let mut rng = Lcg(20_261_002);
        let path = PopcountPath::detect();
        for case in 0..200 {
            let m = 4 + (rng.next() % 9) as usize; // 4 to 12 taxa
            let columns = 1 + (rng.next() % 3000) as usize; // 1 to 3,000 columns
            let rows = random_rows(&mut rng, m, columns);
            let cc = CoCounts::new(&rows, columns as u64);
            for r in 0..quartet_count(m) {
                let q = unrank(r);
                let n4 = path.and4(&rows[q[0]], &rows[q[1]], &rows[q[2]], &rows[q[3]]);
                let fast = pattern_counts(&cc, q, n4);
                assert_eq!(
                    fast,
                    pattern_counts_brute(&rows, columns, q),
                    "case {case} {q:?}"
                );
                assert_eq!(fast.iter().sum::<u64>(), columns as u64);
            }
        }
    }

    #[test]
    fn popcount_paths_agree() {
        let mut rng = Lcg(5);
        let rows = random_rows(&mut rng, 4, 5000);
        let portable = PopcountPath::Portable.and4(&rows[0], &rows[1], &rows[2], &rows[3]);
        let detected = PopcountPath::detect().and4(&rows[0], &rows[1], &rows[2], &rows[3]);
        assert_eq!(portable, detected);
    }

    /// Counting benchmark (run with
    /// `cargo test --release -p qmaws-core -- --ignored benchmark --nocapture`).
    /// Random matrices with about one third ones; every quartet is counted on
    /// one thread; the table step and the quartet step are timed separately.
    #[test]
    #[ignore]
    fn benchmark_counting() {
        let path = PopcountPath::detect();
        println!("popcount path: {path:?}");
        println!("taxa\tcolumns\tquartets\ttables_s\tquartets_s\tquartets_per_s\ttable_bytes");
        let mut rng = Lcg(1);
        for &m in &[25usize, 50, 116] {
            for &cols in &[10_000usize, 50_000, 200_000] {
                let rows = random_rows(&mut rng, m, cols);
                let t = std::time::Instant::now();
                let cc = CoCounts::new(&rows, cols as u64);
                let tables = t.elapsed().as_secs_f64();
                let q = quartet_count(m);
                let t = std::time::Instant::now();
                let mut checksum = 0u64;
                for r in 0..q {
                    let quad = unrank(r);
                    let n4 = path.and4(
                        &rows[quad[0]],
                        &rows[quad[1]],
                        &rows[quad[2]],
                        &rows[quad[3]],
                    );
                    checksum = checksum.wrapping_add(pattern_counts(&cc, quad, n4)[15]);
                }
                let secs = t.elapsed().as_secs_f64();
                std::hint::black_box(checksum);
                println!(
                    "{m}\t{cols}\t{q}\t{tables:.3}\t{secs:.3}\t{:.0}\t{}",
                    q as f64 / secs,
                    cc.bytes()
                );
            }
        }
    }

    #[test]
    fn pattern_names() {
        assert_eq!(pattern_name(12), "1100");
        assert_eq!(pattern_name(0), "0000");
        assert_eq!(pattern_name(15), "1111");
    }
}
