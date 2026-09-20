# Signals

poptop reads files and nothing else until you say otherwise. A monitor that
cannot change the machine is a monitor that cannot break it, so `x` and `X`
do nothing on a default install:

```
 signals are off — `signals = on` in the config, or --signals=on
```

Turn them on and they send TERM (`x`) and KILL (`X`) to the selected
process.

```console
$ poptop --signals=on
```

or, permanently, in `~/.config/poptop/poptop.conf`:

```
signals = on
```

## It asks first, and it names what it is about to stop

```
 send TERM to node — --experimental-modules main (pid 53858)?  y to confirm, anything else cancels
```

The pid is the part that gets misread, and poptop knows the name and the
command line, so it says them. At narrower widths it drops the command
line, then falls back to `send TERM to node (pid 53858)? y/n`. Anything but
`y` cancels, and says so.

## The two rules other monitors cannot offer

Both exist because poptop's process table may be four minutes old, and
every other monitor's is always the present.

**Nothing is sent while scrubbing.**

```
 not while scrubbing — this table is history, and the pid may since have been reused. Space or End to go live
```

**The identity is `(pid, start time)`, not the pid.**

poptop rechecks the pair against the newest sample when you press `y`, and
then against the kernel as the signal is sent — through a pidfd on Linux
5.3 and later. If the pid has been handed to something else since you
selected the row:

```
 pid 53858 is postgres now, not node — nothing was sent
```

This is the failure mode of reading a pid off one screen and typing it into
`kill` on another, and it is the one thing poptop is in a position to stop.

## Every refusal, and what it means

| Message | What happened |
|---|---|
| `signals are off — ...` | `--signals=on` not given |
| `not while scrubbing — ...` | the cursor is not on the live sample; `Space` or `End` |
| `not in a recorded day — ...` | you are in `--read`; those rows are last week's |
| `nothing selected — the arrow keys pick a process` | no row is selected |
| `that row is several processes — \`g\` again to unfold them` | the selection is a folded group |
| `<name> (pid N) is no longer running` | it exited between the prompt and the `y` |
| `pid N is <other> now, not <name> — nothing was sent` | the pid was recycled |
| `this platform did not say when <name> started, ...` | no start time, so the pid cannot be told apart from a later one |
| `pid N is not one process — nothing was sent` | a pid that is not a single process |
| `could not signal <name> (pid N): <error>` | the kernel refused — usually permission |
| `nothing sent to <name>` | you answered anything but `y` |
| `sent TERM to <name> (pid N)` | it went |

Every reason poptop would refuse is checked before the question as well as
after it, so you are told before typing `y` rather than after. A prompt
that can only be answered "no" is worse than the key saying why.

## What poptop will not do

- No `HUP`, `USR1`, or arbitrary signal numbers. TERM and KILL are the two
  anyone opens a monitor to send.
- No signalling a folded row. `g` folds several processes into one, and
  "the one under the cursor" would mean picking one of them, which is not a
  decision a confirmation could describe.
- No killing by pid typed in by hand. The row is the selection.
- No renice, no cgroup writes, no configuration of the machine.
