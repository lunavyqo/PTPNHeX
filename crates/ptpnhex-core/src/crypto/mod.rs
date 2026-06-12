//! Save-data (`SECURE.BIN`) cryptography for Patapon (mode 5).
//!
//! This module implements the keystream cipher and the AES-CMAC integrity
//! hashes documented in `docs/crypto.md`, verified byte-for-byte against a
//! corpus of real saves. The functions take the 16-byte game key as a
//! parameter; key storage lives in [`crate::keys`].

mod cipher;
mod hash;
mod kirk;

pub use cipher::{decrypt_secure, encrypt_secure, SECURE_HEADER_LEN};
pub use hash::{file_list_hash, params_hash, ParamsHashField};
pub use kirk::Block;
