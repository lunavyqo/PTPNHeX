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

#[test]
fn key_items_match_confirmed_ownership() {
    // Anchors the hand-ordered EU_KEY_ITEM_OFFSETS table to a known save: DATA01's
    // owned/not-owned set, read in-game, spans all four categories. A round-trip
    // test alone can't catch a swapped or mis-typed offset (it reads back the same
    // wrong slot it wrote); this assertion against real data does.
    use ptpnhex_core::save::KeyItem;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let slot = SaveSlot::open(dir.join("UCES00995_DATA01"), &KeyProvider::Embedded).unwrap();
    let owned = |s: &str| slot.key_item(KeyItem::from_slug(s).unwrap());
    // owned in DATA01:
    assert!(owned("pon-drum"), "drum");
    assert!(owned("rain-miracle"), "miracle");
    assert!(owned("chakachaka-song"), "song");
    assert!(owned("blank-map"), "quest item");
    // not yet obtained in DATA01:
    assert!(!owned("earthquake-miracle"), "miracle absent");
    assert!(!owned("ponpata-song"), "song absent");
    assert!(!owned("dark-palace-model"), "quest item absent");
    eprintln!("key-item offsets match DATA01's confirmed ownership across all categories");
}

#[test]
fn loadout_slots_flag_matches_corpus_and_toggles() {
    // Anchors the loadout-slot flag (0x1A0F0 bit0) to reality: it is closed on a
    // pre-slot early save (DATA04) and open on a progressed one (DATA46). Then
    // checks the setter both opens and closes it.
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let early = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    assert!(
        !early.loadout_slots(),
        "DATA04 (early) should have slots closed"
    );
    let mut late = SaveSlot::open(dir.join("UCES00995_DATA46"), &KeyProvider::Embedded).unwrap();
    assert!(
        late.loadout_slots(),
        "DATA46 (progressed) should have slots open"
    );

    late.set_loadout_slots(false).unwrap();
    assert!(!late.loadout_slots(), "closing should clear the flag");
    late.set_loadout_slots(true).unwrap();
    assert!(late.loadout_slots(), "reopening should set the flag");
    eprintln!("loadout-slot flag matches the corpus and toggles cleanly");
}

#[test]
fn unlock_all_sets_every_mask_and_is_idempotent() {
    // unlock_all only ORs the accumulator masks, so applying it to a near-complete
    // save (DATA46, which already holds them) changes nothing, and a second pass on
    // any save is a no-op. On an early save it reports the bytes it sets.
    use ptpnhex_core::save::layout::unlock_all_masks;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let mut late = SaveSlot::open(dir.join("UCES00995_DATA46"), &KeyProvider::Embedded).unwrap();
    assert_eq!(
        late.unlock_all().unwrap(),
        0,
        "DATA46 already has every unlock; nothing should change"
    );

    let mut early = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    let changed = early.unlock_all().unwrap();
    assert!(changed > 0, "an early save should gain unlock bits");
    // Every mask bit must now be present, and a second pass is idempotent.
    let masks = unlock_all_masks(early.region()).unwrap();
    for &(off, mask) in masks {
        assert_eq!(
            early.data()[off] & mask,
            mask,
            "mask not fully set at {off:#x}"
        );
    }
    assert_eq!(
        early.unlock_all().unwrap(),
        0,
        "second pass should be a no-op"
    );
    eprintln!("unlock_all set {changed} bytes on DATA04 and is idempotent");
}

#[test]
fn bonus_patapons_match_corpus_and_toggle() {
    // A complete save (DATA46) has every bonus Patapon revived; an early save
    // (DATA04, 0:17) lacks the ones revived later (e.g. Kampon, revived last).
    // The setter both revives and removes a pair.
    use ptpnhex_core::save::BonusPatapon;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let mut late = SaveSlot::open(dir.join("UCES00995_DATA46"), &KeyProvider::Embedded).unwrap();
    for (bp, revived) in late.bonus_patapons() {
        assert!(revived, "DATA46 should have {} revived", bp.name());
    }

    let kimpon = BonusPatapon::from_slug("kimpon").unwrap();
    late.set_bonus_patapon(kimpon, false).unwrap();
    assert!(
        !late.bonus_patapon(kimpon),
        "removing should clear the pair"
    );
    late.set_bonus_patapon(kimpon, true).unwrap();
    assert!(late.bonus_patapon(kimpon), "reviving should set the pair");

    let early = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    let kampon = BonusPatapon::from_slug("kampon").unwrap();
    assert!(
        !early.bonus_patapon(kampon),
        "DATA04 (early) should not have Kampon (revived last)"
    );
    eprintln!("bonus Patapon flags match the corpus and toggle");
}

#[test]
fn army_roster_reads_and_rarepon_round_trips() {
    // Reads the roster of a progressed save, checks every unit has a known class
    // and rarepon code, and round-trips set_unit_rarepon on a working copy (no
    // hardcoded anchor, so it is robust to the live corpus drifting).
    use ptpnhex_core::save::Rarepon;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("rarepon");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let n = slot.army_size();
        assert!(n > 0, "a progressed save should have units");
        for i in 0..n {
            assert!(slot.unit_class(i).is_some(), "unit {i} has a class");
            assert!(
                slot.unit_rarepon_code(i).is_some(),
                "unit {i} has a rarepon code"
            );
        }
        // Past the army, slots are empty.
        assert!(slot.unit_class(n).is_none(), "slot after the army is empty");
    }

    // Round-trip: set unit 0 to each known rarepon through disk and read it back.
    for rarepon in Rarepon::all() {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.set_unit_rarepon(0, rarepon).unwrap();
        slot.save().unwrap();
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.unit_rarepon(0), Some(rarepon));
        assert_eq!(slot.unit_rarepon_code(0), Some(rarepon.code()));
    }
    fs::remove_dir_all(&root).ok();
    eprintln!("army roster read and rarepon set/get round-tripped through disk");
}
