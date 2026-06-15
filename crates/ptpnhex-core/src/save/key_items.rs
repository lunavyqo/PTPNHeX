//! Key items: the altar's drums, miracles, songs, and quest items.
//!
//! These live at the very start of the inventory table, *before* the
//! [materials](crate::save::Material), and use the same record format
//! `count:u8, new:u8, owned:u8, display-index:u8` (see `docs/save-format.md`).
//! Unlike materials and items they are **one-per** unlock tokens: only the owned
//! flag is meaningful (count is always 1 in legitimate saves), and flipping it
//! genuinely unlocks the token in-game — confirmed on hardware, where flagging a
//! never-obtained Earthquake Miracle owned made it performable in a mission.
//!
//! A key item is identified by its canonical [`position`](KeyItem::position),
//! which maps to a fixed offset via
//! [`layout::key_item_offsets`](crate::save::layout::key_item_offsets).
//!
//! Only the 19 tokens mapped here are exposed: the records after them in the
//! head block are never-owned/unused, and forcing them owned freezes the altar.

/// `(display name, slug, category)` for each key item, grouped by category for
/// clean listings. The order matches
/// [`layout::key_item_offsets`](crate::save::layout::key_item_offsets) element
/// for element (the two arrays are paired by index), so the offsets there are
/// listed in this same order rather than ascending.
#[rustfmt::skip]
const DEFS: [(&str, &str, &str); 19] = [
    ("Pon Drum", "pon-drum", "Drum"),
    ("Pata Drum", "pata-drum", "Drum"),
    ("Chaka Drum", "chaka-drum", "Drum"),
    ("Don Drum", "don-drum", "Drum"),
    ("Rain Miracle", "rain-miracle", "Miracle"),
    ("Tailwind Miracle", "tailwind-miracle", "Miracle"),
    ("Storm Miracle", "storm-miracle", "Miracle"),
    ("Earthquake Miracle", "earthquake-miracle", "Miracle"),
    ("Ponpata Song", "ponpata-song", "Song"),
    ("Patapata Song", "patapata-song", "Song"),
    ("Ponpon Song", "ponpon-song", "Song"),
    ("Chakachaka Song", "chakachaka-song", "Song"),
    ("Ponchaka Song", "ponchaka-song", "Song"),
    ("Blank Map", "blank-map", "Key Item"),
    ("Bent Compass", "bent-compass", "Key Item"),
    ("Dusty Crystal", "dusty-crystal", "Key Item"),
    ("Broken Sign", "broken-sign", "Key Item"),
    ("Black Star", "black-star", "Key Item"),
    ("Dark Palace Model", "dark-palace-model", "Key Item"),
];

/// A key item: a drum, miracle, song, or quest item from the altar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyItem {
    index: u8,
}

impl KeyItem {
    /// All key items in catalog order.
    pub fn all() -> impl Iterator<Item = KeyItem> {
        (0..DEFS.len() as u8).map(|index| KeyItem { index })
    }

    /// Looks up a key item by its slug (for example `"earthquake-miracle"`).
    pub fn from_slug(slug: &str) -> Option<KeyItem> {
        DEFS.iter()
            .position(|&(_, s, _)| s == slug)
            .map(|index| KeyItem { index: index as u8 })
    }

    /// Display name (for example `"Earthquake Miracle"`).
    pub fn name(self) -> &'static str {
        DEFS[self.index as usize].0
    }

    /// Stable slug (for example `"earthquake-miracle"`).
    pub fn slug(self) -> &'static str {
        DEFS[self.index as usize].1
    }

    /// Category label (`"Drum"`, `"Miracle"`, `"Song"`, or `"Key Item"`), for
    /// grouping in listings.
    pub fn category(self) -> &'static str {
        DEFS[self.index as usize].2
    }

    /// This key item's canonical position, used to index
    /// [`layout::key_item_offsets`](crate::save::layout::key_item_offsets).
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
        for key_item in KeyItem::all() {
            assert_eq!(KeyItem::from_slug(key_item.slug()), Some(key_item));
            assert!(
                seen.insert(key_item.slug()),
                "duplicate slug {}",
                key_item.slug()
            );
        }
        assert_eq!(seen.len(), 19);
    }

    #[test]
    fn positions_are_zero_based_and_dense() {
        let positions: Vec<usize> = KeyItem::all().map(|k| k.position()).collect();
        assert_eq!(positions, (0..19).collect::<Vec<_>>());
    }

    #[test]
    fn unknown_slug_is_none() {
        assert_eq!(KeyItem::from_slug("not-a-key-item"), None);
    }
}
