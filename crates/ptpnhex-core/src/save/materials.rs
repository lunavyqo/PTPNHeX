//! Crafting materials.
//!
//! Materials live in the inventory table (see `docs/save-format.md`). Every
//! item has a stable byte offset there; a material is identified by its
//! canonical [`position`](Material::position) (0–19, Leather Meat … Magic
//! Alloy), which maps to a fixed offset via
//! [`layout::material_offsets`](crate::save::layout::material_offsets).

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

    /// This material's canonical position (0–19, Leather Meat … Magic Alloy),
    /// used to index [`layout::material_offsets`](crate::save::layout::material_offsets).
    pub fn position(self) -> usize {
        self.index as usize
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
    fn positions_are_zero_based_and_dense() {
        let positions: Vec<usize> = Material::all().map(|m| m.position()).collect();
        assert_eq!(positions, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn unknown_slug_is_none() {
        assert_eq!(Material::from_slug("not-a-material"), None);
    }
}
