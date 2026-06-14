//! Inventory items: stews, unit Memories, and the weapon/gear armory.
//!
//! These live in the same inventory table as crafting [materials](crate::save::Material)
//! and share the record format `count:u8, new:u8, owned:u8, display-index:u8`
//! (see `docs/save-format.md`). They occupy the slots after the materials; an
//! item is identified by its canonical [`position`](Item::position), which maps
//! to a fixed offset via [`layout::item_offsets`](crate::save::layout::item_offsets).
//!
//! The catalog was mapped by writing each slot a distinct count and reading the
//! result back in-game.

/// The highest count an item slot is set to (counts are two-digit, as for
/// materials).
pub const ITEM_MAX: u32 = 99;

/// `(display name, slug, category)` for each item, in catalog order. The order
/// matches [`layout::item_offsets`](crate::save::layout::item_offsets).
#[rustfmt::skip]
const DEFS: [(&str, &str, &str); 83] = [
    ("Gnarly Stew", "gnarly-stew", "Stew"),
    ("Tasty Stew", "tasty-stew", "Stew"),
    ("King's Stew", "kings-stew", "Stew"),
    ("Divine Stew", "divine-stew", "Stew"),
    ("Yaripon's Memory", "yaripons-memory", "Memory"),
    ("Tatepon's Memory", "tatepons-memory", "Memory"),
    ("Yumipon's Memory", "yumipons-memory", "Memory"),
    ("Kibapon's Memory", "kibapons-memory", "Memory"),
    ("Dekapon's Memory", "dekapons-memory", "Memory"),
    ("Megapon's Memory", "megapons-memory", "Memory"),
    ("Wooden Spear", "wooden-spear", "Spear"),
    ("Iron Spear", "iron-spear", "Spear"),
    ("Steel Spear", "steel-spear", "Spear"),
    ("Scorching Spear", "scorching-spear", "Spear"),
    ("Dokaknel's Fang", "dokaknels-fang", "Spear"),
    ("Ancient Spear", "ancient-spear", "Spear"),
    ("Giant Spear \"Bullet\"", "giant-spear-bullet", "Spear"),
    ("Divine Spear", "divine-spear", "Spear"),
    ("Tin Axe", "tin-axe", "Sword"),
    ("Iron Sword", "iron-sword", "Sword"),
    ("Steel Axe", "steel-axe", "Sword"),
    ("Sleep Sword", "sleep-sword", "Sword"),
    ("Flame Sword", "flame-sword", "Sword"),
    ("Ancient Axe", "ancient-axe", "Sword"),
    ("Ancient Sword \"The Butcher\"", "ancient-sword-the-butcher", "Sword"),
    ("Divine Sword", "divine-sword", "Sword"),
    ("Gong's Scythe", "gongs-scythe", "Scythe"),
    ("Wood Shield", "wood-shield", "Shield"),
    ("Steel Shield", "steel-shield", "Shield"),
    ("Ice Shield", "ice-shield", "Shield"),
    ("Ultra Heavy Shield", "ultra-heavy-shield", "Shield"),
    ("Ancient Shield", "ancient-shield", "Shield"),
    ("Giant Shield \"Octagon\"", "giant-shield-octagon", "Shield"),
    ("Divine Shield", "divine-shield", "Shield"),
    ("Wooden Bow", "wooden-bow", "Bow"),
    ("Steel Bow", "steel-bow", "Bow"),
    ("Flame Bow", "flame-bow", "Bow"),
    ("Piercing Bow", "piercing-bow", "Bow"),
    ("Ancient Bow", "ancient-bow", "Bow"),
    ("Giant Bow \"Failnaught\"", "giant-bow-failnaught", "Bow"),
    ("Divine Bow", "divine-bow", "Bow"),
    ("Wooden Halberd", "wooden-halberd", "Halberd"),
    ("Iron Halberd", "iron-halberd", "Halberd"),
    ("Steel Halberd", "steel-halberd", "Halberd"),
    ("Deflecting Halberd", "deflecting-halberd", "Halberd"),
    ("Flame Halberd", "flame-halberd", "Halberd"),
    ("Ancient Halberd", "ancient-halberd", "Halberd"),
    ("Giant Halberd \"Grizzly\"", "giant-halberd-grizzly", "Halberd"),
    ("Divine Halberd", "divine-halberd", "Halberd"),
    ("Horse", "horse", "Horse"),
    ("Tough Horse", "tough-horse", "Horse"),
    ("Strong Horse", "strong-horse", "Horse"),
    ("Crimson Horse", "crimson-horse", "Horse"),
    ("Ancient Horse", "ancient-horse", "Horse"),
    ("Deep Impact", "deep-impact", "Horse"),
    ("Divine Horse", "divine-horse", "Horse"),
    ("Club", "club", "Hammer"),
    ("Iron Hammer", "iron-hammer", "Hammer"),
    ("Steel Mace", "steel-mace", "Hammer"),
    ("Nail Studded Bat", "nail-studded-bat", "Hammer"),
    ("Dream Weaver", "dream-weaver", "Hammer"),
    ("Ancient Hammer", "ancient-hammer", "Hammer"),
    ("Morning Star \"Giganto\"", "morning-star-giganto", "Hammer"),
    ("Divine Axe", "divine-axe", "Hammer"),
    ("Wood Horn", "wood-horn", "Horn"),
    ("Iron Horn", "iron-horn", "Horn"),
    ("Steel Horn", "steel-horn", "Horn"),
    ("Gaeen's Horn", "gaeens-horn", "Horn"),
    ("Ciokin's Horn", "ciokins-horn", "Horn"),
    ("Shookle's Horn", "shookles-horn", "Horn"),
    ("Divine Horn", "divine-horn", "Horn"),
    ("Wooden Helm", "wooden-helm", "Helm"),
    ("Iron Helm", "iron-helm", "Helm"),
    ("Steel Helm", "steel-helm", "Helm"),
    ("Wind Helm", "wind-helm", "Helm"),
    ("Strength Helm", "strength-helm", "Helm"),
    ("Ancient Helm", "ancient-helm", "Helm"),
    ("Giant Helm \"Turtle\"", "giant-helm-turtle", "Helm"),
    ("Divine Helm", "divine-helm", "Helm"),
    ("Bunny Head", "bunny-head", "Animal Helm"),
    ("Scorpiton Helm", "scorpiton-helm", "Animal Helm"),
    ("Spiderton Helm", "spiderton-helm", "Animal Helm"),
    ("Beetleton Helm", "beetleton-helm", "Animal Helm"),
];

/// An inventory item (stew, Memory, weapon, or piece of gear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    index: u8,
}

impl Item {
    /// All items in catalog order.
    pub fn all() -> impl Iterator<Item = Item> {
        (0..DEFS.len() as u8).map(|index| Item { index })
    }

    /// Looks up an item by its slug (for example `"divine-sword"`).
    pub fn from_slug(slug: &str) -> Option<Item> {
        DEFS.iter()
            .position(|&(_, s, _)| s == slug)
            .map(|index| Item { index: index as u8 })
    }

    /// Display name (for example `"Divine Sword"`).
    pub fn name(self) -> &'static str {
        DEFS[self.index as usize].0
    }

    /// Stable slug (for example `"divine-sword"`).
    pub fn slug(self) -> &'static str {
        DEFS[self.index as usize].1
    }

    /// Category label (for example `"Sword"`), for grouping in listings.
    pub fn category(self) -> &'static str {
        DEFS[self.index as usize].2
    }

    /// This item's canonical position, used to index
    /// [`layout::item_offsets`](crate::save::layout::item_offsets).
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
        for item in Item::all() {
            assert_eq!(Item::from_slug(item.slug()), Some(item));
            assert!(seen.insert(item.slug()), "duplicate slug {}", item.slug());
        }
        assert_eq!(seen.len(), 83);
    }

    #[test]
    fn positions_are_zero_based_and_dense() {
        let positions: Vec<usize> = Item::all().map(|i| i.position()).collect();
        assert_eq!(positions, (0..83).collect::<Vec<_>>());
    }

    #[test]
    fn unknown_slug_is_none() {
        assert_eq!(Item::from_slug("not-an-item"), None);
    }
}
