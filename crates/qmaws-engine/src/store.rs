//! Binary files of a run: MAW lists, the full matrix, and quartet counts.
//!
//! All integers are little-endian. Every file starts with a 8-byte magic
//! string naming its format and version.
//!
//! - MAW lists (`work/maws/<taxon>.bin`): magic `QMAWMAW1`, lmin (u32), lmax
//!   (u32), then for each length lmin..=lmax: count (u64) and the codes
//!   (u64 each up to 32 letters, u128 each above).
//! - Matrix (`work/matrix/m_full.bin`): magic `QMAWMAT1`, taxa (u32), columns
//!   (u64), row words (u64), wide flag (u8), then the word lengths (u8 each),
//!   the codes (u64 or u128 each), the column counts (u32 each), and the rows
//!   (u64 words each, one row after another).
//! - Count chunks (`work/chunks/quartet_count_<n>.bin`): one record per quartet
//!   in processing order: rank (u64), then the 16 pattern counts (u32 each).
//! - Weight chunks (`work/chunks/quartet_weight_<n>.bin`): one record per
//!   quartet in processing order: rank (u64), flags (u32: bit 0 = the W2
//!   fits exist, bit 1 = W2c exists), the three W2 log-likelihoods (f64
//!   each) and the three W2c weights (f64 each); absent values are 0.
//! - Quartet decisions (`audit/quartet_decisions.bin.zst`, zstd-compressed):
//!   magic `QMAWDEC1`, number of quartets (u64), then per quartet in rank
//!   order the winning topology (u8: 0 ab|cd, 1 ac|bd, 2 ad|bc, 3 no weight;
//!   the first on equal weights) and its weight times 65,535, rounded (u16).

use qmaws_core::matrix::Matrix;
use qmaws_core::maw::{Codes, MawSet};

pub const MAW_MAGIC: &[u8; 8] = b"QMAWMAW1";
pub const MATRIX_MAGIC: &[u8; 8] = b"QMAWMAT1";
/// Bytes per quartet record in a count chunk.
pub const COUNT_RECORD: usize = 8 + 16 * 4;
/// Bytes per quartet record in a weight chunk.
pub const WEIGHT_RECORD: usize = 8 + 4 + 3 * 8 + 3 * 8;

/// Flag of a weight record: the W2 log-likelihoods exist.
pub const WEIGHT_FITTED: u32 = 1;
/// Flag of a weight record: the W2c weights exist.
pub const WEIGHT_RESAMPLED: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "damaged data file: {}", self.0)
    }
}

impl std::error::Error for StoreError {}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], StoreError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len());
        let Some(end) = end else {
            return Err(StoreError("the file ends too early".into()));
        };
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, StoreError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, StoreError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, StoreError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn u128(&mut self) -> Result<u128, StoreError> {
        Ok(u128::from_le_bytes(self.take(16)?.try_into().unwrap()))
    }
    fn magic(&mut self, m: &[u8; 8]) -> Result<(), StoreError> {
        if self.take(8)? != m {
            return Err(StoreError("unknown file format".into()));
        }
        Ok(())
    }
    fn end(&self) -> Result<(), StoreError> {
        if self.pos != self.b.len() {
            return Err(StoreError("unexpected data at the end".into()));
        }
        Ok(())
    }
}

pub fn maws_to_bytes(set: &MawSet) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + set.bytes() + set.lists.len() * 8);
    out.extend_from_slice(MAW_MAGIC);
    out.extend_from_slice(&(set.lmin as u32).to_le_bytes());
    out.extend_from_slice(&(set.lmax as u32).to_le_bytes());
    for list in &set.lists {
        out.extend_from_slice(&(list.len() as u64).to_le_bytes());
        match list {
            Codes::Short(v) => v
                .iter()
                .for_each(|c| out.extend_from_slice(&c.to_le_bytes())),
            Codes::Long(v) => v
                .iter()
                .for_each(|c| out.extend_from_slice(&c.to_le_bytes())),
        }
    }
    out
}

pub fn maws_from_bytes(bytes: &[u8]) -> Result<MawSet, StoreError> {
    let mut r = Reader { b: bytes, pos: 0 };
    r.magic(MAW_MAGIC)?;
    let lmin = r.u32()? as usize;
    let lmax = r.u32()? as usize;
    if lmin < 2 || lmin > lmax || lmax > qmaws_core::maw::MAX_WORD_LEN {
        return Err(StoreError(format!("bad length range [{lmin}, {lmax}]")));
    }
    let mut lists = Vec::new();
    for len in lmin..=lmax {
        let n = r.u64()? as usize;
        if len <= 32 {
            let raw = r.take(
                n.checked_mul(8)
                    .ok_or_else(|| StoreError("too large".into()))?,
            )?;
            let v = raw
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| u64::from_le_bytes(*c))
                .collect();
            lists.push(Codes::Short(v));
        } else {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(r.u128()?);
            }
            lists.push(Codes::Long(v));
        }
    }
    r.end()?;
    Ok(MawSet { lmin, lmax, lists })
}

pub fn matrix_to_bytes(m: &Matrix) -> Vec<u8> {
    let cols = m.columns_len();
    let row_words = m.rows.first().map_or(0, |r| r.len());
    let wide = matches!(m.codes, Codes::Long(_));
    let mut out = Vec::with_capacity(32 + cols * 13 + m.taxa * row_words * 8);
    out.extend_from_slice(MATRIX_MAGIC);
    out.extend_from_slice(&(m.taxa as u32).to_le_bytes());
    out.extend_from_slice(&(cols as u64).to_le_bytes());
    out.extend_from_slice(&(row_words as u64).to_le_bytes());
    out.push(wide as u8);
    out.extend_from_slice(&m.lens);
    match &m.codes {
        Codes::Short(v) => v
            .iter()
            .for_each(|c| out.extend_from_slice(&c.to_le_bytes())),
        Codes::Long(v) => v
            .iter()
            .for_each(|c| out.extend_from_slice(&c.to_le_bytes())),
    }
    m.ones
        .iter()
        .for_each(|c| out.extend_from_slice(&c.to_le_bytes()));
    for row in &m.rows {
        row.iter()
            .for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
    }
    out
}

pub fn matrix_from_bytes(bytes: &[u8]) -> Result<Matrix, StoreError> {
    let mut r = Reader { b: bytes, pos: 0 };
    r.magic(MATRIX_MAGIC)?;
    let taxa = r.u32()? as usize;
    let cols = r.u64()? as usize;
    let row_words = r.u64()? as usize;
    let wide = r.u8()? == 1;
    if row_words < cols.div_ceil(64) {
        return Err(StoreError("rows are too short for the columns".into()));
    }
    let lens = r.take(cols)?.to_vec();
    let codes = if wide {
        let mut v = Vec::with_capacity(cols);
        for _ in 0..cols {
            v.push(r.u128()?);
        }
        Codes::Long(v)
    } else {
        Codes::Short(
            r.take(cols * 8)?
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| u64::from_le_bytes(*c))
                .collect(),
        )
    };
    let ones = r
        .take(cols * 4)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    let mut rows = Vec::with_capacity(taxa);
    for _ in 0..taxa {
        rows.push(
            r.take(row_words * 8)?
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| u64::from_le_bytes(*c))
                .collect(),
        );
    }
    r.end()?;
    Ok(Matrix {
        taxa,
        lens,
        codes,
        ones,
        rows,
    })
}

/// Appends one quartet record to a count chunk.
pub fn push_count_record(out: &mut Vec<u8>, rank: u64, counts: &[u64; 16]) {
    out.extend_from_slice(&rank.to_le_bytes());
    for &c in counts {
        out.extend_from_slice(&(c as u32).to_le_bytes());
    }
}

/// Reads the records of a count chunk.
pub fn count_records(bytes: &[u8]) -> Result<Vec<(u64, [u32; 16])>, StoreError> {
    if !bytes.len().is_multiple_of(COUNT_RECORD) {
        return Err(StoreError("count chunk has a partial record".into()));
    }
    Ok(bytes
        .as_chunks::<COUNT_RECORD>()
        .0
        .iter()
        .map(|rec| {
            let rank = u64::from_le_bytes(rec[..8].try_into().unwrap());
            let mut c = [0u32; 16];
            for (i, slot) in c.iter_mut().enumerate() {
                let s = 8 + 4 * i;
                *slot = u32::from_le_bytes(rec[s..s + 4].try_into().unwrap());
            }
            (rank, c)
        })
        .collect())
}

/// One quartet's stored weighting result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightRecord {
    pub rank: u64,
    pub flags: u32,
    /// W2 log-likelihoods of ab|cd, ac|bd, ad|bc.
    pub log_likelihoods: [f64; 3],
    /// W2c weights of ab|cd, ac|bd, ad|bc.
    pub w2c: [f64; 3],
}

impl WeightRecord {
    pub fn fitted(&self) -> bool {
        self.flags & WEIGHT_FITTED != 0
    }

    pub fn resampled(&self) -> bool {
        self.flags & WEIGHT_RESAMPLED != 0
    }

    /// The weights used for the tree: W2c, or W2b when no resamples were
    /// made; `None` without a fit.
    pub fn tree_weights(&self) -> Option<[f64; 3]> {
        if !self.fitted() {
            None
        } else if self.resampled() {
            Some(self.w2c)
        } else {
            Some(qmaws_core::weight::w2b(&self.log_likelihoods))
        }
    }

    /// Canonical text for hashing: floating-point values rounded to 9
    /// significant digits (determinism contract).
    pub fn canonical(&self) -> String {
        let f = |v: &[f64; 3]| {
            v.iter()
                .map(|x| format!("{x:.8e}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        format!(
            "{} {} {} {}\n",
            self.rank,
            self.flags,
            f(&self.log_likelihoods),
            f(&self.w2c)
        )
    }
}

pub fn push_weight_record(out: &mut Vec<u8>, r: &WeightRecord) {
    out.extend_from_slice(&r.rank.to_le_bytes());
    out.extend_from_slice(&r.flags.to_le_bytes());
    for v in r.log_likelihoods.iter().chain(&r.w2c) {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

pub fn weight_records(bytes: &[u8]) -> Result<Vec<WeightRecord>, StoreError> {
    if !bytes.len().is_multiple_of(WEIGHT_RECORD) {
        return Err(StoreError("weight chunk has a partial record".into()));
    }
    Ok(bytes
        .as_chunks::<WEIGHT_RECORD>()
        .0
        .iter()
        .map(|rec| {
            let f = |i: usize| {
                let s = 12 + 8 * i;
                f64::from_le_bytes(rec[s..s + 8].try_into().unwrap())
            };
            WeightRecord {
                rank: u64::from_le_bytes(rec[..8].try_into().unwrap()),
                flags: u32::from_le_bytes(rec[8..12].try_into().unwrap()),
                log_likelihoods: [f(0), f(1), f(2)],
                w2c: [f(3), f(4), f(5)],
            }
        })
        .collect())
}

pub const DECISIONS_MAGIC: &[u8; 8] = b"QMAWDEC1";

/// No weight (topology code 3 in the decisions file).
pub const NO_DECISION: u8 = 3;

/// The winning topology of a quartet's weights (the first on equal weights)
/// and its weight quantised to 16 bits.
pub fn decision(w: Option<[f64; 3]>) -> (u8, u16) {
    match w {
        None => (NO_DECISION, 0),
        Some(w) => {
            let mut best = 0;
            for t in 1..3 {
                if w[t] > w[best] {
                    best = t;
                }
            }
            (
                best as u8,
                (w[best].clamp(0.0, 1.0) * 65_535.0).round() as u16,
            )
        }
    }
}

/// Uncompressed decisions file of weights in rank order.
pub fn decisions_to_bytes(weights: &[Option<[f64; 3]>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 3 * weights.len());
    out.extend_from_slice(DECISIONS_MAGIC);
    out.extend_from_slice(&(weights.len() as u64).to_le_bytes());
    for w in weights {
        let (t, v) = decision(*w);
        out.push(t);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Reads an uncompressed decisions file.
pub fn decisions_from_bytes(bytes: &[u8]) -> Result<Vec<(u8, u16)>, StoreError> {
    if bytes.len() < 16 || &bytes[..8] != DECISIONS_MAGIC {
        return Err(StoreError("not a quartet decisions file".into()));
    }
    let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let body = &bytes[16..];
    if body.len() != 3 * n {
        return Err(StoreError(
            "quartet decisions file has the wrong length".into(),
        ));
    }
    Ok(body
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| (c[0], u16::from_le_bytes([c[1], c[2]])))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qmaws_core::matrix::build_full;
    use qmaws_core::maw::extract;

    #[test]
    fn maw_lists_round_trip_and_damage_is_detected() {
        let set = extract(b"ACGTTGCAAGTCCTAGGATCAGT", 2, 6).unwrap();
        let bytes = maws_to_bytes(&set);
        assert_eq!(maws_from_bytes(&bytes).unwrap(), set);
        assert!(maws_from_bytes(&bytes[..bytes.len() - 1]).is_err());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(maws_from_bytes(&extra).is_err());
        assert!(maws_from_bytes(b"NOTAMAWFILE").is_err());
        let long = extract(
            format!("{}A{}", "C".repeat(40), "C".repeat(40)).as_bytes(),
            33,
            40,
        )
        .unwrap();
        assert_eq!(maws_from_bytes(&maws_to_bytes(&long)).unwrap(), long);
    }

    #[test]
    fn matrices_round_trip() {
        let sets: Vec<_> = ["AACGTA", "ACGTAG", "ACGTTC", "CATTGC", "CATGGC"]
            .iter()
            .map(|s| extract(s.as_bytes(), 2, 3).unwrap())
            .collect();
        let m = build_full(&sets, &[2, 3]);
        let bytes = matrix_to_bytes(&m);
        assert_eq!(matrix_from_bytes(&bytes).unwrap(), m);
        assert!(matrix_from_bytes(&bytes[..bytes.len() - 3]).is_err());
    }

    #[test]
    fn count_records_round_trip() {
        let mut out = Vec::new();
        let c: [u64; 16] = std::array::from_fn(|i| i as u64 * 3);
        push_count_record(&mut out, 7, &c);
        push_count_record(&mut out, 9, &[1; 16]);
        let recs = count_records(&out).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].0, 7);
        assert_eq!(recs[0].1[5], 15);
        assert_eq!(recs[1].1, [1; 16]);
        assert!(count_records(&out[..10]).is_err());
    }

    #[test]
    fn weight_records_round_trip_and_round_for_hashing() {
        let recs = [
            WeightRecord {
                rank: 12,
                flags: WEIGHT_FITTED | WEIGHT_RESAMPLED,
                log_likelihoods: [-1234.567890123, -1240.0, -1236.25],
                w2c: [0.9, 0.05, 0.05],
            },
            WeightRecord {
                rank: 3,
                flags: 0,
                log_likelihoods: [0.0; 3],
                w2c: [0.0; 3],
            },
        ];
        let mut bytes = Vec::new();
        for r in &recs {
            push_weight_record(&mut bytes, r);
        }
        assert_eq!(bytes.len(), 2 * WEIGHT_RECORD);
        assert_eq!(weight_records(&bytes).unwrap(), recs.to_vec());
        assert!(weight_records(&bytes[..WEIGHT_RECORD + 1]).is_err());
        assert_eq!(
            recs[0].canonical(),
            "12 3 -1.23456789e3 -1.24000000e3 -1.23625000e3 9.00000000e-1 5.00000000e-2 5.00000000e-2\n"
        );
        // Differences beyond 9 significant digits do not change the text.
        let mut close = recs[0];
        close.log_likelihoods[0] += 1e-7;
        assert_eq!(close.canonical(), recs[0].canonical());
    }
}
