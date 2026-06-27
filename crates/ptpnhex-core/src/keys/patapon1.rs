//! Embedded game keys for supported Patapon releases.
//!
//! This is the **only** file that contains literal key material. Everything
//! else goes through [`crate::keys::KeyProvider`], so these values can be
//! reviewed or removed in one place. The module is compiled only with the
//! default `embedded-keys` feature.
//!
//! Each key is the 16-byte game key the title passes to the save-data utility
//! (see `docs/crypto.md`). A key is obtained from a copy of the game with a
//! PSP key-dumper plugin; until a region's key has been dumped and verified by
//! the round-trip test, its entry is left as `None` and users supply the key
//! at runtime via [`crate::keys::KeyProvider::Bytes`].

use crate::keys::GameKey;
use crate::save::Region;

/// Europe (`UCES00995`).
///
/// The per-title game key, identical across every copy of the game. Confirmed
/// by the `SECURE.BIN` round-trip and hash-reproduction tests over the save
/// corpus.
const EUROPE: Option<GameKey> = Some([
    0x01, 0xAF, 0x6F, 0x00, 0x02, 0x00, 0x70, 0xD5, 0x2E, 0x24, 0x12, 0xC7, 0xE1, 0xFF, 0x83, 0xBA,
]);

/// North America (`UCUS98711`). Pending.
const NORTH_AMERICA: Option<GameKey> = None;

/// Japan (`UCJS10077`). Pending.
const JAPAN: Option<GameKey> = None;

/// Returns the embedded game key for `region`, if one is known.
pub fn game_key(region: Region) -> Option<GameKey> {
    match region {
        Region::Europe => EUROPE,
        Region::NorthAmerica => NORTH_AMERICA,
        Region::Japan => JAPAN,
    }
}
