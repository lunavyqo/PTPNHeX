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
