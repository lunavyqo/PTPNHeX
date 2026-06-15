//! Data-driven field offsets within the decrypted save payload.
//!
//! Each confirmed field is mapped per region here, so the editing layer never
//! hardcodes offsets. Findings are documented (with evidence) in
//! `docs/save-format.md`. Regions whose offset is not yet confirmed return
//! `None` rather than guessing.

use crate::save::Region;

/// The maximum ka-ching (currency) the game supports.
pub const KACHING_MAX: u32 = 99_999;

/// Offset of the ka-ching `u32` (little-endian) for `region`.
///
/// Confirmed for Europe at `0x1A0EC` against two independent save values.
pub fn kaching_offset(region: Region) -> Option<usize> {
    match region {
        Region::Europe => Some(0x1A0EC),
        // US/JP layouts not yet reverse-engineered.
        Region::NorthAmerica | Region::Japan => None,
    }
}

/// Fixed byte offsets of the 20 materials' inventory records, in canonical
/// order (Leather Meat … Magic Alloy), for `region`.
///
/// The inventory is a *fixed table*: every item has a stable offset, and only
/// the record's owned flag and count change between saves (see
/// `docs/save-format.md`). Each record is `count:u8, new:u8, owned:u8,
/// display-index:u8`. Confirmed for Europe against the full corpus and a
/// controlled before/after on hardware (obtaining Magic Alloy flips the flag at
/// `0x19DA4` in place). The 20 material records are contiguous from `0x19D54`
/// except for one non-material slot at `0x19D74`.
pub fn material_offsets(region: Region) -> Option<&'static [usize; 20]> {
    match region {
        Region::Europe => Some(&EU_MATERIAL_OFFSETS),
        Region::NorthAmerica | Region::Japan => None,
    }
}

#[rustfmt::skip]
const EU_MATERIAL_OFFSETS: [usize; 20] = [
    0x19D54, 0x19D58, 0x19D5C, 0x19D60, 0x19D64, 0x19D68, 0x19D6C, 0x19D70,
    0x19D78, 0x19D7C, 0x19D80, 0x19D84, 0x19D88, 0x19D8C, 0x19D90, 0x19D94,
    0x19D98, 0x19D9C, 0x19DA0, 0x19DA4,
];

/// Fixed byte offsets of the 19 key items (drums, miracles, songs, and quest
/// items), in catalog order, for `region`.
///
/// Same fixed-table record as [`material_offsets`]; these are the head records
/// *before* the materials. They are one-per unlock tokens — only the owned flag
/// matters, and flipping it genuinely unlocks the token in-game (hardware
/// confirmed). Mapped for Europe by a distinct-count readback. Only these 19 are
/// exposed: the records after them in the head block are never-owned/unused and
/// forcing them owned freezes the altar.
pub fn key_item_offsets(region: Region) -> Option<&'static [usize; 19]> {
    match region {
        Region::Europe => Some(&EU_KEY_ITEM_OFFSETS),
        Region::NorthAmerica | Region::Japan => None,
    }
}

// Paired index-for-index with `key_items::DEFS`, so the offsets are grouped by
// category (drums, miracles, songs, key items) rather than strictly ascending.
#[rustfmt::skip]
const EU_KEY_ITEM_OFFSETS: [usize; 19] = [
    0x19CE8, 0x19CEC, 0x19CF0, 0x19CF4, // Pon / Pata / Chaka / Don Drum
    0x19CF8, 0x19CFC, 0x19D00, 0x19D04, // Rain / Tailwind / Storm / Earthquake Miracle
    0x19D10, 0x19D24, 0x19D28, 0x19D2C, 0x19D30, // Ponpata / Patapata / Ponpon / Chakachaka / Ponchaka Song
    0x19D08, 0x19D0C, 0x19D14, 0x19D18, 0x19D1C, 0x19D20, // Blank Map / Bent Compass / Dusty Crystal / Broken Sign / Black Star / Dark Palace Model
];

/// Fixed byte offsets of the 83 inventory items (stews, Memories, and the
/// weapon/gear armory), in catalog order, for `region`.
///
/// Same fixed-table record as [`material_offsets`]; these are the slots after
/// the materials. Mapped for Europe by writing each slot a distinct count and
/// reading the result back in-game (see `docs/save-format.md`). The slots are
/// not contiguous — unused/never-obtained slots sit between the categories.
pub fn item_offsets(region: Region) -> Option<&'static [usize; 83]> {
    match region {
        Region::Europe => Some(&EU_ITEM_OFFSETS),
        Region::NorthAmerica | Region::Japan => None,
    }
}

#[rustfmt::skip]
const EU_ITEM_OFFSETS: [usize; 83] = [
    0x19DA8, 0x19DAC, 0x19DB0, 0x19DB4, 0x19DB8, 0x19DBC, 0x19DC0, 0x19DC4,
    0x19DC8, 0x19DCC, 0x19E28, 0x19E2C, 0x19E30, 0x19E34, 0x19E38, 0x19E3C,
    0x19E40, 0x19E44, 0x19E50, 0x19E54, 0x19E58, 0x19E5C, 0x19E60, 0x19E64,
    0x19E68, 0x19E6C, 0x19E70, 0x19E78, 0x19E80, 0x19E84, 0x19E88, 0x19E8C,
    0x19E90, 0x19E94, 0x19EA0, 0x19EA8, 0x19EAC, 0x19EB0, 0x19EB4, 0x19EB8,
    0x19EBC, 0x19EC8, 0x19ECC, 0x19ED0, 0x19ED4, 0x19ED8, 0x19EDC, 0x19EE0,
    0x19EE4, 0x19EF0, 0x19EF8, 0x19EFC, 0x19F00, 0x19F04, 0x19F08, 0x19F0C,
    0x19F18, 0x19F1C, 0x19F20, 0x19F24, 0x19F28, 0x19F2C, 0x19F30, 0x19F34,
    0x19F40, 0x19F44, 0x19F48, 0x19F4C, 0x19F50, 0x19F54, 0x19F5C, 0x19F68,
    0x19F6C, 0x19F70, 0x19F74, 0x19F78, 0x19F7C, 0x19F80, 0x19F84, 0x19F88,
    0x19FC8, 0x19FCC, 0x19FD0,
];
