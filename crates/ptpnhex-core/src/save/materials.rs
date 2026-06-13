//! Crafting materials.
//!
//! Materials live in the inventory record array (see `docs/save-format.md`),
//! not at fixed offsets. Each record is `count:u16, flag:u8, index:u8`; a
//! material is the record whose `flag` marks it owned and whose `index` is in
//! `0x13..=0x26`. Records are kept in acquisition order, so a material is
//! located by its stable `index`, never by a fixed offset.

/// The highest count a material slot displays (counts are shown two-digit).
pub const MATERIAL_MAX: u32 = 99;

/// `(display name, slug)` for each material, in item-id order (ids `0x13`..`0x26`).
const DEFS: [(&str, &str); 20] = [
    ("Leather Meat", "leather-meat"),
    ("Tender Meat", "tender-meat"),
    ("Dream Meat", "dream-meat"),
    ("Mystery Meat", "mystery-meat"),
    ("Stone", "stone"),
    ("Hard Iron", "hard-iron"),
    ("Tytanium Ore", "tytanium-ore"),
    ("Mytheerial", "mytheerial"),
    ("Banal Branch", "banal-branch"),
    ("Cherry Tree", "cherry-tree"),
    ("Hinoki", "hinoki"),
    ("Super Cedar", "super-cedar"),
    ("Eyeball Cabbage", "eyeball-cabbage"),
    ("Crying Carrot", "crying-carrot"),
    ("Predator Pumpkin", "predator-pumpkin"),
    ("Hazy Shroom", "hazy-shroom"),
    ("Sloppy Alloy", "sloppy-alloy"),
    ("Hard Alloy", "hard-alloy"),
    ("Awesome Alloy", "awesome-alloy"),
    ("Magic Alloy", "magic-alloy"),
];

/// Inventory `index` of the first material (`0x13`); the 20 materials occupy
/// the consecutive indices `0x13..=0x26`.
const FIRST_INDEX: u8 = 0x13;

/// A crafting material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Material {
    index: u8,
}

impl Material {
    /// All 20 materials in item-id order.
    pub fn all() -> impl Iterator<Item = Material> {
        (0..DEFS.len() as u8).map(|index| Material { index })
    }

    /// Looks up a material by its slug (for example `"hard-alloy"`).
    pub fn from_slug(slug: &str) -> Option<Material> {
        DEFS.iter()
            .position(|&(_, s)| s == slug)
            .map(|index| Material { index: index as u8 })
    }

    /// Display name (for example `"Hard Alloy"`).
    pub fn name(self) -> &'static str {
        DEFS[self.index as usize].0
    }

    /// Stable slug (for example `"hard-alloy"`).
    pub fn slug(self) -> &'static str {
        DEFS[self.index as usize].1
    }

    /// The stable inventory `index` byte identifying this material
    /// (`0x13..=0x26`).
    pub fn index(self) -> u8 {
        FIRST_INDEX + self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_round_trip_and_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for m in Material::all() {
            assert_eq!(Material::from_slug(m.slug()), Some(m));
            assert!(seen.insert(m.slug()), "duplicate slug {}", m.slug());
        }
        assert_eq!(seen.len(), 20);
    }

    #[test]
    fn indices_match_the_reverse_engineered_range() {
        let ids: Vec<u8> = Material::all().map(|m| m.index()).collect();
        assert_eq!(ids.first(), Some(&0x13));
        assert_eq!(ids.last(), Some(&0x26));
        // The 20 materials occupy consecutive indices with no gaps.
        assert!(ids.windows(2).all(|w| w[1] == w[0] + 1));
    }

    #[test]
    fn unknown_slug_is_none() {
        assert_eq!(Material::from_slug("not-a-material"), None);
    }
}
