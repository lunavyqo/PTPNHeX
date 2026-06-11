//! Typed model over the decrypted save payload.
//!
//! The decryption and editing model are built up over later milestones; for
//! now this module defines the [`Region`] selector shared by the key and
//! cryptography layers.

mod region;

pub use region::Region;
