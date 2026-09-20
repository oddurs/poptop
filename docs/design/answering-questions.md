# The keys, and the questions they answer

| Key | Action |
| --- | --- |
| `q` | quit. `SIGTERM`, `SIGHUP` and Ctrl-C quit the same way, and give the terminal back |
| `Esc` | back out one level: leave the filter or jump box, cancel a signal, let go of the selected process — and with none of those, quit |
| `←` / `→` | scrub through history (hold `Shift` for ten at a time) |
| `b` | jump to a moment: `-2h`, `03:00`, `2026-09-08 03:00` (atop's `-b`) |
| `x` / `X` | send `TERM` / `KILL` to the selected process (needs `signals = on`) |
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
| `?` | list every key; any key puts the list away |

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

## Filtering

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

## Threads

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

## What happened to my process

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

## What the memory is actually holding

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

## Whether the box is busy, or not being given a box

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

## What a process is costing you

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

On macOS, PSS is an em dash: there is no `smaps_rollup`, and nothing else
publishes a per-process share of shared pages. The fault columns and the
virtual size are read from `proc_taskinfo` along with the thread count, for the
processes you own — faults as rates over the interval, like everywhere else —
and are em dashes for the rest. `GROW` works there too: it is derived from RSS,
which sysinfo does publish.

## What the table shows

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

## Which user is eating the machine

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

## Whose process is it

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

## Which cgroup is stalled

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

## What it costs to watch

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

**Budgets that fail.** What poptop itself may cost is written down in
`src/budget.rs`, each figure with the measurement it was set from, and
`./check --perf` fails a release build that goes over one:

| | measured, Mac / Linux container | budget |
| --- | --- | --- |
| a sample, whole | 6.3 ms / 0.17 ms | 25 ms |
| a sample, per process (from 100 processes) | 8.8 µs | 30 µs |
| a 200×60 frame, 900 processes × 600 samples | 4.4 ms / 7.7 ms | 25 ms |
| encode a full store (600 × 400) | 29 ms / 30 ms | 90 ms |
| decode it | 7.7 ms / 11.6 ms | 35 ms |
| its size | 25.0 MB | 30 MB |
| a full store to the first frame | 10.2 ms / 10.5 ms | 35 ms |
| memory after an hour at 400 processes | 66 MB / 62 MB | 100 MB |
| memory growth after the buffer is full | 96 KB / 0 | 8 MB |
| 100 NFSv4.2 mounts' `mountstats` (Linux) | 0.5 ms | 1.5 ms |

CI prints the same figures on every run without enforcing them, because a
shared runner's timing is noise.
