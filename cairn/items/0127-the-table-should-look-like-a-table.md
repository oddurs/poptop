---
id: 127
title: The table should look like a table
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p3
area: ui
depends_on:
- 122
---

## Problem

Details, and they are what the screenshot is actually made of:

- **Column separators.** Activity Monitor draws a hairline between headers and
  nothing between cells. poptop draws neither, so a wide row is a field of
  numbers with nothing to follow down.
- **Alignment as a rule.** Numerics right, text left, units aligned under each
  other. poptop mostly does this and not uniformly.
- **A leading gutter for identity.** Activity Monitor keeps a narrow column at
  the left for an icon. The terminal equivalent is a single character — a
  container mark, a state mark, an arrow for the selection — and poptop's
  selection currently has to be inferred from the background alone.
- **Truncation from the middle.** `Xprotect…` and `com.appl…` are truncated at
  the end, and the useful half of a Java or Electron command line is in the
  middle. poptop already elides commands; the table's name column does not.

## What needs deciding

- **Separators cost a column each.** Between eight columns that is eight
  columns, on a table that is already fighting for width. A separator only in
  the header — which is what Activity Monitor does — costs nothing per row and
  may be enough.
- **The gutter costs a column always.** It earns it only if something is in it
  more often than not.

## Acceptance criteria

- [ ] A wide row can be read across without losing the line
- [ ] Every numeric column is right-aligned and every text column is not
- [ ] The selected row is identifiable without relying on the background
- [ ] A truncated name keeps the half that identifies it
