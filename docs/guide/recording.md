# Recording and replay

poptop needs nothing running before you notice the problem — it starts with
an empty buffer and fills it. But if it *was* running, you can ask it about
yesterday.

There are three levels, and they are independent.

| | What it keeps | Survives |
|---|---|---|
| default | `--window` of samples in memory | nothing |
| `--store=on` | the same buffer, written out on exit | a restart |
| `--log=on` | a sample every `--log-interval`, one file a day | the process |

## The in-memory buffer

```console
$ poptop --window=1h --interval=2s
```

`--window` is time, not a number of samples, so it means the same thing at
any interval. The default is ten minutes at one second.

Every sample keeps a whole process table, so those two flags together are
what decide the memory. A machine with 700 processes at one second and ten
minutes is the default because it is a few hundred megabytes, not because
ten minutes is a natural length of time.

## `--store=on` — keep it across restarts

```console
$ poptop --store=on
```

Written on a clean exit to `$XDG_STATE_HOME/poptop/history` (on macOS,
`~/.local/state/poptop/history` unless you set it), read back at startup.
The window you restart with is the window you had.

A kill -9 loses it, which is the trade: one file written once is cheaper
than a file written every second, and the buffer is not the record. For a
record, use the log.

## `--log=on` — outlive the process

```console
$ poptop --log=on
```

One file a day in `$XDG_STATE_HOME/poptop/log`:

```
$XDG_STATE_HOME/poptop/log/poptop-20260919
```

Nothing poptop draws depends on the log existing. It logs if it is left
running and works if it was not.

| Flag | Default | What it bounds |
|---|---|---|
| `--log-interval=SPAN` | `10m` | how often a sample reaches the log — *not* the sample interval, which stays at `--interval` |
| `--log-days=N` | `7` | days kept |
| `--log-bytes=SIZE` | `512M` | bytes kept across every day |

`--log-bytes` is the bound that actually holds, because a sample carries a
whole process table and a busy machine's day is much larger than a quiet
one's. `--log-days` is the one you reason about.

A typical daemon line:

```console
$ poptop --log=on --log-interval=1m --log-days=30 --log-bytes=2G --store=on
```

### What a crash costs

**An entry is on the disk before `append` returns.** poptop writes each entry
in one call and then asks the kernel to persist it, so a machine that loses
power keeps every entry written before the moment it went down. The case a
log is most wanted for is the machine that went down, and a log that ended
thirty seconds before the event would be worth little.

That costs 4.2 ms an entry on APFS and 2.8 ms on ext4, against 0.09 ms for
the write alone. At the default ten-minute interval it is nothing; at one
second it is under half a percent of the interval. `./check --perf` holds it
to a budget.

Two things it does not promise. On macOS the sync asks the drive to persist
and does not force the drive's own write cache — `F_FULLFSYNC` does, at
roughly ten times the cost, which is not a trade a monitor should make for
you. And the **restart store** is different by design: it is written once, on
a clean exit, so `kill -9` costs at most the session's buffer. The log is the
thing that outlives the process.

### What it costs

Fifteen seconds at one sample a second, on a machine with ~740 processes:

```console
$ poptop --days
2026-09-19  1.3M
```

That is roughly 90 KB per sample uncompressed, and it is almost all process
table. At the default `--log-interval=10m` a day is about 13 MB.

## Reading it back

**`--days`** — what exists and what it costs.

```console
$ poptop --days
2026-09-19  1.3M
```

**`--read DATE`** — the interactive view, on a recorded day.

```console
$ poptop --read 2026-09-19
```

The same keys, the same panels, the same scrubbing — `b` jumps within that
day, and a relative jump like `-2h` is measured from the end of *that day*,
not from now. Signals are refused: those rows are last week's and their
pids belong to other processes.

**`--report [DATE]`** — the summary.

```console
$ poptop --report
poptop report for 2026-09-19
period  20:32:01 to 20:32:16, 13 samples every 1s
cpu     peak 100.0% at 20:32:07 (rustc)
        above 50% for 5s of 19s (rustc)
memory  peak 77.8% at 20:32:02 (node)
        above 50% for 19s of 19s (poptop)
```

Peak and sustained, each with the process responsible at that moment. The
threshold is `--warn`. This is the thing to run the morning after.

**`--export=json|line [DATE]`** — everything, by name.

```console
$ poptop --export=line 2026-09-19 | head -4
#sample.cpu_per_core	0	1	2	3	4	5	6	7	8	9
sample.cpu_per_core	13.461539	9.433963	9.615385	3.7735848	3.6363635	...
#sample.mem	total	used	available	free	swap_total	swap_used	dirty	slab	...
sample.mem	17179869184	13348798464	10410377216	-	7516192768	5997723648	-	...
```

A `-` is a figure the platform does not publish, never a zero. The full
format, and `--schema`, are in [output](../reference/output.md).

### A live feed

`--follow` keeps the export going instead of printing once:

```console
$ poptop --export=json --follow --interval=2s | jq -c '{at, cpu: .cpu_total}'
{"at":1758320461.2,"cpu":18.4}
{"at":1758320463.2,"cpu":22.1}
```

One record per `--interval`, written and flushed as it is taken, so a
consumer reading line by line gets each sample when it happens rather than
8 KB at a time. The first record arrives one interval in: every rate is a
difference between two readings, and there is nothing to difference the
first against.

It stops when you stop it — `SIGTERM`, `SIGHUP`, `--for 10m`, or the reader
going away. `poptop --export=json --follow | head -3` exits 0 like any other
piped output, and so does a feed whose consumer crashed.

**The header rule.** Under `--follow` the line format writes its header block
**once, at the top of the stream** — the same rule as a recorded day, so the
two are the same format and not two dialects. A label that first appears an
hour in brings its own header line then. What this costs: a reader that
attaches to a feed already running, by `tail -f` on a redirect, sees rows
with no header to map them by. Read from the start, keep the header from
the first run, or use `--export=json`, whose records name every field.

It does not write the log — `--log=on` does that, and a feed is a reader.

### Following the log itself

With a date, `--follow` tails that day's file instead of sampling:

```console
$ poptop --export=json 2026-09-19 --follow
```

The day so far, then each entry as it is appended — by whatever poptop is
running with `--log=on`, which need not be the one you are watching it from.
That is the subscription: one process records, any number read.

An entry half written when the follower reaches it is waited for, not called
damage. A day an earlier crash tore reads the way `--export DATE` reads it,
with the same line on stderr. Following today moves to tomorrow's file when
the writer starts one; following a day that is over stays there.

## What the log does about bad days

The log is append-only and self-describing, and it is read defensively
because a machine that crashed mid-write is exactly the machine you want
the log from:

- A torn final entry — the machine went down while writing — costs that
  entry and nothing after it in the day.
- An entry from a different version is skipped, with a count:
  `poptop-20260919: 3 entries could not be read, most likely written by a
  different version`.
- A day file that is not a regular file is refused rather than read.

## Where the files are

Everything honours `$XDG_STATE_HOME`, defaulting to `~/.local/state`:

```
$XDG_STATE_HOME/poptop/history          the --store buffer
$XDG_STATE_HOME/poptop/log/poptop-YYYYMMDD   one day of log
```

Config and themes are elsewhere, under `$XDG_CONFIG_HOME` — see
[configuration](../reference/configuration.md).
