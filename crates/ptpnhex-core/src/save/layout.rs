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

/// Byte range of the inventory record array.
///
/// The inventory is a fixed array of 4-byte records `count:u16, flag:u8,
/// index:u8` (see `docs/save-format.md`). Confirmed for Europe as
/// `0x19ce8..0x1A0E0`: the start is identical across the whole save corpus and
/// the array ends exactly where [`kaching_offset`] begins. Bounding to the real
/// array (rather than the wider region before it) is what keeps a scan from
/// matching stale bytes ahead of the list.
pub fn inventory_region(region: Region) -> Option<std::ops::Range<usize>> {
    match region {
        Region::Europe => Some(0x19CE8..0x1A0E0),
        Region::NorthAmerica | Region::Japan => None,
    }
}
