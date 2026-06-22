//! Rarepons: the special unit variants that set a Patapon's identity.
//!
//! Each unit in the [army roster](crate::save::layout::roster_record_offset)
//! carries its rarepon as a small set of fields inside its record (see
//! `docs/save-format.md`), all editable:
//!
//! * `+0x48` **body** (`u32` name-hash) — the appearance and, derived from it at
//!   runtime, the unit's **stats** (the rarepon's "power").
//! * `+0x4e` low nibble — the displayed **name** (the high nibble is the class).
//! * `+0xA4` **headpiece** id string (`hlmNNN_NN`) and its `+0xC4` name-hash.
//! * `+0xC8` = `0x01` — the headpiece flag (an intrinsic head with no helmet slot,
//!   as every rarepon has; a basic patapon has `0x00` and an equippable helmet).
//! * `+0xD0` = `160 + hlm#` — a numeric echo of the headpiece id.
//!
//! The codes are EU values, confirmed on hardware by constructing a rarepon on a
//! basic unit and reading it back in-game (name, class, stats, headpiece, and the
//! absent helmet slot all matched a naturally-created rarepon).

/// Per-rarepon identity data. `head_num` is the headpiece `hlm` number (10..=15)
/// for the standard headpieces; `Basic` has `None` (it is a plain patapon, not a
/// constructible rarepon here). `name_nibble` is the low nibble of the `+0x4e`
/// byte that selects the displayed name — taken from naturally-created units of
/// that rarepon (Barsala/Mogyoon additionally confirmed in-game).
struct Def {
    name: &'static str,
    slug: &'static str,
    body: u32,
    head_num: Option<u8>,
    head_hash: u32,
    name_nibble: u8,
}

#[rustfmt::skip]
const DEFS: [Def; 7] = [
    Def { name: "Basic",   slug: "basic",   body: 0xFFFF_FFFF, head_num: None,     head_hash: 0,           name_nibble: 0x0 },
    Def { name: "Barsala", slug: "barsala", body: 0xFFCD_FEBE, head_num: Some(15), head_hash: 0xDA21_6E8F, name_nibble: 0xF },
    Def { name: "Mogyoon", slug: "mogyoon", body: 0xFFC0_6E9F, head_num: Some(14), head_hash: 0x629D_09EA, name_nibble: 0xB },
    Def { name: "Tikulee", slug: "tikulee", body: 0xFFA9_6D65, head_num: Some(13), head_hash: 0xFF4A_3153, name_nibble: 0x7 },
    Def { name: "Mofeel",  slug: "mofeel",  body: 0xFFF8_98CF, head_num: Some(12), head_hash: 0x47F6_5636, name_nibble: 0x3 },
    Def { name: "Pyokola", slug: "pyokola", body: 0xFF35_6EEF, head_num: Some(10), head_hash: 0xEDFF_9EBD, name_nibble: 0x1 },
    Def { name: "Gekolos", slug: "gekolos", body: 0xFF61_E4DA, head_num: Some(11), head_hash: 0x5543_F9D8, name_nibble: 0x2 },
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
            .position(|d| d.slug == slug)
            .map(|index| Rarepon { index: index as u8 })
    }

    /// Looks up a rarepon by its raw body code, if known.
    pub fn from_code(code: u32) -> Option<Rarepon> {
        DEFS.iter()
            .position(|d| d.body == code)
            .map(|index| Rarepon { index: index as u8 })
    }

    fn def(self) -> &'static Def {
        &DEFS[self.index as usize]
    }

    /// Display name (for example `"Mogyoon"`).
    pub fn name(self) -> &'static str {
        self.def().name
    }

    /// Stable slug (for example `"mogyoon"`).
    pub fn slug(self) -> &'static str {
        self.def().slug
    }

    /// The raw body code stored at unit-record offset `+0x48`.
    pub fn code(self) -> u32 {
        self.def().body
    }

    /// Whether this is the plain (non-rarepon) "Basic" patapon. Basic cannot be
    /// *constructed* by [`set_unit_rarepon`](crate::SaveSlot::set_unit_rarepon)
    /// because restoring a class-correct basic head and helmet slot is unmapped.
    pub fn is_basic(self) -> bool {
        self.def().head_num.is_none()
    }

    /// The low nibble of `+0x4e` that selects this rarepon's displayed name.
    pub fn name_nibble(self) -> u8 {
        self.def().name_nibble
    }

    /// The headpiece `hlm` number (10..=15), or `None` for Basic.
    pub fn head_num(self) -> Option<u8> {
        self.def().head_num
    }

    /// The headpiece id string (for example `"hlm015_01"`), or `None` for Basic.
    /// Standard for every class except Dekapon, whose headpieces are unmapped.
    pub fn head_id(self) -> Option<String> {
        self.head_num().map(|n| format!("hlm{n:03}_01"))
    }

    /// The headpiece name-hash written at `+0xC4`, or `None` for Basic.
    pub fn head_hash(self) -> Option<u32> {
        self.head_num().map(|_| self.def().head_hash)
    }

    /// The numeric headpiece echo (`160 + hlm#`) written at `+0xD0`, or `None`
    /// for Basic.
    pub fn head_echo(self) -> Option<u8> {
        self.head_num().map(|n| 0xA0 + n)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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

    #[test]
    fn basic_has_no_headpiece_others_do() {
        let basic = Rarepon::from_slug("basic").unwrap();
        assert!(basic.is_basic());
        assert_eq!(basic.head_id(), None);
        assert_eq!(basic.head_hash(), None);
        assert_eq!(basic.head_echo(), None);

        for rarepon in Rarepon::all().filter(|r| !r.is_basic()) {
            assert!(rarepon.head_id().is_some());
            assert!(rarepon.head_hash().is_some());
            // echo is 160 + hlm number
            let n = rarepon.head_num().unwrap();
            assert_eq!(rarepon.head_echo(), Some(0xA0 + n));
        }
    }

    #[test]
    fn head_ids_and_echoes_match_known_values() {
        let barsala = Rarepon::from_slug("barsala").unwrap();
        assert_eq!(barsala.head_id().as_deref(), Some("hlm015_01"));
        assert_eq!(barsala.head_hash(), Some(0xDA21_6E8F));
        assert_eq!(barsala.head_echo(), Some(0xAF));
        assert_eq!(barsala.name_nibble(), 0xF);

        let pyokola = Rarepon::from_slug("pyokola").unwrap();
        assert_eq!(pyokola.head_id().as_deref(), Some("hlm010_01"));
        assert_eq!(pyokola.head_echo(), Some(0xAA));
    }
}
