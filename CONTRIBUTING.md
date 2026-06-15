# Contributing

Thanks for your interest in PTPNHeX. This document describes the conventions
every change to this repository follows.

## Workflow

- `main` is always releasable and is protected: no direct pushes. Every
  change, however small, goes through a pull request with green CI.
- Branch names: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `ci/<slug>`,
  `chore/<slug>` — for example `feat/sfo-parser`.
- Keep branches short-lived and rebased on `main`. Pull requests are
  squash-merged; the PR title becomes the commit message on `main`, so it
  must follow the commit convention below.
- One logical slice of work per PR. No drive-by changes.

## Commit messages

This project follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):

```
type(scope): subject
```

- **Types:** `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `ci`,
  `build`, `chore`.
- **Scopes:** `core`, `crypto`, `sfo`, `save`, `keys`, `cli`, `gui`, `docs`,
  `ci`, `release`.
- Subject in imperative mood, lowercase, no trailing period, at most
  72 characters. Use the body to explain *why* when it is not obvious.
  Breaking changes carry a `BREAKING CHANGE:` footer.
- Commits are atomic: one logical change per commit; never mix refactoring
  with behavior changes.

## Quality gates

CI enforces the following on every pull request; run them locally first:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Public APIs in `ptpnhex-core` are documented with rustdoc.

## Testing with real save data

Real PSP save files are **never** committed to this repository — not as test
fixtures, not in documentation, not anywhere. Committed tests use synthetic
data only.

Integration tests that need real saves read the `PTPNHEX_SAVES_DIR`
environment variable and skip themselves when it is unset:

```sh
PTPNHEX_SAVES_DIR=/path/to/SAVEDATA cargo test --workspace
```

## Versioning and releases

- [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). While the
  project is pre-1.0, minor versions add features and patch versions fix bugs.
- `CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
  every user-visible PR updates the `[Unreleased]` section.
- Releases are annotated tags `vX.Y.Z`; CI drafts the GitHub Release with
  binaries for all supported platforms.

## Issues and planning

Work is tracked with GitHub issues, labels, and milestones. Reference the
issue a PR closes in its description (`Closes #N`).
