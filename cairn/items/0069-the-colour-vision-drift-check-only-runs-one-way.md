---
id: 69
title: The colour-vision drift check only runs one way
type: chore
status: backlog
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p0
effort: m
area: web
---

## Problem

`web/src/cvd.rs` is a hand-copy of the tool's `src/cvd.rs` — the Machado
matrices, the sRGB linearisation, the OKLab conversion. Its module documentation
calls this a hazard and points at `the_simulation_agrees_with_the_tool` as the
guard.

That guard does not do what the comment says. The test computes 3.7 from *the
copy's own matrices*. Change a coefficient in the tool and the copy is
untouched: the site's test still passes while the tool's fails, and the site goes
on showing a simulation the product no longer performs.

It catches drift in the copy. It cannot catch drift in the original, which is
the direction that matters, because the original is the one under development.

The justification written into the module — that a shared crate would widen the
tool's dependency graph — has the arrow backwards. Dependencies point one way. A
path dependency from `web/` to the tool adds nothing to `cargo install poptop`.
Nothing was protected by copying.

## Proposal

One source. Cheapest form: `include!` the tool's module behind a thin shim that
supplies the two things the site needs and the tool does not — hex parsing and
`simulate` returning a colour rather than a distance.

Where that does not fit, a test that runs the tool and compares its output. What
must not remain is a comment claiming a guarantee it does not provide.

## Acceptance criteria

- [ ] The matrices and conversions exist once
- [ ] Changing the tool's coefficients fails a test in `web/`
- [ ] `cargo install poptop` still resolves nothing new
- [ ] The module comment describes what is actually enforced
