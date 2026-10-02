//! A toy computation that exercises the engine: chunks, checkpoints, resume,
//! progress and estimates, without any science.
//!
//! The work is split into blocks. Block `b` sums `splitmix64(seed + i)` over
//! the `BLOCK_SIZE` counter values `i` of that block (wrapping addition). Each
//! block produces 8 bytes, so the output of a run is the same whatever the
//! chunk boundaries are.

/// Counter values per block.
pub const BLOCK_SIZE: u64 = 1 << 20;

/// The SplitMix64 output function (Steele, Lea and Flood, 2014).
pub fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Result of block `block` for `seed`.
pub fn block_sum(seed: u64, block: u64) -> u64 {
    let start = block * BLOCK_SIZE;
    (start..start + BLOCK_SIZE).fold(0u64, |acc, i| {
        acc.wrapping_add(splitmix64(seed.wrapping_add(i)))
    })
}

/// Output bytes of blocks `[start, end)`: 8 bytes per block, little-endian.
pub fn chunk_payload(seed: u64, start: u64, end: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(((end - start) * 8) as usize);
    for b in start..end {
        out.extend_from_slice(&block_sum(seed, b).to_le_bytes());
    }
    out
}

/// Wrapping sum of all block results in a payload.
pub fn payload_total(payload: &[u8]) -> u64 {
    payload
        .as_chunks::<8>()
        .0
        .iter()
        .fold(0u64, |acc, c| acc.wrapping_add(u64::from_le_bytes(*c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_reference_values() {
        // First outputs of the reference SplitMix64 generator seeded with 0:
        // state advances by the golden gamma before each output.
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(0x9E37_79B9_7F4A_7C15), 0x6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn payload_does_not_depend_on_chunk_boundaries() {
        let seed = 42;
        let whole = chunk_payload(seed, 0, 6);
        let mut parts = chunk_payload(seed, 0, 2);
        parts.extend(chunk_payload(seed, 2, 5));
        parts.extend(chunk_payload(seed, 5, 6));
        assert_eq!(whole, parts);
        assert_eq!(whole.len(), 48);
    }

    #[test]
    fn different_seeds_give_different_results() {
        assert_ne!(block_sum(1, 0), block_sum(2, 0));
    }
}
