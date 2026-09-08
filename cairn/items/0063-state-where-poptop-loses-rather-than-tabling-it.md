---
id: 63
title: State where poptop loses rather than tabling it
type: feature
status: done
milestone: web
created: 2026-09-08
updated: 2026-09-08
priority: p2
effort: s
area: web
---

## Problem

The prior-art table is honest and is the right content, but its shape is a
scoreboard: four tools, three columns, poptop in the first row. A table invites
the reader to count wins, and the section's actual purpose is the opposite — to
say plainly that atop is the more capable tool, so that everything else on the
page is believable.

## Proposal

Lead with the concession, in the section's own voice, at a size that is clearly
not a footnote: **atop is the better tool, and here is exactly when.** Then the
one fact that matters — atop *names* processes that started and finished between
two samples; poptop can only tell you they happened, and gives a count.

Keep a compact comparison underneath for the reader who wants it, but demote it:
it is evidence for the sentence above, not the point of the section.

Then the trade, stated once: what poptop has instead is a process table that
rewinds, with nothing to install first.

## Acceptance criteria

- [ ] The concession leads and is not visually softer than the claims elsewhere
- [ ] The short-lived-process gap is named specifically
- [ ] The comparison remains available but is subordinate
- [ ] Links through to the long version in the docs
