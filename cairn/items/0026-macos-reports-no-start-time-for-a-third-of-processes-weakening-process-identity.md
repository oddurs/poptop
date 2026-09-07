---
id: 26
title: macOS reports no start time for a third of processes, weakening process identity
type: bug
status: done
milestone: v1.0
created: 2026-09-06
updated: 2026-09-06
priority: p2
effort: m
area: collect
---

## What happens

A process is identified throughout poptop by `(pid, started)`. `series_for` and
`churn` both key on it, and both say why in their doc comments: on pid alone a
recycled pid splices two unrelated programs into one graph.

On macOS that key is weaker than it looks. Measured on this machine, **170 of
598 processes report `started == 0`** — sysinfo cannot determine a start time
for processes the caller does not own. So for 28% of the table the key degrades
to the pid alone, which is precisely the case it exists to defend against.

The two backends also disagree about what the field means:

- Linux: clock ticks since boot (`/proc/<pid>/stat` field 22), e.g. `264317`
- macOS: seconds since the Unix epoch, e.g. `1788744679`, or `0` when unknown

Nothing compares them across platforms today, so this is latent rather than
broken — but it is undocumented, and item 0014 now writes the field to disk.

## What should happen

Two separate things:

1. **Say what the field is.** It is a platform-specific opaque token whose only
   contract is "stable for the life of one process on this machine". Written
   down, the epoch difference stops being a trap.
2. **Stop `0` standing in for "unknown".** Zero is a value the comparison
   trusts. An `Option`, or a per-process fallback that is actually unique,
   would keep the guarantee the key was introduced for.

Worth checking whether sysinfo exposes a start time under a different call
before adding machinery — the cheapest fix is the one that makes the existing
field correct.

## Acceptance criteria

- [ ] `ProcSample::started` documents its units and that they differ per platform
- [ ] Unknown is distinguishable from a real start time
- [ ] A recycled pid on macOS cannot be mistaken for the process that had it
- [ ] Measured: how many processes still lack an identity after the fix
