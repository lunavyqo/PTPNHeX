//! The mode-5 `SECURE.BIN` keystream cipher.

use super::kirk::{self, slot, Block};
use crate::{Error, Result};

/// Length of the per-save header prepended to an encrypted `SECURE.BIN`.
pub const SECURE_HEADER_LEN: usize = 16;

/// Fixed mask XORed into the seed before the keyslot-`0x12` decrypt.
const SEED_PRE_XOR: Block = [
    0xEC, 0x6D, 0x29, 0x59, 0x26, 0x35, 0xA5, 0x7F, 0x97, 0x2A, 0x0D, 0xBC, 0xA3, 0x26, 0x33, 0x00,
];
/// Fixed mask XORed into the seed after the keyslot-`0x12` decrypt.
const SEED_POST_XOR: Block = [
    0x70, 0x44, 0xA3, 0xAE, 0xEF, 0x5D, 0xA5, 0xF2, 0x85, 0x7F, 0xF2, 0xD6, 0x94, 0xF5, 0x36, 0x3B,
];

/// Derives the keystream for a body of `aligned_len` bytes (a multiple of 16)
/// from the 16-byte header and the game key.
fn keystream(header: &Block, gamekey: &Block, aligned_len: usize) -> Vec<u8> {
    let mut seed = *header;
    kirk::xor(&mut seed, gamekey);
    kirk::xor(&mut seed, &SEED_PRE_XOR);
    let decrypted = kirk::aes_cbc_decrypt_zero_iv(&slot::K12, &seed);
    let mut seed: Block = decrypted.try_into().expect("single block");
    kirk::xor(&mut seed, &SEED_POST_XOR);

    let mut counters = Vec::with_capacity(aligned_len);
    for k in 0..(aligned_len / 16) as u32 {
        counters.extend_from_slice(&seed[..12]);
        counters.extend_from_slice(&(k + 1).to_le_bytes());
    }
    kirk::aes_cbc_decrypt_zero_iv(&slot::K64, &counters)
}

fn apply_keystream(data: &[u8], header: &Block, gamekey: &Block) -> Vec<u8> {
    let aligned_len = (data.len() + 15) & !15;
    let ks = keystream(header, gamekey, aligned_len);
    data.iter().zip(ks.iter()).map(|(a, b)| a ^ b).collect()
}

/// Decrypts an encrypted `SECURE.BIN` blob to its plaintext payload.
pub fn decrypt_secure(blob: &[u8], gamekey: &Block) -> Result<Vec<u8>> {
    if blob.len() <= SECURE_HEADER_LEN {
        return Err(Error::Malformed {
            what: "SECURE.BIN",
            reason: format!("too short: {} bytes", blob.len()),
        });
    }
    let header: Block = blob[..SECURE_HEADER_LEN].try_into().expect("16 bytes");
    Ok(apply_keystream(
        &blob[SECURE_HEADER_LEN..],
        &header,
        gamekey,
    ))
}

/// Encrypts a plaintext payload into a `SECURE.BIN` blob using the given
/// 16-byte header.
///
/// Because the cipher is a keystream, reusing the original header reproduces
/// the original ciphertext exactly; a fresh save supplies a new random header.
pub fn encrypt_secure(plaintext: &[u8], header: &Block, gamekey: &Block) -> Vec<u8> {
    let mut out = Vec::with_capacity(SECURE_HEADER_LEN + plaintext.len());
    out.extend_from_slice(header);
    out.extend_from_slice(&apply_keystream(plaintext, header, gamekey));
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_is_the_inverse_of_decrypt() {
        let gamekey = [0x42u8; 16];
        let header = [0x7Au8; 16];
        let plaintext: Vec<u8> = (0..1000u32).map(|i| (i * 7) as u8).collect();
        let blob = encrypt_secure(&plaintext, &header, &gamekey);
        assert_eq!(blob.len(), 16 + plaintext.len());
        let back = decrypt_secure(&blob, &gamekey).unwrap();
        assert_eq!(back, plaintext);
    }

    #[test]
    fn rejects_short_blob() {
        assert!(decrypt_secure(&[0u8; 16], &[0u8; 16]).is_err());
    }
}
