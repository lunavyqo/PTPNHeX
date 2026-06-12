//! Core library for PTPNHEX, a save editor for Patapon (PSP).
//!
//! This crate contains everything that is independent of a user interface:
//! `PARAM.SFO` parsing and serialization, save-data encryption and
//! decryption, and a typed editing model over the decrypted save payload.
//! The `ptpnhex-cli` and `ptpnhex-gui` crates are thin layers over this API.

pub mod codec;
pub mod crypto;
pub mod error;
pub mod keys;
pub mod save;
pub mod sfo;

pub use error::Error;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
