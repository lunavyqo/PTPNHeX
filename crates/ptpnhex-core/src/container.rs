//! [`SaveSlot`] — a complete save directory (`PARAM.SFO` + `SECURE.BIN`).
//!
//! [`SaveSlot::open`] reads and decrypts a save into an editable plaintext
//! buffer; [`SaveSlot::save`] re-encrypts it, regenerates the integrity hashes
//! the firmware checks, and writes both files back. It writes *only* those two
//! files into the save directory — a real PSP rejects a save folder that
//! contains anything else. To keep a copy of the originals first, use
//! [`SaveSlot::back_up_to`] with a directory outside the save folder.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::crypto::{
    decrypt_secure, encrypt_secure, file_list_hash, params_hash, ParamsHashField, SECURE_HEADER_LEN,
};
use crate::keys::{GameKey, KeyProvider};
use crate::save::Region;
use crate::sfo::ParamSfo;
use crate::{Error, Result};

const SECURE_FILE: &str = "SECURE.BIN";
const SFO_FILE: &str = "PARAM.SFO";

/// Field offsets within a 4-byte inventory record `count:u8, new:u8, owned:u8,
/// display-index:u8` (see `docs/save-format.md`). The display index is
/// recomputed by the game, so editing never writes it.
const RECORD_NEW_FLAG: usize = 1;
const RECORD_OWNED_FLAG: usize = 2;
/// Value of the owned flag when the item is owned; `0` means never obtained.
const INVENTORY_OWNED: u8 = 0x01;

/// `SAVEDATA_PARAMS` hash field offsets (within its 0x80-byte block).
const PARAMS_HASH10: usize = 0x10;
const PARAMS_HASH70: usize = 0x70;
/// `SAVEDATA_FILE_LIST` row: name(13) + hash(16) + pad(3).
const FILE_ROW_LEN: usize = 0x20;
const FILE_ROW_HASH_OFF: usize = 0x0D;

/// An opened, decrypted save.
pub struct SaveSlot {
    dir: PathBuf,
    region: Region,
    gamekey: GameKey,
    /// The 16-byte `SECURE.BIN` header, preserved so an unedited save
    /// re-encrypts byte-identically.
    header: [u8; SECURE_HEADER_LEN],
    sfo: ParamSfo,
    data: Vec<u8>,
}

impl SaveSlot {
    /// Opens and decrypts the save directory `dir`, resolving the game key for
    /// its region through `keys`.
    pub fn open(dir: impl AsRef<Path>, keys: &KeyProvider) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::Unsupported("save directory has no readable name".into()))?;
        let region = Region::detect(dir_name)
            .ok_or_else(|| Error::Unsupported(format!("unrecognized save serial: {dir_name}")))?;
        let gamekey = keys.resolve(region).ok_or_else(|| {
            Error::Unsupported(format!("no game key available for {}", region.serial()))
        })?;

        let sfo = ParamSfo::parse(&fs::read(dir.join(SFO_FILE))?)?;
        let blob = fs::read(dir.join(SECURE_FILE))?;
        if blob.len() < SECURE_HEADER_LEN {
            return Err(Error::Malformed {
                what: "SECURE.BIN",
                reason: format!("too short: {} bytes", blob.len()),
            });
        }
        let header = blob[..SECURE_HEADER_LEN].try_into().expect("16 bytes");
        let data = decrypt_secure(&blob, &gamekey)?;

        Ok(Self {
            dir,
            region,
            gamekey,
            header,
            sfo,
            data,
        })
    }

    /// The save's region.
    pub fn region(&self) -> Region {
        self.region
    }

    /// The decrypted plaintext payload.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable access to the decrypted plaintext payload.
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    /// The save's ka-ching (currency), if the field is mapped for this region.
    pub fn kaching(&self) -> Option<u32> {
        let off = crate::save::layout::kaching_offset(self.region)?;
        let bytes = self.data.get(off..off + 4)?;
        Some(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
    }

    /// Sets the save's ka-ching (currency), capped at
    /// [`crate::save::layout::KACHING_MAX`].
    pub fn set_kaching(&mut self, value: u32) -> Result<()> {
        use crate::save::layout::{kaching_offset, KACHING_MAX};
        if value > KACHING_MAX {
            return Err(Error::Unsupported(format!(
                "ka-ching {value} exceeds the maximum of {KACHING_MAX}"
            )));
        }
        let off = kaching_offset(self.region).ok_or_else(|| {
            Error::Unsupported(format!(
                "ka-ching is not mapped for {}",
                self.region.serial()
            ))
        })?;
        let slot = self
            .data
            .get_mut(off..off + 4)
            .ok_or_else(|| Error::Malformed {
                what: "SECURE.BIN",
                reason: "payload too short for ka-ching field".into(),
            })?;
        slot.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// The count of `material` (`0` if the player has never obtained it).
    pub fn material(&self, material: crate::save::Material) -> u32 {
        self.material_offset(material)
            .map_or(0, |off| self.inventory_count(off))
    }

    /// Every material with its current count, in canonical order.
    pub fn materials(&self) -> Vec<(crate::save::Material, u32)> {
        crate::save::Material::all()
            .map(|m| (m, self.material(m)))
            .collect()
    }

    /// Sets the count of `material` (capped at
    /// [`crate::save::materials::MATERIAL_MAX`]), obtaining it first if the
    /// player never had it (see [`set_inventory`](Self::set_inventory)).
    pub fn set_material(&mut self, material: crate::save::Material, count: u32) -> Result<()> {
        use crate::save::materials::MATERIAL_MAX;
        if count > MATERIAL_MAX {
            return Err(Error::Unsupported(format!(
                "material count {count} exceeds the maximum of {MATERIAL_MAX}"
            )));
        }
        let off = self.material_offset(material).ok_or_else(|| {
            Error::Unsupported(format!(
                "materials are not mapped for {}",
                self.region.serial()
            ))
        })?;
        self.set_inventory(off, count as u8);
        Ok(())
    }

    /// The count of `item` (`0` if the player has never obtained it).
    pub fn item(&self, item: crate::save::Item) -> u32 {
        self.item_offset(item)
            .map_or(0, |off| self.inventory_count(off))
    }

    /// Every item with its current count, in catalog order.
    pub fn items(&self) -> Vec<(crate::save::Item, u32)> {
        crate::save::Item::all()
            .map(|i| (i, self.item(i)))
            .collect()
    }

    /// Sets the count of `item` (capped at [`crate::save::items::ITEM_MAX`]),
    /// obtaining it first if the player never had it (see
    /// [`set_inventory`](Self::set_inventory)).
    pub fn set_item(&mut self, item: crate::save::Item, count: u32) -> Result<()> {
        use crate::save::items::ITEM_MAX;
        if count > ITEM_MAX {
            return Err(Error::Unsupported(format!(
                "item count {count} exceeds the maximum of {ITEM_MAX}"
            )));
        }
        let off = self.item_offset(item).ok_or_else(|| {
            Error::Unsupported(format!("items are not mapped for {}", self.region.serial()))
        })?;
        self.set_inventory(off, count as u8);
        Ok(())
    }

    /// Whether `key_item` (a drum, miracle, song, or quest item) is unlocked.
    pub fn key_item(&self, key_item: crate::save::KeyItem) -> bool {
        self.key_item_offset(key_item)
            .is_some_and(|off| self.data[off + RECORD_OWNED_FLAG] == INVENTORY_OWNED)
    }

    /// Every key item with whether it is unlocked, in catalog order.
    pub fn key_items(&self) -> Vec<(crate::save::KeyItem, bool)> {
        crate::save::KeyItem::all()
            .map(|k| (k, self.key_item(k)))
            .collect()
    }

    /// Sets or clears `key_item`'s owned flag — the altar collection marker (see
    /// [`crate::save::key_items`] for what that does and does not change
    /// in-game). Setting it flags the record owned (and "new" so it flashes),
    /// matching a freshly obtained token; clearing it resets the record. Key
    /// items are one-per, so the count is fixed at 1.
    pub fn set_key_item(&mut self, key_item: crate::save::KeyItem, unlocked: bool) -> Result<()> {
        let off = self.key_item_offset(key_item).ok_or_else(|| {
            Error::Unsupported(format!(
                "key items are not mapped for {}",
                self.region.serial()
            ))
        })?;
        if unlocked {
            self.set_inventory(off, 1);
        } else {
            self.data[off] = 0;
            self.data[off + RECORD_NEW_FLAG] = 0;
            self.data[off + RECORD_OWNED_FLAG] = 0;
        }
        Ok(())
    }

    /// Whether the mission-prep loadout slots (miracle and stew) are open.
    pub fn loadout_slots(&self) -> bool {
        self.loadout_slots_offset()
            .is_some_and(|off| self.data[off] & crate::save::layout::LOADOUT_SLOTS_BIT != 0)
    }

    /// Opens or closes the mission-prep loadout slots (both the miracle and the
    /// stew slot, which share one flag — see [`crate::save::layout`]). Which
    /// miracles are then castable still follows from the miracle key-item tokens.
    pub fn set_loadout_slots(&mut self, open: bool) -> Result<()> {
        let off = self.loadout_slots_offset().ok_or_else(|| {
            Error::Unsupported(format!(
                "loadout slots are not mapped for {}",
                self.region.serial()
            ))
        })?;
        if open {
            self.data[off] |= crate::save::layout::LOADOUT_SLOTS_BIT;
        } else {
            self.data[off] &= !crate::save::layout::LOADOUT_SLOTS_BIT;
        }
        Ok(())
    }

    /// Forces every confirmed progression unlock — all drums, every buildable
    /// unit type, the full mission list, and all boss missions — by OR-ing the
    /// unlock-accumulator masks into the save's unlock bitfields.
    ///
    /// This only ever *sets* bits, and touches only the accumulator bytes, so it
    /// adds unlocks without disturbing the save's current state (it does not open
    /// the mission-prep loadout slots, which are a separate flag — see
    /// [`set_loadout_slots`](Self::set_loadout_slots)). Returns the number of
    /// bytes it changed.
    pub fn unlock_all(&mut self) -> Result<usize> {
        let masks = crate::save::layout::unlock_all_masks(self.region).ok_or_else(|| {
            Error::Unsupported(format!(
                "progression unlocks are not mapped for {}",
                self.region.serial()
            ))
        })?;
        let mut changed = 0;
        for &(off, mask) in masks {
            if off >= self.data.len() {
                continue;
            }
            let before = self.data[off];
            self.data[off] |= mask;
            if self.data[off] != before {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// The offset of the loadout-slots flag, if mapped and in bounds.
    fn loadout_slots_offset(&self) -> Option<usize> {
        let off = crate::save::layout::loadout_slots_offset(self.region)?;
        (off < self.data.len()).then_some(off)
    }

    /// Reads an inventory record's count, treating a not-owned record as `0`.
    fn inventory_count(&self, off: usize) -> u32 {
        if self.data[off + RECORD_OWNED_FLAG] == INVENTORY_OWNED {
            self.data[off] as u32
        } else {
            0
        }
    }

    /// Writes an inventory record's count, obtaining the item first if it was
    /// never owned: the game treats the owned flag as the source of truth and
    /// recomputes the cosmetic display index on load (confirmed on hardware), so
    /// a never-obtained item is added — flagged owned and marked "new" so it
    /// flashes like a real pickup — leaving the display-index byte for the game.
    fn set_inventory(&mut self, off: usize, count: u8) {
        if self.data[off + RECORD_OWNED_FLAG] != INVENTORY_OWNED {
            self.data[off + RECORD_OWNED_FLAG] = INVENTORY_OWNED;
            self.data[off + RECORD_NEW_FLAG] = 1;
        }
        self.data[off] = count;
    }

    /// The fixed offset of a material's inventory record (see
    /// [`set_inventory`](Self::set_inventory) for the record format).
    fn material_offset(&self, material: crate::save::Material) -> Option<usize> {
        let offsets = crate::save::layout::material_offsets(self.region)?;
        let off = offsets[material.position()];
        (off + 4 <= self.data.len()).then_some(off)
    }

    /// The fixed offset of an item's inventory record.
    fn item_offset(&self, item: crate::save::Item) -> Option<usize> {
        let offsets = crate::save::layout::item_offsets(self.region)?;
        let off = offsets[item.position()];
        (off + 4 <= self.data.len()).then_some(off)
    }

    /// The fixed offset of a key item's inventory record.
    fn key_item_offset(&self, key_item: crate::save::KeyItem) -> Option<usize> {
        let offsets = crate::save::layout::key_item_offsets(self.region)?;
        let off = offsets[key_item.position()];
        (off + 4 <= self.data.len()).then_some(off)
    }

    /// The parsed `PARAM.SFO` metadata.
    pub fn sfo(&self) -> &ParamSfo {
        &self.sfo
    }

    /// Mutable access to the `PARAM.SFO` metadata (titles, etc.).
    pub fn sfo_mut(&mut self) -> &mut ParamSfo {
        &mut self.sfo
    }

    /// Re-encrypts the payload, regenerates the integrity hashes, and writes
    /// `SECURE.BIN` and `PARAM.SFO` back.
    ///
    /// Only those two files are ever written into the save directory: a real
    /// PSP rejects a save folder that contains any unexpected file (a stray
    /// `*.bak` is enough to make the save unloadable). To keep a copy of the
    /// originals, call [`SaveSlot::back_up_to`] before saving.
    pub fn save(&self) -> Result<()> {
        let secure = encrypt_secure(&self.data, &self.header, &self.gamekey);
        let sfo_bytes = self.reseal_sfo(&secure)?;

        write_atomic(&self.dir.join(SECURE_FILE), &secure)?;
        write_atomic(&self.dir.join(SFO_FILE), &sfo_bytes)?;
        Ok(())
    }

    /// Copies the current on-disk `SECURE.BIN` and `PARAM.SFO` into `dest_dir`,
    /// creating it if needed, so the originals can be restored after a
    /// [`SaveSlot::save`].
    ///
    /// `dest_dir` must be **outside** the save directory; backing up into the
    /// save folder itself is exactly what would corrupt the save, so it is
    /// refused.
    pub fn back_up_to(&self, dest_dir: impl AsRef<Path>) -> Result<()> {
        let dest_dir = dest_dir.as_ref();
        if same_dir(dest_dir, &self.dir) {
            return Err(Error::Unsupported(
                "refusing to back up into the save directory itself; choose a directory outside it"
                    .into(),
            ));
        }
        fs::create_dir_all(dest_dir)?;
        for file in [SECURE_FILE, SFO_FILE] {
            let src = self.dir.join(file);
            if src.exists() {
                fs::copy(&src, dest_dir.join(file))?;
            }
        }
        Ok(())
    }

    /// Serializes `PARAM.SFO` with every reproducible integrity hash
    /// regenerated for the given encrypted `secure` image.
    ///
    /// The `+0x20` params hash (mode 6) is left untouched: it requires a KIRK
    /// fuse operation that cannot be reproduced in software (see
    /// `docs/crypto.md`).
    fn reseal_sfo(&self, secure: &[u8]) -> Result<Vec<u8>> {
        let params_off = self
            .sfo
            .data_offset("SAVEDATA_PARAMS")
            .ok_or_else(|| missing("SAVEDATA_PARAMS"))?;
        let list_off = self
            .sfo
            .data_offset("SAVEDATA_FILE_LIST")
            .ok_or_else(|| missing("SAVEDATA_FILE_LIST"))?;
        let list_len = self
            .sfo
            .get("SAVEDATA_FILE_LIST")
            .expect("entry present")
            .data_full()
            .len();

        let mut bytes = self.sfo.to_bytes();

        // 1. The per-file hash must be written before the params hashes, which
        //    cover the whole PARAM.SFO (this hash included).
        let row = find_file_row(&bytes[list_off..list_off + list_len], SECURE_FILE)
            .ok_or_else(|| missing("SECURE.BIN row in SAVEDATA_FILE_LIST"))?;
        let hpos = list_off + row + FILE_ROW_HASH_OFF;
        bytes[hpos..hpos + 16].copy_from_slice(&file_list_hash(secure, &self.gamekey));

        // 2. +0x70 (mode 5): computed with +0x10 and +0x70 zeroed.
        let h70 = {
            let mut img = bytes.clone();
            zero(&mut img, params_off + PARAMS_HASH10);
            zero(&mut img, params_off + PARAMS_HASH70);
            params_hash(&img, ParamsHashField::Hash70)
        };
        let p70 = params_off + PARAMS_HASH70;
        bytes[p70..p70 + 16].copy_from_slice(&h70);

        // 3. +0x10 (mode 1): computed with +0x10 zeroed (and +0x70 now set).
        let h10 = {
            let mut img = bytes.clone();
            zero(&mut img, params_off + PARAMS_HASH10);
            params_hash(&img, ParamsHashField::Hash10)
        };
        let p10 = params_off + PARAMS_HASH10;
        bytes[p10..p10 + 16].copy_from_slice(&h10);

        Ok(bytes)
    }
}

fn missing(what: &str) -> Error {
    Error::Malformed {
        what: "PARAM.SFO",
        reason: format!("missing {what}"),
    }
}

fn zero(buf: &mut [u8], at: usize) {
    buf[at..at + 16].fill(0);
}

/// Finds the `SAVEDATA_FILE_LIST` row whose name matches `name`.
fn find_file_row(list: &[u8], name: &str) -> Option<usize> {
    let mut off = 0;
    while off + FILE_ROW_LEN <= list.len() {
        let field = &list[off..off + 13];
        if field.iter().all(|&b| b == 0) {
            break; // unused trailing rows
        }
        let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
        if &field[..end] == name.as_bytes() {
            return Some(off);
        }
        off += FILE_ROW_LEN;
    }
    None
}

/// Whether two paths refer to the same directory, comparing canonical forms
/// when both exist and falling back to a literal comparison otherwise.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = with_suffix(path, ".tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn find_file_row_matches_named_row() {
        let mut list = vec![0u8; FILE_ROW_LEN * 2];
        list[..10].copy_from_slice(b"SECURE.BIN");
        assert_eq!(find_file_row(&list, "SECURE.BIN"), Some(0));
        assert_eq!(find_file_row(&list, "DATA.BIN"), None);
    }

    #[test]
    fn with_suffix_appends() {
        assert_eq!(
            with_suffix(Path::new("/a/SECURE.BIN"), ".tmp"),
            PathBuf::from("/a/SECURE.BIN.tmp")
        );
    }

    #[test]
    fn same_dir_detects_equal_paths() {
        assert!(same_dir(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!same_dir(Path::new("/a/b"), Path::new("/a/c")));
    }
}
