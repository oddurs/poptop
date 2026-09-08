---
id: 71
title: One number, written three times
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

Every still frame declares its row count three times:

    style="--rows: 4"                     for the CSS height reservation
    rows: 4,                              for the Rust template
    config: r#"{"cursor":232,"rows":4}"#  for the browser

and the struct field carries a comment reading "Must match `rows` in `config`".

A comment instructing the reader to keep two fields in sync is an apology for an
interface, shipped in place of fixing it. Nothing catches a mismatch: set the
field to 4 and the config to 5 and the page reserves the wrong height, silently,
which is the exact layout-shift bug the field was added to prevent.

The absence of `serde` — which is deliberate — argues for building the JSON from
the struct by hand. It does not argue for writing the number three times.

## Proposal

Give `Still` typed fields for everything currently in the JSON string, and a
`fn config(&self) -> String` that renders them. One number, three renderings, no
instructions to the reader.

## Acceptance criteria

- [ ] `Still` has no free-form config string
- [ ] The row count is written once per still
- [ ] The CSS custom property and the JSON come from the same field
