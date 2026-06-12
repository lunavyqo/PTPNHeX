//! The AES-CMAC integrity hashes the PSP verifies.

use super::kirk::{self, slot, Block};

const HASH19BC: Block = [
    0xCB, 0x15, 0xF4, 0x07, 0xF9, 0x6A, 0x52, 0x3C, 0x04, 0xB9, 0xB2, 0xEE, 0x5C, 0x53, 0xFA, 0x86,
];

/// Computes the `SAVEDATA_FILE_LIST` per-file hash for an encrypted
/// `SECURE.BIN` (stored in `PARAM.SFO`, and checked by the firmware on load).
pub fn file_list_hash(secure_bin: &[u8], gamekey: &Block) -> Block {
    let aligned_len = (secure_bin.len() + 15) & !15;
    let mut data = secure_bin.to_vec();
    data.resize(aligned_len, 0);

    let mut h = kirk::aes_cmac(&slot::K10, &data);
    kirk::xor(&mut h, &HASH19BC);
    let mut mixed = *gamekey;
    kirk::xor(&mut mixed, &h);
    kirk::aes_encrypt_block(&slot::K10, &mut mixed);
    mixed
}

/// A reproducible `SAVEDATA_PARAMS` hash field.
///
/// The `+0x20` (mode 6) field is intentionally absent: it requires a KIRK
/// "fuse" operation that cannot be reproduced in software (see `docs/crypto.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamsHashField {
    /// `SAVEDATA_PARAMS + 0x10` (chnnlsv mode 1).
    Hash10,
    /// `SAVEDATA_PARAMS + 0x70` (chnnlsv mode 5).
    Hash70,
}

/// Computes a `SAVEDATA_PARAMS` hash over a prepared `PARAM.SFO` image.
///
/// `param_sfo` must be the full `PARAM.SFO` (a multiple of 16 bytes) with the
/// target hash field already zeroed and any earlier-computed fields written,
/// matching the firmware's computation order (see `docs/crypto.md`).
pub fn params_hash(param_sfo: &[u8], field: ParamsHashField) -> Block {
    match field {
        ParamsHashField::Hash10 => kirk::aes_cmac(&slot::K03, param_sfo),
        ParamsHashField::Hash70 => {
            let mut h = kirk::aes_cmac(&slot::K10, param_sfo);
            kirk::xor(&mut h, &HASH19BC);
            h
        }
    }
}
