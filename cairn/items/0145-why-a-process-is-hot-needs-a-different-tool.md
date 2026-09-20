---
id: 145
title: Why a process is hot needs a different tool
type: feature
status: backlog
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

- [ ] Decided in writing, either way, with the reasoning
- [ ] If built: opt-in, and off by default like signals
- [ ] If built: refuses while scrubbing, because stacks have no history
- [ ] If built: native frames only, with interpreted runtimes declined in writing
- [ ] If built: the cost to the sampled process is measured and stated
- [ ] If declined: the README says so and why, rather than listing it as missing
