# First run

```console
$ poptop
```

That is the whole setup. No daemon, no config file, no logfile to enable
first. poptop starts with an empty buffer and fills it as it runs, so the
first thing to know is that the left of the timeline will be blank for a
while and says so:

```
    0 ⠀⠀⠀⠀⠀⠀no history before 20:24:41 — it fills from the right⠀⠀⠀⠀⠀⠀⢸⣿⣿
```

If you want it to remember across restarts, `--store=on`; if you want it to
outlive the process, `--log=on`. Neither is needed for anything on screen.
See [recording](recording.md).

## What is on screen

Three regions, always in the same order, at any width.

```
 LIVE CPU  99.5%  │  MEM  85.6% ██████████░░  SWP  84.3%  │  / 99.7% full  │  en0 ↓21.8M/s ↑192.4K/s
 10 cores ████ ████ ██
── timeline — 7s of 10m00s buffered ────────────────────────────────────────────────────────────────
  100 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣆⣿⣿
  CPU ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢰⣿⣿⣿
    0 ⠀⠀⠀⠀⠀⠀no history before 20:24:41 — it fills from the right⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿
  100 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣤⣤⣤⣤
  MEM ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿
    0 ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿
past                                      7s shown, 1s/slot                                      now
── processes (748) — sort: CPU · memory is the constraint (S) ! io: panel too narrow ───────────────
   CPU%            RSS      S   THR HIST ≤200%     PID USER       COMMAND
   30.3 █▎      166.9M      S    12 ⠀⠀⠀⠀⠀⠀⠀⣀⣀⣀   96543 oddurs     node --experiment…l-modules main
   25.5 █       172.0M      R    45 ⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀   96556 oddurs     Google Chrome
   20.7 ▉       437.8M ▏    S     1 ⠀⠀⠀⠀⠀⠀⠀⣀⣀⣀   28117 oddurs     poptop --interval…l=60s --store=on
   16.9 ▋        13.2M      R    12 ⠀⠀⠀⠀⠀⠀⠀⣀⣀⣀   86077 oddurs     rsst
   10.5 ▍         8.7M      S     3 ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀   96680 oddurs     SafariLaunchAgent
q quit · ←/→ scrub · b jump · +/- zoom · Space live · ↑/↓ select · s sort · / filter · ? more
```

**The header** is the machine in one or two lines: CPU, memory, swap, the
fullest filesystem, the busiest link. Figures appear only when they have
something to say — `CLK` shows up when the kernel is holding the clock
below nominal and stays away when it is not, `STALL` when a cgroup is being
held back. A header that is quiet is a machine with nothing to report.
See [the header reference](../reference/views.md).

**The timeline** is every sample poptop has taken, oldest on the left. The
gutter names each graph and anchors its scale; the footer under it says how
much time is on screen and how much each column is worth. The dashed rules
are `--warn` and `--critical`.

**The process table** is the real process table from the moment under the
cursor — not an average, not an interpolation. The panel title carries the
count, the sort column, and anything poptop wants you to know about the
table itself.

**The footer** is the keys that fit. When some were dropped it ends with
`? more`; `?` lists [all of them](../reference/keys.md).

## The first four keys

| | |
|---|---|
| `←` `→` | move back and forward through history |
| `Space` | stop here, or go back to live |
| `↑` `↓` | pick a process |
| `?` | every key, on screen |

Everything else is built on those. `q` or `Ctrl-C` quits; `Esc` backs out
of whatever is open before it does.

## What the panel title is telling you

```
── processes (748) — sort: CPU · memory is the constraint (S) ! io: panel too narrow ──
```

- `(748)` — how many rows the table is drawn from, after any filter.
- `sort: CPU` — the column it is ordered by. `s` cycles it.
- `· memory is the constraint (S)` — a suggestion, named and not imposed.
  poptop has looked at the machine and thinks memory, not CPU, is what is
  stopping work; `S` sorts by it. The table never reorders itself.
- `! io: panel too narrow` — a reason a column is missing. The other one
  you will see is `! io: 161/740 need root`, which means the same thing for
  a different reason: those figures exist and poptop is not allowed to read
  them. See [platforms](../reference/platforms.md).

## Sizing the terminal

poptop works from about 60 columns. As the window narrows it gives up
columns in a fixed order — the sparkline first, then the bars, then
container, user, threads and so on — so that the command line survives
longest. Nothing reflows depending on which rows happen to be visible, so a
figure does not move when you scroll. [The ladder is
written down](../reference/columns.md#why-a-column-disappeared).

At 100 columns you get most of it. At 140 you get the disk columns without
asking.

## Where to go next

- [Finding what is slow](whats-slow.md)
- [Scrubbing to a moment](scrubbing.md)
- [One process in depth](one-process.md)
- [Groups and trees](groups.md)
- [Recording and replay](recording.md)
