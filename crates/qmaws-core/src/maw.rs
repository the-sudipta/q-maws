//! Minimal absent words (MAWs) with a suffix automaton.
//!
//! A word w = a·u·b (a, b single letters, u possibly empty) is a minimal
//! absent word of s if a·u and u·b occur in s but a·u·b does not. Only MAWs of
//! length at least 2 are produced.
//!
//! Enumeration (as in ML-MAWS, `MAWExtractor.cpp`): walk every path of the
//! suffix automaton from the initial state, that is, every distinct factor x
//! of s with |x| < `lmax`. At the state p reached by x, for each letter b
//! without a transition from p, x·b is a MAW exactly when the suffix-link
//! state q of p has a transition on b and len(q) + 1 >= |x|. (Then x without
//! its first letter lies in q, so it extends by b, while x itself does not.)
//!
//! Words are stored 2 bits per letter (A=0, C=1, G=2, T=3), most significant
//! letter first, in one sorted list per length: within one length, numeric
//! order is lexicographic order. Lengths up to 32 use `u64` codes, lengths 33
//! to 64 use `u128` codes.

use std::cmp::Ordering;

/// Longest supported MAW length.
pub const MAX_WORD_LEN: usize = 64;

const NONE: u32 = u32::MAX;

/// Letter code: A=0, C=1, G=2, T=3; anything else is not a DNA letter.
#[inline]
pub fn letter_code(b: u8) -> Option<u8> {
    match b {
        b'A' => Some(0),
        b'C' => Some(1),
        b'G' => Some(2),
        b'T' => Some(3),
        _ => None,
    }
}

const LETTERS: [u8; 4] = *b"ACGT";

/// Suffix automaton of a DNA string (Blumer et al., 1985), built online.
pub struct SuffixAutomaton {
    next: Vec<[u32; 4]>,
    link: Vec<u32>,
    len: Vec<u32>,
    last: u32,
}

impl SuffixAutomaton {
    /// Builds the automaton of `s`, skipping bytes that are not A, C, G, T.
    pub fn build(s: &[u8]) -> Self {
        let cap = 2 * s.len() + 2;
        let mut sa = SuffixAutomaton {
            next: Vec::with_capacity(cap),
            link: Vec::with_capacity(cap),
            len: Vec::with_capacity(cap),
            last: 0,
        };
        sa.push([NONE; 4], NONE, 0);
        for &b in s {
            if let Some(c) = letter_code(b) {
                sa.add(c as usize);
            }
        }
        sa
    }

    fn push(&mut self, next: [u32; 4], link: u32, len: u32) -> u32 {
        self.next.push(next);
        self.link.push(link);
        self.len.push(len);
        (self.next.len() - 1) as u32
    }

    fn add(&mut self, c: usize) {
        let cur = self.push([NONE; 4], NONE, self.len[self.last as usize] + 1);
        let mut u = self.last;
        while u != NONE && self.next[u as usize][c] == NONE {
            self.next[u as usize][c] = cur;
            u = self.link[u as usize];
        }
        if u == NONE {
            self.link[cur as usize] = 0;
        } else {
            let v = self.next[u as usize][c];
            if self.len[u as usize] + 1 == self.len[v as usize] {
                self.link[cur as usize] = v;
            } else {
                let clone = self.push(
                    self.next[v as usize],
                    self.link[v as usize],
                    self.len[u as usize] + 1,
                );
                while u != NONE && self.next[u as usize][c] == v {
                    self.next[u as usize][c] = clone;
                    u = self.link[u as usize];
                }
                self.link[v as usize] = clone;
                self.link[cur as usize] = clone;
            }
        }
        self.last = cur;
    }

    /// Number of states.
    pub fn states(&self) -> usize {
        self.next.len()
    }
}

/// Codes of the MAWs of one length, sorted ascending (lexicographic order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Codes {
    Short(Vec<u64>),
    Long(Vec<u128>),
}

impl Codes {
    fn new(len: usize) -> Self {
        if len <= 32 {
            Codes::Short(Vec::new())
        } else {
            Codes::Long(Vec::new())
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Codes::Short(v) => v.len(),
            Codes::Long(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Code `i` as `u128` (exact for every length).
    pub fn get(&self, i: usize) -> u128 {
        match self {
            Codes::Short(v) => v[i] as u128,
            Codes::Long(v) => v[i],
        }
    }

    fn push(&mut self, code: u128) {
        match self {
            Codes::Short(v) => v.push(code as u64),
            Codes::Long(v) => v.push(code),
        }
    }

    fn sort(&mut self) {
        match self {
            Codes::Short(v) => v.sort_unstable(),
            Codes::Long(v) => v.sort_unstable(),
        }
        self.shrink();
    }

    /// Releases spare capacity.
    fn shrink(&mut self) {
        match self {
            Codes::Short(v) => v.shrink_to_fit(),
            Codes::Long(v) => v.shrink_to_fit(),
        }
    }

    /// Empties the list and releases its memory.
    fn clear(&mut self) {
        match self {
            Codes::Short(v) => *v = Vec::new(),
            Codes::Long(v) => *v = Vec::new(),
        }
    }

    fn retain_in(&mut self, other: &Codes) {
        match (self, other) {
            (Codes::Short(a), Codes::Short(b)) => intersect(a, b),
            (Codes::Long(a), Codes::Long(b)) => intersect(a, b),
            _ => unreachable!("lists of one length use one representation"),
        }
    }

    /// Approximate heap memory in bytes.
    pub fn bytes(&self) -> usize {
        match self {
            Codes::Short(v) => v.len() * 8,
            Codes::Long(v) => v.len() * 16,
        }
    }
}

fn intersect<T: Ord + Copy>(a: &mut Vec<T>, b: &[T]) {
    let mut j = 0;
    a.retain(|x| {
        while j < b.len() && b[j] < *x {
            j += 1;
        }
        j < b.len() && b[j] == *x
    });
}

/// The MAWs of one sequence, one sorted list per length in
/// `[lmin, lmax]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MawSet {
    pub lmin: usize,
    pub lmax: usize,
    /// `lists[k]` holds the MAWs of length `lmin + k`.
    pub lists: Vec<Codes>,
}

impl MawSet {
    fn empty(lmin: usize, lmax: usize) -> Self {
        MawSet {
            lmin,
            lmax,
            lists: (lmin..=lmax).map(Codes::new).collect(),
        }
    }

    /// The sorted list of MAWs of length `len`, if in range.
    pub fn of_length(&self, len: usize) -> Option<&Codes> {
        (self.lmin..=self.lmax)
            .contains(&len)
            .then(|| &self.lists[len - self.lmin])
    }

    /// Releases the lists of every length not in `keep`.
    pub fn keep_lengths(&mut self, keep: &[usize]) {
        for (k, list) in self.lists.iter_mut().enumerate() {
            if !keep.contains(&(self.lmin + k)) {
                list.clear();
            }
        }
    }

    /// Total number of MAWs.
    pub fn count(&self) -> usize {
        self.lists.iter().map(Codes::len).sum()
    }

    /// Approximate heap memory in bytes.
    pub fn bytes(&self) -> usize {
        self.lists.iter().map(Codes::bytes).sum()
    }

    /// All MAWs as strings, in lexicographic order (for tests and export).
    pub fn to_strings(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::with_capacity(self.count());
        for (k, list) in self.lists.iter().enumerate() {
            let len = self.lmin + k;
            for i in 0..list.len() {
                out.push(decode(list.get(i), len));
            }
        }
        out.sort();
        out
    }
}

/// Decodes a word code of length `len` to letters.
pub fn decode(code: u128, len: usize) -> String {
    (0..len)
        .map(|i| LETTERS[((code >> (2 * (len - 1 - i))) & 3) as usize] as char)
        .collect()
}

/// Encodes a word of A, C, G, T letters.
pub fn encode(word: &str) -> Option<u128> {
    if word.len() > MAX_WORD_LEN {
        return None;
    }
    word.bytes().try_fold(0u128, |acc, b| {
        letter_code(b).map(|c| (acc << 2) | c as u128)
    })
}

/// Compares two words in lexicographic order, given their codes and lengths
/// (a proper prefix sorts first).
pub fn compare_words(a: u128, a_len: usize, b: u128, b_len: usize) -> Ordering {
    let align = |c: u128, l: usize| if l == 0 { 0 } else { c << (128 - 2 * l) };
    align(a, a_len)
        .cmp(&align(b, b_len))
        .then(a_len.cmp(&b_len))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MawError(pub String);

impl std::fmt::Display for MawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MawError {}

fn check_range(lmin: usize, lmax: usize) -> Result<(), MawError> {
    if lmin < 2 || lmin > lmax || lmax > MAX_WORD_LEN {
        return Err(MawError(format!(
            "MAW lengths must satisfy 2 <= lmin <= lmax <= {MAX_WORD_LEN}; got [{lmin}, {lmax}]"
        )));
    }
    Ok(())
}

/// MAWs of `seq` with lengths in `[lmin, lmax]`.
pub fn extract(seq: &[u8], lmin: usize, lmax: usize) -> Result<MawSet, MawError> {
    check_range(lmin, lmax)?;
    let sa = SuffixAutomaton::build(seq);
    let mut set = MawSet::empty(lmin, lmax);

    // Depth-first walk over factors shorter than lmax. Each frame: state,
    // factor code, factor length, next letter to try.
    let mut stack: Vec<(u32, u128, usize, u8)> = vec![(0, 0, 0, 0)];
    while let Some(frame) = stack.last_mut() {
        let (state, code, depth, letter) = *frame;
        if letter == 4 || depth >= lmax {
            stack.pop();
            continue;
        }
        frame.3 += 1;
        let c = letter as usize;
        let to = sa.next[state as usize][c];
        if to == NONE {
            if depth + 1 < lmin {
                continue;
            }
            let link = sa.link[state as usize];
            if link != NONE
                && sa.next[link as usize][c] != NONE
                && sa.len[link as usize] as usize + 1 >= depth
            {
                set.lists[depth + 1 - lmin].push((code << 2) | c as u128);
            }
        } else {
            stack.push((to, (code << 2) | c as u128, depth + 1, 0));
        }
    }
    for list in &mut set.lists {
        list.sort();
    }
    Ok(set)
}

/// Reverse complement of a DNA string (A↔T, C↔G); other bytes become `N`.
pub fn reverse_complement(seq: &[u8]) -> Vec<u8> {
    seq.iter()
        .rev()
        .map(|&b| match b {
            b'A' => b'T',
            b'C' => b'G',
            b'G' => b'C',
            b'T' => b'A',
            _ => b'N',
        })
        .collect()
}

/// Strand-aware MAWs: M(s) ∩ M(reverse complement of s), as in ML-MAWS.
pub fn extract_strand_aware(seq: &[u8], lmin: usize, lmax: usize) -> Result<MawSet, MawError> {
    let mut fwd = extract(seq, lmin, lmax)?;
    let rev = extract(&reverse_complement(seq), lmin, lmax)?;
    for (a, b) in fwd.lists.iter_mut().zip(&rev.lists) {
        a.retain_in(b);
        a.shrink();
    }
    Ok(fwd)
}

/// Brute-force oracle: every MAW of `seq` with length in `[lmin, lmax]`,
/// found by testing the definition on every word over A, C, G, T.
/// Exponential in `lmax`; for tests only.
pub fn brute_force(seq: &[u8], lmin: usize, lmax: usize) -> Vec<String> {
    assert!(lmax <= 12, "the oracle enumerates 4^lmax words");
    // factor[len][code] is true if the word with that code occurs in seq.
    let mut factor: Vec<Vec<bool>> = (0..=lmax).map(|l| vec![false; 1 << (2 * l)]).collect();
    factor[0][0] = true;
    for start in 0..seq.len() {
        let mut code = 0usize;
        for len in 1..=lmax.min(seq.len() - start) {
            match letter_code(seq[start + len - 1]) {
                Some(c) => code = (code << 2) | c as usize,
                None => break,
            }
            factor[len][code] = true;
        }
    }
    let mut out = Vec::new();
    for len in lmin.max(2)..=lmax {
        let suffix_mask = (1usize << (2 * (len - 1))) - 1;
        for code in 0..1usize << (2 * len) {
            let prefix = code >> 2;
            let suffix = code & suffix_mask;
            if !factor[len][code] && factor[len - 1][prefix] && factor[len - 1][suffix] {
                out.push(decode(code as u128, len));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small deterministic generator (64-bit LCG, Knuth MMIX constants).
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
    fn worksheet_two_letter_maws() {
        // Sequence K of the worksheet example: AACGTA. All four letters occur,
        // so xy is a MAW exactly when it is not a neighbouring pair.
        let m = extract(b"AACGTA", 2, 2).unwrap();
        let pairs = ["AA", "AC", "CG", "GT", "TA"];
        let expected: Vec<String> = (0..16)
            .map(|c| decode(c, 2))
            .filter(|w| !pairs.contains(&w.as_str()))
            .collect();
        assert_eq!(m.to_strings(), expected);
    }

    #[test]
    fn g4_automaton_equals_brute_force_on_2000_random_strings() {
        let mut rng = Lcg(20_261_002);
        for case in 0..2000 {
            let sigma = 2 + (rng.next() % 3) as usize; // 2, 3 or 4 letters
            let n = 1 + (rng.next() % 60) as usize; // length 1 to 60
            let s: Vec<u8> = (0..n)
                .map(|_| LETTERS[(rng.next() % sigma as u64) as usize])
                .collect();
            let got = extract(&s, 2, 8).unwrap().to_strings();
            let want = brute_force(&s, 2, 8);
            assert_eq!(got, want, "case {case}: {}", String::from_utf8_lossy(&s));
        }
    }

    #[test]
    fn length_ranges_are_respected() {
        let s = b"ACGTTGCAAGTCCTAGGATC";
        let all = brute_force(s, 2, 7);
        for (lmin, lmax) in [(2, 2), (3, 5), (4, 7), (7, 7)] {
            let want: Vec<String> = all
                .iter()
                .filter(|w| (lmin..=lmax).contains(&w.len()))
                .cloned()
                .collect();
            assert_eq!(extract(s, lmin, lmax).unwrap().to_strings(), want);
        }
        assert!(extract(s, 1, 4).is_err());
        assert!(extract(s, 5, 4).is_err());
        assert!(extract(s, 2, 65).is_err());
    }

    #[test]
    fn long_words_use_the_wide_representation() {
        // A long repeat-free prefix makes MAWs longer than 32 letters rare;
        // check encoding, decoding and ordering of long words directly.
        let w = "ACGT".repeat(10) + "TT"; // 42 letters
        let code = encode(&w).unwrap();
        assert_eq!(decode(code, w.len()), w);
        let mut set = MawSet::empty(40, 42);
        assert!(matches!(set.lists[2], Codes::Long(_)));
        set.lists[2].push(code);
        assert_eq!(set.to_strings(), vec![w]);
        // MAWs of length 33 and more are found in a sequence built for it.
        let s = format!("{}A{}", "C".repeat(40), "C".repeat(40));
        let m = extract(s.as_bytes(), 33, 45).unwrap();
        assert!(m.count() > 0);
        assert_eq!(m.to_strings(), brute_force_long(s.as_bytes(), 33, 45));
    }

    /// Brute force restricted to words that end a factor (feasible for long
    /// lengths on simple sequences).
    fn brute_force_long(seq: &[u8], lmin: usize, lmax: usize) -> Vec<String> {
        let s = String::from_utf8_lossy(seq).into_owned();
        let mut out = std::collections::BTreeSet::new();
        for len in lmin..=lmax {
            for i in 0..=s.len().saturating_sub(len - 1) {
                if i + len - 1 > s.len() {
                    break;
                }
                let prefix = &s[i..i + len - 1];
                for &b in &LETTERS {
                    let w = format!("{prefix}{}", b as char);
                    if !s.contains(&w) && s.contains(&w[1..]) {
                        out.insert(w);
                    }
                }
            }
        }
        out.into_iter().collect()
    }

    #[test]
    fn strand_aware_maws_are_maws_whose_reverse_complement_is_a_maw() {
        let mut rng = Lcg(7);
        for _ in 0..300 {
            let n = 5 + (rng.next() % 50) as usize;
            let s: Vec<u8> = (0..n).map(|_| LETTERS[(rng.next() % 4) as usize]).collect();
            let all: std::collections::BTreeSet<String> =
                brute_force(&s, 2, 6).into_iter().collect();
            let want: Vec<String> = all
                .iter()
                .filter(|w| {
                    let rc = String::from_utf8(reverse_complement(w.as_bytes())).unwrap();
                    all.contains(&rc)
                })
                .cloned()
                .collect();
            assert_eq!(extract_strand_aware(&s, 2, 6).unwrap().to_strings(), want);
        }
    }

    #[test]
    fn word_order_is_lexicographic() {
        let words = ["A", "AC", "ACG", "ACT", "C", "CA", "T", "TT"];
        for i in 0..words.len() {
            for j in 0..words.len() {
                let (a, b) = (words[i], words[j]);
                let got = compare_words(encode(a).unwrap(), a.len(), encode(b).unwrap(), b.len());
                assert_eq!(got, a.cmp(b), "{a} vs {b}");
            }
        }
    }

    #[test]
    fn automaton_size_is_linear() {
        let s: Vec<u8> = (0..10_000).map(|i| LETTERS[(i * 7 + i / 3) % 4]).collect();
        let sa = SuffixAutomaton::build(&s);
        assert!(sa.states() <= 2 * s.len());
    }
}
