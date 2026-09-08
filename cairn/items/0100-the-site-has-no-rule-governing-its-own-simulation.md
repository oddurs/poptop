---
id: 100
title: The site has no rule governing its own simulation
type: docs
status: done
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: s
area: web
---

## Problem

The site has a written rule for colour — status hues mean a state — and one for
type — sans is the human voice, mono is the machine. It has no rule for the
riskiest thing on it: that the landing page shows a *simulation of the product*,
built from fabricated data, drawn by a reimplementation.

Three separate findings in the cross-examination descend from that gap. One of
them was live: a hero rewrite deleted the word "synthetic" from the page and it
described seeded data as a "recorded poptop session" for a whole session before
anyone noticed. It was a caption regression only because nothing said out loud
that the caption was load-bearing.

Two smaller claims in the same paragraph were overstated the same way and
corrected at the same time.

## Proposal

Write the rule down where the other two live, in `assets/css/10-tokens.css`'s
neighbourhood or in `web/README.md`:

> A picture of poptop says where its data came from and who drew it, in the same
> breath as the claim it supports.

And give it a test, the way the other two rules have tests: assert that the
landing page contains a provenance sentence, so deleting it fails the build
rather than surviving a session.

## Acceptance criteria

- [ ] The rule is written next to the colour and type rules
- [ ] A test fails if the landing page stops saying where the buffer came from
- [ ] `audit/README.md` mentions it as a thing the suites exist to protect
