---
id: 262
title: A glyph set cannot be written without a compiler
type: feature
status: backlog
milestone: r15
labels:
- ui
- graph
depends_on:
- 246
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## Problem

Once sets are tables, the only thing standing between a reader and their own
set is that the tables are compiled in. Somebody with a font that has a
beautiful run of block variants, or a terminal that renders one set badly,
cannot fix their own screen.

## Proposal

A set file, beside the theme file: the patterns and their codepoints, the marks
the set claims, a name. Validated on load the way a theme is, reloadable with
the same key, and refused with the offending pattern named rather than drawn as
tofu.

## Acceptance criteria

- [ ] `graph = "path/to/set.toml"` loads a reader's set
- [ ] Validation names what is wrong: a missing pattern, a wide glyph, a claim the table does not support
- [ ] `R` reloads it, as it reloads a theme, so a set can be tuned by looking
