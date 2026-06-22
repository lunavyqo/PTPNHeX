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

    /// The player's chosen name (the "Almighty" name) decoded from the body's
    /// UTF-16LE field, or `None` if the field is not mapped for this region (or
    /// the payload is too short, as on the small system save).
    pub fn player_name(&self) -> Option<String> {
        use crate::save::layout::{player_name_offset, PLAYER_NAME_MAX_CHARS};
        let off = player_name_offset(self.region)?;
        let mut units = Vec::new();
        for i in 0..PLAYER_NAME_MAX_CHARS {
            let p = off + i * 2;
            let unit = u16::from_le_bytes(self.data.get(p..p + 2)?.try_into().ok()?);
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        Some(String::from_utf16_lossy(&units))
    }

    /// Sets the player's name, written as UTF-16LE (NUL-terminated) into the
    /// game-data field so it persists in-game (unlike the save-list label).
    ///
    /// Rejects an empty name, names longer than
    /// [`PLAYER_NAME_MAX_CHARS`](crate::save::layout::PLAYER_NAME_MAX_CHARS)
    /// UTF-16 code units, or names containing characters outside the Basic
    /// Multilingual Plane.
    pub fn set_player_name(&mut self, name: &str) -> Result<()> {
        use crate::save::layout::{player_name_offset, PLAYER_NAME_MAX_CHARS};
        if name.is_empty() {
            return Err(Error::Unsupported("player name must not be empty".into()));
        }
        let units: Vec<u16> = name.encode_utf16().collect();
        if units.iter().any(|&u| (0xD800..=0xDFFF).contains(&u)) {
            return Err(Error::Unsupported(
                "player name contains characters outside the BMP".into(),
            ));
        }
        if units.len() > PLAYER_NAME_MAX_CHARS {
            return Err(Error::Unsupported(format!(
                "player name is {} characters; the maximum is {PLAYER_NAME_MAX_CHARS}",
                units.len()
            )));
        }
        let off = player_name_offset(self.region).ok_or_else(|| {
            Error::Unsupported(format!(
                "the player name is not mapped for {}",
                self.region.serial()
            ))
        })?;
        // The name plus a UTF-16 NUL terminator; the field sits in an all-zero run.
        let field = (PLAYER_NAME_MAX_CHARS + 1) * 2;
        let slot = self
            .data
            .get_mut(off..off + field)
            .ok_or_else(|| Error::Malformed {
                what: "SECURE.BIN",
                reason: "payload too short for the player-name field".into(),
            })?;
        slot.fill(0);
        for (i, unit) in units.iter().enumerate() {
            slot[i * 2..i * 2 + 2].copy_from_slice(&unit.to_le_bytes());
        }
        Ok(())
    }

    /// The play time shown in the save list, parsed as `(hours, minutes,
    /// seconds)` from the `PARAM.SFO` `SAVEDATA_DETAIL` text, if present.
    ///
    /// Play time is *not* stored in the game body — only as this display text
    /// (the PSP has no system-level play-time field).
    pub fn playtime(&self) -> Option<(u32, u8, u8)> {
        let detail = self.sfo().get_str("SAVEDATA_DETAIL")?;
        let token = detail
            .split("Play time:")
            .nth(1)?
            .split_whitespace()
            .next()?;
        let mut parts = token.split(':');
        let h: u32 = parts.next()?.parse().ok()?;
        let m: u8 = parts.next()?.parse().ok()?;
        let s: u8 = parts.next()?.parse().ok()?;
        (m < 60 && s < 60).then_some((h, m, s))
    }

    /// Rewrites the `Play time: HH:MM:SS` value in the `PARAM.SFO` detail
    /// string, leaving the rest of the detail untouched.
    ///
    /// Play time is not stored in the game body, so this edits only the
    /// save-list label — which the game regenerates on its next in-game save.
    /// Whether the game reads the edited value back on load is unconfirmed.
    pub fn set_playtime(&mut self, hours: u32, minutes: u8, seconds: u8) -> Result<()> {
        if hours > 9999 || minutes >= 60 || seconds >= 60 {
            return Err(Error::Unsupported(format!(
                "invalid play time {hours}:{minutes:02}:{seconds:02} \
                 (hours must be <= 9999, minutes and seconds < 60)"
            )));
        }
        let detail = self
            .sfo()
            .get_str("SAVEDATA_DETAIL")
            .ok_or_else(|| Error::Unsupported("save has no SAVEDATA_DETAIL to edit".into()))?
            .to_string();
        const MARKER: &str = "Play time:";
        let at = detail.find(MARKER).ok_or_else(|| {
            Error::Unsupported("SAVEDATA_DETAIL has no \"Play time:\" line".into())
        })?;
        let value_start = at + MARKER.len();
        // The value runs to the next newline (or the end of the string).
        let value_end = detail[value_start..]
            .find('\n')
            .map_or(detail.len(), |n| value_start + n);
        let new_detail = format!(
            "{}{MARKER} {hours:02}:{minutes:02}:{seconds:02}{}",
            &detail[..at],
            &detail[value_end..]
        );
        self.sfo_mut().set_str("SAVEDATA_DETAIL", &new_detail)?;
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

    /// Whether `bonus_patapon` is revived (its unlock bit-pair is set).
    pub fn bonus_patapon(&self, bonus_patapon: crate::save::BonusPatapon) -> bool {
        self.bonus_patapon_flag(bonus_patapon)
            .is_some_and(|(off, mask)| self.data[off] & mask == mask)
    }

    /// Every bonus Patapon with whether it is revived, in catalog order.
    pub fn bonus_patapons(&self) -> Vec<(crate::save::BonusPatapon, bool)> {
        crate::save::BonusPatapon::all()
            .map(|b| (b, self.bonus_patapon(b)))
            .collect()
    }

    /// Revives or removes `bonus_patapon` by setting or clearing its unlock
    /// bit-pair — granting or removing that Patapon's minigame (and, for Kimpon,
    /// Kibapon production). Whether a revived Patapon visibly appears also depends
    /// on the save's current story position (see [`crate::save::bonus_patapon`]).
    pub fn set_bonus_patapon(
        &mut self,
        bonus_patapon: crate::save::BonusPatapon,
        revived: bool,
    ) -> Result<()> {
        let (off, mask) = self.bonus_patapon_flag(bonus_patapon).ok_or_else(|| {
            Error::Unsupported(format!(
                "bonus Patapons are not mapped for {}",
                self.region.serial()
            ))
        })?;
        if revived {
            self.data[off] |= mask;
        } else {
            self.data[off] &= !mask;
        }
        Ok(())
    }

    /// The `(offset, mask)` of a bonus Patapon's flag, if mapped and in bounds.
    fn bonus_patapon_flag(&self, bonus_patapon: crate::save::BonusPatapon) -> Option<(usize, u8)> {
        let flags = crate::save::layout::bonus_patapon_flags(self.region)?;
        let (off, mask) = flags[bonus_patapon.position()];
        (off < self.data.len()).then_some((off, mask))
    }

    /// Whether `bonus_patapon`'s one-time introduction dialog has been seen.
    pub fn bonus_patapon_dialog_seen(&self, bonus_patapon: crate::save::BonusPatapon) -> bool {
        self.bonus_patapon_dialog_flag(bonus_patapon)
            .is_some_and(|(off, mask)| self.data[off] & mask == mask)
    }

    /// Marks `bonus_patapon`'s introduction dialog as seen or unseen. Clearing it
    /// (`seen = false`) makes the one-time intro **replay** on the next interaction;
    /// this is cosmetic and does not affect the revive/minigame (see
    /// [`set_bonus_patapon`](Self::set_bonus_patapon)).
    pub fn set_bonus_patapon_dialog_seen(
        &mut self,
        bonus_patapon: crate::save::BonusPatapon,
        seen: bool,
    ) -> Result<()> {
        let (off, mask) = self
            .bonus_patapon_dialog_flag(bonus_patapon)
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "bonus Patapons are not mapped for {}",
                    self.region.serial()
                ))
            })?;
        if seen {
            self.data[off] |= mask;
        } else {
            self.data[off] &= !mask;
        }
        Ok(())
    }

    /// The `(offset, mask)` of a bonus Patapon's dialog-seen flag, if mapped.
    fn bonus_patapon_dialog_flag(
        &self,
        bonus_patapon: crate::save::BonusPatapon,
    ) -> Option<(usize, u8)> {
        let flags = crate::save::layout::bonus_patapon_dialog_flags(self.region)?;
        let (off, mask) = flags[bonus_patapon.position()];
        (off < self.data.len()).then_some((off, mask))
    }

    /// Whether `bonus_patapon`'s minigame has been played at least once.
    pub fn bonus_patapon_minigame_played(&self, bonus_patapon: crate::save::BonusPatapon) -> bool {
        self.bonus_patapon_played_flag(bonus_patapon)
            .is_some_and(|(off, mask)| self.data[off] & mask == mask)
    }

    /// Marks `bonus_patapon`'s minigame as played or never-played. Cosmetic; it
    /// does not affect the revive/minigame availability (see
    /// [`set_bonus_patapon`](Self::set_bonus_patapon)).
    pub fn set_bonus_patapon_minigame_played(
        &mut self,
        bonus_patapon: crate::save::BonusPatapon,
        played: bool,
    ) -> Result<()> {
        let (off, mask) = self
            .bonus_patapon_played_flag(bonus_patapon)
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "bonus Patapons are not mapped for {}",
                    self.region.serial()
                ))
            })?;
        if played {
            self.data[off] |= mask;
        } else {
            self.data[off] &= !mask;
        }
        Ok(())
    }

    /// The `(offset, mask)` of a bonus Patapon's minigame-played flag, if mapped.
    fn bonus_patapon_played_flag(
        &self,
        bonus_patapon: crate::save::BonusPatapon,
    ) -> Option<(usize, u8)> {
        let flags = crate::save::layout::bonus_patapon_played_flags(self.region)?;
        let (off, mask) = flags[bonus_patapon.position()];
        (off < self.data.len()).then_some((off, mask))
    }

    /// The offset of the loadout-slots flag, if mapped and in bounds.
    fn loadout_slots_offset(&self) -> Option<usize> {
        let off = crate::save::layout::loadout_slots_offset(self.region)?;
        (off < self.data.len()).then_some(off)
    }

    /// The number of units in the army roster (filled records from the start).
    pub fn army_size(&self) -> usize {
        (0..crate::save::layout::ROSTER_CAPACITY)
            .take_while(|&i| self.unit_record(i).is_some())
            .count()
    }

    /// The class name of unit `index` (for example `"Yaripon"`), or `None` if the
    /// roster slot is empty.
    pub fn unit_class(&self, index: usize) -> Option<&'static str> {
        let base = self.unit_record(index)?;
        let id = &self.data[base + crate::save::layout::RECORD_UNIT_ID..][..7];
        Some(unit_class_name(id))
    }

    /// The raw rarepon code at unit `index`, or `None` if the slot is empty.
    pub fn unit_rarepon_code(&self, index: usize) -> Option<u32> {
        let base = self.unit_record(index)?;
        let off = base + crate::save::layout::RECORD_RAREPON;
        Some(u32::from_le_bytes(
            self.data[off..off + 4].try_into().expect("4 bytes"),
        ))
    }

    /// The rarepon of unit `index`, if the slot is filled and its code is one of
    /// the known rarepons.
    pub fn unit_rarepon(&self, index: usize) -> Option<crate::save::Rarepon> {
        self.unit_rarepon_code(index)
            .and_then(crate::save::Rarepon::from_code)
    }

    /// Turns unit `index` into `rarepon`, writing the full identity — body,
    /// name/class byte, headpiece (id, hash, flag, echo) — and mirroring it into
    /// the unit's deployed-formation copy so the change holds in battle too.
    ///
    /// The unit keeps its own class: only the low nibble of the name/class byte
    /// (the displayed name) is changed, and the headpiece is the standard one for
    /// that rarepon. Stats follow the body and are applied by the game.
    ///
    /// # Errors
    /// - the roster slot is empty;
    /// - `rarepon` is [`Basic`](crate::save::Rarepon::is_basic) — reverting a unit
    ///   to a plain patapon (class-specific basic head + helmet slot) is unmapped;
    /// - the unit is a Dekapon, whose headpieces use a different, unmapped id set.
    pub fn set_unit_rarepon(&mut self, index: usize, rarepon: crate::save::Rarepon) -> Result<()> {
        let base = self.unit_record(index).ok_or_else(|| {
            Error::Unsupported(format!(
                "no unit at roster index {index} for {}",
                self.region.serial()
            ))
        })?;
        if rarepon.is_basic() {
            return Err(Error::Unsupported(
                "cannot set a unit to Basic (reverting a rarepon is not supported)".into(),
            ));
        }
        // Dekapon headpieces use a different (unmapped) id family than the other
        // classes, so its rarepon construction isn't supported yet.
        if &self.data[base + crate::save::layout::RECORD_UNIT_ID..][..7] == b"unit007" {
            return Err(Error::Unsupported(
                "rarepon editing for Dekapon units is not yet supported".into(),
            ));
        }

        let gid = u32::from_le_bytes(
            self.data[base + crate::save::layout::RECORD_GID..][..4]
                .try_into()
                .expect("4 bytes"),
        );
        self.write_rarepon_fields(base, rarepon);

        // Mirror onto the deployed-formation copy (best effort): the record(s)
        // there sharing this unit's global id.
        if let Some(fbase) = crate::save::layout::formation_base(self.region) {
            let stride = crate::save::layout::ROSTER_STRIDE;
            for j in 0..crate::save::layout::ROSTER_CAPACITY {
                let rec = fbase + j * stride;
                if rec + stride > self.data.len() {
                    break;
                }
                let is_unit =
                    &self.data[rec + crate::save::layout::RECORD_UNIT_ID..][..4] == b"unit";
                let rec_gid = u32::from_le_bytes(
                    self.data[rec + crate::save::layout::RECORD_GID..][..4]
                        .try_into()
                        .expect("4 bytes"),
                );
                if is_unit && rec_gid == gid {
                    self.write_rarepon_fields(rec, rarepon);
                }
            }
        }
        Ok(())
    }

    /// Writes a (non-basic) rarepon's identity fields into the record at `base`.
    /// Shared by the roster record and its formation copy.
    fn write_rarepon_fields(&mut self, base: usize, rarepon: crate::save::Rarepon) {
        use crate::save::layout::{
            RECORD_HEAD_ECHO, RECORD_HEAD_FLAG, RECORD_HEAD_HASH, RECORD_HEAD_ID,
            RECORD_NAME_CLASS, RECORD_RAREPON,
        };
        // body (appearance + stats)
        self.data[base + RECORD_RAREPON..][..4].copy_from_slice(&rarepon.code().to_le_bytes());
        // name nibble (low), preserving the unit's class nibble (high)
        let nc = base + RECORD_NAME_CLASS;
        self.data[nc] = (self.data[nc] & 0xF0) | (rarepon.name_nibble() & 0x0F);
        // headpiece id string + NUL terminator (ids are fixed-length, no spill)
        let id = rarepon.head_id().expect("non-basic has a head id");
        let hb = id.as_bytes();
        self.data[base + RECORD_HEAD_ID..][..hb.len()].copy_from_slice(hb);
        self.data[base + RECORD_HEAD_ID + hb.len()] = 0;
        // headpiece hash, flag (intrinsic head / no helmet slot), numeric echo
        self.data[base + RECORD_HEAD_HASH..][..4]
            .copy_from_slice(&rarepon.head_hash().expect("non-basic").to_le_bytes());
        self.data[base + RECORD_HEAD_FLAG] = 0x01;
        self.data[base + RECORD_HEAD_ECHO] = rarepon.head_echo().expect("non-basic");
    }

    /// The id of unit `index`'s equipped weapon (for example `"wpn004_008_01"`),
    /// or `None` if the slot is empty or holds no weapon.
    pub fn unit_weapon(&self, index: usize) -> Option<&str> {
        let base = self.unit_record(index)?;
        let off = base + crate::save::layout::RECORD_WEAPON_ID;
        let bytes = &self.data[off..off + 16];
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let id = std::str::from_utf8(&bytes[..end]).ok()?;
        id.starts_with("wpn").then_some(id)
    }

    /// The highest weapon tier available to unit `index` (its family's top tier),
    /// or `None` if the slot is empty or the unit has no weapon.
    pub fn unit_weapon_max_tier(&self, index: usize) -> Option<u8> {
        let (family, _, _) = crate::save::weapon::parse(self.unit_weapon(index)?)?;
        Some(crate::save::weapon::max_tier(family))
    }

    /// Sets unit `index`'s weapon to `tier` within its current weapon family,
    /// writing the new id and its CRC32 name-hash, mirroring the deployed-formation
    /// copy, and granting enough copies of the weapon in inventory to keep it
    /// equipped.
    ///
    /// The game validates a unit's weapon against inventory ownership on load: an
    /// un-owned weapon silently reverts to the family's tier-1 weapon, and each
    /// equipped unit consumes one copy. So this also raises the weapon's inventory
    /// count to cover every unit now wielding it (all confirmed on hardware).
    ///
    /// # Errors
    /// - the roster slot is empty, or the unit has no weapon;
    /// - `tier` is outside `1..=max` for the family (8, or 9 for Tatepon swords);
    /// - the region's weapon layout is unmapped.
    pub fn set_unit_weapon(&mut self, index: usize, tier: u8) -> Result<()> {
        let current = self
            .unit_weapon(index)
            .ok_or_else(|| Error::Unsupported(format!("no weapon at roster index {index}")))?;
        let (family, _, variant) = crate::save::weapon::parse(current)
            .ok_or_else(|| Error::Unsupported(format!("unrecognised weapon id `{current}`")))?;
        let variant = variant.to_owned();
        let max = crate::save::weapon::max_tier(family);
        if tier < 1 || tier > max {
            return Err(Error::Unsupported(format!(
                "weapon tier {tier} out of range 1..={max} for family {family:03}"
            )));
        }
        let new_id = format!("wpn{family:03}_{tier:03}_{variant}");

        // roster record, then every formation copy sharing this unit's global id
        let base = self.unit_record(index).expect("checked by unit_weapon");
        let gid = u32::from_le_bytes(
            self.data[base + crate::save::layout::RECORD_GID..][..4]
                .try_into()
                .expect("4 bytes"),
        );
        self.write_weapon(base, &new_id);
        if let Some(fbase) = crate::save::layout::formation_base(self.region) {
            let stride = crate::save::layout::ROSTER_STRIDE;
            for j in 0..crate::save::layout::ROSTER_CAPACITY {
                let rec = fbase + j * stride;
                if rec + stride > self.data.len() {
                    break;
                }
                let is_unit =
                    &self.data[rec + crate::save::layout::RECORD_UNIT_ID..][..4] == b"unit";
                let rec_gid = u32::from_le_bytes(
                    self.data[rec + crate::save::layout::RECORD_GID..][..4]
                        .try_into()
                        .expect("4 bytes"),
                );
                if is_unit && rec_gid == gid {
                    self.write_weapon(rec, &new_id);
                }
            }
        }

        // grant: raise the weapon's inventory count to cover every unit wielding it
        let inv = crate::save::layout::weapon_inventory_offset(self.region, family, tier)
            .filter(|&o| o + 4 <= self.data.len())
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "weapon inventory slot for family {family:03} tier {tier} is unmapped"
                ))
            })?;
        let wielding = (0..self.army_size())
            .filter(|&i| self.unit_weapon(i) == Some(new_id.as_str()))
            .count() as u32;
        let want = wielding.clamp(1, crate::save::items::ITEM_MAX);
        let count = want
            .max(self.inventory_count(inv))
            .min(crate::save::items::ITEM_MAX) as u8;
        self.set_inventory(inv, count);
        Ok(())
    }

    /// Writes a weapon id string (+ NUL) and its CRC32 name-hash into the record
    /// at `base`. Shared by the roster record and its formation copy.
    fn write_weapon(&mut self, base: usize, id: &str) {
        let bytes = id.as_bytes();
        let off = base + crate::save::layout::RECORD_WEAPON_ID;
        self.data[off..off + bytes.len()].copy_from_slice(bytes);
        self.data[off + bytes.len()] = 0;
        let hash = crate::save::weapon::crc32(bytes);
        self.data[base + crate::save::layout::RECORD_WEAPON_HASH..][..4]
            .copy_from_slice(&hash.to_le_bytes());
    }

    /// The byte offset of unit `index`'s record if the slot is filled (its class
    /// id begins with `unit`), else `None`.
    fn unit_record(&self, index: usize) -> Option<usize> {
        let base = crate::save::layout::roster_record_offset(self.region, index)?;
        if base + crate::save::layout::ROSTER_STRIDE > self.data.len() {
            return None;
        }
        let id = &self.data[base + crate::save::layout::RECORD_UNIT_ID..][..4];
        (id == b"unit").then_some(base)
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

/// Maps a unit class id prefix (`unitNNN`) to its class name.
fn unit_class_name(id: &[u8]) -> &'static str {
    match id {
        b"unit002" => "Yumipon",
        b"unit003" => "Tatepon",
        b"unit004" => "Yaripon",
        b"unit006" => "Kibapon",
        b"unit007" => "Dekapon",
        b"unit008" => "Megapon",
        _ => "Unknown",
    }
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
