---
id: 127
title: The crate has no repository, keywords or categories, and cannot be published
type: chore
status: done
milestone: r7
assignee: Oddur Sigurdsson
labels:
- repo
created: 2026-09-19
updated: 2026-09-19
priority: p1
---

## Problem

`Cargo.toml` names the package, its version, its licence and one line of
description. That is the minimum to build. It is not enough to publish, and
not enough for anyone who finds the crate rather than the repository:

- no `repository`, so crates.io would show no link back here
- no `keywords` or `categories`, so it appears in no search and no listing
- no `rust-version`, so a 2021-era toolchain fails at a syntax error in
  edition 2024 rather than at a clear message
- no `exclude`, so a published crate would carry `cairn/`, `docs/`,
  `validate/results/` and the Linux fixture tree

## Acceptance criteria

- [ ] `repository`, `homepage`, `keywords`, `categories`, `rust-version`
- [ ] `exclude` keeps the package to what building and testing needs
- [ ] `cargo package --list` is inspected and the result recorded here
- [ ] The MSRV is the one actually required, not a guess

## How it was resolved

`repository`, `homepage`, `documentation`, `keywords`, `categories`,
`rust-version` and `exclude` are set.

The MSRV was measured rather than guessed. Edition 2024 puts the floor at
1.85, but `cargo +1.85 check` fails in the dependency graph: ratatui,
sysinfo, time and darling all require 1.88. `cargo +1.88 test` passes — 694
tests — so `rust-version = "1.88"`, and a new `msrv` job in CI builds
`--all-targets` on whatever that field says, so the claim cannot go stale
quietly.

`cargo package` is 81 files, 2.1 MiB, 638 KiB compressed, and its
verification build passes. `tests/` is excluded: 15M of it is a captured
Linux `/proc` and `/sys` tree, which is how this repository tests a backend
it cannot run on the developer's machine. `docs/` stays — `documentation`
points at it, it is 288K, and two unit tests read it with `include_str!`.
