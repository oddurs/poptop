---
id: 94
title: The 404 page exists twice, verbatim
type: chore
status: done
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: s
area: web
---

## Problem

    $ grep -c "That page is not here" src/app.rs src/export.rs
    src/app.rs:2
    src/export.rs:2

From the module documentation of `src/export.rs`, thirty lines above the
duplicate: "the same handlers that serve the site write it to disk, so there is
no second implementation to keep in step."

The routing genuinely is single-sourced — `routes::all()` drives both, and a page
in one and not the other fails a test. What is duplicated is one page's body,
copied because the async handler was inconvenient to call from a synchronous
exporter.

A file that states a principle in its header and violates it in its body is
worse than one that never claimed anything, because the next reader trusts the
header.

## Proposal

Hoist the body into `views::not_found()` and call it from both.

## Acceptance criteria

- [ ] The 404 body exists once
- [ ] Both the server and the export render it from that one place
