---
id: 95
key: v5.0
title: One language under everything
type: milestone
status: backlog
created: 2026-09-13
updated: 2026-09-13
priority: p2
due: 2029-06-01
---

The architectural claim, and the one that decides whether poptop is a good tool
or the reference implementation of its category.

By v4 there are five ways to ask poptop a question, each with its own grammar:
the filter language over processes, the history search over time, `--report`'s
fixed set of findings, `--export`'s record selection, and the panels, which are
Rust. They overlap almost entirely and share no code.

They are all the same query: **a selection over (time × process × metric)**. One
language, one evaluator, one set of tests. Then a panel is a saved query, a
report is a query with a renderer, an alert is a query with a threshold, and a
user can add any of the three without touching the source.

The structural reason this is possible here and nowhere else: every retained
sample is *complete*. Prometheus and netdata decide their aggregations at write
time and discard the process table, so a question nobody anticipated cannot be
answered from their history at any price. poptop keeps the table, so it can
answer questions that had not been thought of when the data was recorded. That
is not a feature — it is the property everything in this milestone sells.
