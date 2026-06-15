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
fn sfo_label_edit_round_trips_and_leaves_secure_untouched() {
    // Editing a PARAM.SFO display string changes only PARAM.SFO; SECURE.BIN is
    // byte-identical, and the save still opens (hashes regenerated correctly).
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("label");
    let save = patapon_saves(&dir).into_iter().next().unwrap();
    let work = working_copy(&save, &root);
    let secure_before = fs::read(work.join("SECURE.BIN")).unwrap();
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.sfo_mut().set_str("SAVEDATA_TITLE", "TST").unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.sfo().get_str("SAVEDATA_TITLE"), Some("TST"));
    assert_eq!(
        fs::read(work.join("SECURE.BIN")).unwrap(),
        secure_before,
        "editing the SFO label must not touch SECURE.BIN"
    );
    fs::remove_dir_all(&root).ok();
    eprintln!("SFO label edit round-tripped; SECURE.BIN untouched");
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
fn never_obtained_material_can_be_added() {
    // DATA50 never obtained Magic Alloy — its record is flagged not-owned. It
    // reads 0, and setting it now *adds* it (flips the owned flag) so it reads
    // back through disk, while an owned-at-zero material (Mystery Meat) edits in
    // place as before.
    use ptpnhex_core::save::Material;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("add");
    let work = working_copy(&dir.join("UCES00995_DATA50"), &root);
    let magic = Material::from_slug("magic-alloy").unwrap();
    let mystery = Material::from_slug("mystery-meat").unwrap();
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.material(magic), 0, "never-obtained reads 0");
        slot.set_material(magic, 50).unwrap(); // adds it
        assert_eq!(slot.material(magic), 50);
        // Owned at count 0 is a different state; it edits in place.
        assert_eq!(slot.material(mystery), 0);
        slot.set_material(mystery, 88).unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.material(magic), 50, "added material survived disk");
    assert_eq!(slot.material(mystery), 88);
    fs::remove_dir_all(&root).ok();
    eprintln!("never-obtained material added and round-tripped; owned-at-zero edited");
}

#[test]
fn obtained_and_absent_materials_both_settable() {
    // DATA04 is early: it owns Leather Meat (2) and Stone (0, owned-at-zero),
    // with most materials not yet obtained. Obtained ones edit; absent ones are
    // added.
    use ptpnhex_core::save::Material;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let mut slot = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    let leather = Material::from_slug("leather-meat").unwrap();
    let stone = Material::from_slug("stone").unwrap();
    let dream = Material::from_slug("dream-meat").unwrap();
    assert_eq!(slot.material(leather), 2);
    assert_eq!(slot.material(stone), 0, "owned at zero");
    assert_eq!(slot.material(dream), 0, "not obtained");
    slot.set_material(leather, 50).unwrap();
    slot.set_material(stone, 50).unwrap();
    slot.set_material(dream, 50).unwrap(); // added
    assert_eq!(slot.material(leather), 50);
    assert_eq!(slot.material(stone), 50);
    assert_eq!(slot.material(dream), 50, "absent material added");
    eprintln!("early save: obtained materials edited, absent ones added");
}

#[test]
fn item_lists_all_83_and_add_round_trips() {
    // Items share the inventory record with materials. Listing yields the full
    // catalog; adding a never-obtained item (Divine Sword on the early DATA04)
    // flips its owned flag and survives the disk round trip.
    use ptpnhex_core::save::Item;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("items");
    let work = working_copy(&dir.join("UCES00995_DATA04"), &root);
    let sword = Item::from_slug("divine-sword").unwrap();
    {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.items().len(), 83);
        assert_eq!(slot.item(sword), 0, "Divine Sword not obtained this early");
    }
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.set_item(sword, 42).unwrap(); // adds it
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.item(sword), 42, "added item survived disk");
    fs::remove_dir_all(&root).ok();
    eprintln!("items listed (83) and a never-obtained item added + round-tripped");
}

#[test]
fn key_item_lists_all_19_and_unlock_lock_round_trips() {
    // Key items share the inventory record but are one-per: only the owned flag
    // matters. Listing yields the full catalog; unlocking then locking a token
    // survives the disk round trip in both directions.
    use ptpnhex_core::save::KeyItem;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("key-items");
    let work = working_copy(&dir.join("UCES00995_DATA04"), &root);
    let quake = KeyItem::from_slug("earthquake-miracle").unwrap();
    {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.key_items().len(), 19);
    }
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.set_key_item(quake, true).unwrap();
        slot.save().unwrap();
    }
    {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert!(slot.key_item(quake), "unlock survived disk");
    }
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.set_key_item(quake, false).unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert!(!slot.key_item(quake), "lock survived disk");
    fs::remove_dir_all(&root).ok();
    eprintln!("key items listed (19) and unlock/lock round-tripped");
}
