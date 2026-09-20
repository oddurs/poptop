---
id: 128
title: Nothing at the top of the README says what poptop needs or whether it works
type: docs
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

The README opens with a sentence and a rendered frame, which is the right
order. What it does not carry is the half-second signal every reader looks
for first: is the build green, what licence is this, what does it run on,
what toolchain does it need.

`## Install` is `cargo build --release`, which assumes a checkout. There is
no `cargo install` line, and no mention that Linux and macOS are both
supported and nothing else is.

## Acceptance criteria

- [ ] Badges: CI, licence, MSRV, platforms — linking to the thing they claim
- [ ] `cargo install --git` as the one-line install
- [ ] Platform support stated where somebody deciding would look
- [ ] No badge that cannot be checked by clicking it

## How it was resolved

Four badges, each linking to the thing it claims: the CI badge to the
workflow's runs, the Rust badge to `Cargo.toml`, the platform badge to
`docs/reference/platforms.md`, the licence badge to `LICENSE`. No crates.io
badge, because nothing has been published.

`cargo install --git` is the one-line install, with the checkout build after
it. The MSRV is stated with where it comes from — ratatui and time, not
poptop — and the platform support is stated as "Linux and macOS, and nothing
else".
