---
id: 84
title: The query language, jump times and theme colours have no property tests
type: chore
status: backlog
milestone: r1
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: m
area: input
---

## Problem

Three parsers take text a person types: `query::parse` (the filter language), `log::parse_when` (what `b` jumps to), and `theme::parse_color` / `config::parse_theme`. They are tested with examples. Nothing checks that every input either parses or returns an error naming where it went wrong, or that what a query prints back parses to the same query.

## Proposal

Add property tests with a small generator for each grammar. Properties: never panics on any string; `parse(display(q)) == q` where a printer exists; every error message names a position or token that is actually in the input.

## Acceptance criteria

- [ ] `query::parse` never panics and round-trips through its display form
- [ ] `log::parse_when` never panics, including for years 0, 9999 and far-future timestamps
- [ ] `parse_color` accepts exactly the documented forms; a test lists the grammar
- [ ] Every error the three parsers return is checked to quote part of its input
