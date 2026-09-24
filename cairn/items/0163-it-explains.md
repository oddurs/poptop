---
id: 163
key: v5.0
title: It explains
type: milestone
status: backlog
created: 2026-09-13
updated: 2026-09-13
priority: p1
due: 2028-01-01
---

poptop can show you any instant and it makes you do the joining. The buffer
holds a complete process table for every moment it retains — nothing else in
this category has that — and the tool still answers "CPU was at 98%" when it is
holding everything needed to answer "because fourteen `cc1plus` under one
`make -j16`, which started at 03:02:11".

This milestone is the turn from displaying to explaining. Every item in it is
built on samples poptop already collects: no new subsystem, no daemon, nothing
that needs installing in advance. That is the point — the explanation is
available on the box you just connected to, which is where nobody else can go.

The discipline is the same as everywhere else in this tool: an explanation that
cannot say how much of the figure it accounts for is a guess with a confident
voice, and poptop would rather say "sixty percent of that is unattributed" than
name a plausible culprit.
