---
id: 11
title: Capture processes that live and die between samples
type: feature
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: l
area: collect
---

## Problem

poptop samples once a second and reads `/proc` at that instant. A process that
lives 200ms never existed as far as poptop is concerned.

That is not an edge case for this tool. A burst of short-lived processes is one
of the most common causes of exactly the spike poptop exists to help you find — so
scrubbing back to the spike shows a process table that cannot explain the graph
above it.

atop solves this and is explicit about it: it reports "resource consumption by
all processes that were active during the interval, so also the resource
consumption by those processes that have finished during the interval". **This
is the substantive capability gap between the two tools**, and the reason the
README points people at atop for fleet history.

## Approaches, in preference order

1. **taskstats over netlink** — the kernel emits an exit record per process.
   Accurate. Needs `CAP_NET_ADMIN`. Linux only.
2. **BSD process accounting** (`acct(2)`) — needs root and a writable accounting
   file; more intrusive.
3. **Higher sample rate** — narrows the window without closing it. Cheap, a
   reasonable interim, and explicitly not a fix. Depends on the collector being
   cheap enough, which is why the E items come first.

## Degrade honestly

Without the capability, say so in the UI. Do not render an interval that
quietly omits what happened in it. Same principle as the gated IO columns:
never a fabricated zero.

## Acceptance criteria

- [ ] A process living 200ms appears in the interval containing it, marked as exited
- [ ] Missing capability is disclosed, not silent
- [ ] macOS degrades explicitly
- [ ] Sampling cost measured before and after with `--bench`

## Progress

**Landed: the honest-degradation half.** poptop now reads `processes` from
`/proc/stat` — the kernel's count of task creations since boot — and reports
what an interval created against what the table can account for:

    ── processes (312) — sort: CPU · 47 came and went ──────────

Counted in tasks rather than processes, because the kernel's counter is: a
`clone` for a thread advances it exactly as a `fork` does, so comparing it
against process rows would report sixteen threads as sixteen invisible
processes. Thread growth inside surviving processes counts on the visible side.
macOS reports `None`, not zero. Measured at 402 processes: 1.068ms/sample
before, 1.026ms after — no cost, since it is one integer parse on a file
already being read.

Verified against a real burst on Linux: 300 short-lived processes gave
`created 300, visible 0, 300 came and went`, with the process table showing 2
rows before and 2 after.

**Outstanding: approach 1, and with it criterion 1.** Capturing the processes
themselves still needs taskstats over netlink.

While prototyping it, the listener registration
(`TASKSTATS_CMD_ATTR_REGISTER_CPUMASK`) was refused with `EINVAL` on every
kernel available here — including with `--privileged`, and for every cpumask
form from `0` upward. A plain per-pid `TASKSTATS_CMD_ATTR_PID` query on the
same socket works and returns a 724-byte record, so the family is alive and the
refusal is specific to registering as an exit listener. `0-4095` returns
`ERANGE` rather than `EINVAL`, so the mask is parsed before being rejected —
the refusal is after parsing, not in it.

Not implemented rather than implemented blind: it is several hundred lines of
unsafe FFI whose entire value is accuracy, and shipping it unverified would be
worse than the gap it closes. Needs a kernel where the exit-record path can
actually be exercised.
