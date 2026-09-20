# Configuration

Everything poptop can be told is a flag, and every flag is also a config
file key with the same name and no leading dashes.

**Lowest precedence first: the built-in default, the config file,
`NO_COLOR`, then the flag.** A wrapper script can override a user's file
without editing it, and `NO_COLOR` outranks the file because the file
records a preference in general while the environment is about this
terminal now.

## The file

`~/.config/poptop/poptop.conf`, honouring `$XDG_CONFIG_HOME`.

```conf
theme    = classic
glyphs   = block    # comments run to the end of the line
color    = 256
warn     = 65       # a build box is busy at 50% and fine
critical = 90
interval = 500ms    # every sample keeps a whole process table,
window   = 30m      # so these two together decide the memory
store    = off      # keep history across restarts
```

An unknown key warns, naming the key and the line, and poptop starts
anyway: one typo should not cost you the tool. A bad *value* warns the same
way and that key alone falls back to its default. The same mistake in a
flag is fatal, because a flag is this run and you are there to read the
error.

## Keys

`key.<action> = <keys>` moves a key. The actions, the key names and what
happens when two actions want one key are in [keys](keys.md); `poptop
--keys` prints the resolved map with where each binding came from.

## Seeing what a setting did

A config file is read once, at startup, above a monitor that then erases
whatever it said. `--config` answers "did my line take effect?" without
watching the graphs: every setting, its value, and where that value came
from — the default, the file and its line, `NO_COLOR`, or the flag.

```console
$ poptop --config --window=30m
glyphs        braille  the default
color         256      NO_COLOR
interval      2s       /home/you/.config/poptop/poptop.conf:2
window        30m      --window=30m
```

`--write-config` writes a commented file of the current settings to the
path above, one comment per setting saying what it is for. It refuses to
overwrite a file that is already there, so it is a starting point rather
than a reset. `color` is written as `auto` unless something asked for a
tier: writing the tier of the terminal it happened to run in would pin it
for every terminal that later reads the file.

## Every setting

| Key / flag | Values | Default | What it does |
|---|---|---|---|
| `graph` | `braille`, `block`, `ascii`, `line` | `braille` | How the timeline is drawn. `line` traces the samples instead of filling under them. Falls back to `ascii` on a Linux console automatically. |
| `glyphs` | as `graph` | — | The older name for `graph`, from when the sets differed only in alphabet. Still read, so existing config files and `--help` examples keep working. |
| `scale` | `zero`, `fit` | `zero` | Where the vertical axis starts. `fit` crops it to the data, which shows small movement and costs the comparison between one graph and the next. |
| `density` | `compact`, `comfortable`, `spacious` | `comfortable` | How much air the layout takes: the margin either side of the content, the gap between columns, and whether panels are separated by a blank row. |
| `mouse` | `on`, `off` | `on` | Whether poptop takes the mouse. While it has it, dragging selects time on the timeline rather than text — hold Shift for the terminal's own selection. |
| `surface` | `auto`, `off` | `auto` | Whether the interface paints its own grounds. `auto` asks the terminal for its background first and builds the layers from it; `off` leaves every ground to the terminal. |
| `smooth` | span | `0s` | How long each of the table's figures is averaged over, weighted towards now — the newest sample is about a third of the figure and the oldest about a tenth. The rows are *ordered* by the same average taken on a beat, so the figures stay live while the order holds still. Off by default: the figures are the sample's own. |
| `color` | `auto`, `mono`, `16`, `256`, `true` | `auto` | Colour tier. `NO_COLOR` forces `mono`. |
| `theme` | `safe`, `classic`, or a file name | `safe` | See [themes](themes.md). |
| `warn` | percentage | `50` | Where "getting busy" begins. |
| `critical` | percentage | `80` | Where "in trouble" begins. Must exceed `warn`. |
| `interval` | span | `1s` | Time between samples. The floor on macOS is 200 ms; below it sysinfo's figures are wrong rather than noisy. |
| `window` | span | `10m` | History retained, as time rather than samples. |
| `store` | `on`, `off` | `off` | Keep history across restarts. See [recording](../guide/recording.md). |
| `log` | `on`, `off` | `off` | Write a daily log that outlives the process. |
| `log-interval` | span | `10m` | How often a sample reaches the log. Not the sample interval. |
| `log-days` | number | `7` | Days of log kept. |
| `log-bytes` | size | `512M` | Bytes of log kept across every day. At the budget the oldest history is dropped, not the newest refused. |
| `signals` | `on`, `off` | `off` | Whether `x` and `X` may signal. See [signals](../guide/signals.md). |
| `view` | `generic`, `memory`, `disk` | `generic` | The view to start in, which `v` cycles. |
| `sort` | `cpu`, `mem`, `disk`, `pid`, `name` | `cpu` | The column to sort by, which `s` cycles. A sort the starting view cannot show warns and falls back to that view's own. |
| `zoom` | `1`, `2`, `4`, `8` | `1` | Samples per timeline slot to start at, which `+`/`-` change. |
| `tree` | `on`, `off` | `off` | Start with the process tree (`t`). Not with `group`. |
| `group` | `off`, `name`, `user`, `container` | `off` | Start with rows folded (`g`). Not with `tree`. |
| `kernel-threads` | `on`, `off` | `off` | Show kernel threads (`K`). Linux only; macOS has none. |
| `io-columns` | `on`, `off` | `on` | Show the per-process disk columns (`i`), where they can be read. |
| `hide-columns` | column names | none | Columns never to draw: `bars`, `rss`, `state`, `thr`, `io`, `mem`, `hist`, `pid`, `user`, `cid`. The width they would have taken goes to the command. |

Booleans also take `true`/`false` and `yes`/`no`. Spans take `ms`, `s`, `m`,
`h`; sizes take `k`, `m`, `g`, `t` and are binary.

## The two settings that decide memory

`interval` and `window` together decide how much poptop holds, because every
sample keeps a whole process table. Ten minutes at one second is 600 process
tables. The README's measurements are in
[docs/design/collection-cost.md](../design/collection-cost.md); as a rule,
halving the interval or doubling the window doubles the memory.

## Where poptop puts things

| Path | What |
|---|---|
| `$XDG_CONFIG_HOME/poptop/poptop.conf` | settings (`~/.config/poptop/poptop.conf`) |
| `$XDG_CONFIG_HOME/poptop/themes/NAME.theme` | themes |
| `$XDG_STATE_HOME/poptop/history` | the restart store, written on a clean exit (`~/.local/state/poptop/history`) |
| `$XDG_STATE_HOME/poptop/log/poptop-YYYYMMDD` | one file a day, while `log = on` |

Nothing is written unless you ask for it: without `store` and `log`, poptop
reads and draws and leaves no trace.
