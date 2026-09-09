# poptop

A system monitor you can rewind, with nothing to set up first.

poptop keeps every sample it takes — including the full process table — so you can
scrub backwards and ask what was eating the box forty seconds ago. It starts
with an empty buffer and fills it as it runs: no daemon, no config, no logfiles,
nothing that had to be running before you noticed the problem.

```
 poptop — PAUSED  -18s · warn 50 · crit 80
CPU  89.2%   WAIT  26.7%   RUN 1/4   BLOCKED 0   MEM  37.5% █████▒▒░░░░░
  4 cores ▇▄▁█
── timeline — 4m59s of 9m59s buffered ────────────────────────────────────────
 100 ⣴⠀⢰⡄⢠⡆⠤⣦⠤⣴⠤⢰⡄⢠⡆⠀⣦⠀⣴⠀⢰⡄⢠⡆⠤⣦⠤⣴⠤⢰⡄⢠⡆⠀⣆⠀⣴⠀⣰⡀⢠⡆⢀⣆⠤⣴⠤⣰⡀⢰⡆⢀⣆⠀⣶⠀⣰⡀⢰⡆⢀⣆⠤⣶⠤⣰⡀⢰⡆⢀⣆⠀⣶
 CPU ⣿⡇⣾⣇⢸⣿⢰⣿⡀⣿⡇⣾⣇⢸⣷⢰⣿⠀⣿⡆⣾⡇⣸⣷⢸⣿⢀⣿⡆⣿⡇⣸⣷⢸⣿⢀⣿⡆⣿⡇⣸⣧⢸⣿⢀⣿⡄⣿⡇⣸⣧⢸⣿⢀⣿⡄⣿⡇⣼⣧⢸⣿⢠⣿⡄⣿⡇⣼⣧⢸⣿⢠⣿
   0 ⣿⣇⣿⣿⣾⣿⣸⣿⣿⣿⣇⣿⣿⣿⣿⣸⣿⣿⣿⣇⣿⣿⣿⣿⣸⣿⣿⣿⣇⣿⣷⣿⣿⣸⣿⣾⣿⣇⣿⣷⣿⣿⣸⣿⣾⣿⣇⣿⣷⣿⣿⣸⣿⣼⣿⣧⣿⣧⣿⣿⣼⣿⣼⣿⣧⣿⣧⣿⣿⣼⣿⣼⣿
 100 ⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤⠀⠤
WAIT ⣤⣶⣦⣤⣄⣀⠤⠀⠤⠀⠤⠀⣀⣠⣤⣴⣶⣤⣄⣀⡀⠀⠤⠀⠤⠀⣀⣠⣤⣴⣶⣤⣤⣀⡀⠀⠤⠀⠤⠀⢀⣀⣤⣤⣶⣦⣤⣄⣀⠀⠤⠀⠤⠀⢀⣀⣤⣤⣶⣦⣤⣄⣀⠀⠤⠀⠤⠀⠤⣀⣠⣤⣴
   0 ⣿⣿⣿⣿⣿⣿⣷⣤⣀⣀⣴⣾⣿⣿⣿⣿⣿⣿⣿⣿⣷⣦⣀⣀⣠⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣦⣄⣀⣠⣴⣿⣿⣿⣿⣿⣿⣿⣿⣿⣶⣄⣀⣀⣴⣾⣿⣿⣿⣿⣿⣿⣿⣿⣷⣦⣀⣀⣤⣾⣿⣿⣿⣿
                                              CPU 89.2%  WAIT 26.7% ▐
2m25s shown, 1s/slot — ←/→ scrub, +/- zoom
── processes (4) · all root — sort: CPU ! io: panel too narrow ───────────────
  CPU%            RSS      S   THR HIST ≤100%     PID COMMAND
  88.4 ███▌    512.0M ▏    S     1 ⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿     824 postgres
  12.5 ▌        32.0M      S     1 ⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀    1190 nginx
   4.2 ▏       148.0M      S     1 ⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀    2077 node
   0.1          12.0M      S     1 ⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀       1 systemd

q quit · ←/→ scrub · b jump · +/- zoom · Space live · ↑/↓ select · s sort
```

The gutter names each graph and anchors its scale; the dashed lines are the
warn and critical thresholds, drawn so the boundary is readable without relying
on colour. Bars absorb the rule where they cross it. The `▐` marks which sample
the cursor is on, down to which half of a braille cell.

The process table below the timeline is the real one from the moment under the
cursor, not an interpolation. Sampling continues while you are scrubbing.

## Prior art, and where poptop actually differs

poptop is not the first tool to let you look backwards, and it is not the most
capable one.

**[atop](https://www.atoptool.nl/)** has recorded historical per-process data
for years. It writes compressed daily logfiles and keeps 28 days by default,
which poptop does not. It also captures processes that started *and finished*
between two samples — and so, now, does poptop: see **Processes that came and
went** below. On logging and retention atop is still the better tool.

**[zenith](https://github.com/bvaisvil/zenith)** has zoomable scroll-back charts
and saves data between runs. Its scrollback is aggregate-only, though: its
history holds CPU, memory, network, disk and GPU series, and its process table
renders from a live map that drops pids as they exit, so scrolling back moves
the charts but not the table.

**htop, btop and bottom** keep no history at all. They render the current
instant.

What poptop offers is narrower than "nobody else does this", and it is a usability
claim rather than a capability one:

- **Nothing has to have been running.** atop can only replay what its daemon
  already recorded. The common case — you connect to a machine that is slow
  *now* — is the case where that daemon was not running. poptop gives you the last
  ten minutes from a cold start.
- **One view, live and historical.** Scrubbing happens inside the running
  monitor, not in a separate replay mode against a logfile.
- **The process table follows the cursor.** Scrub to a spike and the table below
  it is the one from that instant.

If you are running a fleet and want history you can rely on after the fact,
install atop. If you want to know what this box is doing right now and what it
was doing a few minutes ago, that is what poptop is for.

|                                        | poptop | htop | btop | bottom | zenith | atop |
| -------------------------------------- | :--: | :--: | :--: | :----: | :----: | :--: |
| Live view                              |  ●   |  ●   |  ●   |   ●    |   ●    |  ●   |
| Rolling graph of recent values         |  ●   |  ◐¹  |  ●   |   ●    |   ●    |  ○   |
| Move backwards through time            |  ●   |  ○   |  ○   |   ◐²   |   ●    |  ●³  |
| Process table follows the time cursor  |  ●   |  ○   |  ○   |   ○    |   ○⁴   |  ●   |
| Captures processes that exited between samples | ○ | ○ | ○ |   ○    |   ○    |  ●   |
| History survives a restart             |  ○   |  ○   |  ○   |   ○    |   ●    |  ●   |
| Needs something running beforehand     |  ○   |  ○   |  ○   |   ○    |   ○    |  ●⁵  |

● yes · ◐ partial · ○ no

1. htop's Graph meter mode (`GRAPH_METERMODE`, `Meter.c`) keeps a rolling
   scalar buffer sized to the meter width. It is a graph, not navigable
   history, and it is per-meter rather than per-process.
2. bottom can freeze the display (`f`), but freezing gates the update
   (`if !app.data_store.is_frozen()`, `lib.rs`) rather than letting you look
   backwards. poptop keeps sampling while you scrub.
3. atop steps through intervals when replaying a logfile (`atop -r`), which is
   a separate mode rather than the live view.
4. zenith's `HistogramKind` holds only aggregate series; its process table
   renders from a live map that runs
   `.retain(|&k, _| current_pids.contains(&k))`.
5. atop's history requires its daemon to have been recording in advance. This
   row is the whole of poptop's argument.

Every cell above was checked against the tool's source or official
documentation rather than from memory; the footnotes name where.

## Build

```sh
cargo build --release
./target/release/poptop
```

## Keys

| Key | Action |
| --- | --- |
| `q` | quit |
| `←` / `→` | scrub through history (hold `Shift` for ten at a time) |
| `b` | jump to a moment: `-2h`, `03:00`, `2026-09-08 03:00` (atop's `-b`) |
| `+` / `-` | zoom the timeline in and out |
| `Space` | pause on the current sample, or resume live |
| `Home` / `End` | jump to oldest / live |
| `↑` / `↓` | select a process |
| `s` | cycle sort column |
| `S` | sort by whatever is stopping work, when the panel names one |
| `v` | switch column set: generic, memory, disk |
| `t` | toggle the process tree |
| `g` | fold rows: by name, by user, by container, then off |
| `d` | show the selected process's own history instead of the machine's |
| `K` | show kernel threads, which are hidden by default on Linux |
| `i` | toggle per-process disk IO columns |
| `y` | expand the selected process into its threads |
| `C` | show cgroups instead of processes |
| `/` | filter — a substring, or a small query language |

`d` is the one worth trying first. The buffer already holds every retained
sample's whole process table, so "what has *this* process been doing" is a
question the data can answer — it was a ten-column sparkline in a table row, and
it is now the whole timeline panel: CPU, memory, threads and disk over the
window, with the moments the process was **not running** marked rather than
interpolated. When it started and when it went is often the entire answer.

`S` is offered, never applied. The panel names the resource that is stopping
work — `disk is the constraint (S)` — and that key is the only thing that acts
on it, because a table that reorders itself under the reader is worse than one
that does not. It reads the constraint **at the cursor**, so scrubbing back to a
spike tells you what was in the way *then*.

`poptop --once` prints a single plain-text sample and exits, for scripts and cron.
`poptop --bench` times 20 collection passes, for checking the cost of a change.

`--color=auto|mono|16|256|true` picks the colour tier. Detected from
`COLORTERM` and `TERM`, and `NO_COLOR` is honoured. Each tier stands on its own:
monochrome is not a fallback but the proof that no meaning here is carried by
colour alone — thresholds, severity, selection and the paused state all survive
without it.

`--theme=safe|classic|auto` picks the palette. **`safe` is the default and
replaces green with cyan.** Green-and-yellow is the worst available pair for
red-green colour vision deficiency, which affects roughly 8% of men, and every
system monitor ships it: measured in OKLab ΔE×100 under Machado 2009 simulation
against a dark surface, poptop's old green↔yellow separated by **3.7** under
protanopia, against a target of 8. The shipped palette's worst pair among its
five meaning-bearing hues is **10.3**. `--theme=classic` restores
green/yellow/red for anyone who wants the convention back.

Both figures are **enforced in CI**, not asserted in prose: `src/cvd.rs`
implements the same Machado 2009 simulation and OKLab conversion the palette was
measured with, and a test fails the build if any pair among the five
meaning-bearing hues drops below ΔE 8. A second test enforces the contrast
floor, because separation between hues says nothing about whether a hue is
visible at all.

Both have already earned their place. The first caught the 256-colour tier
shipping `series_cpu` and `series_mem` only 6.7 apart, because rounding each
channel to its nearest cube level independently is not perceptually safe. The
replacement then failed the second, sitting at 2.03:1 against the selected-row
background — separated, and invisible.

### Filtering

A bare word is a substring match on the name, the command line, the user or the
pid, which is what `/` has always done. It is also a small query language, for
the questions a substring cannot ask — and each of these is a figure the header
is already showing you and the table had no way to itemise:

```text
state = D              stuck in uninterruptible sleep, which BLOCKED counts
task = D               the process *holding* the blocked thread
container = 9a1f       everything in one container
container = none       everything in none of them
write > 1mb            who is causing the disk saturation just reported
threads > 100          the thing leaking threads
cpu > 5 and user = root
```

Fields: `cpu`, `mem` (`rss`), `threads` (`thr`), `state`, `task`, `container`
(`cid`), `pid`, `read`, `write`, `user`, `name` (`command`). Operators `>` `>=` `<` `<=` `=` `!=`,
joined by `and`. Sizes take `k`/`m`/`g`/`t` and are binary, like the column they
filter: `1mb` is 1048576.

Deliberately smaller than the tool it is borrowed from: no `or`, no negation, no
parentheses, no regex. `field op value` joined by `and` answers every question
above, and the rest is a precedence table and a syntax to document.

A figure the platform could not read matches **nothing** — not `> 0`, and not
`< 1mb` either. A process whose IO could not be read is not one doing no IO, so
it must not answer a question about its IO in either direction; that is the same
refusal the `—` in the column is making. And a malformed query filters nothing
away and says what is wrong, naming every field, because on a one-line filter
box the error message is the only place discovery can happen.

It is evaluated **at the cursor**, so scrubbing answers *what was in D-state at
the moment of the spike* — which is a question no live-only monitor can be
asked.

### Threads

`y` expands the selected process into its threads: tid, the thread's own name,
its state and its share of CPU. The columns a thread has no answer of its own
for — memory, disk, thread count — are em dashes rather than the process's
values, because forty rows each repeating one 900 MB figure would imply forty
copies of it.

The selected process only. A box has roughly eight times as many threads as
processes, and a table that grew ninefold on a keypress would answer *which
thread is spinning* by making the spinning one harder to find. Not in the tree
or while grouped: both order rows by something other than "this process, then
its threads", and the panel says so rather than letting the key do nothing.

**It costs, so it is asked for.** Reading `/proc/<pid>/task/<tid>/stat` for
every thread of every multi-threaded process measured at **3.1 µs per thread** —
1005 threads took a 534 µs sample to 3.63 ms — and each retained thread is 17
bytes of every buffered sample, against 65 for a process.

Nothing is collected until you press `y`. Turning the view back off keeps
collecting for another minute, so scrubbing back over what you were just looking
at still has threads in it, and then it stops. The IO ratchet never lets go and
that is right for it — one extra read per process — but holding this for the
rest of a session because somebody once pressed a key is the worse bargain.

An expansion showing nothing always says why: `threads: from the next sample` at
the live edge, `threads: not collected this far back` when you have scrubbed
past it. A process with no thread rows and no explanation would be
indistinguishable from a single-threaded one.

`task = D` is the other half. `BLOCKED` in the header counts *tasks*, so two
blocked threads inside one healthy-looking process are a number the process
table could not otherwise account for — this is how you get from the figure to
the row, and then `y` to the thread. It finds single-threaded processes too,
which are not collected but whose one thread's state *is* the process's; they
are the commonest contributor to that figure, and a predicate that could not
find them would be answering the wrong question. With no threads collected it
matches nothing in **either** direction — `task != D` must not claim every
process was inspected.

Not on macOS. sysinfo, the backend there, exposes no per-thread accounting;
mach's `task_threads` would, and poptop does not call it yet. The panel says
`threads: not read on macOS` rather than showing one thread per process.

### What happened to my process

A process the OOM killer ended is gone from the next sample with nothing
anywhere saying why. That is the commonest question a monitor is asked, and
poptop could not answer it even with the buffer.

The panel now says `2 processes killed for memory` in the interval it happened,
and **scrubbing back to that moment shows the count beside the process table
from the instant before** — which is a thing no live-only monitor can do, and
which atop can do only from a logfile its daemon had to be writing in advance.

`--once` reports page-in, page-out, swap-in and swap-out as **rates**. The swap
*level* cannot tell a machine that swapped four gigabytes in and out during the
interval from one sitting on four idle gigabytes — they report the same number.
The constraint detector reads that rate now, where before it had to infer memory
pressure from swap *growth* across a window precisely because the level said
nothing. The inference is still there for platforms that publish no rate.

Two units, one file: `pgpgin`/`pgpgout` are in kilobytes and the swap pair is in
*pages*, so one conversion applied to both reports swap at a four-thousandth of
its size — a thrashing box rendered as a quiet one.

### What the memory is actually holding

`MEM 50%` and a total say nothing about the shape of the other half. `--once`
now partitions it: **dirty** pages awaiting writeback, **slab** and the part of
it the kernel can reclaim, **shmem**, **page tables**, and **huge pages**
reserved against used.

The one that earns header space is **`DIRTY`**, and only above 5% of memory. A
box with a fifth of its memory awaiting writeback is about to stall on IO and
every other figure looks fine until it does — while a few megabytes is what an
ordinary machine carries all the time, and a figure that is always there is one
nobody reads.

Coloured on its own scale, like `STL` and the stall figures: a tenth of memory
dirty is serious, and the default warn of 50% would never fire before the
machine had already stalled.

**Why these and not `used`:** `used` is `total - available`, so it already
contains every one of them. A kernel memory leak shows up there as used memory
belonging to no process — precisely the case where the process table cannot
explain the header — and this is where it becomes visible.

An absent line is `—`, not zero. A kernel built without hugetlb has no huge
pages to report; one with none reserved has zero of them, and those are
different answers. `/proc/meminfo` was already being read, so this costs nothing.

### Whether the box is busy, or not being given a box

`/proc/stat`'s CPU line has ten fields and poptop was reading four of them. The
rest were one parse away in a file it reads every sample:

**`STL`** — time the hypervisor took for something else. On a cloud instance
this is the difference between *the box is busy* and *the box is not being given
a box*: every other figure on the header looks healthy while the machine gets
less done, and nothing else can say so. It appears **only when it is over 1%** —
on bare metal it is zero forever, and a figure present on every frame is one
nobody reads by the second day. It sits next to `CLK` because both *qualify*
CPU rather than adding to it.

**`guest`, `irq`, `softirq`** — reported by `--once`. A network-heavy box's
softirq share is the answer to why user time looks low while nothing is idle,
and on a hypervisor guest time is the work rather than the overhead.

**Context switches and interrupts a second** — a box thrashing between threads
looks identical to a busy one without them. Rates, not the since-boot counters
`/proc` publishes, and **absent on the first sample** rather than reporting a
boot's worth of switches as one second's.

None of this costs anything: the file was already being read and parsed, and
`--bench` is unchanged. `guest` is deliberately outside the busy total — the
kernel counts it inside `user` already, and adding it would double it.

### What a process is costing you

atop shows around seventy fields across its process views. The memory view (`v`)
carries the ones that answer a question poptop could not:

```text
  CPU%      RSS  S      PSS      VSZ  MAJF/s     GROW      PID COMMAND
  12.0   900.0M  S   300.0M     4.0G    1200  +40.0M     4001 chrome
```

**`PSS` is the honest answer to "how much memory is this costing".** A shared
page is divided among the processes sharing it, so summing PSS across six Chrome
renderers gives a real total where summing RSS counts their shared pages six
times. That is the caveat the grouped RSS carries — `g` says its total is an
upper bound — and this is the measurement that isn't.

It needs `smaps_rollup`, one extra read per process, **measured at 4.2 µs each**
— 0.85 ms a sample becomes 1.77 ms at 218 processes. So it is read only while
the memory view is open, and atop gates its own behind a key for the same
reason.

**`MAJF/s` is the one that answers "why is this slow".** A process taking major
faults is being paged in from disk while its CPU looks low and its state looks
ordinary. It is a rate over the interval, not the lifetime total `/proc`
publishes, and a process on its first sighting reads zero rather than its whole
life divided by one second.

**`GROW` is derived, not stored.** A growth figure in every retained sample is a
field carried forever to describe one interval, and two adjacent samples already
imply it. It is an em dash across a **seam** — a laptop that slept leaves two
samples twenty minutes apart sitting next to each other, and "grew 400 MB since
the last sample" is a rate that has no interval to be scaled by. It uses the
timeline's own definition of adjacent, so the graph cannot draw a seam where the
column shows a number.

The cost of carrying these: a retained process goes from 70 to 103 bytes, and
the store for 600 samples of 400 processes from 15.8 MB to 24.9 MB. The window
is unchanged — the cap is 64 MB — but it is the largest single increase any of
these fields has cost, and every optional pays its tag byte whether or not the
platform answers it.

Not on macOS: sysinfo publishes no PSS, virtual size or fault counts, so those
three are em dashes rather than zeros. `GROW` works there — it is derived from
RSS, which sysinfo does publish.

### What the table shows

atop spends seven keys on this — `g` generic, `m` memory, `d` disk, `n` network
— each a different column set over the same rows. poptop's table is already at
its width on an eighty-column terminal, so more fields cannot mean more columns.

`v` cycles **generic → memory → disk**. It is a named list of columns over one
renderer, not a second renderer.

The concrete thing it fixes today: `DISK R` and `DISK W` are shown only when
there is room, so on a narrow terminal the figures vanish with nothing to bring
them back. In the disk view they are the point, so they are exempt from that
width test — and the view makes room by dropping the bars and the thread count
rather than by pushing the command off the edge.

**Sort and view cannot disagree.** `s` cycles within the columns the current
view shows, and switching views brings the sort with it when it has to. atop
allows sorting by a column the view does not show, which is an ordering with no
visible reason for it. The panel names both — `memory view, sort: MEM`.

**Two axes, not four.** Tree, grouping, thread expansion and views looked like
four exclusive modes on four keys, which is where interfaces go wrong. They are:

- **what the table is *of*** — processes flat, as a tree, folded by name or by
  container, with one expanded to its threads, or cgroups instead (`t`, `g`,
  `y`, `C`)
- **what it *shows*** — the column set (`v`)

`d` is neither. It changes the *timeline* panel rather than the table, and
counting it as a table mode is what made this look like four axes.

### Which user is eating the machine

On a shared box that is the first question, and the `USER` column cannot answer
it: it is folded away precisely when it is constant, and it is one column of many
rows when it is not. `g` folds by user, and the row *is* the user — CPU, memory
and threads summed across everything they are running.

The identity inverts with the key. Grouping by user makes the username the row's
name, so the `USER` column is dropped: drawing it beside the identity is the
same word twice. That is the opposite of the usual rule, which drops the column
when every row shares a value.

The refusals do not change. A group of processes in three different states has
no state, and says `—`; a group whose IO could not be read for one member has no
total. Selection follows a group by its key, so a user whose process list turns
over completely is still the row that was selected.

### Whose process is it

On a Kubernetes node, "which process is eating the box" has a second half. A
hundred processes named `node` and no way to tell which pod any of them belongs
to is not an answer.

Every process carries a `CID` — twelve characters of its container id, from
`/proc/<pid>/cgroup`, which is what `docker ps` shows and what atop falls back
to. The pod *name* is not in that path at all: atop reads it from the runtime,
with superuser. An id is what poptop can know without asking anybody's
permission.

`g` folds rows, and now cycles: **by name, by user, by container, then off** —
atop's `p`, `u` and `j`, on one key.
Folding by container is the same machinery with a different key, so it is a
choice rather than a fifth exclusive layout for a table that already has four.
Processes in no container are not folded into a heap called "none" — grouping by
container asks what each container is doing, and a bucket holding everything
else answers a different question loudly.

`container = 9a1f` matches on a prefix, because the id shown is twelve
characters of sixty-four and nobody is going to type the rest.
`container = none` is the other question, and one an empty string could not ask.

The column is dropped entirely on a box running no containers, where it would be
twelve columns of nothing — the same rule as the user column. **A process in no
container shows a blank, not an em dash:** the dash means poptop could not tell,
and here it can.

The path differs per runtime *and* per cgroup version, and no machine runs
docker, podman, containerd and plain systemd at once — so the parse is a pure
function with a fixture for each, including a namespaced `0::/../..` observed
from inside a container, which carries no id at all.

### Which cgroup is stalled

poptop reports PSI for the machine. On a container host that answers *something
is stalled on IO* and not *the thing stalled on IO is this pod*, which is the
question. `C` shows cgroups instead of processes — CPU, its quota, memory, IO
and **pressure per cgroup**, sorted with the most pressured first, because the
reason to open it is to find what is stalled.

```text
── cgroups (14) — depth 4, sorted by pressure ──────────────────────
  CPU%  MAX%      MEM     READ    WRITE  PSI CPU  PSI IO  PSI MEM  PROCS CGROUP
   1.0   200    64.0M    1.2M       0B      0.0    61.5      0.0      3     pod-b.slice
   4.0     ∞    64.0M       0B       0B      0.0     0.1      0.0      3     pod-a.slice
```

Per-cgroup pressure is the single most useful thing atop has that poptop did
not, because it is the only metric that *attributes* a stall.

**CPU and memory are subtree totals**, because that is what cgroup v2 publishes:
a parent reads higher than any one child rather than equal to the sum of the
rows beneath it. The rollup is the kernel's arithmetic, not poptop's — which is
the only version that can be right about a cgroup holding both processes and
children.

**Bounded, and it says so.** Four levels below the root, which reaches a
container on a Kubernetes node, and at most 512 nodes a sample. A tree bigger
than that reads `cgroups (first 512 of more)` rather than reporting the count as
if it were the whole hierarchy — a reader hunting a stalled cgroup must not be
handed a list that silently does not contain it.

**Only while the view is open.** Six files a node, and a thousand-cgroup tree
measured at **7.3 ms a sample** against 150 µs without it. That is the most
expensive thing poptop reads by a factor of fifty, so it is not collected for a
panel nobody is looking at, and it is the first thing the budget takes away.

A `—` in a pressure column is a node whose `cgroup.pressure` is switched off,
not a node that never stalls. cgroup v1 has no unified tree and no PSI, so
poptop says so rather than showing an empty table.

### What it costs to watch

Everything optional declares what it costs and how often it is worth reading —
per-process disk IO at ~5.7 µs a process, threads at ~3.1 µs a thread, the
cpufreq policy set as one directory walk a minute. A single boolean could not
express "read this every tenth sample" or "stop reading this, it costs more than
the interval", and the subsystems still to come differ in cost by two orders of
magnitude.

Two things decide. The panel that needs a source asks for it, and a **budget**
gives things up when sampling outgrows its share of the interval — a quarter,
past which the tool is a meaningful part of the load it is measuring, on the box
that is already in trouble. One source at a time, most expensive first, and only
after three consecutive over-budget samples, because one slow sample is a page
fault rather than a verdict.

"Most expensive" is counted on *your* machine, not per unit. IO is dearer per
process than a thread is per thread, but a box with 400 processes has some 3200
threads — so threads are about 9.9 ms against IO's 2.3 ms, and giving up IO
first would cost you the columns and leave the sample just as slow.

The objection to a budget is that it can silently drop a figure. So it never
does: `per-process disk IO withheld, sampling was over budget` sits in the panel
title until you ask for that source again by name, which clears it. If it goes
over budget again it will be given up again — that is poptop telling you the
machine cannot afford it at this interval, and `--interval` is what acts on it.

Only sources whose cost **grows with the machine** are candidates, and only
when they are big enough to be the reason. A directory walk once a minute cannot
be why a sample ran long. Neither can the IO columns, if the process table walk
alone is over budget — so poptop says `sampling takes longer than a quarter of
the interval` and leaves your columns alone, rather than dismantling itself one
at a time chasing a target it cannot reach. `--interval` is what acts on that.

**NFS is not one of the candidates, and that is a decision rather than an
oversight.** Its cost is proportional to the number of NFS mounts —
`/proc/self/mountstats` is 5.3 µs a sample on a machine with one, measured, and
about 5.4 µs for each one after that, so a hundred mounts is half a
millisecond. That is real, and it is spent on a machine whose entire purpose is
those mounts: giving the figure up there would give up the one thing worth
watching to save time on a box that is waiting for the network anyway. The
per-op statistics are scanned rather than collected for the same reason —
NFSv4.2 writes seventy-odd lines a mount and three numbers are wanted, and a
vector per line was most of the cost.

`--glyphs=braille|block|ascii` picks how the timeline is drawn. Braille packs
two samples into every character cell and stacks cells vertically for twelve
distinct heights; `block` needs less font support; `ascii` needs none. A Linux
console (`TERM=linux`) selects `ascii` automatically.

Zooming aggregates samples into slots by **peak, never mean** — averaging a
100% spike with three idle samples would render 25% and hide the exact event
the tool exists to catch. Zoom is clamped to what the buffer can fill, so
zooming out never shrinks the graph into a corner; the empty region on the left
at full zoom is real time from before the buffer starts.

A laptop that sleeps, or a box loaded enough to miss its tick, leaves samples
minutes apart. Drawn as adjacent cells those claim to be one second apart, and
the x-axis quietly stops meaning anything. poptop draws a `┊` seam wherever at least
one interval went unobserved, full height and in chrome so it cannot be read as
a bar:

```text
CPU ⠀⠀⠀⠀⠀⠀⠀⣤⣤⣤⣤⣤⣤⣤⣤⣤⣤⣤⣤⣤┊⣤⣤⣤⣤⣤⣤⣤⣤⣤⣤⣤
  0 ⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿┊⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
1m20s shown, 1s/slot, ┊ time missing — ←/→ scrub, +/- zoom
```

A gap is *a missing sample*, not a slow one, so the line falls at twice the
nominal interval and needs no tuning. Sampling runs on a fixed cadence rather
than one interval after the previous sample finished — otherwise collection and
draw time are added to every period, the timestamps drift away from the rate
they claim, and on a busy box the detector would eventually see a missed tick
on every cell. If the host genuinely cannot sustain the interval, the samples
really are more than one tick apart and the seams are telling you so. Gaps aggregate by **or** for
the same reason values aggregate by peak — zooming out must not be able to
erase an event, least of all at the zoom where the whole buffer is on screen.

## Reading the table

The per-process disk IO columns are **shown by default**, because the header
may have just told you the machine is blocked on IO and the table is where the
culprit is named. A default that hid them would make the default view unable to
answer the question the default view raised.

`/proc/<pid>/io` is mode 0400 and owned by the process owner, so reading other
users' processes needs `CAP_SYS_PTRACE`. Only a permission failure counts
towards that figure: a process that exits between the directory listing and the
read is `NotFound`, which is normal on any box with churn and is not something
root would fix. Kernel threads are excluded from that
accounting entirely — `kworker/*` and friends are root-owned and unreadable,
and on a many-core box they outnumber the real processes, so counting them
would withdraw the columns on exactly the laptop this protects. On a laptop almost every process is
yours; on a box running its services as root while you are not, the columns
would be a wall of em dashes. So poptop **probes** — it collects one real sample,
and if more than half of it came back unreadable it withdraws the columns and
stops collecting for them. That is a question nothing short of trying can
answer, and half a millisecond a sample is not worth paying for a column nobody
can read.

`i` still overrides whichever way the probe went. Someone with partial access
may well want the column for the processes they can see.

The sparkline column is drawn against **one axis shared by every row**, and its
own header names it — `HIST ≤800%`. A legend belongs with the thing it explains,
so the scale sits over the column rather than in the section title.
Scaling each row to its own peak instead would make a process oscillating
between 11% and 13% look exactly like one spiking to 90%, which defeats the only
reason to put the shapes in a column together.

The axis follows the busiest process in the whole buffer, so one using several
cores raises it for everyone: a virtual machine peaking at 500% takes the axis
to 800%, and rows under 20% then draw at the lowest lit level — visible, but
with their shape squashed out of them. That is the cost of a shared axis and it
is worth naming. The alternative was worse: the ladder used to stop at 100%, so
the virtual machine drew as a solid block and its history could not be read at
all — the one row you scrubbed back to look at. Absolute magnitude is still in
the `CPU%` column beside it; the sparkline is for shape.

The axis is taken from every process in the buffer rather than the rows on
screen, so scrolling does not rescale it. Drawn from the visible slice, bringing
a busy process into view would collapse every other row's history to the floor
and springing it back when that process scrolled off.

They also drop on a panel too narrow to carry them, like everything else here.
Every column in the table is a fixed width, so without that rule ratatui
squeezes them all rather than dropping any — an eighty-column terminal rendered
truncated figures under an `RSS` header reading `512.`. Collection is untouched
when they go: the columns are a rendering decision and the ratchet is a history
one, so widening the window brings them back with their history intact.

## Reading the header

```text
 LIVE CPU  33.0%  │  MEM  84.1% ██████████░░  20.7G / 24.0G  SWP  73.3%  │  / 86.4% full  │  en0 598B/s 887B/s  │  UP 4d 14h  PROCS 844  · warn 50 · crit 80
 14 cores ▄▃▂▂ ▅▅▄▃ ▄▃▂▃ ▂▂
```

Figures sit with the resource they are about — compute, memory, storage,
network, then the two facts that are neither symptom nor cause — and the rules
between the groups are wider than the gaps inside them. The order is the one the
question "why is this slow" walks through.

**Where a figure sits and when it is given up are separate decisions.** One
number used to do both, and it read compute, storage, compute, network, memory,
network, memory, machine: the network split in half with memory between the
halves, and `LOAD` — the most compute figure there is — after uptime. Rank still
governs what a narrow terminal drops; it no longer governs where anything sits.

`LIVE` and `PAUSED -12s` sit with the figures because they qualify them: paused
means *these numbers are twelve seconds old*. The marker is the one thing on the
row that is never dropped for room — reading a stale process table as the
current one is the single worst thing this tool could let you do.



```text
CPU  12.4%   WAIT  61.2%   RUN 1/14   BLOCKED 23   MEM  37.5%
```

That is a machine doing almost nothing while being unable to get on with
anything, and it is the case every troubleshooting guide names as the confusing
one: **high load, idle CPU.** Thirty processes blocked on one hung mount give a
load average of thirty on a box whose CPUs are completely idle.

- **WAIT** is the share of the interval the CPU spent idle *with I/O
  outstanding*. It is deliberately not counted in `CPU`, because the CPU
  genuinely had nothing to run — but leaving it at that would make poptop right
  about the CPU being quiet and silent about the reason.
- **RUN** is runnable tasks against cores. A bare count is not a fact anyone can
  act on: four is catastrophic on one core and idle on ninety-six.
- **MEM** is drawn as a composition, not just a level: used, reclaimable cache,
  and genuinely free. "37% used" reads identically on a box with eight
  gigabytes free and on one whose only headroom is page cache it is about to
  have to drop, and the second is the one worth knowing about. The segments
  separate by glyph density — `█` `▒` `░` — so the bar means the same thing on
  a monochrome terminal as everywhere else. On macOS it is two parts, not
  three: `used` and `available` there come from overlapping `vm_stat`
  quantities that routinely sum to more than the machine has, so there is no
  cache/free split to draw and inventing one would report "no free memory" on a
  perfectly healthy box.
- **BLOCKED** is tasks in uninterruptible sleep — the D-state count. There is no
  healthy amount of "stuck in the kernel", so any value at all is coloured.

`RUN` and `BLOCKED` are what a load average conflates into one number, reported
exactly rather than smoothed — and the smoothing a load average adds is what
the timeline is for. `LOAD` is still there, and is the first figure dropped
when the line is tight.

All three come from the `/proc/stat` read poptop already performs every sample,
so they cost nothing: 1.05 ms per sample at 402 processes, unchanged. macOS
publishes no equivalent and shows none of them, rather than a zero that would
claim the box is never stuck.

### Which half of the machine is full

A two-socket box is two machines that share a process table. Node 0 can be idle
with forty gigabytes free while node 1 is at ninety percent CPU and one
gigabyte from swapping, and every whole-machine figure on the header above
averages that into a comfortable middle: `CPU 50%`, `MEM 62%`, nothing wrong.
The processes pinned to node 1 are the ones stalling, and there is no figure on
a whole-machine header that can say so.

So the header grows a third row, but only on a machine that has more than one
node:

```text
  2 nodes n0   6.0% 48.0G free   n1  93.0% 1.0G free
```

Per node: the mean utilisation of the cores it owns, and the memory it has
left. Free rather than used, because on a NUMA box "free" is the number that
decides whether the next allocation stays local or goes across the
interconnect.

**The colour is not the number.** Page cache on a node is memory that comes
back, so the figure is heated on what is *not* reclaimable — free plus
`FilePages` against the node's total. Heating on free alone would paint a node
holding twenty gigabytes of cache critical while the `MEM` figure two rows up
read a comfortable third, which is the same misreading the whole-machine bar
draws three segments to avoid.

**Cores are matched by kernel id, not by position.** `/proc/stat` emits a line
only for an online CPU, so on a machine with `cpu0-7` offlined the first entry
of the per-core vector is `cpu8` — and a node's `cpulist` names absolute ids.
Indexing one into the other hands each node mostly another node's cores, which
is wrong in exactly the case this row exists for. A node with some cores parked
is the mean of the ones that are running; a node with all of them parked reads
`—`, because `0.0%` would say it was idle.

**A single-node machine spends nothing here.** The row is absent, not empty —
its per-node figures would be the two rows above restated, and a permanent row
saying so is a row the process table does not get. The same rule the throttling
and steal figures follow. macOS says nothing at all: it publishes no node
topology, and one invented from core counts would be a guess.

Nodes come from `/sys/devices/system/node`: one small `meminfo` per node each
sample, so the cost is bounded by socket count rather than by anything that
scales with the machine — which is why this is not behind the cost model that
gates threads, PSS and cgroups. Which nodes exist and which cores each owns is
read once and kept: a socket does not appear while poptop is running, and
re-walking the directory every second to learn that is the same cost the clock
policies are gated to a minute for.

A node whose `meminfo` cannot be read is dropped rather than reported with zero
of everything — a container that exposes the directory and restricts the files
under it would otherwise draw `0 B free` on every node, which is the loudest
thing this row can say and a claim rather than an absence. A machine where that
leaves fewer than two readable nodes reports none.

**The row belongs to the machine, not to the sample.** Once a machine has
reported nodes the row is reserved for every sample, and history recorded
before poptop read them says `nodes not recorded in this sample` rather than
leaving it blank. Derived per-sample it looked correct and scrubbed badly:
crossing the boundary moved the timeline and the whole process table up and
down by a row on every keypress, in a tool whose entire point is the rewind.

The row follows the same ladder as the rest of the header: too narrow for every
node and it drops them from the right with `+3`, so the count is never silently
wrong; too narrow for even one and it states `8 nodes` alone.

`--once` prints a line per node, and `nodes   —` on a machine with one.

The header drops its least diagnostic figures first rather than letting the
terminal clip whatever is rightmost — rightmost is not least useful. At sixty
columns you still get all four of the figures above. `--once` reports the same
three, because a script that reads only cpu and memory reads a stalled machine
as an idle one.

## Configuration

`~/.config/poptop/poptop.conf`, honouring `$XDG_CONFIG_HOME`. Every flag is a
`key = value` line without the leading dashes:

```ini
# ~/.config/poptop/poptop.conf
theme    = classic
glyphs   = block    # comments run to the end of the line
color    = 256
warn     = 65       # a build box is busy at 50% and perfectly fine
critical = 90
interval = 500ms    # every sample keeps a whole process table,
window   = 30m      # so these two together decide the memory
```

`warn` and `critical` are where the status colours change, where the timeline
draws its threshold rules, and what the header legend prints. All three read
the same pair, so the printed numbers cannot drift from the behaviour they
describe — that agreement is the entire value of printing them.

One rule governs the pair: **every status band must be reachable.** `heat`
reads `[0, warn)` as ok, `[warn, critical)` as warn and `[critical, 100]` as
critical, so `warn` above zero and `critical` above `warn` is exactly the
condition for none of the three to be empty. That rejects an inverted pair, an
equal one (the warn band would be empty), and `warn = 0` (nothing would ever be
ok, and a rule would sit permanently along the bottom of both graphs).
`critical = 100` is allowed: its band is the single point 100, but a machine
really does reach 100% memory, so the band is reachable rather than empty.

Out-of-range values are rejected rather than clamped — `warn = 150` is someone
who has misunderstood the units, and quietly turning it into 100 hides that
from them for as long as they use the tool. Fractions are fine, and the header
prints them at the precision you gave: `warn = 62.5` shows `warn 62.5`, not a
rounded `62` claiming the colour changes half a point from where it does.

Precedence, lowest first: built-in default, config file, `NO_COLOR`, flag. The
flag always wins, so a wrapper script can override a user's file without
editing it; `NO_COLOR` outranks the file because the file records a preference
in general and the environment is saying something about this terminal now.

**An unknown key warns and poptop starts anyway**, naming the key and the line —
and guessing what you meant:

```
poptop: ~/.config/poptop/poptop.conf:6: unknown key `colour` (did you mean `color`?)
```

A bad line in the file warns; a bad flag is fatal. The asymmetry is deliberate:
a config file is written once and read every run, so one typo must not cost you
the tool, but a flag was typed for *this* run and quietly ignoring it would do
something other than what was asked.

Hand-rolled `key = value`, not TOML. poptop's config surface is genuinely flat,
and `serde` + `toml` would be the largest dependency in the project by an order
of magnitude — in a codebase whose `/proc` parser is deliberately hand-rolled
with no dependencies at all. htop and btop both use `key = value` and neither
has outgrown it.

One table defines every setting once, and both the file and the command line
drive it, so `theme = classic` and `--theme=classic` cannot come to disagree
about what a value means.

### Keeping history across restarts

Off by default, and that is load-bearing rather than cautious:

```ini
store = on
```

poptop's whole position against atop is that **nothing has to have been running
beforehand** — you can install it during an incident and immediately scrub back
through the last ten minutes, because the buffer fills from the moment it
starts. A tool that needs a recorder primed in advance is a different tool, and
it is the one atop already is and does better. So this is a convenience for a
machine you sit in front of often, never the path that argument rests on. With
`store = off` — the default — poptop reads and writes nothing.

Written on a **clean exit** to `$XDG_STATE_HOME/poptop/history`, and read at
startup. Deliberately not a daemon and not a periodic flush: a background
writer is exactly the thing that turns a live tool into a recorder. The cost is
that `kill -9` loses the buffer, which is the right way round for a feature
that must not become load-bearing.

The format is hand-rolled and versioned, like the config parser and the `/proc`
parser. A store written by another version is **discarded, not migrated** — it
is a cache of something the machine will produce again within minutes, and a
migration path would cost more than it saves. So is a truncated or corrupt one:
every failure lands on an empty buffer, because the alternatives are refusing
to start and inventing history.

Names and users are written once into a string table and referenced by index —
the same reason they are `Arc<str>` in memory. A full store of 600 samples at
400 processes is **14 MB, and costs about 145 ms to read at startup**. The file
is capped independently of the buffer's own bound, dropping the *oldest*
samples to fit: the newest are the ones most likely to explain whatever made
you open poptop.

A restored buffer needs no special handling to be honest about the join: the
timeline already draws its seam across the gap and the caption already reads
real time.

**Samples from a previous boot are discarded**, and that is not tidiness. A
process is identified throughout poptop by `(pid, started)`, and on Linux
`started` counts clock ticks *since boot* — so it means something only within
one boot. Early processes land on near-identical start times every boot, so a
live pid 1 would match a restored pid 1 and its `HISTORY` column would render
the previous boot's CPU as this process's own. macOS counts from the epoch and
does not collide, but the check runs on both: the token is deliberately opaque,
and a guard that holds only on the platform whose units you happened to check is
a guard waiting for a third backend. poptop compares `at - uptime`, which every
sample already carries on both platforms, and says how many samples it dropped
and why.

Where the platform will not say when a process started, the token is absent
rather than zero, and that process gets no `HISTORY` sparkline at all. A zero
compares equal to another zero, so two unrelated programs that happened to share
a recycled pid would be drawn as one line — worse than drawing nothing. On macOS
this used to be 171 of 600 processes, because `sysinfo` reports a start time
only for processes you own; poptop now reads `sysctl(KERN_PROC_ALL)` directly,
the same call `ps` uses for `lstart`, and the figure is 0 of 623.

Two poptop windows with `store = on` are fine — each writes through its own
temporary file — but the second to exit replaces the first's history rather
than merging it. Merging two buffers is a different feature.

### Output you can build on

`--once` prints a fixed set of lines for a human who is scripting around them.
This is the other thing — a tool you can build on rather than one you watch.

```sh
poptop --export=json              # one sample, one JSON object, one line
poptop --export=line              # tab-separated, one label per table
poptop --export=json 2026-09-08   # the whole of a recorded day
poptop --schema                   # every record, field, type and unit
```

**The schema is emitted, not documented.** atop's label set lives in its man
page, which is a second thing to keep in step with the code. poptop already
declares every record and field once — for the store's codec — and the walk over
them is generated from that same list, so this is two visitors over one
declaration. A field added to a sample reaches both formats or fails to compile.
Machine-readable output that quietly stops mentioning a metric is worse than
none, and a hand-written list of what to print is exactly that waiting to
happen.

The one thing not generated is the **unit** — `u64` is bytes here and a count
there, so that is a table. It is checked against the schema by a test: a field
with no unit is a build failure, not something a user finds.

```json
{"name": "rss", "type": "integer", "unit": "bytes", "optional": true}
```

**Absence is `null`, never zero.** The rule the whole tool is built on matters
more here, not less: a consumer that cannot tell "nobody said" from "none
happened" will average one into the other. The line format writes `-`, which no
number it emits can be confused with.

**The line format names its columns.** A positional format that does not say
what its positions are is a format whose documentation is somewhere else:

```text
#sample.procs	i	pid	ppid	name	user	cpu	rss	threads	state	started	…
sample.procs	0	37757	26333	postgres	postgres	2.33	615186432	40	S	…
```

One label per table, not one per row — `grep '^sample.procs'` and read them.
Nested rows carry the same `i`, so `sample.procs.io` joins back to the process
it belongs to. The separator is a tab and never appears inside a value, whatever
the kernel had in a process name.

**Stability.** The names are the store's field names, and the store's format is
already versioned: a file written by another version is discarded rather than
guessed at. The same promise applies here — within a version the names, units
and shapes do not change, and `--schema` carries the version so a consumer can
check rather than assume. Across versions fields may be added, and a consumer
that ignores names it does not know will keep working; a field that is *removed*
or *renamed* is a breaking change and will be one deliberately.

### Opening yesterday

The restart store above is a convenience. This is the other half of atop's
argument, and the rule is worth stating before the mechanics:

> **poptop logs if it is left running, and works if it was not.**

The in-session buffer is untouched. A box that has never run poptop still gets
its ten minutes from a cold start, because that is the whole position. The log
is what accumulates when the tool happens to have been up — and it is off until
you ask, once:

```ini
log = on
```

One file a day, `poptop-YYYYMMDD`, in `$XDG_STATE_HOME/poptop/log`. Per-user,
no privileges: `/var/log` would need root or would silently not be written, and
poptop is not a system service.

```sh
poptop --days              # 2026-09-08  12.2M
poptop --read 2026-09-08   # open it, cursor on the oldest sample
```

A recorded day is scrubbed with the same keys as a live one, because it is the
same buffer — the cursor, the process table that follows it, the detail view and
the filter at the cursor all work on a buffer and none of them cares where the
buffer came from. It opens paused on the oldest sample: somebody who opened a
day meant to look at the day.

**`b` jumps to a moment**, as atop's `-b` does. An incident has a time, and reaching it by pressing
the left arrow six hundred times is not a workflow — it is the one thing atop's
`-b` does that poptop had no answer for. The box takes both forms the question
is asked in:

```text
jump to: 03:00   -2h · 03:00 · 2026-09-08 03:00   (Enter to jump, Esc to cancel)
```

Local time, not UTC, and through the C library rather than arithmetic: the
offset on a date is not a constant, and the two nights a year it changes are
exactly the ones somebody is most likely to be reading a log. A relative jump is
measured from the **end of what is retained**, not from the wall clock — in a
day opened with `--read` those are a week apart, and `-2h` there means two hours
before the end of the day you are reading.

**Landing in a gap says so.** poptop already draws a seam wherever an interval
went unobserved; answering "what was happening at 03:00" with the nearest sample
as though it were the one asked for is the same lie the seam exists to prevent,
with your own question attached to it:

```text
nothing recorded at 03:00 — nearest sample is 4m20s away
```

The distance is there because "nothing was recorded then" is only worth saying
with how far away the nearest thing is: four seconds is a hiccup, four hours is
a machine that was switched off. A moment in the **future** is a miss like any other. Treating every future
moment as "keep up" answered the likeliest typo there is: a live session at
10:00, the incident was last night, you type `23:00` — that resolves to tonight,
and reporting `23:00 is now` would be the exact failure this feature exists to
prevent. Only "now" itself resumes the live tail, and in a recorded day nothing
does, because there is no live tail there to resume.

A date that is not on the calendar is refused rather than rounded. `mktime`
normalises `2026-02-30` to 2 March and `24:30` to half past midnight the next
morning, and answering against the text you typed would make a typo read as a
real answer about a day you never asked for. (`24:00` still works: it is a real
way to write the end of a day.)

**Live recording continues while you review.** Sampling does not stop in
`--read`: the collector keeps running and, with `log = on`, today's file keeps
being written while you read last Tuesday's.

Three things follow from that and are easy to get wrong:

- **Live samples are not pushed into it.** The buffer is sized to the day
  exactly, so a push would evict the oldest recorded sample and shift the pinned
  cursor onto a different moment — a day left open for its own length would
  quietly become entirely live samples. Sampling continues (the log keeps being
  written, the collector's counters stay warm); the buffer does not change.
- **The interval is the one the day was recorded at**, taken as the median gap
  between its samples. Almost everything is scaled by it — the timeline draws a
  seam past twice the nominal interval, the growth column will not divide by an
  unknown span, the panel says how much time is buffered — so a ten-minute log
  read at one second is drawn as nothing but seams and labelled as two minutes.
  The median rather than the mean, because a day with a four-hour hole in it,
  where poptop was not running, is still a ten-minute log.
- **The restart store is never written from a replay.** With `store = on`,
  saving a replayed day would replace your real restart history with whatever
  day you opened, and you would get it back on the next ordinary launch.

A day that spans a reboot is kept whole and says so. A process is identified by
pid and start time, and start time only means anything within one boot, so two
unrelated programs either side of the restart can share a pid — the `HISTORY`
column would draw them as one line.

`poptop --once --log=on` writes one sample and exits, so a day can be filled
from cron without leaving a terminal open. It applies retention too, and a log
that cannot be written — a full disk, a read-only state directory, no `HOME` at
all — is a line on stderr and never a reason not to print the sample. Nothing
poptop prints depends on the log.

**Appended while running, not written on exit.** That is the opposite bargain
from the restart store, deliberately: a store that loses the buffer to `kill -9`
is right, because it must not become load-bearing, and a *log* that loses the
day to a crash is useless — the crash is the thing you opened it to look at. A
`kill -9` costs at most the interval since the last append.

**Every entry carries its own schema**, which is what the format in
`persist.rs` was for. Upgrade poptop halfway through a day and the morning is
still readable; an entry this build cannot decode costs that entry and says so,
not the day. A write cut short by a power failure costs the last entry and says
that too.

#### What it costs, measured

At 571 processes on this laptop, one entry is **86.6 KB**. That is more than the
restart store's 24 KB a sample, and the difference is the point: the store
writes one string table for six hundred samples, and every log entry carries its
own so that it can be read on its own.

| `log-interval` | a day | seven days |
| --- | --- | --- |
| `10m` (default) | 12.2 MB | 85 MB |
| `1m` | 121.8 MB | 852 MB |
| `10s` | 730.7 MB | 5.1 GB |

So the interval is the setting that matters, and the default is atop's for the
same reason: a day of ten-minute snapshots is what an incident review reads. Ten
minutes is a **snapshot**, not an average of the ten minutes before it — a spike
between two entries is not in the log, exactly as it is not in atop's.

#### Retention

Bounded by age **and** by bytes, defaulting to seven days and 512 MB:

```ini
log-days  = 7
log-bytes = 512M
```

The byte bound is the one that holds, because a sample carries a whole process
table: a build box with four thousand processes writes several times what a
laptop does at the same settings, so a rule in days alone is a different rule on
every machine.

`log-days` counts **calendar days**, not files: on a machine that runs poptop
occasionally, "the seven newest files" would keep one from last year and expire
nothing. And once the byte budget is spent it stays spent — a large day dropped
must not leave a smaller older one behind it, which would be retention with a
hole in it and an older file told it was past a limit the newer one had already
broken.

It bounds the **writing**, not only the keeping. Today's file is never pruned —
it is the history of the session that is running, and deleting it would take the
thing you are looking at — so a rule that only decided what to keep would watch
`log-interval = 1s` fill a disk in a day and do nothing about it. Once the logs
reach the budget poptop stops appending, and says so **on the panel while it is
true**, not only in the lines it prints when you quit: a disk that filled at
10:00 is something you need to know at 10:00.

One budget, one meaning. `log-bytes` is what poptop's logs may occupy in total —
the write cap and the retention rule are the same number weighed the same way,
because a cap that applied per-file while retention applied across files would
be two limits wearing one name.

Retention runs when the date changes rather than on a timer, so a poptop left
running over midnight applies it. Files poptop did not write are never
candidates: this is the one place in the tool that deletes a user's data, and
the rule that decides is a pure function with its own tests.

### Storage

The header can already say `WAIT 26.7%` and `BLOCKED 30`, and until recently it
stopped there. The next question is always *which device, and how badly*, so it
now names one:

```text
CPU  30.8%   WAIT   5.0%   vda  40.6% 8.6ms   RUN 4/14   BLOCKED 0   MEM  26.9%
```

`vda 40.6% 8.6ms` is utilisation and mean service time — the share of the
interval the device had at least one request in flight, and how long an
operation took. **Saturation, not throughput.** Throughput answers "how much
work went through"; a disk can sit at 100% utilisation moving 2 MB/s of random
reads, which is exactly the case a throughput figure reports as quiet.

One device, the busiest, because the header has room for a figure and not a
panel — a machine with a calm system disk and a saturated data disk must not
report itself calm. A fourth timeline row draws the same figure over time when
the terminal is tall enough for it, after memory rather than instead of it.

Everything comes from `/proc/diskstats` and the arithmetic is `iostat`'s:
utilisation from field 13, service time from the read and write millisecond
counters over completed operations, queue depth from the weighted counter.
Measured against a container writing at 2 GB/s:

```text
DEV     R/s     W/s     READ B/s    WRITE B/s   %util     await  queue
vda      20    2245      2623413   2113476532   48.9%    8.41ms  19.57
vdb       0       0            0            0    0.0%         —   0.00
```

A queue depth of 19.6 at 8.4ms is the diagnostic picture that 48.9% utilisation
understates on its own, and `vdb` shows the rule this codebase keeps everywhere:
**a device that completed nothing has no service time**, and an em dash says so.
Zero would read as an infinitely fast disk, which is the most flattering
possible lie about the figure most worth trusting.

Two filters keep the table honest. Partitions are excluded, because their IO is
already inside their disk's counters and showing `vda` beside `vda1` invites you
to add them up. And a device is listed only once it has completed an operation —
a measurement rather than a rule about names, because this container publishes
forty-odd whole devices, `ram0..15`, `loop0..7` and `nbd0..15`, of which two have
ever done anything. Filtering by name would also hide a loop device that is
actually backing something, which on a machine running containers is a device
worth watching.

macOS reports `—`. `sysinfo::Disks::refresh` costs **12.5ms** steady state,
measured, against a whole sample budget of about 4ms; an em dash is the honest
answer until there is a cheaper route to the same counters. `poptop --help` has
an `ON MACOS` section listing every figure that is Linux-only, which is where a
reader wondering about a missing number will look — poptop does not announce it
at startup, because an absence that never changes is not news twice.

### Filesystem capacity

```text
fs      86.1%  / full, 63.9G of 460.4G available
```

The only figure here that describes a hard failure rather than a slowdown: a
machine out of disk space does not get slower, it stops. It is also the only one
that is a *threshold* rather than a rate — nobody scrubs back forty seconds to
see the disk was a fifth of a percent emptier — so it earns a header figure and
**no graph row**, and says nothing at all until a filesystem passes the warn
threshold you set. That setting is exactly the judgement "close to full"
encodes, and unlike a stall percentage a used-space percentage is the same kind
of quantity it was set for.

Available space, not free space. Most filesystems reserve some for root, so the
free figure says there is room after unprivileged writes have started failing.

Two kinds of duplicate collapse into one, keyed on the **device**. A bind mount
puts one filesystem at several paths — this container has `/dev/vda1` at three —
and APFS puts every volume in one container, so `/dev/disk3s1s1`, `/dev/disk3s5`
and five others are one piece of storage. The survivor takes the **shortest mount
point** and the **least available space**: the name a reader recognises and the
number that decides whether anything is wrong. Before that, this laptop reported
`/System/Volumes/VM 86.1% full`, which was accurate and no use to anybody.

The key is the device rather than the size, because two identically-sized logical
volumes are a normal way to provision a machine — and merging those would not
rename one, it would delete it and report the survivor's figure under the wrong
mount point.

A filesystem you cannot write to cannot fill up, so read-only ones are excluded —
and that is a measurement, not a list of names. It matters more than it sounds:
a squashfs snap mount has no available space by construction, so it reads as
`100.0% full` forever, and an Ubuntu machine carries twenty-odd of them. Without
this the header would pin on `/snap/core22/1234` and the real root filesystem
could never be reported at all.

They are excluded *after* the merge, not before. Since Catalina the volume macOS
mounts at `/` is the sealed, read-only system one and the writable half is
`/System/Volumes/Data` — filtering first removed the root of every Mac. Merging
first keeps the name a reader knows and takes the space from the volume that can
actually run out.

Pseudo-filesystems need no rule about names — `proc`, `sysfs`, `cgroup2` and the
rest report zero blocks and fall out as a measurement. RAM-backed ones are named,
because they report perfectly real sizes and a full `tmpfs` is a memory problem
the header already reports.

**Network filesystems are skipped, and that is a real gap.** `statfs` on an
unresponsive NFS or SMB mount blocks until it answers, and a monitor that freezes
when the fileserver does is worse than one that does not mention the fileserver.
macOS avoids the question — `getfsstat` takes a flag asking it not to wait, and
answers for all twelve filesystems in **7µs**, against the 12.5ms `sysinfo`
charges for the same numbers. Linux has no such flag, so there the exclusion is a
list of type names.

### The network

Two figures, and the ordering between them is the point:

```text
CPU  27.9%   NET 37 retrans   en0 4.4K/s 1.1K/s   MEM  80.4%
```

`en0 4.4K/s 1.1K/s` is throughput — what every monitor shows, and what least
often explains a slow machine. A link at 3% of its capacity dropping 2% of its
packets is slow; one at 90% is usually fine. So throughput sits *below* memory in
the ladder and is given up before it.

`NET 37 retrans` appears only when something has gone wrong, and is kept almost
to the end when it does. A figure reading `NET 0 drops` every second would spend
the scarcest thing on screen to say nothing happened.

Four counters, reported in the order that narrows the problem down rather than by
size:

| counter | what it points at |
|---|---|
| `retrans` | the path between here and elsewhere |
| `listen drops` | a service on this machine not accepting fast enough |
| `drops` | the kernel or a ring buffer |
| `errors` | the link or the cable |

A machine with one retransmit and four hundred errors is telling you about the
cable, but the retransmit is the figure that changes what you do next.

Interfaces are listed once they have carried a byte, the same measurement the
disk table uses — this laptop publishes twenty-seven and thirteen have. The
figure names the busiest rather than aggregating, because a total across
twenty-five idle tunnels and one real link is a number about the tunnels.

There is no network row on the timeline. That panel draws percentages of a fixed
denominator — it prints `100` at the top, rules the warn and critical thresholds
across the graph, and reads out `NET 100.0%` under the cursor. Bytes per second
has no such denominator, and scaling to the window's own peak makes the busiest
sample 100 by construction: an idle laptop moving 8 B/s of loopback painted a
full-scale graph straight through the critical rule. The header carries the
figure until the timeline can draw a series with a scale of its own.

macOS reports throughput and errors, and **em dashes for drops, retransmits and
listen drops**: `sysinfo` counts errors without separating drops, and there is no
TCP counter behind it at all. Three zeroes there would claim a perfectly healthy
network on a machine that cannot see one.

Per-process network attribution is still out of scope — it needs `/proc/net`
inode matching or eBPF and is its own project.

### When the storage is somebody else's machine

```text
…/mnt/data  1.2k op/s  3.4% re    NFSD 480 op/s
```

On a box whose working set lives on NFS, every disk figure poptop draws
describes a local disk that is doing nothing while the machine waits on the
network. `WAIT` says the CPU is idle with I/O outstanding and then strands you;
this names the mount.

**Retransmissions, not throughput.** An NFS mount in trouble is usually one
whose calls are being sent twice — a server under load, a path dropping
packets, a firewall eating idle connections — and its byte rates look perfectly
ordinary throughout. The figure is heated on the share of calls that had to be
resent, on its own thresholds: one percent is worth looking at and five percent
is a mount that is failing, where five percent of a CPU is nothing at all.
Reading the machine's heat ramp straight would paint a dying mount calm.

The share is shown only when something is being resent. A healthy mount
retransmits nothing for weeks, and a permanent `0.0% re` is a figure nobody
reads by the second day — the rule `CLK` and `STL` follow.

`NFSD` appears only where this machine is *serving*. `/proc/net/rpc/nfsd` exists
on any kernel with the module loaded and reads as zeroes, so presence is not the
test: the count of running `nfsd` threads is. A box that does not serve NFS and
a box whose server is quiet are different machines, and only one of them is
worth a figure.

Every figure here is **per second**, like the disk and network ones beside it,
so `--interval` cannot change what a mount appears to be doing. The mean round
trip is the exception and is a mean over the interval: it is already a duration,
and a mount is not faster for having been watched for longer. A mount that
completed no call reads `—` rather than `0.0ms`, which would say every call was
instant.

A mount whose server has **stopped answering** has calls going out and none
coming back, so the share has no denominator; there the header prints the
retransmission rate itself — `300 re/s` — as loudly as the ramp goes. That mount
also outranks a healthy busy one for the header's single slot, because ranking on
completed calls alone would lose the failing mount to any working mount beside
it, which is the one case this figure is for.

`--once` prints a line per mount — calls, retransmissions, mean round trip,
bytes actually read and written — plus the client's totals across every mount
and, where one is running, the server's reply-cache hits and misses and the
calls it refused. Bytes are what crossed the wire, not what the application
asked for; the difference between those two is the page cache, which is not a
fact about the mount.

**This does not block on an unresponsive server, and that is why it is here.**
Filesystem capacity skips network mounts for exactly that reason — `statfs` on
a hung NFS mount waits for the server, and a monitor that freezes when the
fileserver does is worse than one that never mentions it. `/proc/self/mountstats`
is the client's own counters: no RPC, no round trip, and the mount that has
stopped answering is the one this reports on most loudly.

### What poptop will read, and what it will not

atop reports four subsystems poptop does not: NFS, GPU, Infiniband and
last-level cache. NFS is the one above. The other three are declined, and the
reason is a rule rather than a shrug — poptop's whole position is that it starts
on a machine you have just connected to, so anything it needs to be installed
first is a thing it cannot rely on.

**poptop reads what the kernel publishes to an ordinary reader.** A file under
`/proc` or `/sys`, a netlink socket, a syscall. Anything that needs a daemon
running beforehand, a vendor library present, or a privileged helper is an
*optional* source: it may enrich the picture and it is never required for the
tool to work, and its absence shows as `—` rather than as a failure to start.

By that rule:

- **NFS** is in. Three world-readable files, no daemon, no library.
- **GPU** is declined for now. Per-GPU and per-process utilisation needs NVML —
  a vendor library, versioned against the driver, absent on the machines poptop
  is for — or a daemon, which is what atop uses. There is no kernel interface
  that reports it: `/sys/class/drm` publishes a `gpu_busy_percent` for AMD cards
  and nothing comparable for NVIDIA, so building on it would report GPU load on
  one vendor and silence on the other, which is worse than an honest gap. If it
  is built it will be an optional source behind the rule above.
- **Infiniband** is declined. `/sys/class/infiniband` does publish port counters
  without the verbs stack, so this one is *possible* within the rule — it is out
  on scope rather than on principle, and the machines that have it are the
  machines that already have their own fabric monitoring.
- **Last-level cache** is declined. It needs `perf_event_open` with RDT/CMT
  support, which is a privileged interface on most kernels, and a figure that
  works only under `sudo` on some processors is not a figure this tool can put
  on a header.

The rule is written down because it is the answer to every future "should poptop
shell out to X", and answering it four separate times would eventually produce
four different answers.

### Throttling

```text
CPU 100.0%   CLK  62.0%   MEM  41.2% ██████▒▒░░░░  6.6G / 16.0G
```

`CLK 62.0%` is how much of the processor's nominal clock the kernel is currently
allowing. A capped machine reports 100% busy and gets less work done than it did
an hour ago, and every other figure here reads normal — `STALL`, `WAIT` and disk
saturation all look healthy, because nothing is *waiting*. The work is simply
being done more slowly. Without this figure that case is invisible.

Not a temperature. `84°C` makes you infer, and on hardware whose nominal is 85°C
it makes you infer wrongly.

It is the **policy ceiling**, not the current frequency. An idle core clocks
down, which is a healthy machine doing nothing and reads identically to a
throttled one; a ceiling says what the machine is *permitted* to do, so an idle
box reads 100%. That catches whatever the driver reports by lowering its policy
maximum — thermal, power, or a limit somebody set by hand — and not hardware
capping that leaves the policy alone and reports through counters instead.

Shown only below 99% of nominal. Drivers report ceilings a fraction under the
hardware maximum as a matter of course, and a permanent `CLK 99.7%` would be the
figure that taught everyone to ignore it. macOS publishes nothing reachable
without shelling out, so there it is absent rather than 100%.

### Stall pressure

```text
CPU  27.4%   WAIT   6.7%   vda  34.8% 7.2ms   STALL io  6.1%   RUN 2/14   BLOCKED 2
```

`STALL io 6.1%` is Pressure Stall Information: the share of the last ten seconds
in which **every runnable task** was stopped waiting for IO. Not "some task was
waiting", which a busy machine does all day and healthily — every one of them,
with nothing getting done by anybody.

It is coloured against thresholds of its own, 5% and 20%, rather than the warn
and critical percentages you set for everything else. Those are about
utilisation, where 50% is unremarkable; a machine that spent 50% of ten seconds
with nothing at all running is in serious trouble, and borrowing the same
numbers would leave the figure cold until long past the point of caring. Five
percent is half a second in every ten with the machine stopped.

It says something `WAIT` cannot. `iowait` is the CPU's view — idle with IO
outstanding — so a box with plenty of other work to do reports a calm `iowait`
while every task that matters is stuck behind the disk. That is the case this
catches, which is why it earns a figure and a graph row of its own rather than
being folded in beside `WAIT`.

Read from `/proc/pressure/{cpu,io,memory}`, `avg10` only: the longer windows are
the kernel's own smoothing, and poptop has a timeline for that. CPU is collected
and stored but never named in the figure — the kernel documents `full` as
undefined there and reports zero, so including it would win every tie.

**It is optional and treated as such.** `/proc/pressure` needs `CONFIG_PSI=y`,
and some distributions ship it behind `psi=1` on the kernel command line. It was
present on every kernel checked here — but checking four container images tests
*one* kernel, since containers share the host's, so that is much weaker evidence
than it looks. Nothing in the default view depends on it: an absent
`/proc/pressure` means no figure and no graph row rather than a zero, and the
row goes back to the graphs that were already there.

### Identifying a row

The command is the column that says which process a row is, and it is the one
that takes whatever the fixed columns left — at 104 columns with the disk
columns shown, nineteen, one more than the two disk-rate columns together.

That is not enough for a name like `Google Chrome Helper (Renderer)`, and
letting the terminal clip it removes exactly the part that tells three such rows
apart. So the middle goes and both ends stay:

```text
81977   oddurs   17.3  ▊   6.1G  █   S  34   Google Chr…(Renderer)
5613    oddurs   20.0  ▊   514M  ▏   S  27   Google Chr…er (GPU)
```

The head is what a reader scans down the column for; the tail carries the role.
Cutting either alone loses a distinction the other cannot supply. In tree mode
the indent is charged against the name rather than the column, so `│  └─ ` does
not push the tail off the end.

### What the table cannot show

poptop reads `/proc` at an instant, so **a process that lived 200ms never existed
as far as the table is concerned.** That is not an edge case here: a burst of
short-lived processes is one of the commonest causes of exactly the spike you
scrubbed back to find, so the table can end up sitting under a graph it cannot
explain.

poptop cannot show you those processes. What it can do is stop implying they did
not happen:

```text
── processes (312) — sort: CPU · 47 tasks came and went ────────────────────
```

`/proc/stat` publishes how many tasks the kernel has created since boot, so the
difference between two samples is exactly how many were created in between.
Subtract the ones still alive when poptop looked, and the remainder is what came
and went unseen. Naming that number is the same principle as rendering `—`
rather than a fabricated zero: an absence stated is not an absence hidden.

It says **tasks**, not processes, because that is what the kernel counts — a
`clone` for a thread advances it exactly as a `fork` for a process does.
Thread growth inside surviving processes counts on the visible side, but a
thread *pool* that recycles workers creates and destroys them inside an
interval and leaves its thread count unchanged, so its turnover lands here too.
Only a per-process cumulative task counter could separate the two and `/proc`
publishes none, so the figure is reported as what it honestly is rather than
being called something more specific than it is.

The figure is suppressed across a sampling gap: the two samples either side of
a suspend can be hours apart, and billing a whole night's task creation to the
one second the table is describing would be worse than saying nothing. The
timeline already draws a seam there, and both read the same definition of
whether two samples are adjacent.

macOS publishes no equivalent counter, so poptop says nothing there rather than
zero. "I do not know" and "none happened" are opposite answers.

The same rule governs the `THR` column. macOS will only report a thread count
for processes you own — about two thirds of the table — and poptop used to fill
the rest with `1`. That is not a missing figure but a wrong one, and next to the
column beside it, visibly wrong: a virtual machine using three cores rendered as
`300%` beside `1 thread`, and one thread cannot use three cores. The unreadable
ones now show `—`.

In practice the dash lands where it costs nothing. A process busy enough for its
thread count to matter is almost always one of your own: measured across a
660-process table, **no process above 5% CPU had an unreadable thread count.**

**Actually capturing those processes** is what the next section is about.

### Processes that came and went

poptop reads `/proc` at an instant, so a process that lives 200ms never existed
as far as a sampling monitor is concerned. That is not an edge case: a burst of
short-lived processes is one of the commonest causes of exactly the spike you
opened poptop to explain, and scrubbing back to it showed a process table that
could not account for the graph above it.

The kernel will tell you. `taskstats` emits a record for every task that exits,
and poptop registers as a listener — so a process that lived and died between
two samples is a **row in the interval that contains it**, marked `X`, with its
pid, its parent, its user, the CPU it used and its peak memory. It filters,
sorts and searches like any other row: `state = X` finds them all.

    exited  7390  in the last interval
            true pid 37833 0.0%

It needs three things, and poptop says which is missing rather than showing you
an empty interval:

- **`CAP_NET_ADMIN`** — run as root, or grant the capability.
- **The initial PID namespace.** The kernel registers exit listeners only
  there, so this does not work from inside a container.
- **The initial network namespace** — the easy one to miss. Registration
  succeeds anywhere, but delivery goes to a port in the initial net namespace,
  so a listener in its own hears nothing and it looks exactly like a kernel
  without the feature.

**It costs almost nothing.** Measured: 972 µs a sample against 944 µs with it
off, and **+26 µs to capture 145 records** during a burst — the kernel has
already written them, and poptop is only draining a socket.

The CPU on an exited row is this interval's, not the process's whole life:
`taskstats` reports a lifetime total, so a process that ran for three hours at
50% and exits here would otherwise read 540,000,000%. What it had already used
at the last sample is subtracted, which is the same arithmetic every live row
uses — so the number beside an exited process means what the one above it
means.

A burst big enough to overrun the socket buffer is reported, not swallowed:
20,000 exits in one second overran a 4 MB buffer and the kernel dropped every
record of it. poptop asks for 16 MB, and the `N came and went` figure in the
panel title is the reconciliation — the kernel's own count of task creations,
minus what the table can now account for. If records are lost, that figure is
what says so.

Not on macOS, which has no equivalent; `--once` reports `exited — not
collected` rather than a zero.

### Themes

The palette is compiled in, but it is not the only one you can have. A theme is
one file, one line per colour, in `~/.config/poptop/themes/NAME.theme`:

```ini
# ~/.config/poptop/themes/nord.theme
ok         = #8fbcbb    # hex,
series_cpu = 67         # a 256-colour index,
chrome     = darkgray   # or an ANSI name
```

**Every line is optional.** A theme inherits `safe` for anything it does not
name, so overriding two colours is two lines. That is the whole reason btop
ships 41 themes and htop, which hardcodes eight in C, has never gained a ninth:
the barrier decides whether a theme library exists.

The tokens are exactly the ten the palette was built around — `ok`, `warn`,
`critical`, `series_cpu`, `series_mem`, `chrome`, `text`, `text_dim`,
`selection_bg`, `live`. Nothing new is invented for user themes; a vocabulary
the code did not already use would be a second one to keep in step with the
first.

**The built-ins ship as files too**, in [`themes/`](themes/), so the way to
learn the format is to copy one — and a test asserts they match the compiled
palettes colour for colour, so they cannot quietly drift.

A built-in name always wins over a file. A `safe.theme` in your themes
directory would otherwise silently replace the palette everything else in this
project is measured against.

**A colour the terminal cannot show is left alone and reported, not
approximated.** Hex needs a true-colour terminal, an index needs 256 colours, a
name works anywhere, and monochrome ignores all of them:

```
poptop: theme `nord`: this terminal is Ansi16 and cannot show ok, series_cpu;
      keeping ok = cyan, series_cpu = lightblue
```

Squeezing 24-bit hex into sixteen slots would destroy exactly the separation
the palettes were measured for — and those sixteen slots belong to your
terminal theme, not to poptop.

### Measuring a theme

poptop is the only monitor that measures its own palette, and the moment you can
supply your own that guarantee evaporates — unless the validator is turned
outward. So it is:

```
$ poptop --check-theme muddy
muddy: FAIL
  ok          ↔ warn        ΔE   0.1  tritan    below the target of 8
  ok          ↔ critical    ΔE  54.2  deutan
  ...
  critical    on surface         1.09:1          below 3:1
  critical    on selected row    1.40:1          below 3:1
  worst pair: ΔE 0.1   worst contrast: 1.09:1
```

Every pair, not only the failures: a theme passing at ΔE 8.1 is a different
thing from one passing at 30, and the number is the point. **A contributed
theme can arrive with a measurement rather than a screenshot.**

**There is a third outcome, and it is not a pass.** An ANSI name or an index
below 16 is a *slot* — what it looks like belongs to your terminal theme, not
to poptop — so there is genuinely no hue to measure:

```
$ poptop --check-theme ansi
ansi: INCOMPLETE
  ...
  not measured: ok, warn, critical. An ANSI name or an index below 16 is a
  slot, and what it looks like belongs to your terminal theme rather than to
  poptop — there is no hue here to measure. Spell these as `#rrggbb` or a
  256-colour index to have them checked.
```

Only a pass exits zero. A check that could not see the colours has not passed
them, and a script asking "is this theme legible" must not be told yes by
silence. The same applies to `selection_bg`: if the selected-row background is
an unknowable slot, the rows measured against it are absent rather than
invented.

**The check measures at the top tier, not the one it detects.** In a CI job
`TERM` is often unset, which detects as monochrome — so a check that read the
detected tier would find nothing measurable and certify anything, in exactly
the place this command is meant to run. The question is whether the *theme* is
legible, which is a property of the colours it names rather than of the
terminal running the check.

**This is the same instrument CI uses.** The palette tests assert through
`check::Report` rather than a second copy of the arithmetic, so poptop's own
check and yours cannot come to disagree about the same colours.

Separation is asked only of the colours that must be told apart. Legibility is
asked of everything drawn — including `text` and `live`, since a theme that
makes the interface invisible while keeping its status hues distinct is not a
theme anyone can use. `chrome` and `text_dim` are meant to recede, so they have
a lower floor: a border competing with the numbers inside it is a worse border,
but one nobody can find is worse still. And chrome is measured only against the
surface, because the process table has no side borders — it never crosses a
selected row, and a check that fails on things that cannot happen trains people
to ignore it.

**A failing theme still loads**, with one line saying why:

```
poptop: theme `muddy`: ok and warn are only ΔE 0.1 apart (tritan), critical is
      1.09:1 on the surface — run `poptop --check-theme muddy` for the rest
```

It is your terminal and your choice; poptop's job is to have the number and say
it, not to refuse — the same principle as rendering `—` rather than a
fabricated zero.

`--check-theme classic` reports FAIL, and says why that is a decision rather
than a bug: classic exists to restore the green/yellow convention, and
green/yellow is the pair that convention gets wrong under red-green deficiency.
That is the whole argument for `safe` being the default, and it is pinned by a
test so that quietly "improving" classic would break the build rather than
remove the argument.

### What the timeline graphs

`CPU` and `WAIT`, and memory only when there is room for it.

Memory used over ten minutes is a flat line or a slow ramp that repeats what
the header already says, and it was occupying half the most valuable space on
screen. Waiting is the row that turns a stalled machine from a mystery into a
shape, so memory is the one that yields — and comes back as a third row on a
terminal tall enough to carry it. Where the platform publishes no iowait, the
row goes back to memory rather than the graph carrying one it cannot fill.

A series is only drawn if the gutter can name it, because identity here rests
on the label rather than the hue — the third series reuses a colour.

That is a cost decision, not a limit. Searching the whole cube for a sixth hue
clearing ΔE 8 against the existing five returns 77 candidates, the best at ΔE
10.5 against a palette whose current worst pair is 10.3. But every one of them
is adjacent to a hue already in use — orange between `warn` and `critical`,
pale cyan beside `ok`, periwinkle beside `series_cpu` — because the safe
palette already avoids green, the pair red-green deficiency destroys. So a
sixth token would mean another key in every user theme and `--check-theme`
going from ten pairs to fifteen, in exchange for a colour that reads as a
near-miss of an existing one, for a row the gutter names outright.

The hues alternate instead, which guarantees the only thing that matters: two
graphs touching each other never share a colour.

### Sample rate and window

`interval` is the time between samples; `window` is how much history to keep,
**expressed in time rather than a sample count** — the span is what you
actually want, and the buffer size follows from it. Spans are written the way
people write them: `500ms`, `2s`, `10m`, `1h`. A bare number is seconds,
because that is what someone typing `interval = 2` means.

Sub-second sampling narrows the window in which a short-lived process is
invisible; longer intervals stretch the retained span on a quiet box. The ring
buffer already tolerates uneven intervals, because the timestamps are real.

**The two settings cost memory together, not separately.** Every sample retains
its whole process table — that is what makes scrubbing show the real table from
that instant rather than an interpolation — so the buffer is about
`samples × processes × 96 bytes`:

| processes | 10m at 1s | 1h at 1s | 10m at 100ms |
|---|---|---|---|
| 100 | 6 MB | 35 MB | 59 MB |
| 400 | 23 MB | 139 MB | 231 MB |
| 4000 | 231 MB | 1.4 GB | 2.3 GB |

(a buffer holds one more sample than the span needs — `n` samples span `n − 1`
intervals — so the real figures are a few kilobytes above these)

(measured by the `show_sample_footprint` test, so the table can't drift from
the structs)

A day of history at one sample a second and ten minutes at sixty a second are
the same buffer, so poptop bounds the **product** — the sample count — rather
than either setting, and says what the buffer would cost when it refuses:

```
poptop: `window` 86400s at `interval` 500ms is 172801 samples, above the limit
      of 86401; every sample retains a whole process table, which is about
      6663 MB on a 400-process box
```

**The interval floor belongs to the backend**, because the two have different
reasons for having one.

On Linux it is **50 ms**, and the reason is cost: a `/proc` pass is about 1 ms
at 400 processes, so 50 ms already spends 2% of a core and 10 ms would spend
10%. A monitor that is itself the load is not measuring the machine.

On macOS it is **200 ms**, and the reason is correctness: sysinfo needs that
long between CPU refreshes, and below it the per-process figures are not noisy,
they are wrong. Measured on an idle-ish machine, the busiest process reported
`3.5%` at 50 ms and `262.9%` at 100 ms — two and a half cores of work, reported
as three and a half percent, with nothing on screen to say so.

poptop refuses the setting rather than quietly substituting a different one, and
the message carries the reason:

```
poptop: `interval` is 50ms; the fastest this build can sample is 200ms, because
        sysinfo needs 200ms between CPU refreshes on this platform, and below it
        the per-process figures are wrong rather than merely noisy
```

## How it works

The page size is read from `/proc/self/auxv` at startup rather than assumed.
RSS is a count of pages multiplied by it, so a hardcoded 4096 reports every
process at a quarter of its real memory on a 16 KiB-page kernel and a sixteenth
on a 64 KiB one — Asahi and RHEL aarch64 respectively — and nothing about the
output looks wrong. Where the kernel will not say, poptop assumes 4096 and says
that it did.

Two backends behind one `Collector` trait:

- **Linux** (`src/collect/linux.rs`) parses `/proc` directly with nothing but
  `std`. This is the interesting one. It handles the parts that catch people
  out: `/proc/[pid]/stat` field 2 can contain spaces *and* parentheses, so it
  splits on the last `)` rather than on whitespace; `guest` and `guest_nice` in
  `/proc/stat` are already counted inside `user` and `nice`, so summing every
  field double-counts them; and every CPU figure is a delta between two reads,
  which is why the first sample always reports zero.
- **macOS/other** (`src/collect/darwin.rs`) goes through `sysinfo`, so the tool
  runs on a dev laptop. There is no `/proc` here and the mach calls that replace
  it are a different project's worth of `unsafe`.

History lives in a fixed-capacity ring buffer (`src/history.rs`). "Live" is
represented as *absence* of a cursor rather than an index pinned to the end, so
there is exactly one representation of it and pushes never have to fix the
cursor up. When the window slides, a pinned cursor slides with it — otherwise it
would silently drift forward through time while appearing to stay put.

Ten minutes of scrollback at one sample per second, which is roughly 30 MB of
process tables on a busy machine.

## Notes from reading the others

Lessons taken from htop, btop, and bottom, with the measurements that backed
them:

- **Cache what doesn't change.** htop reads a process's cmdline and start time
  once per process *lifetime*, not once per sample, and gets the uid from a
  single `fstat` on the `/proc/<pid>` directory rather than parsing
  `/proc/<pid>/status`. poptop originally parsed that file for every process every
  second: at 400 processes that was 58% of total collection time. Switching to
  `fstat` took a sample from 3.19ms to 1.41ms.
- **Clamp CPU percentages.** htop does `MINIMUM(percent_cpu, activeCPUs * 100)`.
  Without it, a pid reused between two samples diffs the new process against the
  old one's counter and reports thousands of percent. poptop clamps the same
  way, but in the model rather than in a backend: the ceiling is a claim about
  what the hardware can deliver, not a fact about `/proc`, and it lived in the
  Linux collector alone for long enough that nothing said whether macOS did not
  need it or had merely forgotten it. It is now applied to every backend's
  output by `Collector::sample`, so a third backend gets it by existing.
- **Guard the sample interval.** htop carries the comment "period might be 0
  after system sleep" — a real bug someone hit on a laptop, worth knowing about
  before it happens to you.
- **Only collect what is displayed.** htop gates expensive reads behind
  `PROCESS_FLAG_*` bits derived from the visible columns, so turning off a
  column stops the syscalls behind it. poptop collects everything unconditionally;
  this is the right shape to adopt before adding per-process IO and network.
- **Spread expensive work across samples.** For costly `/proc/<pid>/maps`
  parsing htop re-checks each process on a randomised interval rather than doing
  every process on the same tick, which avoids a periodic stall.
- **Have a fallback glyph set.** btop keeps a `tty_mode` symbol table for
  terminals that cannot render braille. Any Unicode-dependent drawing needs a
  plain-ASCII path.

All three are implemented: braille rendering and timeline zoom
(`src/glyphs.rs`, `History::peak_slots`), the process tree (`src/tree.rs`), and
tiered collection (`collect::Needs`).

**Tiered collection.** Core figures — cpu, memory, rss, name, user, state,
threads — are never gated, because the timeline and the default table depend on
them and their history has to be complete. Per-process disk IO is gated on the
column being visible, and it is not cheap: at 400 processes a sample costs
1.55ms without it and 2.28ms with, since it adds a file read per process.

**Gated collection conflicts with rewindable history** in a way htop never has
to face — enable IO at t=300 and the first 300 samples have nothing to show when
you scrub back into them. poptop resolves this by making collection a *ratchet*:
showing the columns starts collection, hiding them does not stop it. Toggling
would otherwise punch holes wherever the column happened to be off, and one
clean boundary is far easier to reason about while scrubbing than several.

Three states are rendered, and **none of them is a zero** — a fabricated zero is
indistinguishable from a genuinely idle process:

| Shown | Meaning |
| --- | --- |
| `·` | history recorded before the column was switched on |
| `—` | unreadable, or the process is too new to have a second reading yet |
| `1.2M/s` | a real rate |

`/proc/<pid>/io` is mode `0400` and owned by the process owner, so reading other
users' processes needs `CAP_SYS_PTRACE`. Only genuinely unreadable processes
prompt for root; a process merely awaiting its second reading also shows a dash
but resolves on its own, and conflating the two produces advice that does not
help.

The tree is a view over `ppid`, which every retained sample already carries, so
the tree you see while scrubbed back is the real hierarchy from that moment.
Every process appears exactly once even when `ppid` is corrupt or cyclic, and a
process orphaned between samples becomes a root rather than disappearing.

**macOS caveat:** `sysinfo` reports no parent for processes the user does not
own, so about a third of pids arrive with `ppid = 0` and appear as roots. The
`/proc` backend has real parentage for everything.

## Roadmap

Open work is tracked in-repo with [cairn](https://github.com/oddurs/cairn) —
see [ROADMAP.md](ROADMAP.md), or `cairn board` in a checkout.

The reasoning behind each decision lives in [`docs/roadmaps/`](docs/roadmaps/),
derived from reading the prior art (htop, btop, bottom, zenith, atop) and
auditing this UI against data-visualisation practice. Start with
[the index](docs/roadmaps/README.md).

## Status

Early. What works: both backends, the timeline with scrubbing and zoom, the
process tree (`t`), grouping (`g`), the per-process history panel (`d`), sorting
including by whatever is constrained (`S`), the query filter, per-process disk
throughput, clock-ceiling reporting, configurable intervals, persisting history
across restarts (`store`), themes with colour-vision validation, and `--once`.

Not there yet: per-process network attribution, which needs `/proc/net` inode
matching or eBPF and is its own project; killing or renicing processes; mouse
support; and capturing processes that live and die entirely between two samples,
where the `taskstats` exit-record path cannot be verified in the environment
available here. Registering as an exit listener returns `EINVAL` while per-pid
queries on the same socket work; the mask parses (the `ERANGE` boundary sits
exactly at `nr_cpu_ids`) and is refused after parsing, which is the kernel
declining exit-listener registration from anything but the initial PID
namespace. Every Linux here is a container, which is by definition not that. It
needs a host, not a better kernel.

## Attribution

Nothing this repository publishes carries a note about what wrote it — not
commit messages, not file contents, not pull request text. All of it is public
history, and none of it is the place for that.

`.githooks/no-attribution` enforces it, and three layers share the one script so
they cannot disagree:

```sh
git config core.hooksPath .githooks   # once per clone
```

The `commit-msg` hook catches it at the moment it would enter history, when the
fix is free. `./check` scans every tracked file, because the hook is opt-in per
clone. CI does both, over the commits a push introduces as well as the files,
and is neither opt-in nor skippable.

The pattern matches tool names, the co-author trailer and the robot emoji —
deliberately not a generic "generated by", which `Cargo.lock`, `ROADMAP.md` and
the theme files all say truthfully about themselves. A check that cries wolf
gets switched off.

It is strict enough to catch its own documentation: this section could not name
the trailer literally, and neither could the commit that added it. That is the
right way round.

## Tests

```sh
./check                        # everything CI enforces, on the host
./check --linux                # the same again inside rust:1-slim
./check --quick                # fmt, build, test; skip clippy and live data

cargo test -- --ignored --nocapture show_frame   # print a rendered frame
```

The last one matters on a Mac: the `/proc` backend is `cfg`'d out of a macOS
build entirely, so it is neither compiled nor tested unless you run it on Linux.

`./check` exists because of one property: **a tool that is missing is a failed
check, never a skipped one.** The `rust:1-slim` image ships without clippy, so
`docker run … cargo clippy` exits with "not installed for the toolchain" — which
looks nothing like a lint warning and is easy to read as a clean run. That is how
a dead field in `collect/linux.rs` reached a pull request: `cargo test` passed on
both platforms, and the clippy step that would have caught it had never once
executed. The script installs the component inside the container, and reports
anything it could not run rather than passing quietly.

UI tests render through ratatui's `TestBackend` and assert on the resulting
buffer, including a 1×1 terminal — a monitor that panics on a small window is
worse than no monitor, and it never shows up in normal use.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
