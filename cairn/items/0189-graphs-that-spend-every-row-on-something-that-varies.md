---
id: 189
title: Graphs that spend every row on something that varies
key: v3.5
type: milestone
status: backlog
created: 2026-09-14
updated: 2026-09-14
priority: p1
area: ui
---

## The thesis

A graph is best when three things are true of it, in this order:

1. **Every row shows something that varies.** A panel whose bottom three rows
   are solid whatever the machine does has thrown away three quarters of its
   resolution before it drew anything.
2. **It says what you are looking at.** Not only the number, but what caused
   it, and — when zoomed — what it is hiding.
3. **It does not lie to do either.** Every one of the above is available to a
   tool willing to mislead; none of them is worth that.

The competition wins on none of these. btop draws beautifully and colours by
magnitude, double-encoding the one thing the height already says. bottom and
htop pin every axis to zero. atop draws nothing at all. netdata draws in a
browser. None of them can label a spike with what caused it, because none of
them keeps the process table at every sample — and that is not something they
can add later, because the data was discarded at write time.

## The three levers, ranked

**Range** is the biggest and the cheapest. It is a decision about the axis, not
about the characters. See 106, 112.

**Resolution** is next and is a commodity: a character cell holds eight levels
and a pixel holds one. The terminal graphics protocols are widely supported
enough now that this is an engineering question rather than a research one. See
108.

**Attribution** is the moat. It is the only one of the three that a competitor
cannot copy, and the only one that makes the graph answer a question rather than
pose one. See 109.

## Acceptance criteria

- [ ] A high flat series is legible without changing any setting
- [ ] A zoomed graph says what it is aggregating away
- [ ] A spike can be attributed without leaving the graph
- [ ] Two panels can be put on one scale and compared
- [ ] Every claim above degrades rather than disappearing on a dumb terminal
