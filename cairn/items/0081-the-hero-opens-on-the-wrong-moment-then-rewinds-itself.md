---
id: 81
title: The hero opens on the wrong moment, then rewinds itself
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p0
effort: m
area: web
---

## Problem

The hero frame starts playing live. It moves, which draws the eye, but motion is
all it says. A visitor who gives the page two seconds sees a terminal doing
something and learns nothing about why this terminal is different from the one
they already have.

The product's whole claim is a *contrast between two moments* — what you see
when you arrive, and what was true before you got there. The frame currently
shows one moment and lets you find the other yourself, if you read the caption
and know to drag.

## Proposal

Spend the page's one motion moment on the argument instead of on movement.

On load the frame sits at a quiet stretch of the buffer: CPU in the single
digits, nothing in the table above a few percent. The caption reads **"This is
what you see when you arrive."**

After a beat, the cursor travels left — the plots redraw under it, the table
repopulates, postgres climbs to 74% — and settles on the incident. The caption
becomes **"This is what happened forty seconds ago."** The badge reads PAUSED
throughout, because that is what it is.

Then it stops and hands over: the hint line says the keys, and every control
works as it does now.

Details that matter:

- The travel is a cursor animation, not playback: it moves *backwards* through
  the buffer, which is the gesture the product is named for.
- Roughly 1.4s of travel after a 0.8s hold. Long enough to read as deliberate,
  short enough that nobody waits through it twice.
- `prefers-reduced-motion` gets the second state immediately, caption and all.
  Not a faster animation — no animation.
- Once a visitor touches any control the intro never runs again for that page
  view.

## Acceptance criteria

- [ ] The frame opens on a quiet cursor, not on live playback
- [ ] One scripted travel backwards, then it stops and stays stopped
- [ ] The caption states both moments, in that order
- [ ] Reduced motion lands on the incident with no travel
- [ ] Touching a control cancels the intro rather than fighting it
- [ ] `audit/interact.js` still passes every keyboard and drag check
