---
id: 92
title: Nothing enforces the browser audit
type: chore
status: done
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p0
effort: m
area: web
---

## Problem

`grep -c audit .github/workflows/site.yml` is 0. `grep -c web check` is 0.

Four Playwright suites cover contrast, heading order, tap targets, layout shift,
the scrubbable frames and the colour-vision control. None runs anywhere but a
laptop, and their results have repeatedly been reported as though they were
properties of the repository.

This project's own position on the matter is unusually explicit.
`.githooks/no-attribution` is wired into three layers specifically so they
cannot disagree, and `./check` treats a missing tool as a failure rather than a
skip, on the stated grounds that a check which does not run is worse than no
check. The audit is the one thing here held to a lower standard than the project
holds itself.

It matters because of what the audit catches. `--ink-faint` sat at 3.08:1
against the surface it was drawn on, across every page, in both themes, and no
human reading the diff noticed. That is the class of bug a machine has to find.

## Proposal

A `browser` job in `site.yml`: install `playwright-core`, start the server,
run the four suites, fail on any non-zero exit.

If browser flake proves intolerable it can become non-blocking and reported —
what is not defensible is quoting the results as guarantees while nothing
enforces them.

## Acceptance criteria

- [ ] CI runs all four suites against a real server
- [ ] A contrast regression fails the build
- [ ] The job is documented in `audit/README.md`
