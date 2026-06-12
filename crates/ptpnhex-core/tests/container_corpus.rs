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
        slot.save_without_backup().unwrap();

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

    // The backup preserves the pre-edit ciphertext.
    assert!(work.join("SECURE.BIN.bak").exists());

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
    slot.save_without_backup().unwrap();
    assert_eq!(fs::read(work.join("SECURE.BIN")).unwrap(), secure);
    assert_eq!(fs::read(work.join("PARAM.SFO")).unwrap(), sfo);

    fs::remove_dir_all(&root).ok();
    eprintln!("SaveSlot edit round-tripped through disk with correct rehashing");
}
