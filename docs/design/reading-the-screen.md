# Reading the screen

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

## Which half of the machine is full

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
