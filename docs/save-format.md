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
| `0x19CE8`–`0x1A0E0`| **inventory record array** — materials, items (see below)  |
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
array of 4-byte records**, not at fixed per-item offsets. For Europe the array
spans `0x19CE8..0x1A0E0` (254 records) — its start is identical across the whole
save corpus, and it ends exactly where ka-ching begins (`0x1A0EC`). Each record
is:

```
count : u16 (LE)      flag : u8      index : u8
```

- **`index`** is a **stable** per-item identifier. It does *not* change between
  saves: Magic Alloy is `index 0x26` in every save that has it.
- **`flag`** is `0x01` when the item is **owned** (the `count` is meaningful) and
  `0x00` for a **known-but-not-owned marker** (count `0`). These `flag = 0x00`
  entries enumerate items the player has registered but does not currently hold.
- **`count`** is the quantity, displayed two-digit and capped at 99 for
  materials.

The records are kept in **acquisition order**, so an item's *absolute offset*
varies from save to save while its `index` does not. Owned items appear in the
order they were obtained, displacing marker entries; the array length stays
constant. An item is located by scanning for its `index`, never by a fixed
offset.

### Materials

The 20 crafting materials are the records with `flag = 0x01` and `index` in
`0x13..=0x26`, in this order:

| index       | materials                                                     |
| ----------- | ------------------------------------------------------------- |
| `0x13`–`0x16` | Leather Meat, Tender Meat, Dream Meat, Mystery Meat         |
| `0x17`–`0x1A` | Stone, Hard Iron, Tytanium Ore, Mytheerial                 |
| `0x1B`–`0x1E` | Banal Branch, Cherry Tree, Hinoki, Super Cedar             |
| `0x1F`–`0x22` | Eyeball Cabbage, Crying Carrot, Predator Pumpkin, Hazy Shroom |
| `0x23`–`0x26` | Sloppy Alloy, Hard Alloy, Awesome Alloy, Magic Alloy       |

All 20 amounts were confirmed against one save (`DATA46`). Locating a material
by its `index` is unique across the corpus: every (save, material) pair yields
at most one owned record.

**Editing semantics.** A material with an owned record is editable in place,
*including when its count is 0* (e.g. Magic Alloy obtained then fully used). A
material with no owned record was never obtained; inserting one is not yet
supported, so editing it is refused rather than risking a neighbouring item. As
a sanity guard, a record whose `flag` reads as owned but whose `count` exceeds
the cap (a rare stale/uninitialized slot, seen in two corpus saves) is ignored.

### Not yet decoded

Each `index` appears **twice** across the 254 records (≈127 items × 2): the
array has a second region (beginning near `0x19EE4`) over a different `index`
set, not yet mapped. The non-material item identities (key items, weapons,
stews) and the exact meaning of the second region remain open; materials only
ever appear once, as an owned record in the first region, which is what makes
them safe to edit today.

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
