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

**Editing semantics.** A material is read from its `count` byte when its `owned`
flag is set, otherwise it is reported as absent (`0`). A material owned at count
`0` (obtained then fully used) is still editable. A material that has **never
been obtained** (`owned = 0`) is refused, not edited — adding it would mean
flipping the `owned` flag and recomputing the display-index counters across the
table, which is not yet supported. Writing a count touches only `byte 0`, so it
leaves the `new` flag (`byte 1`) as it was — confirmed in-game: after setting
every material to 99, only a freshly picked-up item (whose `byte 1` was already
`1`) kept flashing the "NEW!" indicator, while the rest did not.

### Not yet decoded

The non-material item identities (key items, weapons, stews) are not yet mapped
to offsets, and the rule the game uses to assign the `display-index` byte (so a
never-obtained item could be safely *added*) is not yet worked out.

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
