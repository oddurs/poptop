---
id: 49
title: Filtering is a substring match, so the useful questions cannot be asked
type: feature
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## The lesson

bottom's process search is a small query language, not a substring match:

```text
cpu > 0.5 and mem < 0.5
(read > 1 mb or write > 1 mb) and !chrome
state = D
memb > 1000 mb
```

Keywords for every column it shows, the six comparison operators, `and`/`or`/
`!` with parentheses, units on byte quantities, and regex with toggles for case
and whole-word. poptop's filter is `name.contains(needle) || pid.contains(needle)`,
lowercased.

## Why it fits the thesis

"Why is this slow" is usually a question about a *predicate*, not a name. The
useful queries are the ones poptop cannot express:

- `state = D` — everything stuck in uninterruptible sleep, which is what the
  `BLOCKED` figure in the header counts and cannot itemise
- `write > 1mb` — who is causing the disk saturation the header just reported
- `threads > 100` — the thing leaking threads

Each of those is a header figure the reader is already looking at, and the table
has no way to answer "which processes are *that*".

It is also worth more here than in bottom, because of the buffer: a predicate
evaluated while scrubbing answers "what was in D-state at the moment of the
spike", which is a question no live-only tool can be asked.

## What needs deciding

- **Scope.** The full grammar is a parser, a test suite and a syntax to
  document. A useful subset — `field op value` joined by `and`, with units —
  is most of the value for a fraction of it. Deciding where to stop is the
  work.
- **Discoverability.** A query language nobody knows the keywords for is worse
  than a substring match. bottom's answer is documentation; a better one might
  be completion or an error that names the fields.
- **Not breaking the simple case.** A bare word must keep meaning "substring",
  or every existing use of `/` breaks.

## Acceptance criteria

- [ ] `state = D`, `write > 1mb` and `and` work, with units on byte fields
- [ ] A bare word still filters by substring
- [ ] A malformed query says what is wrong and filters nothing away
- [ ] The predicate is applied at the cursor while scrubbing, not to the live sample
