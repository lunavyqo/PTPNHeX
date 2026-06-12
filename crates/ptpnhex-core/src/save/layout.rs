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

/// Byte range of the inventory list (where material entries are scanned for).
///
/// Confirmed for Europe as `0x19000..0x1A0E0` — after the unit/equipment
/// array and before [`kaching_offset`]; verified to contain no false material
/// matches across the save corpus.
pub fn inventory_region(region: Region) -> Option<std::ops::Range<usize>> {
    match region {
        Region::Europe => Some(0x19000..0x1A0E0),
        Region::NorthAmerica | Region::Japan => None,
    }
}
