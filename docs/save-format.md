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
| `0x19000`–end     | text pool mixed with numeric stats (ka-ching lives here)    |

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

## The inventory list (materials and items)

Stackable items (materials, stews, and other consumables) are stored as a
**compacted, position-indexed list**, not at fixed offsets. Each entry is a
`u32`:

```
count : u16 (LE)      item slot : u16 (LE)
```

Two properties make this list subtle, and both have been verified against the
corpus:

- **Only obtained items are stored**, packed together with no gaps. An item the
  player has never obtained has *no entry at all* (not even a zero-count one).
- **The "item slot" value is a running position counter, not a stable type
  identifier.** Entries are numbered consecutively in canonical order, so when
  an item is missing, every entry after it shifts down by one — the same slot
  number means a *different item* in a different save.

Concretely: a save that has Magic Alloy stores it at slot `0x2601`, with the
first stew at `0x2701`. A save that never obtained Magic Alloy has no Magic
Alloy entry, so the first stew takes slot `0x2601` instead. **The save data does
not record what each entry *is*** — the game maps slots to item types with an
internal table that is not in the save.

For Europe the list is scanned within `0x19000..0x1A0E0` (after the
unit/equipment array, before ka-ching).

### Materials

The 20 crafting materials occupy the first material slots, in this order:

| order | materials                                               |
| ----- | ------------------------------------------------------- |
| 1–4   | Leather Meat, Tender Meat, Dream Meat, Mystery Meat     |
| 5–8   | Stone, Hard Iron, Tytanium Ore, Mytheerial              |
| 9–12  | Banal Branch, Cherry Tree, Hinoki, Super Cedar          |
| 13–16 | Eyeball Cabbage, Crying Carrot, Predator Pumpkin, Hazy Shroom |
| 17–20 | Sloppy Alloy, Hard Alloy, Awesome Alloy, Magic Alloy    |

In a save where all 20 are present they fall at slots `0x1301`..`0x2601` (low
byte `0x01`), confirmed against all 20 amounts read from one save (`DATA46`).
Counts display two-digit and are capped at 99.

**Reliability caveat (important):** because slots are positional, a material can
only be located reliably while it and every earlier material are present. The
common materials are obtained early and stay present, so they are reliable; but
a *missing* late material (e.g. Magic Alloy) shifts the slots, and addressing it
by a fixed slot will hit the next item instead. Editing must therefore confirm
an entry really is the requested material before writing — and editing an item
the player has never obtained is not supported. (Some unexplained `b2 = 0`
marker entries appear exactly where items go missing; understanding them is the
likely path to handling absences robustly.)

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
