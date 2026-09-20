# Troubleshooting

Every message poptop prints about something it could not read or could not
do, and what to do about it.

poptop's rule is that it says what it does not know rather than filling the
gap with a zero. Most of what follows is not a fault.

## Marks in the table

| Mark | Meaning |
|---|---|
| `—` | poptop could not read this figure |
| `·` | poptop read it and there is nothing to report |
| `?` | a state it could not determine |

A row of `—` is a process poptop is not allowed to read — on macOS that is
most of the system daemons, including pid 1. The row stays in the table so
the tree is not orphaned.

## In the panel rule

**`! io: 183/732 need root`**
: 183 of those processes belong to other users. `/proc/<pid>/io` needs
  `CAP_SYS_PTRACE` on Linux; macOS needs root. Their `DISK R`/`DISK W`
  cells show `—`. Run poptop as root, or grant the capability, if you need
  them. On a box running its services as root, poptop withdraws the columns
  entirely after one sample rather than drawing a wall of dashes; `i` puts
  them back.

**`! io: panel too narrow`**
: not a permission problem. The terminal is too narrow for the IO columns
  and they were given up. Widen it, or press `i`.

**`· io: not collected here`**
: the platform does not publish per-process IO at all.

**`· threads: not read on this platform`**
: `y` cannot expand a process into thread rows here — macOS publishes the
  count but not the threads. The `THR` column still has the number.

**`cgroups — not collected here`**
: `C` pressed on a machine with no cgroup v2. macOS has none; a Linux box
  with cgroup v1 only will say the same.

**`· 101 git (g folds them)`**
: not a problem. poptop noticing that 101 rows share a name and offering
  `g`. See [groups](groups.md).

**`· memory is the constraint (S)`**
: also not a problem. poptop's opinion about what is stopping work. `S`
  sorts by it; nothing happens unless you press it.

**`processes (0)`, with nothing under it**
: the filter matched nothing, or you grouped by container on a machine with
  none. poptop does not currently say which —
  [known](../reference/views.md#modes).

## On the timeline

**`no history before 20:24:41 — it fills from the right`**
: poptop started at 20:24:41 and the buffer is not full yet. Use
  `--store=on` to keep it across restarts, or `--log=on` to keep it beyond
  the process. See [recording](recording.md).

**`nothing recorded at -2h — nearest sample is 1h59m away`**
: `b` landed somewhere with no sample. This is deliberate: poptop will not
  show you the nearest sample as though it were the moment you asked for.

**`┊ not running`** in the detail footer
: the selected process did not exist for part of the window. The gap is
  marked rather than interpolated.

## In the header

**`steal — not published here`**, **`oomkill — not published here`**
: the machine does not publish this figure. Not a zero.

**A figure that is simply absent**
: `CLK`, `STALL`, `WAIT`, `BLOCKED` and the rest appear only when they have
  something to say. A quiet header is a quiet machine.

## Starting up

**`poptop: the monitor needs a terminal on stdin and stdout; for a script,
`--once` prints a sample and `--export=json` every metric`**
: you piped poptop, or ran it from cron. Use `--once` or `--export`.
  Exit 2.

**`poptop: --read needs a date, as YYYY-MM-DD. `poptop --days` lists
them`**
: `--read` with no argument. Exit 2.

**`poptop: `2026-02-30` is not a date`**, **`poptop: `08/09/2026` is not a
date`**, **`poptop: `today` is not a date`**
: dates are `YYYY-MM-DD`, and must be on the calendar.

**`poptop: `warn` is 90 and `critical` is 50; warn must be below
critical`**
: the thresholds crossed over. Exit 2.

**`poptop: unrecognised option '--nonsense'`**
: followed by the usage. Exit 2.

**`poptop: --once does not take `--report``**
: one command word at a time.

**`poptop: /tmp/ptc/poptop/poptop.conf:1: unknown key `thme` (did you mean
`theme`?)`**
: a typo in the config file. poptop names the file, the line and the
  nearest real key, and **starts anyway**. One typo should not cost you the
  tool. The warning is printed after the alternate screen closes, so look
  for it when you quit.

**`poptop: no theme `nope`: not a built-in, and
~/.config/poptop/themes/nope.theme does not exist`**
: check `--check-theme` and [themes](../reference/themes.md).

## Reading a recorded day

**`poptop-20260919: 3 entries could not be read, most likely written by a
different version`**
: the log is append-only and self-describing; entries it does not recognise
  are skipped and counted rather than stopping the read.

**A day that ends early**
: the last entry was torn — the machine went down mid-write. That entry is
  lost and nothing before it is.

## It is using too much memory

Every sample keeps a whole process table, so `--window` × `--interval` is
the cost:

```console
$ poptop --window=2m --interval=2s
```

`--log-bytes` bounds the log on disk (default `512M`), and `--log-days`
(default `7`) is the one to reason about.

## Colours are wrong, or unreadable

```console
$ poptop --color=mono      # no colour at all
$ poptop --color=256       # force the tier
$ poptop --theme=classic   # green/yellow/red instead of the safe default
$ poptop --check-theme safe
```

`NO_COLOR` is honoured. The default theme replaces green with cyan because
green and yellow separate by only ΔE 3.7 under simulated protanopia. See
[themes](../reference/themes.md).

## The drawing looks wrong

```console
$ poptop --glyphs=block    # if braille renders as boxes
$ poptop --glyphs=ascii    # last resort; automatic on a Linux console
```

## Exit statuses

| | |
|---|---|
| `0` | done as asked, including `q`, `Ctrl-C`, `SIGTERM`, `SIGHUP`, and the terminal going away |
| `1` | could not: a recorded day that cannot be read, a failure reading the machine, a `--check-theme` verdict other than PASS |
| `2` | would not: a command line or setting that cannot be run as written, no state directory for a command that needs one, or the interactive monitor without a terminal |

## Something else

Open an issue with `poptop --once` output and your platform:
<https://github.com/oddurs/poptop/issues>.
