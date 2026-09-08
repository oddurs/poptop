---
id: 11
title: Capture processes that live and die between samples
type: feature
status: backlog
milestone: v2.0
created: 2026-09-05
updated: 2026-09-08
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

The listener registration (`TASKSTATS_CMD_ATTR_REGISTER_CPUMASK`) is refused
with `EINVAL` on every kernel available here, including with `--privileged`. A
plain per-pid `TASKSTATS_CMD_ATTR_PID` query on the same socket works and
returns a record, so the family is alive and the refusal is specific to
registering as an exit listener.

**Diagnosed, on a second pass.** The refusal is *not* the prototype's fault, and
two plausible causes are ruled out:

- **Not a missing NUL terminator.** The kernel copies the cpumask with
  `nla_strscpy`, which reserves a byte for the terminator, so an attribute sized
  `strlen` loses its last character — and that would have explained the
  asymmetry below exactly. It does not: sending the mask with and without the
  trailing NUL gives byte-identical results.
- **Not a parse failure.** The `ERANGE`/`EINVAL` boundary sits exactly at
  `nr_cpu_ids`. On a fourteen-CPU box, `0-13` and `13` are refused with `EINVAL`
  while `0-14` and `14` are refused with `ERANGE`. So `cpulist_parse` runs, the
  mask is valid, and the refusal happens *after* it.

What is left is `add_del_listener`, and the `EINVAL` it can return for a
well-formed mask is the namespace gate: the kernel refuses exit-listener
registration from anything but the initial PID and user namespaces. **Every
Linux available here is a container**, which is by definition not that — so this
is not a kernel that lacks the feature, it is an environment that cannot reach
it. `--pid=host` would test the hypothesis directly and hangs on Docker Desktop
without producing output.

So the requirement is sharper than "a kernel where this can be exercised": it
needs a Linux host where poptop runs in the **initial** PID namespace — bare
metal, or a VM, but not a container.

Not implemented rather than implemented blind: it is several hundred lines of
unsafe FFI whose entire value is accuracy, and shipping it unverified would be
worse than the gap it closes.

## 2026-09-08

Refiled from v0.2 to v2.0. v0.2 asked for the data-fidelity gaps against atop to be "closed or disclosed", and the diagnosis above discloses this one thoroughly — it is an environment that cannot reach the kernel path, not an unknown. Closing it belongs with the milestone named for the guarantee it is: nothing escapes.
