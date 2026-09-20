# Output for scripts and reports

poptop is a terminal program, and four flags let something else read what it
sees. All of them work whether or not poptop has ever been left running:
`--once`, `--export` and `--schema` describe the machine now, while
`--report`, `--days` and `--read` need [a recording](../guide/recording.md).

## `--once`: one sample, as text

```console
$ poptop --once
cpu     22.6%  (10 cores)
fs      97.8%  / full, 5.1G of 228.3G available
mem     81.5%  13.0G / 16.0G used, 7.0G available
swap    80.6%  4.0G / 5.0G
load    2.11 2.33 2.38
steal   —  not published here
...
procs   758
```

Every figure poptop has, one per line, with `—` and a reason where the
platform does not publish it. Safe to pipe into `head`: a closed pipe ends
it quietly rather than panicking.

## `--export`: every metric, by name

`--export=json` writes one JSON object per sample, one per line:

```console
$ poptop --export=json | head -1
{"at":1789863678.193517,"cpu_total":13.110904,"cpu_per_core":[18.44,13.59,...],"iowait":null,...}
```

`--export=line` writes a tab-separated line format, with a `#` header line
naming the fields of each record before its first row:

```console
$ poptop --export=line
#sample.cpu_per_core	0	1	2	3	4	5	6	7	8	9
sample.cpu_per_core	69	57	47.058823	35.294117	29.807693	...
#sample.mem	total	used	available	free	swap_total	swap_used	dirty	...
sample.mem	17179869184	14027816960	7532593152	-	5368709120	4326555648	-	...
```

A missing figure is `null` in JSON and `-` in the line format — never a
zero, for the same reason the table draws `—`.

With a date, `--export=json 2026-09-08` writes the whole of that recorded
day instead of the machine now.

### `--follow`: a feed rather than a snapshot

```console
$ poptop --export=json --follow --for 10m >feed.jsonl
```

One record per `--interval`, each written and flushed as it is taken. The
first arrives one interval in, since a rate needs two readings. The feed
ends on `SIGTERM`, on `SIGHUP`, when `--for SPAN` has passed, or when the
reader goes away — a closed pipe exits 0, as `--once` does.

The line format writes its header block **once, at the top of the stream**,
which is the same rule a recorded day follows; a label appearing for the
first time later brings its own header then. A reader that attaches to a
feed that is already running therefore has no header to map the columns by:
read from the start, or use `--export=json`.

`--for` without `--follow` is refused rather than ignored.

### `--follow` with a date: a day log as it is written

```console
$ poptop --export=json 2026-09-19 --follow | jq -c '{at, cpu: .cpu_total}'
```

What the day already holds, oldest first, and then each entry as it is
appended — by a poptop running with `--log=on`, which may be this one or
another process entirely. Nothing is read twice, nothing is skipped, and an
entry that is half written when the follower reaches it is waited for rather
than reported as damage: a length running past the end of a growing file is
a writer partway through, not a torn entry.

A day a crash tore earlier reads exactly as `--export=json DATE` reads it —
the entry that was cut short is skipped, with a line on stderr saying so, and
the entries either side of it are read.

Following the day that is happening now moves to the next day's file when
the writer starts one, so a feed left running overnight keeps going. Following
a day that is over stays on that day: it was asked for.

Notes about the file go to stderr, never into the feed, so a consumer
parsing records never has to parse prose.

## `--schema`: what the export contains

```console
$ poptop --schema | head -8
{
  "version": "0.1.0",
  "records": {
    "MemStat": [
      {"name": "total", "type": "integer", "unit": "bytes", "optional": false},
      {"name": "used", "type": "integer", "unit": "bytes", "optional": false},
```

Every record, field, type and unit that `--export` can produce. It is
generated from the same definitions the exporter uses, so it cannot drift —
a test checks every exported sample against it.

## `--report`: what a day was like

```console
$ poptop --report
poptop report for 2026-09-19
period  20:32:01 to 20:32:16, 13 samples every 1s
cpu     peak 100.0% at 20:32:07 (rustc)
        above 50% for 5s of 19s (rustc)
memory  peak 77.8% at 20:32:02 (node)
        above 50% for 19s of 19s (poptop)
```

Peak and sustained, each with what was responsible — the process named
beside a figure is the one holding it up at that moment. Today unless you
name a date.

## `--days` and `--read`

```console
$ poptop --days
2026-09-19  1.0M  from 00:00

$ poptop --read 2026-09-19
```

`--days` lists what has been recorded, what it costs, and when each day's
earliest surviving sample was taken — a day whose morning was dropped to stay
inside `log-bytes` starts later than midnight, and the size alone would not
say so. `--read` opens a
day in the interactive view: the same keys, the same panels, scrubbing
through a recorded day instead of the live buffer.

## Exit status

`0` on a clean exit, including `q`, `Ctrl-C`, `SIGTERM`, `SIGHUP` and the
terminal going away. `1` for something that failed while running, `2` for a
command line poptop could not read — which is also when it prints the usage.
