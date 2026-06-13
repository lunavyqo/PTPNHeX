//! `SaveSlot` workflow tests against the real corpus.
//!
//! Uses the embedded game key (no `PTPNHEX_GAMEKEY` needed) and operates on
//! temporary copies so the corpus is never modified. Self-skips without
//! `PTPNHEX_SAVES_DIR`.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use ptpnhex_core::crypto::file_list_hash;
use ptpnhex_core::keys::KeyProvider;
use ptpnhex_core::SaveSlot;

const FILE_LIST_HASH_OFF: usize = 0x55D;
const GAMEKEY: [u8; 16] = [
    0x01, 0xAF, 0x6F, 0x00, 0x02, 0x00, 0x70, 0xD5, 0x2E, 0x24, 0x12, 0xC7, 0xE1, 0xFF, 0x83, 0xBA,
];

fn saves_dir() -> Option<PathBuf> {
    std::env::var_os("PTPNHEX_SAVES_DIR").map(PathBuf::from)
}

fn patapon_saves(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            let name = p.file_name()?.to_str()?;
            (name.starts_with("UCES00995") && p.join("SECURE.BIN").is_file()).then_some(p)
        })
        .collect();
    v.sort();
    v
}

/// Copies a save's `PARAM.SFO` and `SECURE.BIN` into a fresh working directory,
/// preserving the original directory name (needed for region detection).
fn working_copy(src: &Path, root: &Path) -> PathBuf {
    let dst = root.join(src.file_name().unwrap());
    fs::create_dir_all(&dst).unwrap();
    for f in ["PARAM.SFO", "SECURE.BIN"] {
        fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    dst
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ptpnhex-{}-{}", tag, std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn save_without_edits_is_byte_identical() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("identical");
    let mut n = 0;
    for save in patapon_saves(&dir) {
        let work = working_copy(&save, &root);
        let orig_secure = fs::read(work.join("SECURE.BIN")).unwrap();
        let orig_sfo = fs::read(work.join("PARAM.SFO")).unwrap();

        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.save().unwrap();

        assert_eq!(
            fs::read(work.join("SECURE.BIN")).unwrap(),
            orig_secure,
            "{}",
            work.display()
        );
        assert_eq!(
            fs::read(work.join("PARAM.SFO")).unwrap(),
            orig_sfo,
            "{}",
            work.display()
        );
        n += 1;
    }
    fs::remove_dir_all(&root).ok();
    assert!(n > 0, "no saves found");
    eprintln!("SaveSlot resealed {n} unedited saves byte-identically");
}

#[test]
fn edit_persists_and_rehashes_through_disk() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("edit");
    let save = patapon_saves(&dir).into_iter().next().unwrap();
    let work = working_copy(&save, &root);

    let original = {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let original = slot.data().to_vec();
        slot.data_mut()[0x100] ^= 0xFF;
        slot.save().unwrap();
        original
    };

    // save() writes ONLY the two real files into the save directory — a stray
    // file (e.g. a *.bak) makes a real PSP reject the save.
    let mut entries: Vec<String> = fs::read_dir(&work)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        vec!["PARAM.SFO".to_string(), "SECURE.BIN".to_string()]
    );

    // Reopen: the edit survived the encrypt -> disk -> decrypt round trip.
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.data()[0x100], original[0x100] ^ 0xFF);
    assert_eq!(slot.data().len(), original.len());

    // The regenerated file-list hash is correct for the new ciphertext.
    let secure = fs::read(work.join("SECURE.BIN")).unwrap();
    let sfo = fs::read(work.join("PARAM.SFO")).unwrap();
    assert_eq!(
        &sfo[FILE_LIST_HASH_OFF..FILE_LIST_HASH_OFF + 16],
        file_list_hash(&secure, &GAMEKEY)
    );

    // Resealing is idempotent: saving again changes nothing.
    slot.save().unwrap();
    assert_eq!(fs::read(work.join("SECURE.BIN")).unwrap(), secure);
    assert_eq!(fs::read(work.join("PARAM.SFO")).unwrap(), sfo);

    fs::remove_dir_all(&root).ok();
    eprintln!("SaveSlot edit round-tripped through disk with correct rehashing");
}

#[test]
fn back_up_to_copies_originals_outside_and_refuses_the_save_dir() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("backup");
    let save = patapon_saves(&dir).into_iter().next().unwrap();
    let work = working_copy(&save, &root);
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();

    // Backing up to a directory outside the save folder copies the originals.
    let backup = root.join("backup");
    slot.back_up_to(&backup).unwrap();
    assert_eq!(
        fs::read(backup.join("SECURE.BIN")).unwrap(),
        fs::read(work.join("SECURE.BIN")).unwrap()
    );
    assert_eq!(
        fs::read(backup.join("PARAM.SFO")).unwrap(),
        fs::read(work.join("PARAM.SFO")).unwrap()
    );

    // Backing up into the save directory itself is refused.
    assert!(slot.back_up_to(&work).is_err());

    fs::remove_dir_all(&root).ok();
    eprintln!("back_up_to copied originals and refused the save directory");
}

#[test]
fn kaching_matches_confirmed_values() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let mut by_name = std::collections::HashMap::new();
    for save in patapon_saves(&dir) {
        let slot = SaveSlot::open(&save, &KeyProvider::Embedded).unwrap();
        // The small system save (DATA00) has no ka-ching field; skip it.
        if let Some(k) = slot.kaching() {
            assert!(k <= 99_999, "{}: ka-ching {k} out of range", save.display());
            let name = save.file_name().unwrap().to_str().unwrap().to_string();
            by_name.insert(name, k);
        }
    }
    // Confirmed in-game against the player's screen.
    assert_eq!(by_name.get("UCES00995_DATA01"), Some(&564));
    assert_eq!(by_name.get("UCES00995_DATA50"), Some(&598));
    eprintln!("ka-ching read correctly for {} saves", by_name.len());
}

#[test]
fn set_kaching_round_trips_through_disk() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("kaching");
    // Use a full game save (the small DATA00 has no ka-ching field).
    let save = patapon_saves(&dir)
        .into_iter()
        .find(|s| s.file_name().unwrap().to_str().unwrap() == "UCES00995_DATA01")
        .expect("DATA01 present");
    let work = working_copy(&save, &root);

    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.set_kaching(7777).unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.kaching(), Some(7777));

    fs::remove_dir_all(&root).ok();
    eprintln!("ka-ching edit round-tripped through disk");
}

#[test]
fn materials_match_confirmed_values() {
    use ptpnhex_core::save::Material;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    // Player-confirmed material counts for DATA46, in canonical (item-id) order.
    let expected = [
        29, 21, 20, 7, 57, 11, 19, 0, 1, 70, 23, 13, 1, 41, 11, 3, 20, 13, 8, 0,
    ];
    let save = dir.join("UCES00995_DATA46");
    let slot = SaveSlot::open(&save, &KeyProvider::Embedded).unwrap();
    let got: Vec<u32> = slot.materials().into_iter().map(|(_, c)| c).collect();
    assert_eq!(got, expected, "materials mismatch for DATA46");
    // Every full save reads all 20 in range without panicking.
    assert_eq!(Material::all().count(), 20);
    eprintln!("materials matched confirmed values for DATA46");
}

#[test]
fn set_material_round_trips_through_disk() {
    use ptpnhex_core::save::Material;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("materials");
    // DATA46 lists all materials, so any is editable in place.
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);
    let stone = Material::from_slug("stone").unwrap();
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.material(stone), 57);
        slot.set_material(stone, 99).unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.material(stone), 99);
    // Other materials are untouched.
    assert_eq!(
        slot.material(Material::from_slug("cherry-tree").unwrap()),
        70
    );

    fs::remove_dir_all(&root).ok();
    eprintln!("material edit round-tripped through disk");
}

#[test]
fn owned_at_zero_material_is_editable_not_mishit() {
    // DATA50 owns Magic Alloy at count 0 (flag-owned, count 0). The old
    // positional reader mis-hit a neighbouring item here; the record walk must
    // find the real Magic Alloy slot and edit it in place.
    use ptpnhex_core::save::Material;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("ownedzero");
    let work = working_copy(&dir.join("UCES00995_DATA50"), &root);
    let magic = Material::from_slug("magic-alloy").unwrap();
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.material(magic), 0, "Magic Alloy starts owned-at-zero");
        slot.set_material(magic, 77).unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.material(magic), 77);
    fs::remove_dir_all(&root).ok();
    eprintln!("owned-at-zero material edited in place without mis-hitting");
}

#[test]
fn stale_over_cap_slot_is_not_reported_or_editable() {
    // DATA04 has a stale slot for material #14 (index 0x20) whose flag reads as
    // owned but whose count is 256 — impossible for a 99-capped material. It
    // must be treated as never-obtained: read 0, and refuse to edit.
    use ptpnhex_core::save::Material;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let eyeball = Material::from_slug("eyeball-cabbage").unwrap();
    let mut slot = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    assert_eq!(
        slot.material(eyeball),
        0,
        "stale over-cap slot reads as absent"
    );
    assert!(
        slot.set_material(eyeball, 50).is_err(),
        "editing a never-obtained material is refused, not mis-hit"
    );
    eprintln!("stale over-cap inventory slot ignored");
}
