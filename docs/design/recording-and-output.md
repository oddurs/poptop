# Recording, replay and output

## What happened while you were asleep

`atopsar` reads a logfile and prints reports; it is how atop is used from cron.
poptop could show you any instant and could not describe a stretch of them —
"what was the worst hour yesterday" is a question the buffer contains the answer
to and the interface could not ask.

```sh
poptop --report              # today
poptop --report 2026-09-08   # a recorded day
```

```text
period  03:00:00 to 10:50:00, 48 samples every 10m00s
cpu     peak 99.0% at 09:40:00 (backup)
        above 50% for 2h10m of 8h00m, worst 30m00s run 74.0% from 06:20:00 (cc1plus)
memory  peak 78.0% at 06:20:00 (cc1plus)
        above 50% for 4h40m of 8h00m, worst 30m00s run 78.0% from 06:20:00 (cc1plus)
iowait  peak 61.0% at 09:40:00
        above 50% for 10m00s of 8h00m, worst 30m00s run 21.0% from 09:20:00
```

Two things make this more than atopsar's version.

**It names what was responsible.** poptop retains whole process tables, so the
report can say *which* process owned the worst minute rather than only that the
minute was bad. atop cannot produce that from its own logs at default settings,
because its process records are per-interval. It is also why the peak and the
run above name different processes in the example: the spike was `backup`, and
the afternoon was `cc1plus`. Those are different answers to different questions
and a report that gave one of them would send you after the wrong thing.

**Peak and sustained are never merged into a mean.** A machine that hit 100% for
one second and a machine that sat at 60% for an hour both average to something
unremarkable, and only one of them was in trouble. So there are three figures,
each answering its own question: the highest it reached and when, how long it
spent above your `warn` threshold, and the worst five-minute run. A period
shorter than that window reports no run at all rather than calling its whole
length "the worst five minutes".

**The run's window is stated because it is not always five minutes.** A run
needs at least three samples to be a run rather than a pair, so on a log written
every ten minutes it widens to thirty — which is why the example above says
`worst 30m00s run`. Left at five it would have been one sample, and every report
would have printed its peak twice under a second name.

**A run never spans a hole.** Sliding a window over indices rather than over
time would assert a five-minute run whose last sample was two hours after its
first, on the same report whose second line says which stretches were not
recorded.

**Time above the threshold is counted in samples**, not by subtracting
timestamps: a machine switched off for two hours between a busy sample and the
next one did not spend those two hours busy. Where the period has holes in it
the report says so first, because every figure below is about the parts that
were watched.

A figure the platform never reported is absent from the report rather than a
line of zeroes — `stall` and `iowait` simply do not appear on macOS. A report is
read by somebody who was not there, so an invented zero costs more here than
anywhere else.

It needs no terminal, which is the point: `poptop --report | mail -s "$(hostname)
yesterday" ops@` is the whole cron job.

## Output you can build on

`--once` prints a fixed set of lines for a human who is scripting around them.
This is the other thing — a tool you can build on rather than one you watch.

```sh
poptop --export=json              # one sample, one JSON object, one line
poptop --export=line              # tab-separated, one label per table
poptop --export=json 2026-09-08   # the whole of a recorded day
poptop --schema                   # every record, field, type and unit
```

A live export collects **every** optional source, unlike the interactive view
where each is gated on a panel being open: a script asking for every metric by
name means it, and `null` because poptop chose not to ask is indistinguishable
from `null` because the kernel does not publish it. It is also safe to pipe into
`head` — a broken pipe ends the output rather than the process.

**Exit status.** Data goes to stdout and every complaint to stderr, one
`poptop:` line each. `0` is done as asked. `1` is could not: a recorded day
that cannot be read, a failure reading the machine, or a `--check-theme`
verdict other than PASS. `2` is would not: a command line or setting that
cannot be run as written, no state directory for a command that needs one, or
the interactive monitor started without a terminal. `tests/cli.rs` runs the
built binary and holds each of these to it.

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
the kernel had in a process name; nor does a line ending.

**Every row under a label has exactly the columns its header names.** That is
the property the format lives or dies by, and it is the one thing a record
*inside* another can break: a sub-record that is present opens a table of its
own and contributes no column to the row above it, so writing a `-` for the
absent case made rows disagree with their header and every field after the gap
read as its neighbour. A process with no `io` is simply a process with no row
under `sample.procs.io` — which is where a reader looks for it. JSON keeps the
`null`, because JSON has room to.

The header block is written **once per stream**, not once per sample, so a
whole day is one column map and a hundred and forty-four rows.

Two encodings worth stating, because a line format has no types: a boolean is
`1` or `0`, and a record whose fields are all sub-records — `pressure` — has no
row of its own, only its children's.

**Floats carry no more digits than they were measured with.** Every percentage
poptop reports is an `f32`; widening one by a cast gives `51.70830535888672`,
ten digits of arithmetic nobody measured.

**The line format's grammar**, which `tests/export.rs` reads every export
back by:

```text
stream  = { header | row }
header  = "#" label { TAB column } NL     once per label, before its first row
row     = label { TAB value } NL          exactly as many values as its header
label   = "sample" { "." field }
value   = "-" | number | "1" | "0" | text
```

`-` is absent. A boolean is `1` or `0`. Text holds no TAB, CR or LF: each is
written as a space. Every sample's own `sample` row comes after the rows of
the records inside it, so a reader can collect rows until it sees one. The
format is lossy for text in exactly those three characters, and in one more
case: an optional text field whose value is exactly `-` reads as absent. When
text has to survive intact, use JSON. The test reads the line export of a
recorded day and checks each value against the JSON export of the same day.

**Stability.** The names are the store's field names, and the store's format is
already versioned: a file written by another version is discarded rather than
guessed at. The same promise applies here — within a version the names, units
and shapes do not change, and `--schema` carries the version so a consumer can
check rather than assume.

Across versions, what a consumer can rely on:

- **Compatible:** a new field or a new record. A consumer that ignores names it
  does not know keeps working. In the line format a new field is a new column,
  so a consumer should find columns by the header's names, not by position.
- **Breaking:** a field removed or renamed, its type or unit changed, a list
  becoming a record or the reverse, or a field that was never `null` becoming
  one in JSON. These happen deliberately, in a version that says so.

The schema is checked into the repository as `tests/golden/schema.json`, and a
test fails if `--schema` differs from it. Changing the schema means
regenerating that file, so every change is a diff someone reviews against the
list above. Every exported JSON sample in the tests, live and recorded, is
validated against the schema the same binary prints: every field present,
none extra, each of its declared type, and `null` only where it is optional.

## Opening yesterday

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

### What it costs, measured

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

### Retention

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
