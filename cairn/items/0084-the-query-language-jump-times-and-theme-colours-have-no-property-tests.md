---
id: 84
title: The query language, jump times and theme colours have no property tests
type: chore
status: done
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

- [x] `query::parse` never panics and round-trips through its display form
- [x] `log::parse_when` never panics, including for years 0, 9999 and far-future timestamps
- [x] `parse_color` accepts exactly the documented forms; a test lists the grammar
- [x] Every error the three parsers return is checked to quote part of its input

## How it was resolved

**Found and fixed.** Typing a leap second, `23:59:60`, into the jump box was refused with "`2027-01-15` is not a date on the calendar", blaming today's date. `parse_clock` accepts `:60` on purpose. But impossible dates were caught after the fact, by checking that `mktime` landed on the day typed, and a leap second at 23:59 lands on the next day. Month lengths are now checked where the date is parsed (`Date::days_in_month`, proleptic Gregorian), and the landing check is gone. This also fixes `--read 2026-02-30`, which used to report "nothing recorded" instead of "not a date".

**Tests.** Never-panics tests over mangled input for `query::parse` (including every prefix of every query, since it parses on each keystroke), `log::parse_when` (at `now` = the epoch, today and year 9999), `parse_color`, `config::apply_file` and `parse_theme`. Every error from the query and jump parsers is checked to quote part of what was typed. `parse_color`'s grammar is listed as a test: every name, all 256 indexes, hex in either case, and a list of near misses it must refuse. `write_color` → `parse_color` round-trips for every colour either can produce. Query spacing and case are checked not to change a query.

`query::parse` has no display form, so "round-trips through its display form" became "spacing and case do not change a query".

Fuzz target: `typed`, which covers the filter, the jump box, colours (with the round trip as its assertion), config and theme files.
