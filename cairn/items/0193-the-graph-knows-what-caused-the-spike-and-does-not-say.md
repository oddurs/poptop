---
id: 193
title: The graph knows what caused the spike and does not say
type: feature
status: backlog
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p1
area: ui
---

## Problem

Every sample in the buffer carries the complete process table. So for any spike
on the timeline, poptop holds the answer to "what was that" — and makes you
scrub to it and read the table to find out.

The graph is the thing you are looking at when the question occurs. It should
answer it there.

## The fix

Annotate the graph's own extremes. The worst cell in the visible window gets a
label naming the process most responsible for it:

```
  100                         ╭─╮  ← cargo build --release (8 threads)
  CPU  ────────────────────────╯ ╰──────────
```

Not every peak — one, the one that matters, chosen the way `--report` already
chooses. The mechanism exists; it has never been pointed at the live view.

## Why this is the moat

Prometheus, netdata, atop and btop all decide their aggregations at write time
and discard the process table. They can tell you CPU was at 100 at 14:32. They
cannot tell you what was running, ever, because that was never stored. It is not
a feature they have not got round to; it is a property of their storage, and
retrofitting it means changing what they write.

poptop kept the samples. This is the item that spends that.

## What needs deciding

- **Space.** A label is 30 characters in a panel that may be 60 wide. It has to
  join the existing degradation ladder rather than invent a new one.
- **"Most responsible".** Highest CPU in that cell is the obvious answer and is
  wrong when four processes each take a quarter. Needs a rule that can say "no
  single process" rather than naming an arbitrary one.
- **Stability.** A label that changes every frame as the window slides is worse
  than none.

## Acceptance criteria

- [ ] The worst moment on screen is named without scrubbing to it
- [ ] Says "no single process" rather than naming one arbitrarily
- [ ] Does not move or flicker while the window slides
- [ ] Drops before the graph does when the panel narrows
