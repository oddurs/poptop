# Views and modes

The table shows one set of columns at a time, and `v` cycles the set.
Everything else here is a mode on top of it.

## Column sets (`v`)

| View | Columns beyond the identity | For |
|---|---|---|
| generic (default) | CPU%, RSS, state, threads, history | what is busy |
| memory | PSS, VSZ, MAJF/s, GROW | what memory is actually costing, and what is paging |
| disk | DISK R, DISK W, always | what is touching the device |

The disk columns also appear in the generic view when the terminal is wide
enough. `v` is what brings them back when it is not — that is the concrete
thing views fix.

## Modes

| Key | Mode | What changes |
|---|---|---|
| `t` | tree | Rows nest under their parent. Ordering is by branch, so the busiest branch is first. |
| `g` | group | Rows fold by name, then by user, then by container, then off. A folded row shows `×N` for its pid and sums what can be summed. |
| `y` | threads | The *selected* process expands into its threads. Only that one: a box has eight times as many threads as processes. |
| `d` | detail | The timeline is replaced by the selected process's own history — CPU, memory, threads and disk — at full width. |
| `C` | cgroups | The table shows cgroups instead of processes: what each is using and how stalled it is. Linux, cgroup v2. |
| `K` | kernel threads | Shows them. Hidden by default on Linux, where they outnumber real processes several times over. |
| `i` | IO columns | Shows or hides the per-process disk columns. |

Tree and grouping are mutually exclusive: grouping destroys parentage by
construction, so a grouped tree would be a tree of things that are not
processes.

## Sorting

`s` cycles the sort column, within the columns the current view shows —
sorting by a column you cannot see is an order with no visible reason.

`S` accepts the panel's suggestion. When poptop can tell what is stopping
work — the disk is saturated, memory is stalling — the title says so and
`S` sorts by it. It is never applied on its own: a table that reorders
itself under the reader is worse than one that does not.

A process poptop could not read sorts after the processes it could, not
among the idle ones. Its zeros were never measured.
