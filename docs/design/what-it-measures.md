# What it measures, and what it will not

## Storage

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

## Filesystem capacity

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

## The network

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

## When the storage is somebody else's machine

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

## What poptop will read, and what it will not

atop reports four subsystems poptop does not: NFS, GPU, Infiniband and
last-level cache. NFS is the one above, and GPU load came in later where the
OS publishes it. The rest are declined, and the reason is a rule rather than a
shrug — poptop's whole position is that it starts
on a machine you have just connected to, so anything it needs to be installed
first is a thing it cannot rely on.

**poptop reads what the kernel publishes to an ordinary reader.** A file under
`/proc` or `/sys`, a netlink socket, a syscall. Anything that needs a daemon
running beforehand, a vendor library present, or a privileged helper is an
*optional* source: it may enrich the picture and it is never required for the
tool to work, and its absence shows as `—` rather than as a failure to start.

By that rule:

- **NFS** is in. Three world-readable files, no daemon, no library.
- **GPU load** is in, where the OS publishes it (0234). Per-GPU and
  per-process utilisation on NVIDIA needs NVML — a vendor library, versioned
  against the driver, absent on the machines poptop is for — and that is still
  declined. What changed is the reading of the objection to the rest. It was
  that `/sys/class/drm` publishes `gpu_busy_percent` for AMD cards and nothing
  comparable for NVIDIA, so poptop would report load on one vendor and silence
  on the other. But poptop's silence is not a zero: a GPU that publishes no
  load gets no figure and no row, exactly as a Mac gets no `WAIT`, and the
  [platforms reference](../reference/platforms.md) says which GPUs publish.
  An honest gap is still honest when something beside it is filled in. And
  macOS publishes the Apple GPU's own utilisation to any user in the IO
  registry, which covers every Mac poptop runs on.
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

## Throttling

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

## Stall pressure

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

## Identifying a row

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

## What the table cannot show

poptop reads `/proc` at an instant, so **a process that lived 200ms never existed
as far as the table is concerned.** That is not an edge case here: a burst of
short-lived processes is one of the commonest causes of exactly the spike you
scrubbed back to find, so the table can end up sitting under a graph it cannot
explain.

poptop cannot show you those processes. What it can do is stop implying they did
not happen:

```text
── processes (312) · 47 tasks came and went ─────────────────────────────────
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

## Processes that came and went

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
