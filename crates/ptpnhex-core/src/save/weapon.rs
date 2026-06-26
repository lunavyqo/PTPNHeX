//! Weapon ids and their CRC32 name-hash.
//!
//! A unit's equipped weapon is the id string `wpnFFF_TTT_VV` at
//! [`RECORD_WEAPON_ID`](crate::save::layout::RECORD_WEAPON_ID), followed `0x20`
//! bytes later (at [`RECORD_WEAPON_HASH`](crate::save::layout::RECORD_WEAPON_HASH))
//! by its name-hash. That hash is the standard CRC32 (zlib/IEEE) of the exact id
//! string — verified against every weapon, helmet and shield id in the save
//! corpus — so a weapon's hash is computed, never looked up.
//!
//! `FFF` is the weapon family (fixed per class), `TTT` the tier (higher =
//! stronger), `VV` a variant suffix. Each class has one family: Yumipon `001`,
//! Tatepon `003`, Yaripon `004`, Kibapon `006`, Dekapon `007`, Megapon `008`.

/// CRC32 (zlib/IEEE: reflected, init `0xFFFFFFFF`, final XOR) of `bytes` — the
/// name-hash the game stores next to every weapon, helmet and shield id.
pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Splits a weapon id into `(family, tier, variant)` — for example
/// `"wpn004_008_01"` → `(4, 8, "01")` — or `None` if it isn't a
/// `wpnFFF_TTT_VV` string.
pub(crate) fn parse(id: &str) -> Option<(u16, u8, &str)> {
    let rest = id.strip_prefix("wpn")?;
    let mut parts = rest.split('_');
    let family: u16 = parts.next()?.parse().ok()?;
    let tier: u8 = parts.next()?.parse().ok()?;
    let variant = parts.next()?;
    if parts.next().is_some() || variant.is_empty() {
        return None;
    }
    Some((family, tier, variant))
}

/// The highest tier for a weapon family. Every family tops out at `8` (the
/// Divine weapon); the Tatepon sword family (`003`) has a 9th, Gong's Scythe.
pub fn max_tier(family: u16) -> u8 {
    if family == 3 {
        9
    } else {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_real_name_hashes() {
        // (id, name-hash read straight out of a real save record)
        assert_eq!(crc32(b"wpn001_008_01"), 0x4D9F_B8E2);
        assert_eq!(crc32(b"wpn003_009_01"), 0xB706_D8FA);
        assert_eq!(crc32(b"wpn004_008_01"), 0x057F_B686);
        assert_eq!(crc32(b"hlm014_01"), 0x629D_09EA);
        assert_eq!(crc32(b"sld008_01"), 0xAE26_069D);
    }

    #[test]
    fn parses_weapon_ids() {
        assert_eq!(parse("wpn004_008_01"), Some((4, 8, "01")));
        assert_eq!(parse("wpn003_009_01"), Some((3, 9, "01")));
        assert_eq!(parse("hlm014_01"), None);
        assert_eq!(parse("wpn004_008"), None);
        assert_eq!(parse("none"), None);
    }

    #[test]
    fn max_tiers() {
        assert_eq!(max_tier(3), 9);
        assert_eq!(max_tier(4), 8);
        assert_eq!(max_tier(8), 8);
    }
}
