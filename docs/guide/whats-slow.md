# Finding what is slow

The machine is unhappy and you want to know what is doing it. This is the
whole sequence, and it is three keys.

## 1. Read the header first

```
 LIVE CPU  17.6%  │  MEM  77.9% █████████░░░  SWP  81.9%  │  / 97.9% full  │  en0 ↓32.9K/s ↑194.5K/s  │  UP 11d 0h 40m  PROCS 737
 10 cores ▃▂▂▂ ▁▁▃▃ ▂▂
```

The header answers "which resource" before you look at a single process.
Four things worth knowing about it:

- **Figures appear when they have something to say.** `CLK 62%` means the
  kernel is holding the clock below nominal — a processor flat out and
  getting two thirds of the work done, which nothing else here can
  distinguish from a healthy busy machine. `STALL`, `WAIT`, `BLOCKED` and
  `steal` work the same way. A quiet header is a machine with nothing to
  report, not a poptop that is not looking.
- **`CPU 99%` with ten cores at `████ ████ ██`** is ten busy cores. `CPU
  99%` with `█▁▁▁ ▁▁▁▁ ▁▁` is one core busy and nine idle, which is a
  single-threaded bottleneck, and the per-core strip is the only thing on
  screen that separates the two.
- **A dash is not a zero.** `steal — not published here` means the figure
  does not exist on this machine, and poptop will not invent a `0.0%` that
  you would then reason from.
- **`/ 97.9% full`** is the fullest filesystem, not necessarily root.

## 2. Let the panel title tell you what is actually scarce

```
── processes (748) — sort: CPU · memory is the constraint (S) ! io: panel too narrow ──
```

`memory is the constraint (S)` is poptop having looked at the machine and
concluded that CPU is not the thing stopping work. Press `S` and the table
sorts by what it named. It is a suggestion and never applied by itself: a
table that reorders under the reader is worse than one that does not.

When nothing is obviously scarce the title says nothing about it, and `s`
cycles the sort column by hand.

## 3. Narrow the table

`/` opens the filter. A bare word matches the command line:

```
/firefox
```

Or a query, which is the one worth learning:

```
/cpu > 5 and user = oddurs
```

```
── processes (6) — sort: CPU · 101 git (g folds them) ! io: 181/724 need root ──
   CPU%            RSS      S   THR    DISK R    DISK W HIST ≤25%      PID USER       COMMAND
   24.5 █         7.1M      S     2         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿   71595 oddurs     talagentd
   16.7 ▋        14.5M      R    12         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⣶⣶   86077 oddurs     rsst
    9.5 ▍       251.1M      S    63         0    4.0K/s ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣠   47182 oddurs     firefox
    7.1 ▎       465.0M ▏    S     1         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀   28117 oddurs     poptop
    6.6 ▎       112.6M      S    11         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀   38359 oddurs     ghostty
    6.2 ▎        22.2M      S    28         0         0 ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀     927 oddurs     UA Mixer Engine -silent
```

`(724)` became `(6)`, and the count in the title is how you know the filter
did what you meant. The [filter grammar](../reference/filter.md) has the
fields and operators.

## The columns that answer "why"

`HIST` is the one people miss. It is the same process's recent history, one
sparkline per row, on the scale named in the header — `HIST ≤25%` means the
tallest mark in that column is 25%.

```
   24.5 █         7.1M      S     2  ⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿   71595 talagentd
    7.1 ▎       465.0M ▏    S     1  ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀   28117 poptop
```

The first has been at this level for the whole window; the second only
started. One of those is a runaway and the other is a process doing its
job, and the instantaneous `CPU%` cannot tell them apart.

`·` in a column means the process has nothing to report there. `—` means
poptop could not read it. They are [different marks on
purpose](../reference/columns.md#marks).

## Switch to the view that matches the resource

`v` cycles CPU → memory → disk. Each replaces the columns with the ones
that answer a different question.

**Memory (`v` once).** `PSS` is the honest per-process figure where it can
be read, `VSZ` what has been reserved, `MAJF/s` major faults per second —
the process going to disk for pages — and `GROW` the change since the last
sample.

```
── processes (737) — memory view, sort: CPU · 102 git (g folds them) ──
   CPU%            RSS      S       PSS      VSZ  MAJF/s     GROW HIST ≤50%      PID USER       COMMAND
   24.6 █       110.6M      S         —   416.4G       0   +32.0K ⠀⠀⠀⠀⠀⠀⠀⢠⣠⣤   38359 oddurs     ghostty
   10.0 ▍       407.9M ▏    S         —   420.5G       3   +16.2M ⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀   52641 oddurs     node --experimental-modules main
    7.2 ▎        40.0M      S         —   415.6G       0        · ⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀   55321 oddurs     herdr server
```

`MAJF/s 3` with `GROW +16.2M` is a process growing and paying for it.
`GROW ·` is a process that is not growing at all. `PSS —` is macOS, which
does not publish it; on Linux those figures are there.

**Disk (`v` twice).** `DISK R` and `DISK W` at full width, whatever the
terminal.

```
── processes (732) — disk view, sort: CPU · 101 git (g folds them) ! io: 183/732 need root ──
   CPU%      RSS S     DISK R    DISK W HIST ≤50%      PID USER       COMMAND
   25.2   110.7M S          0         0 ⠀⠀⠀⠀⠀⠀⠀⣄⣤⣤   38359 oddurs     ghostty
```

`! io: 183/732 need root` is the honest part: 183 of those processes belong
to other users and their IO counters are not readable without privilege.
Those rows show `—`, not `0`.

## When the answer is "all of them"

Forty workers each at 3% is not visible one row at a time. `g` folds
processes sharing a name into one row — see [groups and
trees](groups.md) — and the panel title volunteers it when it is worth
doing:

```
── processes (724) — sort: CPU · 101 git (g folds them) ──
```

101 `git` processes is a fact about the machine that no per-process sort
will surface.

## Then go back in time

Everything above answers about the sample under the cursor. If the problem
already happened, the same keys work four minutes ago —
[scrubbing](scrubbing.md).
