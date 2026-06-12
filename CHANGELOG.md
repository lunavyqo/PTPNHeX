# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Region model and an isolated, feature-gated game-key provider, plus a
  written specification of the save-data encryption scheme, as groundwork for
  `SECURE.BIN` decryption.

### Changed

- Replaced the speculative cryptography notes with the concrete, corpus-verified
  mode-5 `SECURE.BIN` algorithm: the keystream cipher, the KIRK key-vault
  constants, the CMAC-based integrity hashes, and the known mode-6 limitation.
- `PARAM.SFO` parser and writer with a byte-identical round-trip guarantee,
  typed accessors for string and integer entries, and bounded setters for
  save titles and descriptions.
- Project scaffolding: Cargo workspace with `ptpnhex-core`, `ptpnhex-cli`, and
  `ptpnhex-gui` crates, continuous integration, and contribution guidelines.
