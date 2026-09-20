# One process in depth

You have found the row. Now you want its shape over time, its children, its
threads, and — if it comes to that — a way to stop it.

## Select it

`↑`/`↓` move the selection, and the selected row is marked in the margin:

```
   CPU%            RSS      S   THR    DISK R    DISK W HIST ≤50%      PID USER       COMMAND
   25.4 █         4.7M      S     1   29.4M/s         0 ⠀⠀⠀⠀⠀⠀⠀⢰⣴⣤   13633 oddurs     du
▶  22.6 ▉       110.5M      S    13         0         0 ⠀⠀⠀⠀⠀⠀⣤⣀⣀⣤   38359 oddurs     ghostty
   16.4 ▋        14.5M      S    12         0         0 ⠀⠀⠀⠀⠀⠀⣀⣀⣀⣀   86077 oddurs     rsst
```

The selection is sticky across samples: it follows the process, not the row
position, so scrubbing and re-sorting do not silently move it to a
different program. `Esc` lets it go.

## `d` — its own history, at full width

`d` replaces the machine's timeline with the selected process's:

```
── ghostty — 7s of 10m00s buffered ───────────────────────────────────
   50 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
  CPU ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
    0 ⠀⠀⠀⠀⠀⠀no history before 20:30:05 — it fills from the right⠀⠀⠀⠀
   10 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
  MEM ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
    0 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
past                                           7s shown, 1s/slot
```

It is the `HIST` sparkline given the whole panel: CPU, memory, threads and
disk, over the window on screen, with the axis fitted to this process
rather than to the machine. `ghostty` peaking at 27% is drawn against 50,
not squashed against 100.

`+` and `-` widen the window the same way they do for the machine, and the
cursor is shared, so scrubbing moves both.

**Gaps are marked, not interpolated.** Where the process was not running,
the graph draws `┊` and says so in the footer:

```
past                              6s shown, 1s/slot, ┊ not running                               now
```

A process that restarts every thirty seconds looks like a process that
restarts every thirty seconds, instead of a smooth line through the gaps.

Press `d` with nothing selected and the panel title asks:

```
── timeline — 6s of 10m00s — ↑/↓ to pick a process first ──
```

`d` again puts the machine's timeline back.

## `t` — where it sits in the tree

```
── processes (734) — sort: CPU · tree ! io: 183/734 need root ──
   CPU%            RSS      S   THR    DISK R    DISK W HIST ≤50%      PID USER       COMMAND
      —              —      ?     —         —         — ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀       1 ?          launchd
   15.0 ▋       110.7M      R    11         0         0 ⠀⠀⠀⠀⠀⠀⣄⣤⣤⣄   38359 oddurs     ├─ ghostty
    6.4 ▎        40.1M      S   105         0         0 ⠀⠀⠀⠀⠀⠀⣀⣀⣀⣀   55321 oddurs     ├─ herdr server
    0.0         560.0K      S     1         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀   73900 oddurs        ├─ -fish
    0.1           7.5M      S     5         0         0 ⠀⠀⠀⠀⠀⠀⣀⣀⣀⣀   73993 oddurs        │  └─ harrow
    0.0         560.0K      S     1         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀   52185 oddurs        ├─ -fish
    0.2         232.6M      S    15         0         0 ⠀⠀⠀⠀⠀⠀⣀⣀⣀⣀   53858 oddurs        │  └─ node --experimental-modules main
    0.0           5.8M      S    13         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀   53931 oddurs        │  └─ railway mcp
```

Two things to read here. First, the tree answers "whose worker is this" in
one glance: `harrow` under `-fish` under `herdr server`.

Second, look at pid 1. `launchd` is a row of em dashes — poptop cannot read
it without privilege, and rather than showing you `0.0` and `0` it shows
you that it does not know. The tree opens anyway, so its children are still
visible. Nothing in the table is ever a fabricated zero; see
[platforms](../reference/platforms.md).

`t` again goes back to the flat table. Grouping (`g`) is not available in
the tree — a tree of folded rows has no single parent.

## `y` — its threads

`y` expands the selected process into its threads, and only the selected
one, so a table of seven hundred processes does not become a table of nine
thousand rows.

On macOS the count is all that is published:

```
── processes (733) · threads: not read on this platform — sort: CPU · 101 git (g folds them) ──
```

The `THR` column still has the number. On Linux the threads appear as rows.

## `x` and `X` — stopping it

Off unless you asked for them:

```console
$ poptop --signals=on
```

and then `x` sends TERM, `X` sends KILL, each after a confirmation naming
the process. poptop will not send to a table you have scrubbed back to, and
rechecks the (pid, start time) pair before sending. The reasoning and the
full rules are in [signals](signals.md).

## Getting back out

`Esc` backs out one level at a time — the detail panel, then the selection.
`q` quits from anywhere.
