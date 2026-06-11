//! Game-key handling for save-data decryption.
//!
//! Modes 3 and 5 (see `docs/crypto.md`) require a title's 16-byte game key.
//! This module abstracts where that key comes from so the rest of the crate
//! never embeds key material directly:
//!
//! - [`KeyProvider::Embedded`] uses the compiled-in table in
//!   [`patapon1`], available only with the default `embedded-keys` feature.
//! - [`KeyProvider::Bytes`] uses a key supplied at runtime (from a file or
//!   environment variable), so a build without `embedded-keys` is still fully
//!   functional when the user brings their own key.
//!
//! Keeping every literal key value in [`patapon1`] means the embedded keys
//! can be reviewed — or removed — in exactly one place.

#[cfg(feature = "embedded-keys")]
pub mod patapon1;

use crate::save::Region;

/// A 16-byte save-data game key.
pub type GameKey = [u8; 16];

/// Source of the game key used to decrypt or encrypt a save.
#[derive(Debug, Clone)]
pub enum KeyProvider {
    /// Use the key compiled into the binary for the save's region.
    ///
    /// Only meaningful when the crate is built with the `embedded-keys`
    /// feature; otherwise [`KeyProvider::resolve`] returns `None`.
    Embedded,
    /// Use the given key as-is (supplied by the user at runtime).
    Bytes(GameKey),
}

impl KeyProvider {
    /// Resolves the game key for `region`, or `None` if no key is available
    /// (for example [`KeyProvider::Embedded`] in a build compiled without the
    /// `embedded-keys` feature, or for a region without a known key).
    pub fn resolve(&self, region: Region) -> Option<GameKey> {
        match self {
            KeyProvider::Bytes(key) => Some(*key),
            #[cfg(feature = "embedded-keys")]
            KeyProvider::Embedded => patapon1::game_key(region),
            #[cfg(not(feature = "embedded-keys"))]
            KeyProvider::Embedded => {
                let _ = region;
                None
            }
        }
    }
}
