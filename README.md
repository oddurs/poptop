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

q quit · ←/→ scrub · +/- zoom · Space live · ↑/↓ select · s sort · / filter
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
for years. It writes compressed daily logfiles, keeps 28 days by default, and
does one thing poptop cannot: it captures processes that started *and finished*
between two samples. If a burst of short-lived processes spiked your machine,
atop can name them and poptop cannot — see
[`docs/roadmaps/05-data-fidelity.md`](docs/roadmaps/05-data-fidelity.md).
poptop will at least tell you they happened (below), but a count is not a list.
On raw capability atop is the better tool.

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
| `+` / `-` | zoom the timeline in and out |
| `Space` | pause on the current sample, or resume live |
| `Home` / `End` | jump to oldest / live |
| `↑` / `↓` | select a process |
| `s` | cycle sort column |
| `t` | toggle the process tree |
| `i` | toggle per-process disk IO columns |
| `/` | filter by name or pid |

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

**Actually capturing those processes** needs `taskstats` over netlink, which
needs `CAP_NET_ADMIN` — tracked in the roadmap, and the remaining substantive
capability gap against atop.

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
process tree (`t`), sorting, filtering, per-process disk throughput,
configurable intervals, persisting history across restarts (`store`), themes
with colour-vision validation, and `--once`.

Not there yet: per-process network attribution, which needs `/proc/net` inode
matching or eBPF and is its own project; killing or renicing processes; mouse
support; and capturing processes that live and die entirely between two samples,
where the `taskstats` exit-record path is written but cannot be verified on any
kernel available here — listener registration returns `EINVAL` while per-pid
queries work.

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
