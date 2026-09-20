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
| `glyphs` | `braille`, `block`, `ascii` | `braille` | How the timeline is drawn. Falls back to `ascii` on a Linux console automatically. |
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
| `log-bytes` | size | `512M` | Bytes of log kept across every day. |
| `signals` | `on`, `off` | `off` | Whether `x` and `X` may signal. See [signals](../guide/signals.md). |

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
