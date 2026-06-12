//! KIRK cryptographic primitives used by the save-data scheme.
//!
//! Everything reduces to AES-128 keyed by a value from the KIRK key vault.
//! See `docs/crypto.md` for the algorithm and the provenance of these
//! constants (public PSP reverse-engineering facts).

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use aes::Aes128;
use cmac::{Cmac, Mac};

/// A 16-byte block.
pub type Block = [u8; 16];

/// KIRK key-vault entries used by the mode-5 save-data paths.
pub mod slot {
    use super::Block;
    /// Slot `0x03` — params hash, mode 1.
    pub const K03: Block = [
        0x98, 0x02, 0xC4, 0xE6, 0xEC, 0x9E, 0x9E, 0x2F, 0xFC, 0x63, 0x4C, 0xE4, 0x2F, 0xBB, 0x46,
        0x68,
    ];
    /// Slot `0x10` — file hash and params hash, mode 5.
    pub const K10: Block = [
        0x32, 0x29, 0x5B, 0xD5, 0xEA, 0xF7, 0xA3, 0x42, 0x16, 0xC8, 0x8E, 0x48, 0xFF, 0x50, 0xD3,
        0x71,
    ];
    // Slot 0x11 (params hash, mode 6) is intentionally omitted: that hash
    // needs a KIRK fuse operation we cannot reproduce. See docs/crypto.md.
    /// Slot `0x12` — cipher key derivation.
    pub const K12: Block = [
        0x5D, 0xC7, 0x11, 0x39, 0xD0, 0x19, 0x38, 0xBC, 0x02, 0x7F, 0xDD, 0xDC, 0xB0, 0x83, 0x7D,
        0x9D,
    ];
    /// Slot `0x64` — cipher keystream.
    pub const K64: Block = [
        0x03, 0xB3, 0x02, 0xE8, 0x5F, 0xF3, 0x81, 0xB1, 0x3B, 0x8D, 0xAA, 0x2A, 0x90, 0xFF, 0x5E,
        0x61,
    ];
}

/// Encrypts one block in place with AES-128 (ECB).
pub fn aes_encrypt_block(key: &Block, block: &mut Block) {
    let cipher = Aes128::new(key.into());
    cipher.encrypt_block(block.into());
}

/// AES-128-CBC decryption with a zero IV over `data` (length a multiple of 16).
///
/// A single-block call is therefore plain ECB decryption.
pub fn aes_cbc_decrypt_zero_iv(key: &Block, data: &[u8]) -> Vec<u8> {
    debug_assert_eq!(data.len() % 16, 0);
    let cipher = Aes128::new(key.into());
    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; 16];
    for chunk in data.chunks_exact(16) {
        let mut block: Block = chunk.try_into().expect("chunk is 16 bytes");
        let ciphertext = block;
        cipher.decrypt_block((&mut block).into());
        for (b, p) in block.iter_mut().zip(prev.iter()) {
            *b ^= *p;
        }
        out.extend_from_slice(&block);
        prev = ciphertext;
    }
    out
}

/// AES-128-CMAC over `data`.
pub fn aes_cmac(key: &Block, data: &[u8]) -> Block {
    let mut mac =
        <Cmac<Aes128> as KeyInit>::new_from_slice(key).expect("AES-128 key length is valid");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// XORs `b` into `a` (both 16 bytes).
pub fn xor(a: &mut Block, b: &Block) {
    for (x, y) in a.iter_mut().zip(b.iter()) {
        *x ^= *y;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn cbc_single_block_is_ecb() {
        // Decrypt then encrypt round-trips a single block under zero IV.
        let key = slot::K10;
        let ct = [0x11u8; 16];
        let pt = aes_cbc_decrypt_zero_iv(&key, &ct);
        let mut back: Block = pt.as_slice().try_into().unwrap();
        aes_encrypt_block(&key, &mut back);
        assert_eq!(back, ct);
    }

    #[test]
    fn cmac_is_deterministic() {
        let a = aes_cmac(&slot::K10, b"hello world 1234");
        let b = aes_cmac(&slot::K10, b"hello world 1234");
        assert_eq!(a, b);
        let c = aes_cmac(&slot::K10, b"hello world 1235");
        assert_ne!(a, c);
    }
}
