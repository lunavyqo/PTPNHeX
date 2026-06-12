//! Crypto validation against a real save corpus.
//!
//! Requires two environment variables and self-skips when either is missing,
//! so CI (which has neither saves nor a key) stays green:
//!
//! ```sh
//! PTPNHEX_SAVES_DIR=/path/to/SAVEDATA \
//! PTPNHEX_GAMEKEY=01af6f00020070d52e2412c7e1ff83ba \
//! cargo test -p ptpnhex-core --test crypto_corpus
//! ```
//!
//! No real save or key material is committed; both are supplied locally.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use ptpnhex_core::crypto::{
    decrypt_secure, encrypt_secure, file_list_hash, params_hash, ParamsHashField, SECURE_HEADER_LEN,
};

// Constant layout across the UCES00995 corpus (all PARAM.SFO are identical in
// structure; verified by the SFO round-trip test).
const FILE_LIST_HASH_OFF: usize = 0x55D;
const PARAMS_OFF: usize = 0x11B0;

fn gamekey() -> Option<[u8; 16]> {
    let hex = std::env::var("PTPNHEX_GAMEKEY").ok()?;
    let hex = hex.trim();
    let mut key = [0u8; 16];
    for (i, b) in key.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

fn corpus() -> Option<(PathBuf, [u8; 16])> {
    let dir = std::env::var_os("PTPNHEX_SAVES_DIR").map(PathBuf::from)?;
    Some((dir, gamekey()?))
}

fn patapon_saves(dir: &PathBuf) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
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

#[test]
fn decrypt_then_reencrypt_is_byte_identical() {
    let Some((dir, key)) = corpus() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR and PTPNHEX_GAMEKEY");
        return;
    };
    let mut n = 0;
    for save in patapon_saves(&dir) {
        let blob = std::fs::read(save.join("SECURE.BIN")).unwrap();
        let header: [u8; 16] = blob[..SECURE_HEADER_LEN].try_into().unwrap();
        let plaintext = decrypt_secure(&blob, &key).unwrap();
        // Structured plaintext, not noise: real saves are mostly zero.
        let zeros = plaintext.iter().filter(|&&b| b == 0).count();
        assert!(
            zeros * 2 > plaintext.len(),
            "{}: plaintext looks unstructured ({zeros}/{} zero)",
            save.display(),
            plaintext.len()
        );
        let reencrypted = encrypt_secure(&plaintext, &header, &key);
        assert_eq!(
            reencrypted,
            blob,
            "round trip differs for {}",
            save.display()
        );
        n += 1;
    }
    assert!(n > 0, "no saves found");
    eprintln!("byte-identical decrypt/encrypt round trip on {n} saves");
}

#[test]
fn reproduces_stored_hashes() {
    let Some((dir, key)) = corpus() else {
        eprintln!("skipped: set PTPNHEX_SAVES_DIR and PTPNHEX_GAMEKEY");
        return;
    };
    let mut n = 0;
    for save in patapon_saves(&dir) {
        let secure = std::fs::read(save.join("SECURE.BIN")).unwrap();
        let sfo = std::fs::read(save.join("PARAM.SFO")).unwrap();

        // File-list hash (over the encrypted SECURE.BIN, with the game key).
        let stored = &sfo[FILE_LIST_HASH_OFF..FILE_LIST_HASH_OFF + 16];
        assert_eq!(
            file_list_hash(&secure, &key),
            stored,
            "file hash: {}",
            save.display()
        );

        // Params +0x10 (mode 1): computed over the SFO with +0x10 zeroed.
        let mut img = sfo.clone();
        img[PARAMS_OFF + 0x10..PARAMS_OFF + 0x20].fill(0);
        assert_eq!(
            params_hash(&img, ParamsHashField::Hash10),
            &sfo[PARAMS_OFF + 0x10..PARAMS_OFF + 0x20],
            "params 0x10: {}",
            save.display()
        );

        // Params +0x70 (mode 5): computed with +0x10 and +0x70 zeroed.
        let mut img = sfo.clone();
        img[PARAMS_OFF + 0x10..PARAMS_OFF + 0x20].fill(0);
        img[PARAMS_OFF + 0x70..PARAMS_OFF + 0x80].fill(0);
        assert_eq!(
            params_hash(&img, ParamsHashField::Hash70),
            &sfo[PARAMS_OFF + 0x70..PARAMS_OFF + 0x80],
            "params 0x70: {}",
            save.display()
        );
        n += 1;
    }
    assert!(n > 0, "no saves found");
    eprintln!("reproduced file + params hashes on {n} saves");
}
