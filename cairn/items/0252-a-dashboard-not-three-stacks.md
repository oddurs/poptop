---
id: 252
key: r13
title: A dashboard, not three stacks
type: milestone
status: backlog
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

The third milestone, and the one the first two are for: a layout that can hold
what poptop already collects.

The layout is a vertical stack fixed at build time (0176). Disks, network,
stalls, cgroups, NUMA nodes, NFS, sensors, the battery and the GPU have nowhere
to live, which is why the header's degradation ladder is fourteen entries deep
and why a two-hundred-column terminal shows the same two graphs an eighty-column
one does.

A dashboard here means four things, in this order:

1. **A layout tree** — rows and columns with weights, leaves that are tiles.
2. **Tiles that know how to be small** — each declares a minimum and a ladder
   of what it gives up, and the solver drops and stacks by priority. This is
   the rule poptop already follows everywhere, applied to tiles instead of to
   clauses.
3. **Tiles bound to a subject** — the machine, a process, a cgroup, a device, a
   container. Without that a dashboard is the same graph twelve times.
4. **Layouts that can be named, saved and shipped** — presets for the shapes
   people actually watch, and a reader's own in the config file.

Two hard constraints. The single stack is not replaced: below a breakpoint the
tree collapses to header, table, timeline, because `ssh box; poptop` in an
eighty-column window is what poptop is. And every tile reads the clock from
r12's axis — one window, one cursor, one slot grid — because a dashboard whose
tiles disagree about which instant a column is is a wall of unrelated pictures.
