# Colour

## Themes

The palette is compiled in, but it is not the only one you can have. A theme is
one file, one line per colour, in `~/.config/poptop/themes/NAME.theme`:

```ini
# ~/.config/poptop/themes/nord.theme
ok         = #8fbcbb    # hex,
series_cpu = 67         # a 256-colour index,
chrome     = darkgray   # or an ANSI name
```

**Every line is optional.** A theme inherits `safe` for anything it does not
name, so overriding two colours is two lines. That is the whole reason btop
ships 41 themes and htop, which hardcodes eight in C, has never gained a ninth:
the barrier decides whether a theme library exists.

The tokens are exactly the ten the palette was built around — `ok`, `warn`,
`critical`, `series_cpu`, `series_mem`, `chrome`, `text`, `text_dim`,
`selection_bg`, `live`. Nothing new is invented for user themes; a vocabulary
the code did not already use would be a second one to keep in step with the
first.

**The built-ins ship as files too**, in [`themes/`](../../themes/), so the way to
learn the format is to copy one — and a test asserts they match the compiled
palettes colour for colour, so they cannot quietly drift.

A built-in name always wins over a file. A `safe.theme` in your themes
directory would otherwise silently replace the palette everything else in this
project is measured against.

**A colour the terminal cannot show is left alone and reported, not
approximated.** Hex needs a true-colour terminal, an index needs 256 colours, a
name works anywhere, and monochrome ignores all of them:

```
poptop: theme `nord`: this terminal is Ansi16 and cannot show ok, series_cpu;
      keeping ok = cyan, series_cpu = lightblue
```

Squeezing 24-bit hex into sixteen slots would destroy exactly the separation
the palettes were measured for — and those sixteen slots belong to your
terminal theme, not to poptop.

## Measuring a theme

poptop is the only monitor that measures its own palette, and the moment you can
supply your own that guarantee evaporates — unless the validator is turned
outward. So it is:

```
$ poptop --check-theme muddy
muddy: FAIL
  ok          ↔ warn        ΔE   0.1  tritan    below the target of 8
  ok          ↔ critical    ΔE  54.2  deutan
  ...
  critical    on surface         1.09:1          below 3:1
  critical    on selected row    1.40:1          below 3:1
  worst pair: ΔE 0.1   worst contrast: 1.09:1
```

Every pair, not only the failures: a theme passing at ΔE 8.1 is a different
thing from one passing at 30, and the number is the point. **A contributed
theme can arrive with a measurement rather than a screenshot.**

**There is a third outcome, and it is not a pass.** An ANSI name or an index
below 16 is a *slot* — what it looks like belongs to your terminal theme, not
to poptop — so there is genuinely no hue to measure:

```
$ poptop --check-theme ansi
ansi: INCOMPLETE
  ...
  not measured: ok, warn, critical. An ANSI name or an index below 16 is a
  slot, and what it looks like belongs to your terminal theme rather than to
  poptop — there is no hue here to measure. Spell these as `#rrggbb` or a
  256-colour index to have them checked.
```

Only a pass exits zero. A check that could not see the colours has not passed
them, and a script asking "is this theme legible" must not be told yes by
silence. The same applies to `selection_bg`: if the selected-row background is
an unknowable slot, the rows measured against it are absent rather than
invented.

**The check measures at the top tier, not the one it detects.** In a CI job
`TERM` is often unset, which detects as monochrome — so a check that read the
detected tier would find nothing measurable and certify anything, in exactly
the place this command is meant to run. The question is whether the *theme* is
legible, which is a property of the colours it names rather than of the
terminal running the check.

**This is the same instrument CI uses.** The palette tests assert through
`check::Report` rather than a second copy of the arithmetic, so poptop's own
check and yours cannot come to disagree about the same colours.

Separation is asked only of the colours that must be told apart. Legibility is
asked of everything drawn — including `text` and `live`, since a theme that
makes the interface invisible while keeping its status hues distinct is not a
theme anyone can use. `chrome` and `text_dim` are meant to recede, so they have
a lower floor: a border competing with the numbers inside it is a worse border,
but one nobody can find is worse still. And chrome is measured only against the
surface, because the process table has no side borders — it never crosses a
selected row, and a check that fails on things that cannot happen trains people
to ignore it.

**A failing theme still loads**, with one line saying why:

```
poptop: theme `muddy`: ok and warn are only ΔE 0.1 apart (tritan), critical is
      1.09:1 on the surface — run `poptop --check-theme muddy` for the rest
```

It is your terminal and your choice; poptop's job is to have the number and say
it, not to refuse — the same principle as rendering `—` rather than a
fabricated zero.

`--check-theme classic` reports FAIL, and says why that is a decision rather
than a bug: classic exists to restore the green/yellow convention, and
green/yellow is the pair that convention gets wrong under red-green deficiency.
That is the whole argument for `safe` being the default, and it is pinned by a
test so that quietly "improving" classic would break the build rather than
remove the argument.
