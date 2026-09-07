---
id: 36
title: Widths are counted in characters, not display columns
type: bug
status: backlog
milestone: later
created: 2026-09-07
updated: 2026-09-07
priority: p3
area: ui
---

## Problem

`elide_middle`, `fit`, `full_width` and `short_mount` all measure text with
`chars().count()`. That is the number of scalar values, not the number of
terminal columns. A CJK name or an emoji occupies two columns per character, so
a name eliminated "to nineteen columns" can draw thirty-eight and be clipped by
the terminal anyway — defeating the point of eliding deliberately.

The same applies to the header: a group of figures whose content is
double-width would overflow the row.

## Why it is not fixed yet

It needs a width table. `ratatui` already depends on `unicode-width` and exposes
`Span::width()`, which is the right answer for the header — the figures are
already `Span`s. `elide_middle` needs per-character widths to decide where to
cut, which is a direct `unicode-width` call and so a new declared dependency.

Worth doing in one change rather than half here and half there.

## Acceptance criteria

- [ ] Header figures are measured with `Span::width`
- [ ] `elide_middle` cuts on display columns and never exceeds its budget
- [ ] A test with a double-width name asserts the drawn row fits its column
