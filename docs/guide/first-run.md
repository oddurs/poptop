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

Five bands, always in the same order, at any width.

```
 File  Edit  View  Go  Process  Help                                                       F10 menu
 LIVE CPU  99.5%  │  MEM  50.0% ██████▒▒░░░░   SWP  50.0%  │  UP   1d 01h 00m   PROCS    4
  4 cores █▄▁█
 CPU   Memory   Disk    sort CPU · avg 5s                                  All processes · 4 · root
── processes (4) · all root · history flat · io: not collected here ────────────────────────────────
 4 shown · CPU 93.4% · MEM 788.0M (5%) · 70 threads
 ▾CPU%            RSS      S   THR    DISK R    DISK W     PID COMMAND
  30.3 █▎      166.0M      S    12         ·         ·   96543 node
  25.5 █       172.0M      S    45         ·         ·   96556 Google Chrome
  20.7 ▉       437.0M ▏    S     1         ·         ·   28117 poptop
  16.9 ▋        13.0M      S    12         ·         ·   86077 rsst



── timeline — 7s of 9m59s buffered ─────────────────────────────────────────────────────────────────
  100                                                                                           ⣿⣿⣿⣿
  CPU                                                                                           ⣿⣿⣿⣿
                                                                                                ⣿⣿⣿⣿
    0                    no history before 01:03:06 — it fills from the right                   ⣿⣿⣿⣿
  100                                                                                           ⠤⠀⠀⠀
  MEM                                                                                           ⣤⣤⣤⣤
    0                                                                                           ⣿⣿⣿⣿
past                                      7s shown, 1s/slot                                      now
q quit · F10 menu · ←/→ scrub · b jump · +/- zoom · Space live · ↑/↓ select · s sort · ? more
```

**The menu bar** names every command there is, beside the key that also runs
it. `F10` opens it, `Alt` and a title's underlined letter opens one directly.
It is there to be read once and then not needed.

**The header** is the machine in one or two lines: CPU, memory, swap, the
fullest filesystem, the busiest link. Figures appear only when they have
something to say — `CLK` shows up when the kernel is holding the clock
below nominal and stays away when it is not, `STALL` when a cgroup is being
held back. A header that is quiet is a machine with nothing to report.
See [the header reference](../reference/views.md).

**The strip** belongs to the table under it, and answers three questions left
to right: which resource the columns are about (`CPU`, `Memory`, `Disk` —
`Tab` moves between them, or click one), what was done to the rows (the sort,
the folding, the averaging), and which rows are in the list at all. That last
part never disappears, so a table that has been filtered can never be mistaken
for the whole machine. `/` types the filter there too.

**The process table** is the real process table from the moment under the
cursor — not an average, not an interpolation. Its rule says only what the
table cannot show: rows withheld, a measurement given up, tasks that lived
and died between two samples.

**The timeline** is every sample poptop has taken, oldest on the left. It sits
under the table because it is how the table got here. The gutter names each
graph and anchors its scale; the row under it says how much time is on screen
and how much each column is worth. The dashed rules are `--warn` and
`--critical`.

If the graphs are boxes rather than marks, your font does not have the glyphs
poptop defaults to. `poptop --check-glyphs` draws every set it can use — pick
one that looks like a graph and set `graph = NAME`.

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

## What the strip and the rule are telling you

```
 CPU   Memory   Disk    sort CPU · tree            nginx · 3 of 748 · root
── processes (3) · 12 kernel hidden ! io: panel too narrow ──────────────
```

The strip is what you chose:

- `CPU  Memory  Disk` — which columns the table is carrying. The current one
  is underlined. `Tab` moves between them, `1`-`3` open one directly, and so
  does a click.
- `sort CPU · tree` — the ordering and the folding. `s` cycles the sort, `t`
  and `g` change what a row stands for. The sorted column also wears a caret
  in its own header, so the ordering is named where the ordering happens.
- `nginx · 3 of 748 · root` — which rows are in the list. Unfiltered it reads
  `All processes · 748`. This clause never disappears: a table that does not
  say it has been narrowed is a table that lies about the machine.

The rule under it is what poptop cannot show you:

- `(3)` — how many processes the rows stand for, which is not the same as how
  many rows there are once `g` folds them.
- `· 12 kernel hidden` — rows withheld, and why.
- `! io: panel too narrow` — a reason a column is missing. The other one
  you will see is `! io: 161/740 need root`, which means the same thing for
  a different reason: those figures exist and poptop is not allowed to read
  them. See [platforms](../reference/platforms.md).
- `· memory is the constraint (S)` — a suggestion, named and not imposed.
  poptop has looked at the machine and thinks memory, not CPU, is what is
  stopping work; `S` sorts by it. The table never reorders itself.

On a terminal too short to spend a row on the strip, the settings move back
into the rule — they are never stated nowhere.

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
