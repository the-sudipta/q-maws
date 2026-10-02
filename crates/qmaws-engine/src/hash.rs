//! SHA-256 helpers with lowercase hexadecimal output.

use sha2::{Digest, Sha256};

/// SHA-256 of `bytes` as 64 lowercase hexadecimal characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex(&hasher.finalize())
}

/// Incremental SHA-256 for data that arrives in pieces.
#[derive(Clone, Default)]
pub struct StreamHasher {
    inner: Sha256,
}

impl StreamHasher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }

    /// Finishes the hash and returns it as lowercase hexadecimal.
    pub fn finish_hex(self) -> String {
        to_hex(&self.inner.finalize())
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_digests() {
        // FIPS 180-2 test vectors.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn streaming_equals_one_shot() {
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let mut h = StreamHasher::new();
        for piece in data.chunks(997) {
            h.update(piece);
        }
        assert_eq!(h.finish_hex(), sha256_hex(&data));
    }
}
