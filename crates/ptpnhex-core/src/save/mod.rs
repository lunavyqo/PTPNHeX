//! Typed model over the decrypted save payload.
//!
//! [`Region`] selects a layout; [`layout`] maps confirmed fields to offsets.
//! Editing accessors live on [`crate::SaveSlot`].

pub mod layout;
mod region;

pub use region::Region;
