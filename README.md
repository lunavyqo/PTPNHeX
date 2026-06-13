# PTPNHEX

A save editor for Patapon™ (PSP), in the spirit of tools like PKHeX: open a save,
inspect and edit its contents, and write it back in a form the game accepts.

> **Unofficial project.** PTPNHEX is not affiliated with, endorsed by, or
> sponsored by Sony Interactive Entertainment. Patapon is a trademark of its
> respective owner. This tool only operates on save data you created with your
> own copy of the game.

## Status

Early development. Nothing is usable yet.

| Region | Serial      | Status  |
| ------ | ----------- | ------- |
| EU     | `UCES00995` | planned |
| US     | `UCUS98623` | later   |
| JP     | `UCJS10054` | later   |

## Planned features

- Decrypt and re-encrypt `SECURE.BIN` save data with correct `PARAM.SFO` hash updates
- Edit money (ka-ching), materials, items and equipment, army composition, mission progress, and miracles
- `ptpnhex` command-line interface and a cross-platform desktop GUI
- Optional backups of the originals to a directory you choose

## Installation

Release binaries for Windows, macOS (Intel and Apple Silicon), and Linux
(x86-64 and ARM64) will be attached to GitHub Releases once the first version
is tagged. Building from source requires a stable Rust toolchain:

```sh
cargo build --release -p ptpnhex-cli
```

## Usage

Inspect a save directory:

```sh
ptpnhex info path/to/UCES00995_DATA01
```

Set the ka-ching (currency) value and write the save back (add
`--backup-dir <DIR>` to copy the originals somewhere safe first):

```sh
ptpnhex set-kaching path/to/UCES00995_DATA01 99999
ptpnhex set-kaching path/to/UCES00995_DATA01 99999 --backup-dir ~/ptpnhex-backups/DATA01
```

List crafting materials, or set one (or all of them) — counts cap at 99:

```sh
ptpnhex materials path/to/UCES00995_DATA01
ptpnhex set-material path/to/UCES00995_DATA01 hard-alloy 99
ptpnhex set-material path/to/UCES00995_DATA01 all 99
```

More fields are added as they are reverse-engineered (see
[docs/save-format.md](docs/save-format.md)). Run `ptpnhex --help` for the full
command list.

## Save-file safety

The editor writes **only** `SECURE.BIN` and `PARAM.SFO` into a save folder —
never a stray backup or temporary file, because a real PSP refuses to load a
save directory that contains anything unexpected. To keep the originals, pass
`--backup-dir <DIR>` (a directory outside the save folder) when editing, or
simply work on copies of your saves.

## Legal notes

- The cryptographic routines in this project are an original implementation,
  written from publicly available documentation of the PSP save-data format.
  No code from the game or its firmware is included.
- This repository contains no game assets, no save files, and no copyrighted
  material belonging to the game's publisher.
- You must own the game to have save data to edit. This tool does not enable
  piracy and does not interact with any online service.

## License

[MIT](LICENSE)
