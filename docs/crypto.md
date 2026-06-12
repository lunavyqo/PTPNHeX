# PSP savedata cryptography

This document describes the `SECURE.BIN` encryption scheme used by Patapon
(EU, `UCES00995`) and implemented in `ptpnhex-core::crypto`.

The algorithm and constants are drawn from public reverse-engineering
documentation of the PSP's KIRK engine and `sceChnnlsv` save-data scheme; the
numeric values below are public facts (cryptographic keys and algorithm
parameters), not original expression. No source code is copied from existing
GPL projects; where they exist, they were used only to cross-check output
bytes. The scheme described here has been **validated byte-for-byte against a
corpus of 51 real saves** (see "Validation").

## Public references

- psdevwiki — "KIRK" (command set and key vault).
- psdevwiki — "PSP Savedata" (savedata parameters and hash layout).
- The documented `sceChnnlsv` / AMCTRL save-data function set.

## Save directory layout

A Patapon save directory contains:

- `PARAM.SFO` — metadata (parsed by `ptpnhex-core::sfo`). Three regions matter
  to the cryptography:
  - `SAVEDATA_PARAMS` — 128-byte block. Byte 0 holds the encryption-mode bits;
    offsets `+0x10`, `+0x20`, `+0x70` hold integrity hashes (below).
  - `SAVEDATA_FILE_LIST` — one row per data file; each row's bytes `+0x0D..+0x1D`
    hold a 16-byte per-file hash. For these saves the `SECURE.BIN` row's hash is
    at absolute offset `0x55D`.
- `SECURE.BIN` — the encrypted data file. Its plaintext is the save payload the
  editor reads and writes.

## Mode selection

The encryption mode is read from `SAVEDATA_PARAMS[0]`, never hardcoded:

| `PARAMS[0]` | bits         | game key? | chnnlsv mode | notes                |
| ----------- | ------------ | --------- | ------------ | -------------------- |
| `0x01`      | `0x01`       | no        | 1            | fixed key only       |
| `0x21`      | `0x01｜0x20` | yes       | 3            | game key, older hash |
| `0x41`      | `0x01｜0x40` | yes       | 5            | game key, newer hash |

Patapon EU is `0x41` → **mode 5** (`0x40` ⇒ `encryptmode` 4, the newer hash
key set). US/JP or other-SDK titles may differ, so the value is always decoded
at runtime.

## KIRK primitive

All operations reduce to **AES-128-CBC with a zero IV** keyed by a value from
the KIRK key vault (KIRK command 7 = decrypt, command 4 = encrypt — a single
block with a zero IV is therefore plain ECB). The key-vault entries used by the
mode-5 paths:

| slot   | value                              | used by              |
| ------ | ---------------------------------- | -------------------- |
| `0x03` | `9802C4E6EC9E9E2FFC634CE42FBB4668` | params hash, mode 1  |
| `0x10` | `32295BD5EAF7A34216C88E48FF50D371` | file hash, params m5 |
| `0x11` | `46F25E8E4D2AA540730BC46E47EE6F0A` | params hash, mode 6  |
| `0x12` | `5DC71139D01938BC027FDDDCB0837D9D` | cipher key derivation|
| `0x64` | `03B302E85FF381B13B8DAA2A90FF5E61` | cipher keystream     |

`sceChnnlsv` mixing constants:

| name      | value                              |
| --------- | ---------------------------------- |
| `key19CC` | `7044A3AEEF5DA5F2857FF2D694F5363B` |
| `key19DC` | `EC6D29592635A57F972A0DBCA3263300` |
| `hash198C`| `FAAA50EC2FDE5493AD14B2CEA53005DF` |
| `hash19BC`| `CB15F407F96A523C04B9B2EE5C53FA86` |

## The cipher (mode 5)

`SECURE.BIN` is a keystream cipher. Let `gamekey` be the 16-byte game key and
`AES_dec(key, data)` / `AES_enc(key, block)` be AES-128 in CBC mode with a zero
IV.

**Decryption** of a `SECURE.BIN` blob:

1. Split off the leading 16-byte header: `header = blob[0..16]`, `body =
   blob[16..]`. Zero-pad `body` to a multiple of 16 (`alen`).
2. `crypted = header XOR gamekey`.
3. `seed = AES_dec(slot_0x12, crypted XOR key19DC) XOR key19CC`.
4. Build the counter blocks: for block index `k = 0..alen/16-1`,
   `C[k] = seed[0..12] ‖ u32_le(k + 1)`.
5. `keystream = AES_dec(slot_0x64, C[0]‖C[1]‖…)` (continuous CBC, zero IV).
6. `plaintext = body XOR keystream` (trimmed to the real data length).

**Encryption** is the same XOR against the same keystream. The keystream
depends only on `(header, gamekey)`, so re-encrypting with the original header
reproduces the original ciphertext exactly; a fresh save generates a new random
16-byte header.

## Integrity hashes

The firmware checks several hashes; all derive from an **AES-CMAC** (the KIRK
key-vault AES used as the CMAC block cipher).

### Per-file hash (`SAVEDATA_FILE_LIST`, offset `0x55D`)

Computed over the **encrypted** `SECURE.BIN` (zero-padded to a multiple of 16),
mixing in the game key:

```
h = AES_CMAC(slot_0x10, padded_secure_bin)
h = h XOR hash19BC
file_hash = AES_enc(slot_0x10, gamekey XOR h)
```

### `SAVEDATA_PARAMS` hashes (no game key)

Computed over the **entire `PARAM.SFO`** (4912 bytes, already 16-aligned) with
the target hash field zeroed, in this order. Each is an AES-CMAC with a
slot/constant chosen by mode:

| offset | mode | slot   | post-XOR   | notes                     |
| ------ | ---- | ------ | ---------- | ------------------------- |
| `+0x20`| 6    | `0x11` | `hash19BC` | computed first            |
| `+0x70`| 5    | `0x10` | `hash19BC` | computed second           |
| `+0x10`| 1    | `0x03` | none       | computed last             |

The order matters: each hash is computed while the later fields are still
present and earlier ones already written. (modes 3/4 would post-XOR `hash198C`.)

### The mode-6 limitation

The `+0x20` hash uses chnnlsv mode 6, whose finalization invokes a KIRK "fuse"
command (command 5/8) backed by hardware state that cannot be reproduced in
software. This affects **every** PC-based PSP save tool. Consequently this one
hash cannot be regenerated off-device. In practice the PSP does not appear to
verify it when *loading* a save (PC tools reliably produce loadable saves); the
editor leaves the original `+0x20` value in place and this is confirmed on real
hardware as the M1 exit gate.

## Write path

After editing the plaintext, a save is resealed by:

1. Re-encrypting the plaintext (XOR keystream) and writing `SECURE.BIN`.
2. Recomputing the per-file hash and the `+0x10` / `+0x70` params hashes, and
   writing them into `PARAM.SFO`.

The `ParamSfo` writer reproduces every other byte exactly, so only these fields
change between a load and a re-save.

## The game key

Mode 3/5 require the title's 16-byte game key. It is **not** stored in the save;
the game passes it to the save-data utility at runtime. It is obtained from a
copy of the game — dumped with a PSP key-dumper plugin (e.g. SGKeyDumper, which
writes `PSP/SAVEPLAIN/<save>/<GAMEID>.bin`) or read from an emulator. In this
project the key is supplied through `keys::KeyProvider` (runtime `Bytes`, or the
compiled-in `keys::patapon1` table behind the default `embedded-keys` feature).

## Validation

The scheme above was validated against a corpus of 51 real `UCES00995` saves:

1. Decryption produces structured plaintext containing the game's item
   identifiers (`unit…`, `wpn…`, `hlm…`, `sld…`).
2. The recomputed per-file hash matches the stored `PARAM.SFO` hash on **all 51
   saves**.
3. The `+0x10` and `+0x70` params hashes likewise match exactly.

The corresponding Rust integration tests run when `PTPNHEX_SAVES_DIR` points at
a local save corpus and self-skip otherwise; real saves are never committed.
