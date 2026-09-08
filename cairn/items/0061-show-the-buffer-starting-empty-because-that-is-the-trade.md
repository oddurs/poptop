---
id: 61
title: Show the buffer starting empty, because that is the trade
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: m
area: web
---

## Problem

"Nothing had to be running first" is the claim that separates poptop from atop,
and it is currently a paragraph. It is also half a limitation — poptop knows
nothing about the time before you launched it — and stating a limitation in
prose while stating the advantage in prose lets a sceptical reader assume the
prose is doing the work.

## Proposal

One wide band, full width of the content column, showing a buffer as it
actually is a few minutes after launch.

The left third: nothing. Not a flat line — *no data*, drawn as the chrome-hatch
the tool uses for a region it cannot speak about, labelled **"before you started
poptop"**. Underneath, in the same weight as everything else on the page:
*no data here, and no daemon that should have been running to collect it.*

The right two-thirds: the real buffer, filling.

Between them, at the boundary, a mark and the words **"you noticed the
problem"**.

That is the whole argument in one picture: atop would have the left third, and
would have needed to be enabled last month to get it. poptop has the right
two-thirds, and needed nothing.

## Acceptance criteria

- [ ] The empty region reads as absent data, not as zero
- [ ] The launch boundary is marked and labelled
- [ ] The limitation is stated as plainly as the advantage
- [ ] Uses the same buffer as the rest of the page
