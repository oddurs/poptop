---
id: 75
title: ui::section takes five positional arguments
type: chore
status: done
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p2
effort: s
area: web
---

## Problem

    ui::section("problem", "the problem", "you are always late", Some("…"), body)

Two of those strings both appear in the section header and a reader has to count
commas to know which is the gutter and which is the heading.

The `rule` / `rule_heading` / `rule_inner` split is not the complaint — that
distinction is real and fixed an outline bug where the document jumped from h1
to h3. The complaint is the signature, particularly next to `Still` in the same
file, which is a struct with named fields.

## Proposal

Make `section` take a struct, like its neighbour.

## Acceptance criteria

- [ ] Section call sites name their arguments
- [ ] The h2-versus-div distinction is preserved
