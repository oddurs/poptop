---
id: 172
title: Why a process is hot needs a different tool
type: feature
status: done
milestone: v5.1
depends_on:
- 76
created: 2026-09-13
updated: 2026-09-13
priority: p2
area: collect
---

## Problem

poptop names the process burning the CPU. The next question is always "doing
what", and the answer is `perf record`, or `py-spy dump`, or `rbspy`, or
`eu-stack` — none of them installed, each needing its own incantation, and by
the time one is fetched the spike is over.

`py-spy dump --pid 1234` is the gesture this is competing with, and it is a very
good gesture: no restart, no instrumentation, an answer in a second.

## The shape

A key on the selected process: sample its stacks for a moment and draw the
result as a terminal flamegraph, in the panel, over the process already under
the cursor.

```text
  postgres 4823 — 412 stacks over 2.0s
  ████████████████████████████████████████  100%  (root)
  ██████████████████████████████▏            74%  ExecScan
  ███████████████████▏                       48%    ExecQual
  ███████▏                                   18%      slot_getattr
  ████▏                                      11%  heap_getnext
```

## Why this is the hardest item here, and worth it anyway

It is the first thing poptop would do that is not reading a file. Sampling
stacks needs `perf_event_open` or `process_vm_readv` plus unwinding, both of
which are privileged on most kernels and neither of which is portable. The
reading rule from v2.1 says exactly what to do with that: it is an **optional
source** — enriches the picture, never required, absent honestly when it cannot
be had — and opt-in the way signals are.

It is worth it because it closes the loop. Every other item in this milestone
narrows "what is wrong" from the machine to a process. This is the only one that
gets inside the process, and it is the difference between a monitor and the
thing you reach for during an incident.

## What needs deciding

- **Whether at all.** This is a positioning decision like signals were, and it
  should be decided in writing either way. The argument against: poptop's whole
  privileged surface is reading files, and this is `ptrace`-adjacent. The
  argument for: the alternative is a tool nobody has installed, and poptop
  already knows the pid, the command line and the moment.
- **Native stacks only, or interpreters too.** A C stack is unwindable with
  frame pointers or DWARF. A Python or Ruby stack needs interpreter-specific
  reading, which is what py-spy and rbspy *are* — that is their whole
  difficulty, and poptop should not reimplement them. Native only, and say so.
- **Live only.** Stacks cannot be sampled from history. The panel has to refuse
  while scrubbing exactly as the signal does, and for the same reason.
- **What it costs the target.** Sampling perturbs. The figure has to be measured
  and stated, and the default duration kept short enough that nobody has to
  think about it.

## Acceptance criteria

- [x] Decided in writing, either way, with the reasoning
- [x] If built: opt-in, and off by default like signals
- [x] If built: refuses while scrubbing, because stacks have no history
- [x] If built: native frames only, with interpreted runtimes declined in writing
- [x] If built: the cost to the sampled process is measured and stated
- [x] If declined: the README says so and why, rather than listing it as missing

## How it was resolved

**The rule: an entry is on the disk before `append` returns.** One `sync_data` after the write, every entry. Measured first, then chosen:

| | flush only | flush + `sync_data` |
| --- | --- | --- |
| APFS, M-series Mac | 0.087 ms | 4.189 ms |
| ext4, Linux arm64 container | 0.020 ms | 2.781 ms |

An 87 KB entry, which is what a ~740-process machine writes. At the default ten-minute interval that is nothing; at a one-second `log-interval` it is under half a percent of the interval. The alternatives — sync every N entries, or on rotation — buy a fraction of a percent back and give up the property that makes the log worth having, so they were not taken.

`sync_data` rather than `sync_all`: the length is the metadata that matters and a data sync carries it.

**What it does not promise**, both now in the recording guide: on macOS this is `fsync`, which asks the drive to persist and does not force the drive's own write cache (`F_FULLFSYNC` does, at roughly ten times the cost — not a trade a monitor should make for someone). And the restart store is deliberately different: written once on a clean exit, so `kill -9` costs at most the session's buffer.

**The test.** A power cut needs hardware this does not have. What is checked instead is the property the call promises: a child process appends three entries and then `raise(SIGKILL)`s itself — no unwinding, no destructors, no exit path — and the parent reads all three back whole. Without the sync the entries are the exit path's responsibility; with it they are the writer's.

**The budget.** `log: append one entry, synced` measured 3.8 ms against a 40 ms budget, so a regression that made the sync ten times dearer would fail `./check --perf` rather than being discovered by someone's disk.

