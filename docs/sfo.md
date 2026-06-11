# PARAM.SFO format notes

`PARAM.SFO` is the metadata file present in every PSP save directory. It uses
the PSF container format: a small key/value store. This document records the
format as implemented in `ptpnhex-core::sfo`, plus observations from a corpus
of real save files.

Public references: the PSF format is documented on community wikis
(psdevwiki, "PARAM.SFO") and has been stable across the PSP's lifetime.

## Layout

All integers are little-endian.

| offset | size | field              |
| ------ | ---- | ------------------ |
| 0x00   | 4    | magic `"\0PSF"`    |
| 0x04   | 4    | version (`0x0101` = 1.1) |
| 0x08   | 4    | key table start    |
| 0x0C   | 4    | data table start   |
| 0x10   | 4    | entry count        |
| 0x14   | 16×n | index table        |

Index table entry (16 bytes):

| offset | size | field |
| ------ | ---- | ----- |
| 0x00   | 2    | key offset (relative to key table) |
| 0x02   | 2    | data format |
| 0x04   | 4    | data length (bytes in use) |
| 0x08   | 4    | data max length (bytes allocated) |
| 0x0C   | 4    | data offset (relative to data table) |

Data formats:

| value    | meaning |
| -------- | ------- |
| `0x0004` | raw bytes / UTF-8 without NUL terminator |
| `0x0204` | NUL-terminated UTF-8 string |
| `0x0404` | little-endian int32 |

Keys are NUL-terminated ASCII strings, stored consecutively in the key table
in index order. Values live in the data table at their recorded offsets; the
allocated block is `max_length` bytes and bytes past `data_length` are zero.
For `0x0204` strings, `data_length` includes the NUL terminator.

## Corpus observations

Verified across 81 real save files from several games (all parsed and
reserialized byte-identically):

- The index table is immediately followed by the key table (no gap).
- The key table is zero-padded so the data table starts 4-byte aligned.
- The data table runs to exactly the end of the file (no trailing bytes).
- Value padding past `data_length` is always zero.

## Save-relevant entries

A PSP save's `PARAM.SFO` carries these entries (observed on Patapon EU,
`UCES00995`, and consistent with other titles in the corpus):

| key                  | format   | content |
| -------------------- | -------- | ------- |
| `CATEGORY`           | `0x0204` | `MS` for memory-stick saves |
| `PARENTAL_LEVEL`     | `0x0404` | parental control level |
| `SAVEDATA_DETAIL`    | `0x0204` | long description shown in the XMB |
| `SAVEDATA_DIRECTORY` | `0x0204` | save directory name |
| `SAVEDATA_FILE_LIST` | `0x0004` | 3168-byte table: file name(13) + pad(3) + per-file hash(16) per row |
| `SAVEDATA_PARAMS`    | `0x0004` | 128-byte block: encryption mode bits and hash blocks |
| `SAVEDATA_TITLE`     | `0x0204` | subtitle shown in the XMB |
| `TITLE`              | `0x0204` | game title |

`SAVEDATA_PARAMS` and the per-file hashes in `SAVEDATA_FILE_LIST` are written
by the save-data encryption pipeline and must be recomputed whenever
`SECURE.BIN` is re-encrypted; their semantics are documented with the
cryptography implementation in `docs/crypto.md`.
