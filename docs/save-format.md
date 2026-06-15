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
| `0x0000`–`0x1000` | header: assorted counters and progress values              |
| `0x1000`–`0x19000`| **unit / equipment array** — fixed-size records (see below) |
| `0x19CE8`–`0x1A0E0`| **inventory table** — fixed per-item records (see below)    |
| `0x1A0EC`–end     | numeric stats (ka-ching) and other fields                  |

### The unit / equipment array

From `0x1000` the save holds a regular array of **fixed-size records, one every
`0x104` (260) bytes**, with no exceptions. Each record carries ASCII identifiers
for a unit and its gear, for example:

```
unit004_01_01   wpn004_003_01   hlm014_01          (a unit + weapon + helmet)
unit003_01_01   rwpn003_009_01  hlm015_01  sld008_01 (… with a rare weapon + shield)
```

Identifier prefixes: `unit` (unit type), `wpn` / `rwpn` (weapon / rare weapon),
`hlm` (helmet), `sld` (shield). Because every record has the same shape,
decoding one record decodes them all.

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
the same record format. 83 of them were mapped by writing each slot a distinct
count and reading the result back in-game by name. In catalog order:

| count | category    | first offset | items                                   |
| ----- | ----------- | ------------ | --------------------------------------- |
| 4     | Stews       | `0x19DA8`    | Gnarly, Tasty, King's, Divine           |
| 6     | Memories    | `0x19DB8`    | Yari/Tate/Yumi/Kiba/Deka/Megapon's      |
| 8     | Spears      | `0x19E28`    | Wooden … Divine Spear                    |
| 8     | Swords/Axes | `0x19E50`    | Tin Axe … Divine Sword                   |
| 1     | Scythe      | `0x19E70`    | Gong's Scythe                            |
| 7     | Shields     | `0x19E78`    | Wood … Divine Shield                     |
| 7     | Bows        | `0x19EA0`    | Wooden … Divine Bow                      |
| 8     | Halberds    | `0x19EC8`    | Wooden … Divine Halberd                  |
| 7     | Horses      | `0x19EF0`    | Horse … Divine Horse                     |
| 8     | Hammers     | `0x19F18`    | Club … Divine Axe                        |
| 7     | Horns       | `0x19F40`    | Wood … Divine Horn                       |
| 8     | Helms       | `0x19F68`    | Wooden … Divine Helm                     |
| 4     | Animal Helms| `0x19F88`    | Bunny Head, Scorpiton, Spiderton, Beetleton |

The slots are not contiguous — unused/never-obtained slots sit between the
categories. The exact per-item offsets are the `EU_ITEM_OFFSETS` table in
`save/layout.rs`. Editing works exactly as for materials (read/add by owned
flag). Two notes from the mapping: owning a unit **Memory** item does not by
itself unlock building that unit (a separate, mission-gated flag governs that,
not yet found), and Divine weapons display with the player name appended.

### Key items (drums, miracles, songs, quest items)

The records *before* the materials, at the head of the table, are a distinct
kind: **one-per unlock tokens** rather than stackable items (the count is always
1). 19 of them are real, mapped by the same distinct-count readback, grouped:

| count | category  | items                                                       |
| ----- | --------- | ----------------------------------------------------------- |
| 4     | Drums     | Pon, Pata, Chaka, Don Drum                                  |
| 4     | Miracles  | Rain, Tailwind, Storm, Earthquake Miracle                  |
| 5     | Songs     | Ponpata, Patapata, Ponpon, Chakachaka, Ponchaka Song       |
| 6     | Key items | Blank Map, Bent Compass, Dusty Crystal, Broken Sign, Black Star, Dark Palace Model |

These occupy `0x19CE8`–`0x19D30`; the exact per-token offsets are the
`EU_KEY_ITEM_OFFSETS` table in `save/layout.rs`.

The owned flag here is the altar's **collection marker, not the in-game
capability** — the same lesson as the unit Memories (owned ≠ buildable).
Hardware testing made this clear:

- Locking a drum (clearing its flag) on a progressed save still leaves it
  **usable** in a mission, and its combo songs still play. So drum/song
  availability ignores this flag and is driven by story progress.
- Adding stews or miracles to an early save does **not** make the mission
  stew/miracle placement slots appear — those slots are story-gated.
- The flag *does* matter in one case: selecting a miracle within a mission slot
  the story has **already** opened. Flagging a never-obtained Earthquake Miracle
  on a progressed save (which already had the miracle slot) made it selectable;
  the same flag on an early save with no slot does nothing.

So editing these toggles what the altar shows as collected, while the actual
abilities and mission slots live in a separate, not-yet-mapped story/progress
structure. The records that follow these 19 (up to `0x19D54`, where the
materials begin) are never-owned/unused; forcing them owned **freezes the
altar**, so the editor exposes only the 19 valid tokens.

### Not yet decoded

The records *after* the last item (`0x19FD4..0x1A0E0`) are mostly empty/unused,
but a few are real (some weapons and a key item, "Meden's Trophy"); they have not
all been named.

### A note on the in-game display

The item menu shows a **fixed, non-scrolling grid** of 4 columns × 33 rows (132
cells; 4 items per row), the same on the altar inventory and the mission-prep
gear screen. Items beyond that window still exist and function — gear stays equipped, and "optimize loadout"
considers the full list — but they cannot be selected by hand in the menu. So an
edit that adds a very large number of items is safe for the save, yet can push
some items out of reach in the menu; a tidy, in-window inventory is the better
default.

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
