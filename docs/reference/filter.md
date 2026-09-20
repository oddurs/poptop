# Filter

`/` opens the filter. It applies as you type, to the sample under the
cursor — so with the timeline scrubbed back four minutes, it answers about
that moment. `Esc` or `Enter` leaves the box keeping what you typed; `/`
then `Enter` clears it.

## A bare word

A word with no operator is a substring match, case-insensitive, against the
process name, its command line, its user and its pid. `/ruby` finds every
ruby, and `/4821` finds that pid.

## A query

| Field | Matches |
|---|---|
| `cpu` | CPU percentage |
| `mem`, `rss` | resident memory |
| `threads`, `thr` | thread count |
| `state` | process state letter |
| `task` | the state of *any thread* of the process |
| `container`, `cid` | container id |
| `pid` | process id |
| `read`, `write` | disk throughput |
| `user` | owner |
| `name`, `command` | name or command line |

Operators are `>`, `>=`, `<`, `<=`, `=`, `!=`, joined by `and`. There is no
`or` and no regular expressions: `field op value` joined by `and` answers the
questions the filter exists for, and the line is drawn deliberately.

Sizes take `k`, `m`, `g`, `t`, and are binary like the columns: `1mb` is
1048576. Text comparisons take only `=` and `!=`.

```text
state = D              everything stuck in uninterruptible sleep
write > 1mb            who is causing the disk saturation
threads > 100          the thing leaking threads
cpu > 5 and user = root
task = D               a process with any thread stuck, which the header counts
```

## What it refuses to do

**A figure that could not be read matches nothing.** Not `> 0`, and not
`< 1mb` either. A process whose IO is unreadable is not a process doing no
IO, so it does not answer a question about its IO in either direction.

**A malformed query filters nothing away, and says what is wrong.** The
error names the part it could not read, and the panel title carries it so
the table is never silently filtering on a query you have not finished
typing.

**A word that is not a field is still a word.** `chrome --type=renderer`
contains an `=` and is matched as text, because command lines are full of
them. A near miss for a real field name — `cpuu`, `memm` — gets an error
naming the fields instead, since otherwise there is no way to learn them.
