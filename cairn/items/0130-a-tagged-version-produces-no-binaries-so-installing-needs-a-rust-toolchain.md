---
id: 130
title: A tagged version produces no binaries, so installing needs a Rust toolchain
type: chore
status: done
milestone: r7
assignee: Oddur Sigurdsson
labels:
- repo
created: 2026-09-19
updated: 2026-09-19
priority: p2
---

## Problem

There is no release workflow. Tagging `v0.1.0` would produce a tag and
nothing else: no built binaries, no notes, no checksums. Everyone who wants
to run poptop has to install Rust first, which is a strange requirement for
a program whose whole argument is that it needs nothing set up beforehand.

## Proposal

A workflow on a `v*` tag that builds the four targets that matter —
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
`x86_64-apple-darwin`, `aarch64-apple-darwin` — and attaches them to a
GitHub release with checksums.

## Acceptance criteria

- [ ] `.github/workflows/release.yml`, on `v*` tags and `workflow_dispatch`
- [ ] Four targets, each a stripped release build
- [ ] A `SHA256SUMS` file beside them
- [ ] The tag's version is checked against `Cargo.toml` and disagreeing fails
- [ ] RELEASING.md says how to cut one

## How it was resolved

`.github/workflows/release.yml`, on `v*` tags and `workflow_dispatch`.

A `version` job runs first and fails if the tag disagrees with `Cargo.toml`,
before four builds are spent on it. Four targets, each `--release --locked`
and each run — `--version` and `--once` — on the machine that built it,
because poptop declares its FFI by hand and a binary nobody executed is the
one that finds out about a struct layout in production. arm64 Linux builds
on a native arm64 runner for the same reason, rather than cross-compiling.

`publish` collects them, writes one `SHA256SUMS`, and creates the release
with `gh` rather than a third-party action. `workflow_dispatch` builds and
uploads artifacts and publishes nothing, so the workflow can be exercised
without a tag.
