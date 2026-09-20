---
id: 140
title: Nothing says why the worst moment was the worst
type: feature
status: backlog
milestone: v5.0
depends_on:
- 75
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

Scrub to a spike and poptop shows the machine figures for that instant and the
process table underneath. The reader does the joining: read 98% off the header,
scan the table, notice fourteen rows called `cc1plus`, remember that `make -j16`
is the parent, work out that they account for most of it.

Every one of those steps is arithmetic over data already in the sample. The tool
has the answer and makes you assemble it.

`--report` took the first step — it names the process responsible for a peak and
for a run. That is one figure and one name. This is the general form, at any
moment the cursor is on.

## Why poptop and not the others

htop, btop and bottom have the live table and no history, so "why was it slow
four minutes ago" is not a question they can be asked. atop has the history but
its process records are per-interval, so the moment of the spike is averaged
before it is written. netdata has per-second metrics and no process attribution
at that resolution.

poptop holds a **complete process table at the instant in question**, which is
exactly and only what this needs. Nothing has to have been running beforehand.

## What it looks like

A panel — a key, not a mode — over the sample under the cursor:

```text
  cpu 98.2%   87% accounted for
      71%  14 × cc1plus        parent 8841 make -j16, started 03:02:11
      12%  1 × ld              parent 8841 make -j16
       4%  1 × rustc
      13%  unattributed — 41 processes below 1% each
```

## What needs deciding

- **How much it is allowed to claim.** Per-process CPU does not sum to the
  machine total: kernel time, short-lived processes and rounding all leak. The
  figure that keeps this honest is the **unattributed remainder**, stated every
  time — an explanation that cannot say how much it accounts for is a guess in
  a confident voice. When the remainder is large the panel should say so loudly
  rather than rank three processes that explain nothing.
- **Which figures get an explanation.** CPU, memory and disk have obvious
  per-process attribution. Stall pressure does not — it is a property of the
  machine, and the honest join is "these processes were in D state", which is
  weaker and must be labelled as weaker.
- **Grouping.** "14 × cc1plus" is the useful unit, not fourteen rows. That is
  the `g` grouping applied to the explanation, and it should agree with `g`.
- **Whether it explains the cursor, or the worst moment.** Both: at the cursor
  by default, and `--report` should carry the same paragraph for each of its
  findings rather than the one name it has now.

## Acceptance criteria

- [ ] Any moment in the buffer can be explained without leaving the panel
- [ ] The unattributed remainder is always stated, never implied
- [ ] Processes are grouped the way `g` groups them
- [ ] The parent that spawned a group is named where there is one
- [ ] A figure with no honest attribution says so rather than inventing one
- [ ] `--report` uses the same explanation, so the two cannot disagree
