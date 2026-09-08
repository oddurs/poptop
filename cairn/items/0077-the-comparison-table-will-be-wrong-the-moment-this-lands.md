---
id: 77
title: The comparison table will be wrong the moment this lands
type: docs
status: backlog
milestone: v2.2
created: 2026-09-08
updated: 2026-09-08
priority: p2
area: docs
---

## Problem

The README carries a comparison against htop, btop, bottom, zenith and atop, and
a positioning paragraph that says the honest pitch is "a usability position
against atop, not a capability one".

Both were true and carefully verified when written. Both become wrong as the
v2.0–v2.2 items land, and a comparison table that overstates is worse than none
— it was written specifically to prevent the overclaim it would then be making.

## What has to be re-decided, not just re-worded

The positioning rests on atop needing its daemon to have been running
beforehand. That argument survives all of this work: poptop still gives you the
last ten minutes on a box that has never run it. What changes is that "and it
does less" stops being true.

So the claim becomes stronger and narrower at once, and the temptation will be
to state it broadly. The rule that got it right the first time — every row
verified against source or official docs, and rows where poptop loses are
present and marked — is what should be applied again.

## What needs deciding

- Whether atop remains the tool the README sends people to, and for what. Fleet
  history with a daemon that was already running is a real answer that poptop
  should keep giving.
- Whether the table grows rows for what v2.1 adds, or whether a table of forty
  rows stops being a comparison and becomes a feature list.

## Acceptance criteria

- [ ] Re-verified against atop's source or man page at the time of writing, not
      from this plan
- [ ] Rows where poptop still loses are present and marked
- [ ] The zero-setup claim is stated without "nothing else does this"
- [ ] Dated, so the next reader knows what it was checked against
