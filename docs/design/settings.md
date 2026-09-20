# Configuration

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

## Keeping history across restarts

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

## Doing something about it

htop, btop, bottom and atop can all kill a process — htop's `--readonly` flag
exists to turn that off, btop binds `k`, bottom binds `dd` and F9 with a signal
picker, atop binds `k`. poptop could not, and the argument for leaving it that
way was a real one: **a monitor that cannot change the machine
is a monitor that cannot break it.** poptop's entire privileged surface is
otherwise reading files, and that is worth something — it can be handed to
anyone, on anything, without a thought about what a mis-key does.

So the property is kept. `x` and `X` do nothing until you say otherwise, once:

```ini
signals = on
```

The reason for building it at all is that the alternative is not "nothing
happens". It is somebody reading a pid off one screen and typing it into another
terminal, which is exactly where a pid gets mistyped — and by the time it is
typed, the number may belong to something else. poptop knows the name and the
command line, so the question names what it is about to stop:

```text
 send TERM to postgres — /usr/local/pgsql/bin/postgres -D /var/db (pid 4823)?  y to confirm, anything else cancels
```

`x` sends `TERM`, `X` sends `KILL`, and there is no third option. A picker of
thirty-one signals is a list of ways to get it wrong, and anybody who needs
`SIGUSR1` is already in a shell.

**Two rules poptop can offer and the others cannot.** Every other monitor's
process table is the present. poptop's may be four minutes old, which is a
hazard none of them have — and the identity poptop already uses everywhere,
`(pid, start time)`, is exactly what fixes it:

- **Nothing is sent while scrubbing**, and nothing at all from a day opened
  with `--read`. That table is history — in the second case somebody else's
  history — and the key says so rather than waiting for you to type `y`.
- **The pair is rechecked against the newest sample**, not against the row you
  selected. A process that has exited is named as gone; a pid the kernel has
  since handed to something else is refused *by name* — `pid 4823 is sshd now,
  not postgres — nothing was sent`.
- **And then against the kernel, at the moment of sending.** The newest sample
  can be a whole `--interval` old, and a pid can be handed on inside one. So
  the start time is read again from the kernel, not from a sample, just
  before the signal goes. On Linux 5.3 and later that read is made through a
  pidfd, and the signal is sent through the same descriptor. It names the
  process, not the number, so if the process exits in between, nothing is
  signalled. On macOS, or where `pidfd_open` is missing or filtered, the
  signal is an ordinary `kill` straight after the read. The process would
  have to exit, and its pid be given to a new one, between two consecutive
  system calls. That window is narrow, but it is not zero, and poptop does not
  claim it is.

A process whose start time the platform would not report is never signalled at
all, because without it a recycled pid cannot be told from the one you picked.
Nor is a folded row (`g`): that is several processes, and which of them to stop
is not a decision a confirmation could describe.

The prompt fits the terminal it is drawn on, giving up the command line first
and then the sentence explaining `y`, down to `y/n`. The name and the pid are
what the question *is*, and clipping took the pid — while every key was being
swallowed by a modal state with no visible way out. Only a bare `y` confirms:
`Ctrl-Y` cancels like everything else, since one accidental chord should not be
the only thing that sends a signal.

Failures are the operating system's own words. `Operation not permitted` for
somebody else's process is the answer, and dressing it up would only hide which
of the several reasons it was.
