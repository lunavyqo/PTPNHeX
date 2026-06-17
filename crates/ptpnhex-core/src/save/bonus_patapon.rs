//! Bonus Patapons: the five revived in Patapolis, each granting a minigame.
//!
//! Patapolis revives a bonus Patapon when you bury a cap dropped in a mission.
//! Each revive sets a **bit-pair** in the progress/unlock region (see
//! `docs/save-format.md`): a contiguous run from `0x1AD71` bit 4 to `0x1AD72`
//! bit 5, one pair per Patapon in revive order. Both bits of a pair flip
//! together, and setting the pair is what makes that Patapon — and its minigame —
//! available. Kimpon's pair additionally unlocks the **Kibapon** unit class.
//!
//! The pair → Patapon mapping was found offline against the save corpus (using
//! each cap's count byte as a per-Patapon "revived" timing marker) and confirmed
//! on hardware for all five. A bonus Patapon is identified by its canonical
//! [`position`](BonusPatapon::position), which maps to a fixed `(offset, mask)`
//! via [`layout::bonus_patapon_flags`](crate::save::layout::bonus_patapon_flags).
//!
//! Caveat: setting the bit reflects the *permanent* unlock, but whether the
//! Patapon visibly appears in Patapolis also depends on the save's current story
//! position — a very early save may not render a Patapon revived much later until
//! the story reaches that point.

/// `(display name, slug, minigame)` for each bonus Patapon, in revive order
/// (paired index-for-index with
/// [`layout::bonus_patapon_flags`](crate::save::layout::bonus_patapon_flags)).
/// `minigame` is the in-game minigame name where known, else an empty string.
#[rustfmt::skip]
const DEFS: [(&str, &str, &str); 5] = [
    ("Pakapon",      "pakapon", ""),
    ("Kimpon",       "kimpon",  ""),
    ("Fah Zakpon",   "zakpon",  "Pop Bean the Legume"),
    ("Rah Gashapon", "gashpon", "Simmer Slurp the Cooking Pot"),
    ("Kampon",       "kampon",  ""),
];

/// A bonus Patapon revived in Patapolis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BonusPatapon {
    index: u8,
}

impl BonusPatapon {
    /// All bonus Patapons in catalog (revive) order.
    pub fn all() -> impl Iterator<Item = BonusPatapon> {
        (0..DEFS.len() as u8).map(|index| BonusPatapon { index })
    }

    /// Looks up a bonus Patapon by its slug (for example `"kimpon"`).
    pub fn from_slug(slug: &str) -> Option<BonusPatapon> {
        DEFS.iter()
            .position(|&(_, s, _)| s == slug)
            .map(|index| BonusPatapon { index: index as u8 })
    }

    /// Display name (for example `"Fah Zakpon"`).
    pub fn name(self) -> &'static str {
        DEFS[self.index as usize].0
    }

    /// Stable slug (for example `"zakpon"`).
    pub fn slug(self) -> &'static str {
        DEFS[self.index as usize].1
    }

    /// The Patapon's minigame name, or `None` if it has not been recorded.
    pub fn minigame(self) -> Option<&'static str> {
        let m = DEFS[self.index as usize].2;
        (!m.is_empty()).then_some(m)
    }

    /// This Patapon's canonical position, used to index
    /// [`layout::bonus_patapon_flags`](crate::save::layout::bonus_patapon_flags).
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
        for bp in BonusPatapon::all() {
            assert_eq!(BonusPatapon::from_slug(bp.slug()), Some(bp));
            assert!(seen.insert(bp.slug()), "duplicate slug {}", bp.slug());
        }
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn positions_are_zero_based_and_dense() {
        let positions: Vec<usize> = BonusPatapon::all().map(|b| b.position()).collect();
        assert_eq!(positions, (0..5).collect::<Vec<_>>());
    }

    #[test]
    fn unknown_slug_is_none() {
        assert_eq!(BonusPatapon::from_slug("not-a-patapon"), None);
    }
}
