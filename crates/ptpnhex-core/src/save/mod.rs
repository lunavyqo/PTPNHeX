//! Typed model over the decrypted save payload.
//!
//! [`Region`] selects a layout; [`layout`] maps confirmed fields to offsets.
//! Editing accessors live on [`crate::SaveSlot`].

pub mod layout;
pub mod materials;
mod region;

pub use materials::Material;
pub use region::Region;
