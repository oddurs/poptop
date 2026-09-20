# Scrubbing to a moment

This is the thing poptop exists for. Everything on screen — the header, the
graphs, the whole process table — is drawn from one sample, and the arrow
keys move which one.

## Move

| | |
|---|---|
| `←` `→` | one sample back or forward |
| `Shift-←` `Shift-→` | ten at a time |
| `Home` | the oldest sample retained |
| `End` | back to live |
| `Space` | stop here, or resume |
| `b` | jump to a named moment |

The header says where you are:

```
 PAUSED  -3s CPU  11.3%  │  MEM  77.5% █████████░░░  SWP  81.2%  │  / 97.9% full  │  en0 ↓9.6K/s ↑3.8K/s
```

`LIVE` becomes `PAUSED  -3s`, and the timeline's right-hand label changes
from `now` to a cursor:

```
past                                           9s shown, 1s/slot                                           ▐
```

The `▐` is which sample you are on, down to which half of a braille cell.

**Sampling does not stop.** poptop keeps taking samples while you are four
minutes back; `End` returns to a present that has moved on. This is the
difference between scrubbing and freezing.

## Jump to a moment

`b` takes a distance or a time, as atop's `-b` does:

```
jump to: -2h█   -2h · 03:00 · 2026-09-08 03:00   (Enter to jump, Esc to cancel)
```

- `-2h`, `-90s`, `-15m` — a distance back from the end of what is retained.
  In a day opened with `--read` that end is not today.
- `03:00` — a time today, local.
- `2026-09-08 03:00` — a date and time.

If nothing was recorded there, poptop says so instead of showing you the
nearest sample as though it were the one you asked for:

```
 nothing recorded at -2h — nearest sample is 1h59m away
```

That message is the whole point. A monitor that silently rounds your
question to the nearest thing it has is a monitor that will one day let you
diagnose the wrong minute.

## Zoom

`+` and `-` change how much time is on screen, and the timeline's footer
always says what one column is worth:

```
past                                           9s shown, 1s/slot                                           ▐
past                                          13s shown, 1s/slot                                         ▐ now
```

Zooming out aggregates; zooming in does not interpolate. The scale in the
gutter (`100`, `50`, `25`) refits to the data on screen, so a window in
which nothing exceeded 25% is drawn at full height against 25, not
flattened against 100.

## What still works while scrubbed

All of it. `/`, `s`, `S`, `g`, `t`, `d`, `v`, `i`, `y` all answer about the
sample under the cursor. You can open a filter on a moment four minutes ago
and the count in the panel title is that moment's count.

The one exception is signals: `x` and `X` do nothing while scrubbing. That
process table is history, and the pid in it may belong to something else
now. See [signals](signals.md).

## Working out when it started

The `HIST` column is a per-process sparkline over the same window, so a
scrubbed table carries its own recent past with it:

```
   CPU%            RSS      S   THR HIST ≤50%      PID USER       COMMAND
   18.5 ▊        14.3M      S    12 ⠀⠀⠀⠀⠀⢀⣀⣀⣀⣀   86077 oddurs     rsst
    9.9 ▍        48.3M      S    11 ⠀⠀⠀⠀⠀⢀⣀⣀⣀⣄   38359 oddurs     ghostty
```

The leading blanks are not zeroes — they are before the process existed, or
before poptop started. For one process at full width, `d` gives it the
whole timeline: see [one process in depth](one-process.md).

## Keeping more than ten minutes

The default window is ten minutes, because every sample keeps a whole
process table and that is what costs memory.

```console
$ poptop --window=1h --interval=2s
```

Those two together decide the cost. To keep history across restarts, or
past the life of the process entirely, see [recording](recording.md).
