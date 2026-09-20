---
id: 121
title: Configuration and themes have no reference page
type: docs
status: backlog
milestone: r6
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: docs
---

## Problem

Every setting exists twice — a flag and a config key — and the list is in the middle of the README beside a worked example. Themes have a format, a tier ladder and a colourblind-safe default, all argued for and nowhere tabulated.

## Acceptance criteria

- [ ] `docs/reference/configuration.md`: every key, its flag, its default, its unit
- [ ] `docs/reference/themes.md`: the file format, the tokens, the tiers, `NO_COLOR`
- [ ] A test holds the documented key list against the settings table in `config.rs`
