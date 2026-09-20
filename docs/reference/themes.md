# Themes

`theme = safe` (the default), `theme = classic`, or the name of a file in
`~/.config/poptop/themes/`.

## Why `safe` is the default

poptop's colours carry meaning: a hue says *what kind of thing this is*, and
you are expected to read it without thinking. Green and yellow — the
convention every monitor uses — separate by only ΔE 3.7 under simulated
protanopia, against a target of 8. Red-green deficiency affects roughly 8%
of men. So `safe` replaces green with cyan, and `classic` is there for
anyone who wants the convention back:

```sh
poptop --theme=classic
```

## The file

One line per colour; every line optional. A theme inherits `safe` for
anything it does not name.

```conf
# ~/.config/poptop/themes/nord.theme
ok         = #8fbcbb    # hex,
series_cpu = 67         # a 256-colour index,
chrome     = darkgray   # or an ANSI name
```

| Token | What it colours |
|---|---|
| `ok` | a figure that is fine |
| `warn` | at or past `warn` |
| `critical` | at or past `critical` |
| `series_cpu` | the CPU series, wherever it is drawn |
| `series_mem` | the memory series |
| `chrome` | rules, dividers, the tree's spine — structure, not data |
| `text` | ordinary text |
| `text_dim` | text that is deliberately quiet |
| `selection_bg` | the selected row's background |
| `selection_fg` | the selected row's text. Inherits `text` |
| `live` | the `LIVE` marker and the selection's mark |
| `border` | panel borders. Inherits `chrome` |
| `gap` | the seam where sampling stopped and started again. Inherits `chrome` |

A value that will not parse warns, naming the token and the line, and that
token alone falls back.

## Checking a theme

```sh
poptop --check-theme nord
```

It measures every pair of meaning-bearing hues under simulated protanopia,
deuteranopia and tritanopia, and every hue against the surfaces it is drawn
on, then says `PASS` or names what is too close:

```text
safe: PASS
  ok          ↔ warn        ΔE  16.2  protan
  ok          ↔ critical    ΔE  17.0  deutan
  ...
  ok          on surface         9.56:1
```

`classic` reports its own caveat rather than failing quietly: it is
knowingly below the target, for the reason above.

## Colour tiers

`color` picks how much colour poptop uses: `true`, `256`, `16`, `mono`, or
`auto`, which reads `$COLORTERM` and `$TERM`. `NO_COLOR` forces `mono`, and
outranks the config file.

Every meaning poptop carries in colour is also carried by something else — a
bar's length, a dashed rule, a mark in the margin — so `mono` loses nothing
but speed of reading. The reasoning is in
[docs/design/colour.md](../design/colour.md).

## Trying a colour

`R` reads the theme file again, and a theme file that changes on disk is
picked up on the next sample — one `stat` a second, beside the several
hundred reads a sample already makes. Edit, save, and the colours change
under the running monitor.

A file that no longer parses keeps the colours already on screen: the
half-applied theme of a file mid-edit is worse than the one you had. The
footer says which file was read and whether any line was ignored. A
built-in has no file to read, and says so rather than appearing to do
nothing.

## Why there is no third series colour

The graphs alternate `series_cpu` and `series_mem` rather than giving each
series its own hue. There is no sixth hue here: the palette avoids green
for colour-vision reasons, and what is left is warning-orange or too close
to `ok`. Alternating guarantees the thing that matters — two graphs
touching each other never share a colour — and `--check-theme` measures
every pair that carries meaning. A third series hue would have to survive
that check first.
