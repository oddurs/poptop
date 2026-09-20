# Keys

Every key poptop takes. The same list is on screen under `?`, and in
`poptop --help`; a test holds all three to each other, so if one of them is
wrong here, the build fails.

| Key | What it does |
|---|---|
| `q` | quit |
| `Esc` | back out one level: a box, a signal, the selection — then quit |
| `←/→` | scrub through history, ten at a time with Shift |
| `b` | jump to a moment: -2h, 03:00, 2026-09-08 03:00 |
| `+/-` | zoom the timeline in and out |
| `Space` | pause on this sample, or go back to live |
| `Home, End` | the oldest sample, or live |
| `↑/↓` | select a process |
| `s` | cycle the sort column |
| `S` | sort by what the panel names as the constraint |
| `/` | filter: a word, or a query like `cpu > 5 and user = root` |
| `x, X` | send TERM or KILL to the selected process (--signals=on) |
| `t` | the process tree |
| `g` | fold processes by name, then by user, then by container |
| `d` | the selected process's own history in place of the machine's |
| `y` | the selected process's threads |
| `v` | the next tab: memory, then disk |
| `C` | cgroups in place of processes |
| `K` | kernel threads |
| `Tab` | the next tab, Shift-Tab the previous, 1-9 one by number |
| `Enter` | the inspector on the selected process |
| `F10` | the menu bar, which names every command there is |
| `?` | this list |

`k` and `j` also move the selection, and `h` and `l` also scrub.

## Things worth knowing

**`Esc` backs out, it does not always quit.** With the filter box open it
leaves the box; at a signal prompt it cancels the signal; with a process
selected it lets the process go. Only with none of those does it quit. `q`
quits from anywhere, and so does `Ctrl-C`.

**The footer shows what fits.** At 120 columns about a dozen hints fit, and
the rest are dropped from the least useful end. When any were dropped the
footer ends with `? more`.

**Keys act on the sample under the cursor.** If you have scrubbed back four
minutes, `d`, `/`, `s` and the rest all answer about that moment, not about
now. Sampling continues behind you either way; `Space` or `End` returns to
live.

**Signals are off until you ask.** `x` and `X` do nothing without
`--signals=on`, and the key says so when you press it. See
[the signals guide](../guide/signals.md).
