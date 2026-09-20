# Groups and trees

Forty workers at 3% each do not appear in a table sorted by CPU. They are
120% of a core and they are invisible. `g` and `t` are the two ways of
making them visible, and they answer different questions.

## `g` — fold rows that share something

`g` cycles: by name → by user → by container → off.

poptop volunteers it in the panel title when it would tell you something:

```
── processes (724) — sort: CPU · 101 git (g folds them) ! io: 181/724 need root ──
```

101 processes called `git` is a fact about the machine no per-process sort
will ever surface.

### By name

```
── processes (730) — sort: CPU · grouped by name ──
   CPU%            RSS      S   THR HIST ≤100%     PID USER       COMMAND
    0.7         397.1M ▏    —  1212               ×101 oddurs     git
    0.7          43.4M      —     9                 ×7 oddurs     python3
    0.5         125.0M      —   273                ×21 oddurs     railway
    0.5          12.9M      —     —                 ×2 —          cloudd
    0.5           8.5M      —     6                 ×6 oddurs     bash
```

- **The PID column becomes a count.** `×101` is how many were folded.
- **CPU, memory and threads are summed.** 1212 threads across 101 `git`
  processes.
- **State, history and the command line are not.** A group has no single
  one of those, and says so with `—` rather than picking one arbitrarily.
- **The user column is summed only when it agrees.** `cloudd ×2` shows `—`
  for user because its two members do not share one.

**The summed memory is an upper bound.** Forked workers share an
interpreter heap copy-on-write, and RSS counts it once per member. The
`PSS` column in the memory view (`v`) is the measurement that sums
correctly; where the platform publishes it, use that.

### By user and by container

```
── processes (564) — sort: CPU · grouped by user ! io: 167/730 need root ──
   CPU%            RSS      S   THR    DISK R    DISK W HIST ≤100%     PID COMMAND
  365.1 ████+     9.7G ██▍   —  5099   24.0M/s  104.0K/s               oddurs
    4.5 ▏        20.4M      —     5         —         —                root
```

Two rows, and the answer to "which login is doing this" in one key. Note
the `USER` column is gone: when the rows *are* users, repeating it in a
column would spend width to say nothing, and that width goes to the disk
columns instead.

Grouping by container needs containers. On a machine with none you get an
empty table — [that is a known rough edge](../reference/views.md#modes).

### What `g` does not do

It is not available in the tree. Press `t` and `g` stops folding: a tree of
folded rows has no single parent to hang them from.

## `t` — the tree

Where `g` answers "how much in total", `t` answers "whose".

```
── processes (734) — sort: CPU · tree ──
   CPU%            RSS      S   THR HIST ≤50%      PID USER       COMMAND
      —              —      ?     —              1 ?          launchd
   15.0 ▋       110.7M      R    11 ⣄⣤⣤⣄      38359 oddurs     ├─ ghostty
    6.4 ▎        40.1M      S   105 ⣀⣀⣀⣀      55321 oddurs     ├─ herdr server
    0.0         560.0K      S     1            73900 oddurs        ├─ -fish
    0.1           7.5M      S     5 ⣀⣀⣀⣀      73993 oddurs        │  └─ harrow
```

Sorting still applies, within each level. A parent poptop cannot read is
still drawn — as dashes — so that its children are not orphaned off the
table.

## Which to reach for

| Question | Key |
|---|---|
| "Is it one runaway or forty small ones?" | `g` |
| "How much is this whole service using?" | `g` |
| "Which login is doing this?" | `g`, twice |
| "What started this?" | `t` |
| "Are these the same job's workers?" | `t` |
| "Is this container the problem?" | `g`, three times, or `C` |

## `C` — cgroups

On Linux with cgroup v2, `C` replaces the process table with cgroups: what
each is using, and how stalled it is (PSI).

```
── cgroups — not collected here ──
  CPU%  MAX%      MEM     READ    WRITE  PSI CPU  PSI IO  PSI MEM  PROCS CGROUP
```

That is what it looks like on macOS, which has no cgroups. The title says
so rather than leaving you to guess from an empty table. See
[platforms](../reference/platforms.md).
