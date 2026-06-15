# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Key-item editing: `SaveSlot::key_item` / `key_items` / `set_key_item` over the
  19 altar tokens at the head of the inventory table — 4 drums, 4 miracles, 5
  songs, and 6 quest items. These are one-per tokens, so editing toggles their
  owned flag. What the flag does in-game depends on the category (hardware
  tested): for **songs** it is the "command learned" gate (removing a scroll
  disables that command, given the drums); for **miracles** it makes a miracle
  castable, but only after the story has opened the mission miracle slot; for
  **drums** it is cosmetic (button availability is story-gated). The underlying
  prerequisites — drum availability and the mission miracle/stew slots — live in
  a separate story/progress structure, not the inventory. Only the 19 valid
  tokens are exposed (the unused slots after them freeze the altar). Exposed on
  the CLI as `key-items` (list, grouped by category) and
  `set-key-item <slug|all> <on|off>`.
- Item editing: `SaveSlot::item` / `items` / `set_item` over the 83 inventory
  items after the materials — 4 stews, the 6 unit Memories, and the full
  weapon/gear armory (spears, swords, scythe, shields, bows, halberds, horses,
  hammers, horns, helms, animal helms). They share the materials' fixed-table
  record, so counts edit in place and a never-obtained item is added. Mapped by
  writing each slot a distinct count and reading it back in-game. Exposed on the
  CLI as `items` (list, grouped by category) and `set-item <slug|all> <count>`.
- Save-list label editing: `set-title` and `set-detail` set the `SAVEDATA_TITLE`
  and `SAVEDATA_DETAIL` strings the PSP shows for a save (handy for telling saves
  apart, since the folder number is not the on-screen order). These are display
  labels — the game regenerates the detail from its own data on its next save —
  and editing them leaves `SECURE.BIN` untouched.
- Materials editing: `SaveSlot::material` / `materials` / `set_material` over
  the reverse-engineered inventory table, with all 20 crafting materials located
  by their fixed record offset and an owned flag (verified against the save
  corpus and controlled before/after experiments on real hardware). Counts edit
  in place (capped at 99); a material the player never obtained is **added** by
  setting its owned flag (the game recomputes the menu ordering itself), so
  `set-material … all 99` completes the whole list. Exposed on the CLI as
  `materials` (list) and `set-material <name|all> <count>`.
- `ptpnhex` command-line interface with `info` (region, save title/detail, and
  ka-ching) and `set-kaching` (write a new value); editing commands take an
  optional `--backup-dir <DIR>` to copy the originals outside the save folder
  first.
- Ka-ching (currency) editing: `SaveSlot::kaching` / `set_kaching`, backed by a
  data-driven field layout and confirmed against real saves. First entry in the
  reverse-engineered `docs/save-format.md`.
- `SaveSlot` container that opens a save directory, decrypts it for editing,
  and writes it back — re-encrypting and regenerating the integrity hashes.
  `save` writes only `SECURE.BIN` and `PARAM.SFO` into the save folder (a real
  PSP rejects a save directory that contains any other file); `back_up_to`
  copies the originals to a directory outside the save folder on request. The
  Patapon EU game key is embedded, so opening a European save needs no setup.
- Mode-5 `SECURE.BIN` cryptography in `ptpnhex-core::crypto`: the keystream
  cipher (decrypt and encrypt) and the AES-CMAC integrity hashes, verified
  byte-for-byte against a real save corpus through opt-in integration tests.
- Region model and an isolated, feature-gated game-key provider, plus
  documentation of the validated save-data encryption scheme (the keystream
  cipher, KIRK key-vault constants, CMAC hashes, and the mode-6 limitation).
- `PARAM.SFO` parser and writer with a byte-identical round-trip guarantee,
  typed accessors for string and integer entries, and bounded setters for
  save titles and descriptions.
- Project scaffolding: Cargo workspace with `ptpnhex-core`, `ptpnhex-cli`, and
  `ptpnhex-gui` crates, continuous integration, and contribution guidelines.
