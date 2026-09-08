---
id: 74
title: Page furniture is filed under the components layer
type: chore
status: backlog
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p2
effort: s
area: web
---

## Problem

`assets/css/65-sections.css` is 404 lines, entirely inside `@layer components`,
and every rule in it — `.twin__gap`, `.band-key`, `.cvd__chip` — belongs to
exactly one page.

The layer's name is its contract. A reader looking for reusable parts wades
through landing-page scaffolding, and the next person to add a page follows the
precedent.

## Proposal

Declare a `page` layer immediately after `components`: same precedence, honest
name. Move `65-sections.css` into it.

## Acceptance criteria

- [ ] Landing-page furniture is not in the `components` layer
- [ ] Cascade order is unchanged — the audit still passes at every width
