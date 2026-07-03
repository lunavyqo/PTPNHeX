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
fn player_name_matches_corpus_and_round_trips() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    // The whole corpus is one player, so the name is constant where present.
    // (The small system save DATA00 has no name field and returns None.)
    let mut seen = 0;
    for save in patapon_saves(&dir) {
        let slot = SaveSlot::open(&save, &KeyProvider::Embedded).unwrap();
        if let Some(name) = slot.player_name() {
            assert_eq!(name, "Bbra", "{}", save.display());
            seen += 1;
        }
    }
    assert!(seen > 0, "no save exposed a player name");

    // Round-trip a new name through disk, and reject an over-long one.
    let root = temp_root("name");
    let save = patapon_saves(&dir)
        .into_iter()
        .find(|s| s.file_name().unwrap().to_str().unwrap() == "UCES00995_DATA01")
        .expect("DATA01 present");
    let work = working_copy(&save, &root);
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert!(slot.set_player_name("ThisNameIsWayTooLong").is_err());
        slot.set_player_name("Patapon7").unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.player_name().as_deref(), Some("Patapon7"));

    fs::remove_dir_all(&root).ok();
    eprintln!("player name read as Bbra and edit round-tripped");
}

#[test]
fn playtime_parses_and_set_round_trips() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    // Parses the SFO "Play time:" line where present.
    let mut seen = 0;
    for save in patapon_saves(&dir) {
        let slot = SaveSlot::open(&save, &KeyProvider::Embedded).unwrap();
        if let Some((_, m, s)) = slot.playtime() {
            assert!(m < 60 && s < 60, "{}", save.display());
            seen += 1;
        }
    }
    assert!(seen > 0, "no save exposed a play time");

    // Round-trip an edit; the SFO-only change must leave SECURE.BIN untouched.
    let root = temp_root("playtime");
    let save = patapon_saves(&dir)
        .into_iter()
        .find(|s| s.file_name().unwrap().to_str().unwrap() == "UCES00995_DATA01")
        .expect("DATA01 present");
    let work = working_copy(&save, &root);
    let secure_before = fs::read(work.join("SECURE.BIN")).unwrap();
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert!(slot.set_playtime(0, 70, 0).is_err());
        slot.set_playtime(12, 34, 56).unwrap();
        slot.save().unwrap();
    }
    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.playtime(), Some((12, 34, 56)));
    assert!(slot
        .sfo()
        .get_str("SAVEDATA_DETAIL")
        .unwrap()
        .contains("Play time: 12:34:56"));
    assert_eq!(
        fs::read(work.join("SECURE.BIN")).unwrap(),
        secure_before,
        "editing play time must not touch SECURE.BIN"
    );

    fs::remove_dir_all(&root).ok();
    eprintln!("play time parsed and edit round-tripped");
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
fn bonus_patapon_dialog_seen_matches_corpus_and_toggles() {
    // A complete save (DATA46) has every intro dialog seen; an early save (DATA04)
    // has only the earliest Patapon's (Pakapon, talked to ~0:48) — Kampon's, seen
    // last (~48:18), is unseen. The setter both marks seen and clears (replay).
    use ptpnhex_core::save::BonusPatapon;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let mut late = SaveSlot::open(dir.join("UCES00995_DATA46"), &KeyProvider::Embedded).unwrap();
    for bp in BonusPatapon::all() {
        assert!(
            late.bonus_patapon_dialog_seen(bp),
            "DATA46 should have {}'s intro seen",
            bp.name()
        );
    }

    let kimpon = BonusPatapon::from_slug("kimpon").unwrap();
    late.set_bonus_patapon_dialog_seen(kimpon, false).unwrap();
    assert!(
        !late.bonus_patapon_dialog_seen(kimpon),
        "clearing should mark the intro unseen"
    );
    late.set_bonus_patapon_dialog_seen(kimpon, true).unwrap();
    assert!(
        late.bonus_patapon_dialog_seen(kimpon),
        "setting should mark the intro seen"
    );

    let early = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    let kampon = BonusPatapon::from_slug("kampon").unwrap();
    assert!(
        !early.bonus_patapon_dialog_seen(kampon),
        "DATA04 (early) should not have Kampon's intro seen"
    );
    eprintln!("bonus Patapon dialog-seen flags match the corpus and toggle");
}

#[test]
fn bonus_patapon_minigame_played_matches_corpus_and_toggles() {
    // A complete save (DATA46) has every minigame played; the earliest save
    // (DATA04, 0:17) has none. The setter both marks played and clears.
    use ptpnhex_core::save::BonusPatapon;
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let mut late = SaveSlot::open(dir.join("UCES00995_DATA46"), &KeyProvider::Embedded).unwrap();
    for bp in BonusPatapon::all() {
        assert!(
            late.bonus_patapon_minigame_played(bp),
            "DATA46 should have {}'s minigame played",
            bp.name()
        );
    }
    let kimpon = BonusPatapon::from_slug("kimpon").unwrap();
    late.set_bonus_patapon_minigame_played(kimpon, false)
        .unwrap();
    assert!(!late.bonus_patapon_minigame_played(kimpon));
    late.set_bonus_patapon_minigame_played(kimpon, true)
        .unwrap();
    assert!(late.bonus_patapon_minigame_played(kimpon));

    let early = SaveSlot::open(dir.join("UCES00995_DATA04"), &KeyProvider::Embedded).unwrap();
    for bp in BonusPatapon::all() {
        assert!(
            !early.bonus_patapon_minigame_played(bp),
            "DATA04 (earliest) should have no minigame played, but {} reads played",
            bp.name()
        );
    }
    eprintln!("bonus Patapon minigame-played flags match the corpus and toggle");
}

#[test]
fn army_roster_reads_and_rarepon_round_trips() {
    // Reads the roster of a progressed save, checks every unit has a known class
    // and rarepon code, and round-trips the full set_unit_rarepon recipe on a
    // working copy (no hardcoded anchor, so it is robust to corpus drift).
    use ptpnhex_core::save::Rarepon;
    // Record-relative offsets (mirror of save::layout) verified by this test.
    const ROSTER_BASE: usize = 0x20;
    const STRIDE: usize = 0x104;
    const NAME_CLASS: usize = 0x4E;
    const HEAD_ID: usize = 0xA4;
    const HEAD_HASH: usize = 0xC4;
    const HEAD_FLAG: usize = 0xC8;
    const HEAD_ECHO: usize = 0xD0;

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("rarepon");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    let (target, dekapon) = {
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
        let target = (0..n)
            .find(|&i| slot.unit_class(i) != Some("Dekapon"))
            .expect("a non-Dekapon unit to build on");
        let dekapon = (0..n).find(|&i| slot.unit_class(i) == Some("Dekapon"));
        (target, dekapon)
    };

    // Basic (revert) is refused; Dekapon (unmapped headpieces) is refused.
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let basic = Rarepon::from_slug("basic").unwrap();
        assert!(
            slot.set_unit_rarepon(target, basic).is_err(),
            "Basic refused"
        );
        if let Some(d) = dekapon {
            let barsala = Rarepon::from_slug("barsala").unwrap();
            assert!(
                slot.set_unit_rarepon(d, barsala).is_err(),
                "Dekapon refused"
            );
        }
    }

    // Round-trip every real rarepon through disk; verify the full identity lands.
    for rarepon in Rarepon::all().filter(|r| !r.is_basic()) {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        slot.set_unit_rarepon(target, rarepon).unwrap();
        slot.save().unwrap();
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.unit_rarepon(target), Some(rarepon));
        assert_eq!(slot.unit_rarepon_code(target), Some(rarepon.code()));

        let base = ROSTER_BASE + target * STRIDE;
        let d = slot.data();
        assert_eq!(
            d[base + NAME_CLASS] & 0x0F,
            rarepon.name_nibble(),
            "{} name nibble",
            rarepon.slug()
        );
        let id = rarepon.head_id().unwrap();
        assert_eq!(&d[base + HEAD_ID..][..id.len()], id.as_bytes(), "head id");
        assert_eq!(d[base + HEAD_ID + id.len()], 0, "head id NUL-terminated");
        assert_eq!(
            u32::from_le_bytes(d[base + HEAD_HASH..][..4].try_into().unwrap()),
            rarepon.head_hash().unwrap(),
            "head hash"
        );
        assert_eq!(d[base + HEAD_FLAG], 0x01, "headpiece flag");
        assert_eq!(
            d[base + HEAD_ECHO],
            rarepon.head_echo().unwrap(),
            "head echo"
        );
    }
    fs::remove_dir_all(&root).ok();
    eprintln!("army roster read and full rarepon recipe round-tripped through disk");
}

#[test]
fn set_weapon_round_trips_grants_and_mirrors() {
    use ptpnhex_core::save::layout;

    const ROSTER_BASE: usize = 0x20;
    const STRIDE: usize = 0x104;
    const UNIT_ID: usize = 0x50;
    const WEAPON_ID: usize = 0x74;
    const WEAPON_HASH: usize = 0x94;
    const GID: usize = 0x24;

    // Independent CRC32 (zlib/IEEE) so the test checks the stored hash itself.
    fn crc32(bytes: &[u8]) -> u32 {
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
    // family + tier from a "wpnFFF_TTT_VV" id.
    fn fam_tier(id: &str) -> (u16, u8) {
        let mut p = id.strip_prefix("wpn").unwrap().split('_');
        (
            p.next().unwrap().parse().unwrap(),
            p.next().unwrap().parse().unwrap(),
        )
    }

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("weapon");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    let target = {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        (0..slot.army_size())
            .find(|&i| slot.unit_weapon(i).is_some())
            .expect("a unit with a weapon")
    };

    // Out-of-range tiers are rejected.
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert!(slot.set_unit_weapon(target, 0).is_err(), "tier 0 rejected");
        assert!(
            slot.set_unit_weapon(target, 250).is_err(),
            "an absurd tier is rejected"
        );
    }

    // Max the weapon, save, reopen, and verify everything landed.
    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    let max = slot.unit_weapon_max_tier(target).unwrap();
    let gid = u32::from_le_bytes(
        slot.data()[ROSTER_BASE + target * STRIDE + GID..][..4]
            .try_into()
            .unwrap(),
    );
    slot.set_unit_weapon(target, max).unwrap();
    slot.save().unwrap();

    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    let id = slot.unit_weapon(target).unwrap().to_owned();
    let (family, tier) = fam_tier(&id);
    assert_eq!(tier, max, "weapon set to the family's top tier");

    let d = slot.data();
    let base = ROSTER_BASE + target * STRIDE;
    let hash = u32::from_le_bytes(d[base + WEAPON_HASH..][..4].try_into().unwrap());
    assert_eq!(hash, crc32(id.as_bytes()), "weapon CRC32 name-hash");

    // The inventory item is granted, so the game keeps it equipped.
    let inv = layout::weapon_inventory_offset(slot.region(), family, tier).unwrap();
    assert_eq!(d[inv + 2], 1, "weapon owned in inventory");
    assert!(d[inv] >= 1, "weapon count covers the wielding unit");

    // The deployed-formation copy (if this unit is deployed) mirrors the id.
    if let Some(fbase) = layout::formation_base(slot.region()) {
        for j in 0..layout::ROSTER_CAPACITY {
            let rec = fbase + j * STRIDE;
            if rec + STRIDE > d.len() {
                break;
            }
            let rec_gid = u32::from_le_bytes(d[rec + GID..][..4].try_into().unwrap());
            if &d[rec + UNIT_ID..][..4] == b"unit" && rec_gid == gid {
                let end = d[rec + WEAPON_ID..][..16]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(16);
                assert_eq!(
                    &d[rec + WEAPON_ID..][..end],
                    id.as_bytes(),
                    "formation copy mirrors the weapon"
                );
            }
        }
    }

    // Regression: `formation_base` must be the true START of the deployed array. The
    // first deployed unit is in the first filled slot; if the base is even one record
    // too high (it was, at 0x30878), that unit's battle copy is silently skipped and it
    // fights with stale gear. Assert that no formation slot precedes the base — i.e. the
    // preceding record is neither a unit nor an empty `none` slot.
    if let Some(fbase) = layout::formation_base(slot.region()) {
        let prev = fbase - STRIDE;
        let precedes_unit = &d[prev + UNIT_ID..][..4] == b"unit";
        let precedes_empty = &d[prev..][..4] == b"none";
        assert!(
            !precedes_unit && !precedes_empty,
            "formation_base {fbase:#x} is not the array start: a deployed slot sits at {prev:#x}"
        );
    }

    fs::remove_dir_all(&root).ok();
    eprintln!("set_weapon round-tripped: id + CRC32, inventory grant, formation mirror");
}

#[test]
fn set_unit_class_reclasses_and_mirrors() {
    use ptpnhex_core::save::layout;

    const ROSTER_BASE: usize = 0x20;
    const STRIDE: usize = 0x104;
    const UNIT_ID: usize = 0x50;
    const GID: usize = 0x24;

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("class");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    // A filled unit, and a target class different from its current one.
    let (target, from, to) = {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let i = (0..slot.army_size())
            .find(|&i| slot.unit_class(i).is_some())
            .expect("a filled unit");
        let from = slot.unit_class(i).unwrap().to_owned();
        let to = if from == "Tatepon" {
            "Yaripon"
        } else {
            "Tatepon"
        };
        (i, from, to.to_owned())
    };
    assert_ne!(from, to, "the test changes the class");

    // An unknown class is rejected.
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert!(
            slot.set_unit_class(target, "Wizard").is_err(),
            "unknown class rejected"
        );
    }

    // Reclass, save, reopen — the functional class id changed and persisted.
    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    let gid = u32::from_le_bytes(
        slot.data()[ROSTER_BASE + target * STRIDE + GID..][..4]
            .try_into()
            .unwrap(),
    );
    slot.set_unit_class(target, &to).unwrap();
    slot.save().unwrap();

    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(
        slot.unit_class(target),
        Some(to.as_str()),
        "class changed and persisted"
    );

    // The id is written in the canonical `unitNNN_01_01` form.
    let d = slot.data();
    let base = ROSTER_BASE + target * STRIDE;
    let idb = &d[base + UNIT_ID..base + UNIT_ID + 13];
    assert!(
        idb.starts_with(b"unit") && &idb[7..] == b"_01_01",
        "canonical class id form"
    );

    // The deployed-formation copy (if this unit is deployed) mirrors the class.
    if let Some(fbase) = layout::formation_base(slot.region()) {
        for j in 0..layout::ROSTER_CAPACITY {
            let rec = fbase + j * STRIDE;
            if rec + STRIDE > d.len() {
                break;
            }
            let rec_gid = u32::from_le_bytes(d[rec + GID..][..4].try_into().unwrap());
            if &d[rec + UNIT_ID..][..4] == b"unit" && rec_gid == gid {
                assert_eq!(
                    &d[rec + UNIT_ID..][..13],
                    &d[base + UNIT_ID..][..13],
                    "formation copy mirrors the class"
                );
            }
        }
    }

    fs::remove_dir_all(&root).ok();
    eprintln!("set_unit_class reclassed {from} -> {to}: +0x50 id + formation mirror");
}

#[test]
fn set_unit_reborn_missions_writes_and_mirrors() {
    use ptpnhex_core::save::layout;

    const ROSTER_BASE: usize = 0x20;
    const STRIDE: usize = 0x104;
    const UNIT_ID: usize = 0x50;
    const GID: usize = 0x24;
    const REBORN: usize = 0x3C;
    const MISSIONS: usize = 0x40;

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("counters");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    // A filled unit.
    let target = {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        (0..slot.army_size())
            .find(|&i| slot.unit_class(i).is_some())
            .expect("a filled unit")
    };

    // An empty roster slot is rejected.
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let empty = slot.army_size();
        assert!(
            slot.set_unit_reborn(empty, 1).is_err(),
            "empty slot rejected"
        );
    }

    // Set both counters, save, reopen — the values changed and persisted.
    let gid = {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let gid = u32::from_le_bytes(
            slot.data()[ROSTER_BASE + target * STRIDE + GID..][..4]
                .try_into()
                .unwrap(),
        );
        slot.set_unit_reborn(target, 4242).unwrap();
        slot.set_unit_missions(target, 999_999).unwrap();
        slot.save().unwrap();
        gid
    };

    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(slot.unit_reborn(target), Some(4242), "reborn persisted");
    assert_eq!(
        slot.unit_missions(target),
        Some(999_999),
        "missions persisted"
    );

    // Raw roster bytes are the little-endian u32s.
    let d = slot.data();
    let base = ROSTER_BASE + target * STRIDE;
    assert_eq!(
        u32::from_le_bytes(d[base + REBORN..][..4].try_into().unwrap()),
        4242
    );
    assert_eq!(
        u32::from_le_bytes(d[base + MISSIONS..][..4].try_into().unwrap()),
        999_999
    );

    // The deployed-formation copy (if this unit is deployed) mirrors both.
    if let Some(fbase) = layout::formation_base(slot.region()) {
        for j in 0..layout::ROSTER_CAPACITY {
            let rec = fbase + j * STRIDE;
            if rec + STRIDE > d.len() {
                break;
            }
            let rec_gid = u32::from_le_bytes(d[rec + GID..][..4].try_into().unwrap());
            if &d[rec + UNIT_ID..][..4] == b"unit" && rec_gid == gid {
                assert_eq!(
                    u32::from_le_bytes(d[rec + REBORN..][..4].try_into().unwrap()),
                    4242,
                    "formation copy mirrors reborn"
                );
                assert_eq!(
                    u32::from_le_bytes(d[rec + MISSIONS..][..4].try_into().unwrap()),
                    999_999,
                    "formation copy mirrors missions"
                );
            }
        }
    }

    fs::remove_dir_all(&root).ok();
    eprintln!("set_unit_reborn/missions wrote +0x3C/+0x40 + formation mirror");
}

#[test]
fn set_unit_weapon_family_equips_a_foreign_weapon() {
    use ptpnhex_core::save::layout;

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("crossweapon");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    // A Yumipon natively wields a bow (wpn001); give it a horn (family 8, Divine).
    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    let yumipon = (0..slot.army_size())
        .find(|&i| slot.unit_class(i) == Some("Yumipon"))
        .expect("a Yumipon in DATA46");
    slot.set_unit_weapon_family(yumipon, 8, 8).unwrap();
    slot.save().unwrap();

    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert_eq!(
        slot.unit_weapon(yumipon),
        Some("wpn008_008_01"),
        "the foreign horn is equipped and survives a save round trip"
    );
    let inv = layout::weapon_inventory_offset(slot.region(), 8, 8).unwrap();
    assert_eq!(
        slot.data()[inv + 2],
        1,
        "the horn is granted (owned) in inventory"
    );

    // An unknown family or an out-of-range tier is rejected.
    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    assert!(
        slot.set_unit_weapon_family(yumipon, 99, 8).is_err(),
        "unknown weapon family rejected"
    );
    assert!(
        slot.set_unit_weapon_family(yumipon, 8, 99).is_err(),
        "out-of-range tier rejected"
    );

    fs::remove_dir_all(&root).ok();
    eprintln!("set_unit_weapon_family equipped a foreign horn on a Yumipon");
}

#[test]
fn set_shield_and_horse_round_trip_and_helmet_is_gated() {
    use ptpnhex_core::save::layout;

    const ROSTER_BASE: usize = 0x20;
    const STRIDE: usize = 0x104;
    const SHIELD_HASH: usize = 0xF4;

    fn crc32(bytes: &[u8]) -> u32 {
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

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("gear");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    let (tatepon, kibapon, rarepon) = {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let n = slot.army_size();
        let t = (0..n)
            .find(|&i| slot.unit_class(i) == Some("Tatepon"))
            .expect("a Tatepon");
        let k = (0..n)
            .find(|&i| slot.unit_class(i) == Some("Kibapon"))
            .expect("a Kibapon");
        let r = (0..n)
            .find(|&i| slot.unit_helmet(i).is_none())
            .expect("a unit without a helmet slot (a rarepon)");
        (t, k, r)
    };

    // Class/slot gating and range checks.
    {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert!(
            slot.set_unit_shield(kibapon, 8).is_err(),
            "shield refused on a non-Tatepon"
        );
        assert!(
            slot.set_unit_horse(tatepon, 8).is_err(),
            "horse refused on a non-Kibapon"
        );
        assert!(
            slot.set_unit_helmet(rarepon, 8).is_err(),
            "helmet refused on a rarepon (no helmet slot)"
        );
        assert!(slot.set_unit_shield(tatepon, 0).is_err(), "tier 0 refused");
        assert!(slot.set_unit_shield(tatepon, 9).is_err(), "tier 9 refused");
    }

    // Max the shield and the mount; verify id + CRC32 + inventory grant through disk.
    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    slot.set_unit_shield(tatepon, 8).unwrap();
    slot.set_unit_horse(kibapon, 8).unwrap();
    slot.save().unwrap();

    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    let d = slot.data();

    assert_eq!(slot.unit_shield(tatepon), Some("sld008_01"));
    let sb = ROSTER_BASE + tatepon * STRIDE;
    assert_eq!(
        u32::from_le_bytes(d[sb + SHIELD_HASH..][..4].try_into().unwrap()),
        crc32(b"sld008_01"),
        "shield CRC32"
    );
    assert_eq!(
        d[layout::shield_inventory_offset(slot.region(), 8).unwrap() + 2],
        1,
        "Divine Shield granted"
    );

    assert_eq!(slot.unit_horse(kibapon), Some("hlm008_06"));
    assert_eq!(
        d[layout::horse_inventory_offset(slot.region(), 8).unwrap() + 2],
        1,
        "Divine Horse granted"
    );

    fs::remove_dir_all(&root).ok();
    eprintln!(
        "shield + horse round-tripped (id + CRC32 + grant); helmet correctly gated to basic units"
    );
}

#[test]
fn gear_up_maxes_all_gear_and_keeps_rarepons() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("gearup");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    // Snapshot each unit's rarepon identity before gearing up.
    let before: Vec<Option<u32>> = {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        (0..slot.army_size())
            .map(|i| slot.unit_rarepon_code(i))
            .collect()
    };

    let changed = {
        let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let c = slot.max_army_gear();
        slot.save().unwrap();
        c
    };
    assert!(changed > 0, "geared up at least one unit");

    let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    for (i, &rarepon_before) in before.iter().enumerate() {
        let max = slot.unit_weapon_max_tier(i).unwrap();
        let weapon = slot.unit_weapon(i).unwrap();
        let tier: u8 = weapon.split('_').nth(1).unwrap().parse().unwrap();
        assert_eq!(tier, max, "unit {i} weapon at family max");

        if slot.unit_class(i) == Some("Tatepon") {
            assert_eq!(
                slot.unit_shield(i),
                Some("sld008_01"),
                "unit {i} shield maxed"
            );
        }
        if slot.unit_class(i) == Some("Kibapon") {
            assert_eq!(
                slot.unit_horse(i),
                Some("hlm008_06"),
                "unit {i} mount maxed"
            );
        }

        // Rarepon identity must be untouched.
        assert_eq!(
            slot.unit_rarepon_code(i),
            rarepon_before,
            "unit {i} rarepon identity unchanged"
        );
    }

    fs::remove_dir_all(&root).ok();
    eprintln!("gear_up maxed every unit's gear and left rarepon identities unchanged");
}

#[test]
fn add_unit_duplicates_with_gear_and_headpiece_and_caps_at_six() {
    use ptpnhex_core::save::layout;

    const ROSTER_BASE: usize = 0x20;
    const STRIDE: usize = 0x104;
    const GID: usize = 0x24;
    const GROUP: usize = 0x20;
    const HEAD_ECHO: usize = 0xD0;

    let Some(dir) = saves_dir() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR");
        return;
    };
    let root = temp_root("addunit");
    let work = working_copy(&dir.join("UCES00995_DATA46"), &root);

    // A Dekapon is the strict case: its headpiece (hlm007_07) is count-gated, so
    // a duplicate must grant it in inventory or render bald in game.
    let (dek, head_inv, head_before, n_before) = {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        let dek = (0..slot.army_size())
            .find(|&i| slot.unit_class(i) == Some("Dekapon"))
            .expect("DATA46 fields Dekapons");
        let echo = slot.data()[ROSTER_BASE + dek * STRIDE + HEAD_ECHO];
        let head_inv = layout::headpiece_inventory_offset(slot.region(), echo)
            .expect("a Dekapon's headpiece is in the headpiece table");
        (dek, head_inv, slot.data()[head_inv], slot.army_size())
    };

    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    let new_idx = slot.add_unit(dek).unwrap();
    assert_eq!(new_idx, n_before, "the duplicate appends after the army");
    assert_eq!(slot.army_size(), n_before + 1, "army grew by one");
    assert_eq!(slot.unit_class(new_idx), slot.unit_class(dek), "same class");
    assert_eq!(
        slot.unit_rarepon_code(new_idx),
        slot.unit_rarepon_code(dek),
        "same rarepon identity"
    );

    // Newborn state: group index == the fresh serial, counters zero, the per-gear
    // indices unset — a duplicate equal to a game-created unit bar its identity.
    {
        let d = slot.data();
        let nb = ROSTER_BASE + new_idx * STRIDE;
        let serial = u32::from_le_bytes(d[nb + GID..][..4].try_into().unwrap());
        let group = u32::from_le_bytes(d[nb + GROUP..][..4].try_into().unwrap());
        assert_eq!(group, serial, "group index equals the new serial");
        for off in layout::RECORD_NEWBORN_ZERO {
            let v = u32::from_le_bytes(d[nb + off..][..4].try_into().unwrap());
            assert_eq!(v, 0, "newborn counter at +{off:#x} is zero");
        }
        for off in layout::RECORD_NEWBORN_UNSET {
            let v = u32::from_le_bytes(d[nb + off..][..4].try_into().unwrap());
            assert_eq!(v, u32::MAX, "newborn gear index at +{off:#x} is unset");
        }
        assert_eq!(
            d[head_inv],
            head_before + 1,
            "the count-gated headpiece is granted for the new wearer"
        );
    }

    // Survives a re-encrypt / reopen from disk.
    slot.save().unwrap();
    {
        let slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
        assert_eq!(slot.army_size(), n_before + 1, "army size persisted");
        assert_eq!(
            slot.unit_class(new_idx),
            Some("Dekapon"),
            "Dekapon persisted"
        );
        assert_eq!(
            slot.data()[head_inv],
            head_before + 1,
            "headpiece grant persisted"
        );
    }

    // The squad caps at SQUAD_MAX: raise it to the cap, then a further add fails.
    let mut slot = SaveSlot::open(&work, &KeyProvider::Embedded).unwrap();
    while (0..slot.army_size())
        .filter(|&i| slot.unit_class(i) == Some("Dekapon"))
        .count()
        < layout::SQUAD_MAX
    {
        slot.add_unit(dek).unwrap();
    }
    assert!(
        slot.add_unit(dek).is_err(),
        "a seventh unit of a class is refused (the deploy screen holds six)"
    );

    fs::remove_dir_all(&root).ok();
    eprintln!(
        "add_unit: duplicated a Dekapon with gear + headpiece grant, capped at {}",
        layout::SQUAD_MAX
    );
}
