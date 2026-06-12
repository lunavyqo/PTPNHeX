//! [`SaveSlot`] — a complete save directory (`PARAM.SFO` + `SECURE.BIN`).
//!
//! [`SaveSlot::open`] reads and decrypts a save into an editable plaintext
//! buffer; [`SaveSlot::save`] re-encrypts it, regenerates the integrity hashes
//! the firmware checks, and writes both files back (backing up the originals).

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

    /// The count of `material` in this save (`0` if the player has never
    /// obtained it, in which case it is absent from the inventory list).
    pub fn material(&self, material: crate::save::Material) -> u32 {
        match self.material_offset(material) {
            Some(off) => u16::from_le_bytes([self.data[off], self.data[off + 1]]) as u32,
            None => 0,
        }
    }

    /// Every material with its current count, in canonical order.
    pub fn materials(&self) -> Vec<(crate::save::Material, u32)> {
        crate::save::Material::all()
            .map(|m| (m, self.material(m)))
            .collect()
    }

    /// Sets the count of `material` (capped at
    /// [`crate::save::materials::MATERIAL_MAX`]).
    ///
    /// Only materials already present in the save can be edited; setting one
    /// the player has never obtained is not yet supported and returns an
    /// error. Any reasonably progressed save lists all materials (even at
    /// count zero).
    pub fn set_material(&mut self, material: crate::save::Material, count: u32) -> Result<()> {
        use crate::save::materials::MATERIAL_MAX;
        if count > MATERIAL_MAX {
            return Err(Error::Unsupported(format!(
                "material count {count} exceeds the maximum of {MATERIAL_MAX}"
            )));
        }
        let off = self.material_offset(material).ok_or_else(|| {
            Error::Unsupported(format!(
                "{} is not present in this save and cannot be added yet",
                material.name()
            ))
        })?;
        self.data[off..off + 2].copy_from_slice(&(count as u16).to_le_bytes());
        Ok(())
    }

    /// Finds the offset of a material's count `u16` by scanning the inventory
    /// region for its item id.
    fn material_offset(&self, material: crate::save::Material) -> Option<usize> {
        let region = crate::save::layout::inventory_region(self.region)?;
        let end = region.end.min(self.data.len());
        let id = material.item_id().to_le_bytes();
        let mut off = region.start;
        while off + 4 <= end {
            if self.data[off + 2] == id[0] && self.data[off + 3] == id[1] {
                return Some(off);
            }
            off += 4;
        }
        None
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
    /// `SECURE.BIN` and `PARAM.SFO`, backing up the originals to `*.bak`.
    pub fn save(&self) -> Result<()> {
        self.write(true)
    }

    /// Like [`SaveSlot::save`] but without writing `*.bak` backups.
    pub fn save_without_backup(&self) -> Result<()> {
        self.write(false)
    }

    fn write(&self, backup: bool) -> Result<()> {
        let secure = encrypt_secure(&self.data, &self.header, &self.gamekey);
        let sfo_bytes = self.reseal_sfo(&secure)?;

        let secure_path = self.dir.join(SECURE_FILE);
        let sfo_path = self.dir.join(SFO_FILE);
        if backup {
            back_up(&secure_path)?;
            back_up(&sfo_path)?;
        }
        write_atomic(&secure_path, &secure)?;
        write_atomic(&sfo_path, &sfo_bytes)?;
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

fn back_up(path: &Path) -> Result<()> {
    if path.exists() {
        fs::copy(path, with_suffix(path, ".bak"))?;
    }
    Ok(())
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
            with_suffix(Path::new("/a/SECURE.BIN"), ".bak"),
            PathBuf::from("/a/SECURE.BIN.bak")
        );
    }
}
