---
id: 118
title: There is nowhere to look up a key, a column or a mark
type: docs
status: done
milestone: r6
assignee: Oddur Sigurdsson
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: m
area: docs
---

## Problem

The keys are documented in three places that a test now keeps in step (`--help`, the footer, the `?` overlay), but a reader who wants the list outside the program has only `--help`, which is prose. Columns are worse: `HIST ≤25%`, `×24`, `·`, `—`, `+352.0K`, `████+` all mean something precise and none of it is written down anywhere a reader can find in one place. The filter language is described inside a README section about filtering.

## Proposal

A reference under `docs/`: every key, every column, every mark, the filter grammar and the views, in tables. Generated where it can be — the key list already exists as `ui::HELP` — and checked by a test where it cannot.

## Acceptance criteria

- [ ] `docs/reference/keys.md`, `columns.md`, `filter.md`, `views.md`
- [ ] A test holds the documented key list against `ui::HELP`
- [ ] Every mark the table can draw is listed with what it means

## How it was resolved

`docs/reference/` now holds `keys.md`, `columns.md`, `filter.md` and
`views.md`. The key table is the same list as `ui::HELP`, in the same order,
and `src/docs_tests.rs` holds the page against it — both the list and, for
each key, that the page's row mentions the longest word of the on-screen
description. Swapping `t` for `T` in the page fails two tests.

`columns.md` tabulates every column in every view, every mark (`—`, `·`,
`?`, `×24`, `████+`, the braille history, `+352.0K`), and the panel-title
notes that explain why a column is missing — including the width ladder.
