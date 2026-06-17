# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Documentation

- Documented the **bonus-Patapon** unlocks in `docs/save-format.md`: the revive/
  unlock flags clustered around `0x1AD71` (with `0x1AD71` bit 6, the Sandy Paradise
  gate, hardware-confirmed to open the fifth minigame and Kibapon production) and
  the separate dialog-seen flags in the `0x1AD9C`/`0x1AD9D` cluster. Replaced the
  byte-granular accumulator/volatile classification with a **bit-precise** one (the
  mixed bytes `0x1AD71`, `0x1AD84`–`85`, `0x1AD88`–`8A` hold real unlock bits next
  to volatile ones), and corrected the earlier claim that the mountain minigame was
  not a permanent unlock — it is `0x1AD71` bit 6, which a byte-granular pass had
  wrongly excluded.
- Completed the inventory map in `docs/save-format.md`: documented the two hidden
  categories — **Caps** (`0x19D34`–`0x19D48`, red background; the death cap plus
  five minigame-unlock caps) and **Trophies** (purple background; boss/enemy heads
  and Meden) — the gap items (Iron Shield/Bow, Fast Horse, Ancient Horn, Gong's
  Helm), the special items (Spear of Protection, the hack-only Late Tatepon/Yumipon
  weapons), a developer-placeholder weapon family (one Divine-model per category,
  −1 HP), the unwearable helm placeholders, the "(delete)" removed-content helms,
  and the table's true extent (null padding after `0x19FE8`). Corrected the earlier
  "the slots after the key items freeze the altar" note: only two of those eight
  (`0x19D4C`, `0x19D50`) are invalid; the other six are the Cap category.
- Documented the per-unit **rarepon** field in `docs/save-format.md`: the `u32`
  at record offset `+0x48` holds the rarepon id (a 32-bit name-hash) that sets a
  unit's body/appearance, with a table of the confirmed body codes (Barsala,
  Mogyoon, Tikulee, Mofeel, Pyokola, Gekolos, and none/basic). Hardware-confirmed
  in both directions and cross-class. Also noted that the `u32` at `+0xC4` is the
  equipped helmet's name-hash, not a unit attribute.
- Documented the army roster array in `docs/save-format.md`: the fixed
  123-record (`0x104`-byte) array from `0x0020` with the first *N* filled (the
  army size, also at `0x14`, grows 5→27 across the corpus), the per-record
  field layout (class, equipped weapon/helmet/shield, numeric stats), the six
  unit classes mapped by weapon family, and the note that the records near
  `0x30000` are a reordered formation rather than a copy. Corrects the earlier
  coarse `0x1000`–`0x19000` estimate and the "second copy" assumption.
- Expanded the *Progress and mission unlocks* section of `docs/save-format.md`
  with the byte-level decode of the `0x1AD70`–`0x1ADB0` unlock bitfields
  (separating the unlock-accumulator bytes from volatile current-state),
  documented that this region is the master unlock table behind drums, unit
  building, missions, and boss missions (confirmed by a forward "unlock
  everything" hardware test), and mapped the mission-prep loadout slots (miracle
  and stew) to bit 0 of `0x1A0F0`.
- Expanded `docs/save-format.md` into a full save-format reference: complete
  per-item and per-key-item offset tables (every offset, not just category
  summaries), and a new *Progress and mission unlocks* section documenting the
  unit roster array, the mission counter at `0x0`, and the unlock bitfields at
  `0x1AD70`–`0x1ADB0` (the mission-gate mechanism behind drums, the miracle/stew
  slots, and unit-building).

### Added

- Unit rarepon editing: `SaveSlot::army_size` / `unit_class` / `unit_rarepon` /
  `unit_rarepon_code` / `set_unit_rarepon` read and edit each roster unit's
  rarepon (the `u32` at record offset `+0x48`), with a `Rarepon` catalog of the
  confirmed variants (Barsala, Mogyoon, Tikulee, Mofeel, Pyokola, Gekolos, and
  basic). Exposed on the CLI as `units` (list the army with each unit's class and
  rarepon) and `set-rarepon <index> <slug>`. Editing only `+0x48` yields a
  body-only hybrid, matching the hardware-confirmed behaviour.
- Progression unlocks: `SaveSlot::unlock_all` forces every confirmed unlock —
  all drums, every buildable unit type (including the mission-gated classes such
  as Kibapon), the full mission list, all boss missions, and every bonus-Patapon
  minigame — by OR-ing the unlock-accumulator masks into the `0x1AD70`–`0x1ADB0`
  bitfields. The masks are **bit-precise**: where a byte mixes accumulator bits
  with volatile current-state bits, only the accumulator bits are set, so current
  state is left intact and OR-ing can only add unlocks. Exposed on the CLI as
  `unlock-all`. Confirmed by forward unlock-everything tests on real hardware,
  including the `0x1AD71` bit 6 gate (Sandy Paradise) that opens the fifth bonus
  minigame and Kibapon production — the one the earlier byte-granular mask missed.
- Loadout-slot editing: `SaveSlot::loadout_slots` / `set_loadout_slots` open or
  close the mission-prep miracle and stew slots (one flag, bit 0 of `0x1A0F0`,
  controls both). Exposed on the CLI as `set-loadout-slots <on|off>`. Located by
  hardware bisection; this flag is separate from the unlock bitfields.
- Key-item editing: `SaveSlot::key_item` / `key_items` / `set_key_item` over the
  19 altar tokens at the head of the inventory table — 4 drums, 4 miracles, 5
  songs, and 6 quest items. These are one-per tokens, so editing toggles their
  owned flag. What the flag does in-game depends on the category (hardware
  tested): for **songs** it is the "command learned" gate (removing a scroll
  disables that command, given the drums); for **miracles** it makes a miracle
  castable, but only after the story has opened the mission miracle slot; for
  **quest items** it opens the matching hidden boss fight (adding all six revealed
  six hidden missions); for **drums** it is cosmetic (button availability is
  story-gated). The underlying
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
