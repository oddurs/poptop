---
id: 211
title: The table should look like a table
type: feature
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p3
area: ui
depends_on:
- 206
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

## What was decided

Half of this had already landed by the time it was picked up, which is worth
recording rather than quietly ticking.

**Reading across a wide row** is answered by the zebra striping from the
surfaces work, not by separators. Separators cost a column each and there are
eight of them; the header is a raised band rather than a hairline, which is
stronger and costs nothing per row.

**The leading gutter was declined.** It costs a column always and earns it only
if something is in it more often than not. The selection is carried by bold, a
foreground *and* a background — three channels, one of which survives every
tier — so it does not rest on the ground, which is what the gutter was for.

**Middle truncation was already there** — `elide_middle`, used by the command
column, which is the name column.

**Alignment was a convention followed by hand and checked nowhere.** It is now a
field on `Column`, and the header reads it rather than each of fourteen push
sites choosing for itself. So a column cannot be declared numeric and drawn
left, which was possible an hour ago and is the only thing here that was
actually broken.
