//! Integration tests against real PSP save directories.
//!
//! These tests read saves from the directory named by the
//! `PTPNHEX_SAVES_DIR` environment variable and skip themselves when it is
//! unset, so CI (which has no save data) stays green. Run locally with:
//!
//! ```sh
//! PTPNHEX_SAVES_DIR=/path/to/SAVEDATA cargo test --workspace
//! ```

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use ptpnhex_core::sfo::ParamSfo;

fn saves_dir() -> Option<PathBuf> {
    std::env::var_os("PTPNHEX_SAVES_DIR").map(PathBuf::from)
}

/// Every PARAM.SFO in the corpus must parse and reserialize byte-identically.
#[test]
fn param_sfo_roundtrip_is_byte_identical_for_all_real_saves() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: PTPNHEX_SAVES_DIR is not set");
        return;
    };

    let mut checked = 0usize;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path().join("PARAM.SFO");
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read(&path).unwrap();
        let sfo = ParamSfo::parse(&raw)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
        assert_eq!(
            sfo.to_bytes(),
            raw,
            "round trip is not byte-identical for {}",
            path.display()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no PARAM.SFO files found under {}",
        dir.display()
    );
    eprintln!("verified byte-identical round trip for {checked} files");
}

/// Patapon saves must expose the expected metadata through typed accessors.
#[test]
fn patapon_sfo_values_are_readable() {
    let Some(dir) = saves_dir() else {
        eprintln!("skipped: PTPNHEX_SAVES_DIR is not set");
        return;
    };

    let mut checked = 0usize;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let dir_path = entry.unwrap().path();
        if !dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("UCES00995"))
        {
            continue;
        }
        let raw = std::fs::read(dir_path.join("PARAM.SFO")).unwrap();
        let sfo = ParamSfo::parse(&raw).unwrap();
        assert_eq!(sfo.get_str("TITLE"), Some("PATAPON"));
        assert_eq!(sfo.get_str("CATEGORY"), Some("MS"));
        let params = sfo.get("SAVEDATA_PARAMS").unwrap();
        assert_eq!(params.data().len(), 0x80);
        checked += 1;
    }
    assert!(
        checked > 0,
        "no Patapon saves found under {}",
        dir.display()
    );
    eprintln!("verified SFO values for {checked} Patapon saves");
}
