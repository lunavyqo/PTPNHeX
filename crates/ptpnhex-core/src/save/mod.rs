//! Typed model over the decrypted save payload.
//!
//! [`Region`] selects a layout; [`layout`] maps confirmed fields to offsets.
//! Editing accessors live on [`crate::SaveSlot`].

pub mod bonus_patapon;
pub mod items;
pub mod key_items;
pub mod layout;
pub mod materials;
pub mod rarepon;
mod region;

pub use bonus_patapon::BonusPatapon;
pub use items::Item;
pub use key_items::KeyItem;
pub use materials::Material;
pub use rarepon::Rarepon;
pub use region::Region;
