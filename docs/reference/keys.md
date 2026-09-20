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
| `v` | the next view: memory, then disk |
| `C` | cgroups in place of processes |
| `K` | kernel threads |
| `i` | the disk IO columns |
| `R` | read the theme file again, for trying a colour |
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

## Moving them

Every row above is an action with a name, and a config file can give it
different keys:

```conf
key.quit           = q, Q
key.filter         = /, f
key.kernel-threads = ctrl-k
```

A key is a character (`q`, `X`, `/`), one of `esc`, `enter`, `space`, `tab`,
`backspace`, `delete`, `insert`, `left`, `right`, `up`, `down`, `home`,
`end`, `pageup`, `pagedown`, or one of those with `ctrl-` or `alt-` in front.
Case is the character's own: `x` and `X` are different keys, which is why
`TERM` and `KILL` can sit beside each other.

Binding a key another action already holds is refused, naming the action
that keeps it — a key that asks for two things is one that can only ever do
one of them. An action can have several keys; listing none leaves it
unreachable, which is how to turn one off.

`poptop --keys` prints every action, the keys bound to it, and where the
binding came from. The `?` list on screen shows the default keys and says
when a file has moved any of them.

Two keys are not in the map. Inside the filter and the jump box every key is
text or editing, so nothing there can be rebound; and `Ctrl-C` always quits,
because raw mode means poptop never sees a SIGINT and that is the reflex for
leaving a full-screen program.

| Action | Default |
|---|---|
| `quit` | `q` |
| `back` | `esc` |
| `scrub-back` | `left`, `h` |
| `scrub-forward` | `right`, `l` |
| `jump` | `b` |
| `zoom-in` | `+`, `=` |
| `zoom-out` | `-`, `_` |
| `pause` | `space` |
| `oldest` | `home` |
| `live` | `end` |
| `select-up` | `up`, `k` |
| `select-down` | `down`, `j` |
| `page-up` | `pageup` |
| `page-down` | `pagedown` |
| `sort` | `s` |
| `sort-constraint` | `S` |
| `filter` | `/` |
| `signal-term` | `x` |
| `signal-kill` | `X` |
| `tree` | `t` |
| `group` | `g` |
| `detail` | `d` |
| `threads` | `y` |
| `view` | `v` |
| `cgroups` | `C` |
| `kernel-threads` | `K` |
| `io-columns` | `i` |
| `reload-theme` | `R` |
| `help` | `?` |
