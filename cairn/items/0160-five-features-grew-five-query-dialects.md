---
id: 160
title: Five features grew five query dialects
type: feature
status: backlog
milestone: v7.0
depends_on:
- 146
- 149
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

By the end of v4 there are five ways to ask poptop a question:

| | asks | grammar |
| --- | --- | --- |
| the filter (`/`) | which processes, now | `cpu > 5 and user = root` |
| the search (0088) | when did this happen | the same, plus time |
| `--report` | what happened over a period | a fixed list, in Rust |
| `--export` | give me the records | a record name |
| the panels (0091) | draw this series | Rust |

They overlap almost completely and share no code. Every one of them can select by
process attribute; three can select by time; two can aggregate. Each has its own
parser, its own errors, its own tests, and its own bugs — and the fourth and
fifth are not available to users at all, because they are source code.

## What it should be

**One language over one thing: a selection across (time × process × metric).**

```text
  cpu > 80 and name ~ cc1plus                  a filter
  cpu > 80 for 30s                             a search over time
  max(cpu) by name over 1h                     a report line
  mem.used, mem.cached over 10m                a panel
  sum(pss) by service where user = postgres    all three at once
```

One evaluator, one error message, one set of tests. And then the thing that makes
it worth doing:

- **A panel is a saved query.** Adding one stops being a source change.
- **A report is a query plus a renderer.**
- **An alert (0103) is a query plus a threshold.**
- **An export is a query with no renderer at all.**

## The structural argument, which is the whole milestone

Every other tool in this space decides its aggregations at write time. Prometheus
stores the series you configured; netdata stores the metrics its collectors
emit; atop writes per-interval process records. Ask any of them a question that
was not anticipated when the data was written and the answer does not exist at
any price — the raw material was discarded.

poptop retains **complete samples**. Every process, every field, at every instant
it kept. That means a query language over poptop's history can answer questions
that had not been thought of when the data was recorded — and that property is
not a feature anybody can add to the others later, because their history is
already lossy.

This item is what turns that property from a fact about the storage into
something a user can reach.

## What needs deciding

- **Whether this is worth the churn.** It is a rewrite of five working things.
  The case for doing it is that the fifth and sixth question are coming
  regardless — alerts, user panels — and each would otherwise grow a sixth and
  seventh dialect. The case against is that a query language is a support
  surface, and a bad one is forever.
- **How far the language goes.** Selection, comparison, `and`, one aggregate, one
  time window, one grouping. That covers every use above. Joins, subqueries and
  arithmetic between series are where this becomes a product nobody asked for.
- **What it is called and how it is typed.** The filter box is one line and that
  is right for a filter; a report query is not a one-liner. A file of named
  queries — the config the panels are defined in — is probably where the long
  ones live.
- **Compatibility.** The existing filter syntax is documented, in the README, and
  in muscle memory. It must keep working exactly, which means the new language
  has to be a superset rather than a replacement.

## Acceptance criteria

- [ ] One parser and one evaluator serve filter, search, report and panels
- [ ] Every existing filter expression still means exactly what it meant
- [ ] A panel can be defined without changing the source
- [ ] The language has one error path, and it names what it could not parse
- [ ] A query over history is measured against a full retention window
- [ ] The grammar is stated in full, and fits on one page
