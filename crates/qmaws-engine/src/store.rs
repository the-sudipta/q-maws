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

use qmaws_core::matrix::Matrix;
use qmaws_core::maw::{Codes, MawSet};

pub const MAW_MAGIC: &[u8; 8] = b"QMAWMAW1";
pub const MATRIX_MAGIC: &[u8; 8] = b"QMAWMAT1";
/// Bytes per quartet record in a count chunk.
pub const COUNT_RECORD: usize = 8 + 16 * 4;

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
}
