# Patapon save format (decrypted `SECURE.BIN`)

This document maps the layout of a Patapon save *after* decryption. For how the
save is decrypted and re-sealed, see `docs/crypto.md`; this file is about what
the plaintext payload actually contains.

It is a living document — fields are added as they are confirmed, each with the
evidence that confirms it. Offsets are into the decrypted payload and are
little-endian unless noted. Region is **Europe (`UCES00995`)** unless stated;
US/JP layouts are not yet mapped.

## How a save is identified

On the PSP every save shows as the title **"Game data"**; you tell them apart by
the detail text:

```
Almighty: <player name>
Adventure:  - <current location> -
Play time: HH:MM:SS
```

The save *directory* number (`..._DATA01`, `..._DATA17`, …) is **not** the
on-screen order and **not** chronological — a low number can be a late save.
To reconstruct play order, sort by the `Play time` value. (`DATA00` is a small
system save, not a normal game save, and does not contain the fields below.)

## Overall structure

A full game save's payload is 205520 bytes and falls into three broad regions:

| range             | contents                                                    |
| ----------------- | ----------------------------------------------------------- |
| `0x0000`–`0x0020` | header: counters and progress values (see below)           |
| `0x0020`–`0x7D0C` | **army roster** — fixed-size unit records (see below)      |
| `0x19CE8`–`0x1A0E0`| **inventory table** — fixed per-item records (see below)    |
| `0x1A0EC`–end     | numeric stats (ka-ching), progress flags, and other fields |

### The army roster array

From `0x0020` the save holds the player's army as a regular array of
**fixed-size records, one every `0x104` (260) bytes**, with no exceptions. The
array has a **fixed capacity of 123 records** (so it reserves `0x0020`–`0x7D0C`);
only the first *N* are filled, where *N* is the current army size — `5` units on
a fresh save, growing monotonically to `27` by the end of the corpus. The
remaining records are zeroed reserve.

The header just before it carries the army size: the `u32` at **`0x14`** is the
unit count (matching the filled-record count on most saves), and the constant
`123` at `0x08`/`0x0C` is the capacity. (`0x00` is the missions counter; `0x04`
is a constant `2`.)

Each record carries ASCII identifiers for a unit and its equipped gear at fixed
record-relative offsets, each preceded by a 4-byte hash:

| offset | field |
| --- | --- |
| `+0x00` | a name slot (`none` when unnamed) |
| `+0x50` | `unitNNN_01_01` — the class |
| `+0x74` | `wpnNNN_III_VV` / `rwpnNNN_III_VV` — equipped weapon (or rare weapon) |
| `+0xA4` | `hlmNNN_II` — equipped helmet |
| `+0xD4` | `sldNNN_II` — equipped shield (shield units only) |

```
unit002_01_01   wpn001_008_01   hlm014_01          (a Yaripon + spear + helmet)
unit003_01_01   rwpn003_009_01  hlm015_01  sld008_01 (a Tatepon + rare weapon + shield)
```

The `NNN` in a class id selects the unit type, identified by the weapon family it
carries:

| class id | weapon | unit |
| --- | --- | --- |
| `unit002` | `wpn001` (spears) | Yaripon |
| `unit003` | `rwpn003` + `sld` | Tatepon |
| `unit004` | `wpn004` (bows) | Yumipon |
| `unit006` | `wpn006` | Kibapon |
| `unit007` | `wpn007` (hammers) | Dekapon |
| `unit008` | `wpn008` (horns) | Megapon |

The equipped-gear ids reuse the **same family/index taxonomy as the inventory
armory** — `wpn001_008` is the spear family, item 8 (the eighth spear catalogued
under *Items*) — so units and the armory share one item-id space. Between the
identifiers each record also holds numeric fields (unit level and stats); those
are not yet individually decoded. (One field nearby, the `u32` at `+0xC4`, is the
**name-hash of the equipped helmet** — `hlm015`→`0xDA216E8F`, `hlm014`→`0x629D09EA`,
and so on — not a unit attribute.)

#### Rarepon (the `u32` at `+0x48`)

The `u32` at record offset `+0x48` (just before the class id) is the unit's
**rarepon** — the special variant that sets its body/appearance. It was confirmed
on hardware in both directions: writing another rarepon's code makes the body
change to that rarepon (working across every class), and recreating the unit as a
plain Barsala reverts the code. Each value is a 32-bit name-hash (the high byte is
always `0xFF`); the codes are shared across classes, so the same value is the same
rarepon whether on a Yaripon, a Megapon, or any other unit:

| code (`u32` LE) | rarepon |
| --- | --- |
| `0xFFCDFEBE` | Barsala |
| `0xFFC06E9F` | Mogyoon |
| `0xFFA96D65` | Tikulee |
| `0xFFF898CF` | Mofeel |
| `0xFF356EEF` | Pyokola |
| `0xFF61E4DA` | Gekolos |
| `0xFFFFFFFF` | none / basic |

This is the only per-unit field that tracks the rarepon: a unit's displayed name,
headpiece, and base stats are derived from this id rather than stored alongside it,
so editing `+0x48` alone produces an otherwise-Barsala unit wearing a different
rarepon's body — a combination the game cannot normally create.

A second, **smaller** set of unit records appears near `0x30000`. It is **not** a
copy of the roster: it holds verbatim copies of *some* unit records but in a
different order and count (on the endgame save, 27 roster records versus 21 here,
with a different class composition), so it is most likely the **deployed
battle-formation** arrangement rather than a backup. Its exact layout is not yet
mapped.

## Confirmed fields

| field    | offset    | type      | range   | notes                         |
| -------- | --------- | --------- | ------- | ----------------------------- |
| Ka-ching | `0x1A0EC` | u32 (LE)  | 0–99999 | in-game currency              |

**Ka-ching** was confirmed by reading the value off the game's screen for two
saves and finding the offset that uniquely held each value (`DATA01` = 564,
`DATA50` = 598). It behaves like currency across the whole corpus — it rises and
falls and never exceeds the game's 99999 cap.

## The inventory (materials and items)

Stackable items (materials, stews, and other consumables) live in a **fixed
table** of 4-byte records. For Europe it spans `0x19CE8..0x1A0E0` (254 records):
its start is identical across the whole save corpus, and it ends exactly where
ka-ching begins (`0x1A0EC`).

The defining property is that **every item has a stable byte offset** — the
offset *is* the item's identity. A record is:

```
byte 0: count          (u8)   — quantity (materials are capped at 99)
byte 1: new            (u8)   — the in-game "NEW!" flashing indicator (1 = new)
byte 2: owned          (u8)   — 1 = owned, 0 = never obtained
byte 3: display-index  (u8)   — a per-save cosmetic counter — IGNORE
```

Owning or not owning an item only flips its `owned` flag (and count) **in
place**; the record never moves. The `display-index` byte *does* change between
saves — it is a separate running number the game assigns to owned and not-owned
items for menu display — but it is not the item's identity and must not be used
to locate anything. (Trusting that byte is what produced two earlier wrong
models and the "set Magic Alloy → got stews" bug.)

### Materials

The 20 crafting materials occupy 20 fixed offsets, in canonical order
(Leather Meat … Magic Alloy):

| offsets             | materials                                                     |
| ------------------- | ------------------------------------------------------------- |
| `0x19D54`–`0x19D60` | Leather Meat, Tender Meat, Dream Meat, Mystery Meat           |
| `0x19D64`–`0x19D70` | Stone, Hard Iron, Tytanium Ore, Mytheerial                    |
| `0x19D78`–`0x19D84` | Banal Branch, Cherry Tree, Hinoki, Super Cedar                |
| `0x19D88`–`0x19D94` | Eyeball Cabbage, Crying Carrot, Predator Pumpkin, Hazy Shroom |
| `0x19D98`–`0x19DA4` | Sloppy Alloy, Hard Alloy, Awesome Alloy, Magic Alloy          |

The records are contiguous from `0x19D54` except for one non-material slot at
`0x19D74` (a not-owned marker, present in every save). These offsets reproduce
the 20 counts confirmed in-game for one save (`DATA46`) exactly, and a
controlled before/after on real hardware — obtaining one Magic Alloy, which cost
one Mytheerial — confirmed them again: Magic Alloy's record flipped to owned at
`0x19DA4` and Mytheerial dropped by one at its own offset, both in place.
Finally, writing each material a distinct count (`1`–`20` in this order) and
reading them back in-game matched **every** material by name — so each offset is
individually confirmed, not just the set as a whole.

**Editing semantics.** A material is read from its `count` byte when its `owned`
flag is set, otherwise it is reported as absent (`0`). Editing covers three
cases, all confirmed in-game:

- **Owned (any count, including 0):** write the `count` byte. Touching only
  `byte 0` leaves the `new` flag as it was — after setting every material to 99,
  only a freshly picked-up item (`byte 1 = 1`) kept flashing "NEW!".
- **Never obtained (`owned = 0`): add it.** Set `owned = 1`, write the `count`,
  and set `new = 1` so it flashes like a real pickup. The `display-index` byte
  is left as-is — the game recomputes it. This was proven by writing the 20
  materials' display indices *reversed*: in-game the menu still showed them in
  normal order, and an in-game re-save rewrote the indices back to canonical.
  Adding the 17 materials an early save (`DATA04`) had never obtained then
  showed all of them in-game at the set count.

Because the game owns the `display-index` byte, editing never writes it.

### Items (stews, Memories, weapons, gear)

The slots after the materials hold the player's consumables and **armory**, in
the same record format. All 83 were mapped by writing each slot a distinct count
and reading the result back in-game by name. The slots are not contiguous —
unused/never-obtained slots sit between the categories. Full per-item offsets
(Europe), in catalog order:

| offset    | item                          | category    |
| --------- | ----------------------------- | ----------- |
| `0x19DA8` | Gnarly Stew                   | Stew        |
| `0x19DAC` | Tasty Stew                    | Stew        |
| `0x19DB0` | King's Stew                   | Stew        |
| `0x19DB4` | Divine Stew                   | Stew        |
| `0x19DB8` | Yaripon's Memory              | Memory      |
| `0x19DBC` | Tatepon's Memory              | Memory      |
| `0x19DC0` | Yumipon's Memory              | Memory      |
| `0x19DC4` | Kibapon's Memory              | Memory      |
| `0x19DC8` | Dekapon's Memory              | Memory      |
| `0x19DCC` | Megapon's Memory              | Memory      |
| `0x19E28` | Wooden Spear                  | Spear       |
| `0x19E2C` | Iron Spear                    | Spear       |
| `0x19E30` | Steel Spear                   | Spear       |
| `0x19E34` | Scorching Spear               | Spear       |
| `0x19E38` | Dokaknel's Fang               | Spear       |
| `0x19E3C` | Ancient Spear                 | Spear       |
| `0x19E40` | Giant Spear "Bullet"          | Spear       |
| `0x19E44` | Divine Spear                  | Spear       |
| `0x19E50` | Tin Axe                       | Sword       |
| `0x19E54` | Iron Sword                    | Sword       |
| `0x19E58` | Steel Axe                     | Sword       |
| `0x19E5C` | Sleep Sword                   | Sword       |
| `0x19E60` | Flame Sword                   | Sword       |
| `0x19E64` | Ancient Axe                   | Sword       |
| `0x19E68` | Ancient Sword "The Butcher"   | Sword       |
| `0x19E6C` | Divine Sword                  | Sword       |
| `0x19E70` | Gong's Scythe                 | Scythe      |
| `0x19E78` | Wood Shield                   | Shield      |
| `0x19E80` | Steel Shield                  | Shield      |
| `0x19E84` | Ice Shield                    | Shield      |
| `0x19E88` | Ultra Heavy Shield            | Shield      |
| `0x19E8C` | Ancient Shield                | Shield      |
| `0x19E90` | Giant Shield "Octagon"        | Shield      |
| `0x19E94` | Divine Shield                 | Shield      |
| `0x19EA0` | Wooden Bow                    | Bow         |
| `0x19EA8` | Steel Bow                     | Bow         |
| `0x19EAC` | Flame Bow                     | Bow         |
| `0x19EB0` | Piercing Bow                  | Bow         |
| `0x19EB4` | Ancient Bow                   | Bow         |
| `0x19EB8` | Giant Bow "Failnaught"        | Bow         |
| `0x19EBC` | Divine Bow                    | Bow         |
| `0x19EC8` | Wooden Halberd                | Halberd     |
| `0x19ECC` | Iron Halberd                  | Halberd     |
| `0x19ED0` | Steel Halberd                 | Halberd     |
| `0x19ED4` | Deflecting Halberd            | Halberd     |
| `0x19ED8` | Flame Halberd                 | Halberd     |
| `0x19EDC` | Ancient Halberd               | Halberd     |
| `0x19EE0` | Giant Halberd "Grizzly"       | Halberd     |
| `0x19EE4` | Divine Halberd                | Halberd     |
| `0x19EF0` | Horse                         | Horse       |
| `0x19EF8` | Tough Horse                   | Horse       |
| `0x19EFC` | Strong Horse                  | Horse       |
| `0x19F00` | Crimson Horse                 | Horse       |
| `0x19F04` | Ancient Horse                 | Horse       |
| `0x19F08` | Deep Impact                   | Horse       |
| `0x19F0C` | Divine Horse                  | Horse       |
| `0x19F18` | Club                          | Hammer      |
| `0x19F1C` | Iron Hammer                   | Hammer      |
| `0x19F20` | Steel Mace                    | Hammer      |
| `0x19F24` | Nail Studded Bat              | Hammer      |
| `0x19F28` | Dream Weaver                  | Hammer      |
| `0x19F2C` | Ancient Hammer                | Hammer      |
| `0x19F30` | Morning Star "Giganto"        | Hammer      |
| `0x19F34` | Divine Axe                    | Hammer      |
| `0x19F40` | Wood Horn                     | Horn        |
| `0x19F44` | Iron Horn                     | Horn        |
| `0x19F48` | Steel Horn                    | Horn        |
| `0x19F4C` | Gaeen's Horn                  | Horn        |
| `0x19F50` | Ciokin's Horn                 | Horn        |
| `0x19F54` | Shookle's Horn                | Horn        |
| `0x19F5C` | Divine Horn                   | Horn        |
| `0x19F68` | Wooden Helm                   | Helm        |
| `0x19F6C` | Iron Helm                     | Helm        |
| `0x19F70` | Steel Helm                    | Helm        |
| `0x19F74` | Wind Helm                     | Helm        |
| `0x19F78` | Strength Helm                 | Helm        |
| `0x19F7C` | Ancient Helm                  | Helm        |
| `0x19F80` | Giant Helm "Turtle"           | Helm        |
| `0x19F84` | Divine Helm                   | Helm        |
| `0x19F88` | Bunny Head                    | Animal Helm |
| `0x19FC8` | Scorpiton Helm                | Animal Helm |
| `0x19FCC` | Spiderton Helm                | Animal Helm |
| `0x19FD0` | Beetleton Helm                | Animal Helm |

(This table mirrors the `EU_ITEM_OFFSETS` array in `save/layout.rs`, the
authoritative source the editor uses.) Editing works exactly as for materials
(read/add by owned flag). Two notes from the mapping: owning a unit **Memory**
item does not by itself unlock building that unit (a separate, mission-gated flag
governs that — see *Progress and mission unlocks* below), and Divine weapons
display with the player name appended.

### Key items (drums, miracles, songs, quest items)

The records *before* the materials, at the head of the table, are a distinct
kind: **one-per unlock tokens** rather than stackable items (the count is always
1). 19 of them are real, mapped by the same distinct-count readback. They occupy
`0x19CE8`–`0x19D30`:

| offset    | item              | category |
| --------- | ----------------- | -------- |
| `0x19CE8` | Pon Drum          | Drum     |
| `0x19CEC` | Pata Drum         | Drum     |
| `0x19CF0` | Chaka Drum        | Drum     |
| `0x19CF4` | Don Drum          | Drum     |
| `0x19CF8` | Rain Miracle      | Miracle  |
| `0x19CFC` | Tailwind Miracle  | Miracle  |
| `0x19D00` | Storm Miracle     | Miracle  |
| `0x19D04` | Earthquake Miracle| Miracle  |
| `0x19D10` | Ponpata Song      | Song     |
| `0x19D24` | Patapata Song     | Song     |
| `0x19D28` | Ponpon Song       | Song     |
| `0x19D2C` | Chakachaka Song   | Song     |
| `0x19D30` | Ponchaka Song     | Song     |
| `0x19D08` | Blank Map         | Key Item |
| `0x19D0C` | Bent Compass      | Key Item |
| `0x19D14` | Dusty Crystal     | Key Item |
| `0x19D18` | Broken Sign       | Key Item |
| `0x19D1C` | Black Star        | Key Item |
| `0x19D20` | Dark Palace Model | Key Item |

(Mirrors `EU_KEY_ITEM_OFFSETS` in `save/layout.rs`. Offsets are not in ascending
order because the table is grouped by category for readable listings.)

What the owned flag *does* in-game depends on the category — hardware testing
showed three different behaviours:

- **Songs — the flag is the "command learned" gate (functional).** Removing the
  Attack (Pon-Pon-Pata-Pon) and Defend (Chaka-Chaka-Pata-Pon) scrolls from a
  progressed save disabled those commands in a mission (only March still worked);
  adding the two missing song scrolls to a save that already had the drums made
  those commands usable. A combo also needs its constituent drums to be
  available, but given the drums, the song flag teaches or un-teaches the command.
- **Miracles — the flag selects the miracle, but only after the story opens the
  slot (conditionally functional).** Flagging Earthquake or Storm on a progressed
  save (which already had the mission miracle slot) made each castable; the same
  flag on an early save with no slot does nothing.
- **Quest items — the flag is functional.** These six are location items that
  grant access to hidden boss fights. Adding all six to an early save (which had
  none) made six hidden boss missions appear on the world map, so setting the
  flag opens the corresponding fight.
- **Drums — the flag is cosmetic.** Clearing a drum's flag on a progressed save
  still leaves it usable, and forcing a drum owned on an early save does not make
  it work. Drum-button availability is governed by story progress, not this flag.

So the **prerequisites** — which drum buttons work, and whether the mission
miracle/stew slots exist at all — live in a separate **story/progress structure**
(documented under *Progress and mission unlocks* below; the unit Memories hang off
the same kind of gate: owned ≠ buildable). The miracle-*summon* command itself has
no scroll among these 19 and is purely story-gated.

The eight records that follow these 19 (`0x19D34`–`0x19D50`, up to `0x19D54` where
the materials begin) split into **six real items and two invalid slots** (see
*Caps* below): force-owning the two invalid ones (`0x19D4C`, `0x19D50`) crashes the
altar, which is why the editor exposes only valid tokens.

### Caps (`0x19D34`–`0x19D48`)

The six records between the key items and the materials are a hidden **"Cap"**
category (a **red** slot background, distinct from the Trophies' purple). They are
the caps Patapons drop, normally never held in the inventory:

| offset | item | role |
| --- | --- | --- |
| `0x19D34` | Cap | the death/revival cap (dropped on death, buried in Patapolis) |
| `0x19D38` | Pakapon's Cap | mission loot → becomes Pakapon → unlocks the tree minigame |
| `0x19D3C` | Kimpon Cap | → Kimpon + its minigame |
| `0x19D40` | Zakapon Cap | → Zakapon + its minigame |
| `0x19D44` | Kampon Cap | → Kampon + its minigame |
| `0x19D48` | Gashpon Cap | → Gashpon + its minigame |

`0x19D4C` and `0x19D50` are invalid (crash the altar when force-owned).

The "role" column describes what each cap is *in the game's fiction* (mission loot
that becomes a bonus Patapon and its minigame); it does **not** describe the effect
of the inventory flag. Force-owning a cap here only makes the item appear in the
altar — it does **not** unlock the corresponding minigame. These caps are never
meant to be held in the inventory at all (that they can be force-owned is the
anomaly), so they are datamine artifacts, not a handle on the minigame-unlock
question. The actual bonus-minigame unlocks live in the progress/story structure,
not in this inventory flag.

### Trophies, the special items, and the end of the table

Scattered among the never-obtained slots between the materials and the table's end
are a second hidden category and a family of special/unused items, all confirmed by
force-owning each slot and reading it in-game. Their exact stats will be sourced
from the community wiki rather than measured slot by slot.

- **Trophies** — a hidden category with a **purple** background and "(no
  translation needed)" descriptions, normally unobtainable. It holds the boss/enemy
  trophies (`0x19DD0`–`0x19E24`: *Head of Dodonga/Majidonga/Zaknel/Dokaknel/Gaeen/
  Dogaeen*, *Ciokina's/Cioking's Pincer*, *Head of Shookle/Shooshokle/Gorl/Motiti/
  Momoti/Motsitsi*, and the "-cheek" creatures *Kacheek/Picheek/Parcheek/Poocheek/
  Gancheek* plus terrain *Kacheek* variants) and, separately, **Meden** at
  `0x19FE8`. The category is not offset-contiguous.
- **Real items in the gaps** (just never obtained in the corpus): **Iron Shield**
  (`0x19E7C`, the 8th shield, between Wood and Steel), **Iron Bow** (`0x19EA4`),
  **Fast Horse** (`0x19EF4`), **Ancient Horn** (`0x19F58`), **Gong's Helm**
  (`0x19FA4`).
- **Legitimate but special:** **Spear of Protection** (`0x19E48`) — a real item
  with a real description, obtainable by loading Patapon 1 demo save data.
- **Hack-only weapons** (used by the tutorial bosses, name text partly coloured):
  **Sword of the Late Tatepon** (`0x19E74`) and **Bow of the Late Yumipon**
  (`0x19EC0`).
- **A developer-placeholder weapon family** — one per category, each with a "(no
  translation needed)" name, the **Divine model** of its category, and a single
  stat effect of **−1 HP**: bow (`0x19EC4`), spear (`0x19E4C`), halberds
  (`0x19EE8`, `0x19EEC`), hammers (`0x19F38`, `0x19F3C`, Divine *Axe* model), and
  horns (`0x19F60`, `0x19F64`). Two anomalies: the placeholder **horses** (`0x19F10`,
  `0x19F14`) have a unique custom model (not borrowed), and the placeholder
  **horns** render misplaced (the model floats up-and-right). The two placeholder
  **shields** (`0x19E98`, `0x19E9C`) share the −1 HP signature with the textures
  `仮` and a white dot-circle.
- **Unwearable helms** — `0x19F8C`–`0x19FA0` and `0x19FA8`–`0x19FC4` (14 slots) are
  helm-icon placeholders that cannot be equipped.
- **Removed items** — `0x19FD4`–`0x19FE4` are five **"(delete)"** helms (a distinct
  placeholder string for removed content).
- **End of the table** — after `0x19FE8` (Meden), the slots `0x19FEC`–`0x1A0DC` are
  **null padding**: force-owning them has no effect (they render as empty cells), so
  the item table effectively ends there.

Three placeholder-string classes distinguish the slot kinds: **"(no translation
needed)"** = present but unlocalized (unused/dev/hack), **"(delete)"** = removed
item, and an altar **crash** = an invalid slot.

### A note on the in-game display

The item menu shows a **fixed, non-scrolling grid** of 4 columns × 33 rows (132
cells; 4 items per row), the same on the altar inventory and the mission-prep
gear screen. Items beyond that window still exist and function — gear stays equipped, and "optimize loadout"
considers the full list — but they cannot be selected by hand in the menu. So an
edit that adds a very large number of items is safe for the save, yet can push
some items out of reach in the menu; a tidy, in-window inventory is the better
default.

## Progress and mission unlocks

Most of what looks "owned but unusable" — drum buttons, the mission miracle/stew
slots, which unit types can be built — is governed not by the inventory but by a
separate **progression system**. Several pieces of it are mapped: the unit roster
array, the mission counter, the master unlock bitfields, and the mission-prep
loadout-slot flag.

### The army roster array (`0x0020`–`0x7D0C`)

From `0x0020` the save holds the player's army as fixed `0x104`-byte unit records
(detailed under *Overall structure*): a 123-record capacity with the first *N*
filled, where *N* (the army size, also at `0x14`) grows from 5 to 27 across the
corpus as units are recruited. Each record names the unit's class and its
equipped weapon, helmet, and shield, with numeric level/stat fields between. A
separate, reordered set of unit records near `0x30000` is most likely the
deployed formation, not a copy. (This is the roster, not the *buildable* gate —
unit-building is gated by the unlock bitfields below.)

### The mission counter (`0x0`)

The byte at `0x0` is a monotonic counter of missions/battles completed (it climbs
across a playthrough and never decreases).

### The unlock bitfields (`0x1AD70`–`0x1ADB0`)

After ka-ching, the region `0x1A0F0+` mixes volatile current-state (the available-
mission list, current map) with **persistent unlock bitfields** at roughly
`0x1AD70`–`0x1ADB0`. These accumulate set bits as the player progresses and
approach all-`0xFF` near 100% completion — **each bit corresponds to one unlock**
(a learned command, an opened mission, etc.). For example the Chaka drum's unlock
is bits in `0x1AD78`/`0x1AD87`.

This is the **master unlock table**, and copying it forward unlocks almost
everything at once. A controlled hardware test — OR-ing a near-complete save's
unlock bytes onto an early save — simultaneously made **all four drums** usable,
**every unit type buildable**, the **entire mission list** available, and **all
boss missions** open (the boss missions came on with *no* location items in the
inventory, so the quest items merely set these same bits), with nothing visibly
desynced. So drum availability, unit-building, mission/boss availability, and most
minigames are all gated here, not by separate per-feature flags.

Classifying every byte by whether its bits are strictly accumulating across the
chronological corpus separates the genuine unlock bits from volatile state that
happens to sit in the same span:

| Bytes | Kind |
| --- | --- |
| `0x1AD72`–`74`, `0x1AD77`–`7D`, `0x1AD86`–`87`, `0x1AD8B`–`8E`, `0x1AD94`–`9D`, `0x1AD9F`–`A1` | **Unlock accumulators** — bits only ever set, never cleared; the real unlock table |
| `0x1AD71`, `0x1AD84`–`85`, `0x1AD88`–`8A`, `0x1AD9E`, `0x1ADAF` | Volatile current-state — bits clear from save to save |
| remainder | Constant `0x00` padding |

Only the accumulator bytes should be copied to "unlock everything"; the volatile
bytes are left as the target save's own to avoid desyncing its current state. The
mechanism was confirmed both directions on hardware — clearing a save's Chaka
unlock bits (`0x1AD78`/`0x1AD87`, both accumulator bytes) **disabled** the drum in
a mission (the token still showed), and writing the captured "learned" bits onto a
save that never had Chaka **enabled** it. Nearby, a separate `u32` array at
`0x1A630+` holds per-category play counts (not unlock flags).

### The mission-prep loadout slots (`0x1A0F0`)

The slots that hold a **miracle and a stew** during mission preparation are *not*
in the unlock bitfields above — copying the full `0x1AD70`–`0x1ADB0` region leaves
them closed. They are gated by a single flag: **bit 0 of `0x1A0F0`**. Setting it
opens *both* slots together (a save with the bit clear shows neither slot; setting
only this bit restores both); with the slots open, which miracles are castable
then follows from the miracle tokens in the inventory. The flag first sets early in
the story (around the fifth mission) and persists thereafter.

Not yet decoded: the meaning of each *individual* unlock bit (which bit opens which
specific mission/command). One feature is known **not** to live here — the mountain
minigame stays locked even with the whole unlock region and every nearby persistent
flag set, so it appears to be gated by volatile current chapter/map state rather
than a permanent unlock bit.

## How fields are confirmed

Two complementary methods:

1. **Anchoring.** Read a known value off the game's screen, then search the
   decrypted save for that exact value. A unique match pinpoints the field; a
   second known value removes any doubt. (This is how ka-ching was found —
   blind structural diffing alone could not label it.)
2. **Controlled diffing.** Change exactly one thing in-game (or compare two
   saves that differ by one event) and diff the payloads; the bytes that move
   identify the field. The chronological save corpus provides many such
   before/after pairs for free.
