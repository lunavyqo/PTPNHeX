//! Crafting materials.
//!
//! Materials are stored as entries in a variable inventory list (see
//! `docs/save-format.md`), not at fixed offsets: each entry is a `u32`
//! holding a `u16` count and a `u16` item id, and the list only contains
//! items the player has obtained, in acquisition order. A material is
//! therefore located by scanning the inventory region for its item id.

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

/// First material item id (`0x1301`); subsequent materials step by `0x100`.
const FIRST_ITEM_ID: u16 = 0x1301;

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

    /// The inventory item id identifying this material.
    pub fn item_id(self) -> u16 {
        FIRST_ITEM_ID + (self.index as u16) * 0x100
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
    fn item_ids_match_the_reverse_engineered_range() {
        let ids: Vec<u16> = Material::all().map(|m| m.item_id()).collect();
        assert_eq!(ids.first(), Some(&0x1301));
        assert_eq!(ids.last(), Some(&0x2601));
    }

    #[test]
    fn unknown_slug_is_none() {
        assert_eq!(Material::from_slug("not-a-material"), None);
    }
}
