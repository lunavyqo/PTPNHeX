//! Rarepons: the special unit variants that set a Patapon's appearance.
//!
//! Each unit in the [army roster](crate::save::layout::roster_record_offset)
//! carries a **rarepon id** — the `u32` at record offset `+0x48` (see
//! `docs/save-format.md`). It is a 32-bit name-hash shared across classes, so the
//! same value is the same rarepon whether on a Yaripon, a Megapon, or any other
//! unit. A unit's name, headpiece, and base stats derive from this id rather than
//! being stored separately, so writing it is what changes a unit's rarepon.
//!
//! Only the rarepons whose codes have been observed are exposed here; the codes
//! are EU values, confirmed on hardware in both directions (writing a code
//! changes the body cross-class; recreating a unit as a Barsala reverts it).

/// `(display name, slug, code)` for each known rarepon. `code` is the raw `u32`
/// (little-endian when stored) written at unit-record offset `+0x48`.
#[rustfmt::skip]
const DEFS: [(&str, &str, u32); 7] = [
    ("Basic",   "basic",   0xFFFF_FFFF),
    ("Barsala", "barsala", 0xFFCD_FEBE),
    ("Mogyoon", "mogyoon", 0xFFC0_6E9F),
    ("Tikulee", "tikulee", 0xFFA9_6D65),
    ("Mofeel",  "mofeel",  0xFFF8_98CF),
    ("Pyokola", "pyokola", 0xFF35_6EEF),
    ("Gekolos", "gekolos", 0xFF61_E4DA),
];

/// A rarepon variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rarepon {
    index: u8,
}

impl Rarepon {
    /// All known rarepons in catalog order.
    pub fn all() -> impl Iterator<Item = Rarepon> {
        (0..DEFS.len() as u8).map(|index| Rarepon { index })
    }

    /// Looks up a rarepon by its slug (for example `"mogyoon"`).
    pub fn from_slug(slug: &str) -> Option<Rarepon> {
        DEFS.iter()
            .position(|&(_, s, _)| s == slug)
            .map(|index| Rarepon { index: index as u8 })
    }

    /// Looks up a rarepon by its raw code, if known.
    pub fn from_code(code: u32) -> Option<Rarepon> {
        DEFS.iter()
            .position(|&(_, _, c)| c == code)
            .map(|index| Rarepon { index: index as u8 })
    }

    /// Display name (for example `"Mogyoon"`).
    pub fn name(self) -> &'static str {
        DEFS[self.index as usize].0
    }

    /// Stable slug (for example `"mogyoon"`).
    pub fn slug(self) -> &'static str {
        DEFS[self.index as usize].1
    }

    /// The raw rarepon code stored at unit-record offset `+0x48`.
    pub fn code(self) -> u32 {
        DEFS[self.index as usize].2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_and_codes_round_trip_and_are_unique() {
        let mut slugs = std::collections::HashSet::new();
        let mut codes = std::collections::HashSet::new();
        for rarepon in Rarepon::all() {
            assert_eq!(Rarepon::from_slug(rarepon.slug()), Some(rarepon));
            assert_eq!(Rarepon::from_code(rarepon.code()), Some(rarepon));
            assert!(
                slugs.insert(rarepon.slug()),
                "duplicate slug {}",
                rarepon.slug()
            );
            assert!(
                codes.insert(rarepon.code()),
                "duplicate code {:#X}",
                rarepon.code()
            );
        }
        assert_eq!(slugs.len(), 7);
    }

    #[test]
    fn unknown_slug_and_code_are_none() {
        assert_eq!(Rarepon::from_slug("not-a-rarepon"), None);
        assert_eq!(Rarepon::from_code(0x1234_5678), None);
    }
}
