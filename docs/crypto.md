# PSP savedata cryptography notes

This document specifies the `SECURE.BIN` encryption scheme as implemented (or
to be implemented) in `ptpnhex-core::crypto`. It is written from **public
documentation** of the PSP firmware and the KIRK hardware crypto engine. No
code is copied from GPL implementations; existing tools (PPSSPP, SED-PC) are
used only as behavioural oracles for byte-level verification.

## Public references

- psdevwiki — "KIRK" (KIRK command set, key vault seeds).
- psdevwiki — "PSP Savedata" (savedata parameter and hash layout).
- The `amctrl` / `sceChnnlsv` module documentation (Sony PSP SDK function
  prototypes; the AMCTRL "BBMac" / "BBCipher" primitives).

All constants below must be confirmed against a known plaintext during
implementation (see "Verification").

## High-level structure

A PSP save directory contains:

- `PARAM.SFO` — metadata (parsed by `ptpnhex-core::sfo`). Two fields drive
  the cryptography:
  - `SAVEDATA_PARAMS` — 128-byte block. Byte 0 holds the **encryption mode
    bits**. It also stores hash blocks the firmware verifies.
  - `SAVEDATA_FILE_LIST` — one row per data file, each carrying a 16-byte
    per-file hash.
- `SECURE.BIN` — the encrypted data file. Its plaintext is the actual save
  payload edited by this project.

For the Patapon EU corpus, `SAVEDATA_PARAMS[0] == 0x41`. Decoding (mirroring
the firmware's mode selection):

- bit 0 (`0x01`): the data file is encrypted.
- bit 6 (`0x40`): use the **per-game key** and the SDK ≥ 4 hash key set.

So `0x41` is the "game-key, newer SDK" variant commonly labelled **mode 5**.
The mode must always be read from the file, never hardcoded, because US/JP
releases or other SDK versions may differ.

## KIRK primitives needed

KIRK is the PSP's crypto coprocessor. The savedata scheme uses three of its
commands; all can be reproduced in software with AES-128 and SHA-1:

- **CMD7 / CMD4 — AES-128-CBC decrypt / encrypt with a key-vault key.** The
  key is not supplied directly; a *key seed index* selects a 16-byte key from
  the KIRK key vault, and AES-CBC runs with a zero IV over the payload.
- **CMD11 — SHA-1.** Used as the hashing core of the savedata MAC.
- (PRNG / CMD14 is only needed to generate the random per-save seed when
  *encrypting*; for deterministic round-trip tests the original seed is
  reused instead.)

The key-vault seeds required by the savedata paths are a small fixed subset
documented on the psdevwiki KIRK page; the exact indices used by each mode
must be pinned during implementation and locked by the round-trip test.

## The AMCTRL savedata scheme

Two primitives, built on the KIRK commands above:

### BBCipher (the data cipher)

```
cipher_init(mode, gamekey) -> ckey
cipher_update(ckey, buffer)        // in place, length multiple of 16
```

- A 16-byte working key is derived from a fixed cipher seed. When the mode
  uses a game key (modes 3/5), the game key is mixed in (XOR / KIRK-encrypt
  step) before use; modes 1/2 skip this and use the seed alone.
- The encrypted `SECURE.BIN` begins with a **0x10-byte header** that stores
  the per-save random seed; the body follows. Decryption derives the data
  key from (header seed, working key) and runs AES-CBC decrypt over the body.

### BBMac (the data hash / MAC)

```
mac_init(mode) -> mkey
mac_update(mkey, buffer)
mac_final(mkey, gamekey) -> hash[16]
```

- A CBC-MAC-style construction over the data using a KIRK key, finalised with
  a mode-dependent key (again mixing the game key for game-key modes).
- The result is the 16-byte value stored in `SAVEDATA_FILE_LIST` for the
  file, and is what the firmware recomputes and checks on load.

## What must be rewritten after editing the plaintext

When `SECURE.BIN` is re-encrypted, the firmware will reject the save unless
all of the following are regenerated consistently (this is the job of
`crypto::sfo_hash`):

1. The 16-byte **per-file hash** in `SAVEDATA_FILE_LIST` (output of the data
   MAC over the new ciphertext/plaintext).
2. The **hash blocks inside `SAVEDATA_PARAMS`** — computed over the whole
   `PARAM.SFO` with the hash fields zeroed, using mode-dependent keys.

The `ParamSfo` writer already reproduces every other byte exactly, so only
these fields change between a load and a re-save.

## Encryption modes (summary table)

| `PARAMS[0]` | bits        | game key? | label  | notes                    |
| ----------- | ----------- | --------- | ------ | ------------------------ |
| `0x01`      | enc         | no        | mode 1 | fixed key only           |
| `0x21`      | enc + 0x20  | yes       | mode 3 | game key, older SDK hash |
| `0x41`      | enc + 0x40  | yes       | mode 5 | game key, newer SDK hash |

Patapon EU (`UCES00995`) is mode 5.

## The per-game key

Modes 3 and 5 require the title's 16-byte **game key**. It is not stored in
the save; the game passes it to the savedata utility at runtime. Two supported
ways to obtain it (see `keys` module and README):

1. **Bring your own key** — dump it from your own copy with a PSP key-dumper
   plugin (e.g. SGKeyDumper), or read it from PPSSPP while the game runs. This
   is the clean, always-legal path and the one the `KeyProvider::File` /
   `KeyProvider::Env` options serve.
2. **Embedded table** — the known key for a supported title may be compiled in
   behind the default `embedded-keys` feature, isolated in
   `keys::patapon1`, so the build is zero-configuration for end users and the
   key can be stripped in one place if ever needed.

## Verification

The implementation is only trusted once it reproduces known bytes. The
round-trip test (run with `PTPNHEX_SAVES_DIR` set) must pass for **all** real
saves before any edited save is written to hardware:

1. `decrypt(SECURE.BIN)` yields structured plaintext (not noise).
2. `encrypt(decrypt(SECURE.BIN), seed = original)` is **byte-identical** to the
   original `SECURE.BIN`.
3. Recomputing the `SAVEDATA_FILE_LIST` and `SAVEDATA_PARAMS` hashes from our
   pipeline reproduces the bytes already present in the original `PARAM.SFO`.

Passing (2) and (3) simultaneously proves both the game key and the full
cipher/MAC pipeline are correct. A fresh-seed re-encryption is then validated
by loading the save in PPSSPP and, finally, on real PSP hardware.
