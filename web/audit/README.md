# Checks a compiler cannot run

Three scripts that drive a real browser against a running site. They are not
part of the build and nothing depends on them; `cargo test` covers what can be
answered from the markup, and these cover what can only be answered once the
page has been laid out, painted and clicked.

```sh
npm install playwright-core          # once; drives the Chrome already installed
cargo run -- serve &                 # or `cargo dev`
node audit/audit.js                  # 9 pages × 4 widths × 2 themes
node audit/headings.js               # every heading still renders as one
node audit/interact.js               # does the thing actually work
node audit/sections.js               # the landing page's pictures and claims
node audit/perf.js                   # layout shift and weight, throttled
```

All five exit non-zero on failure, so `for f in audit headings interact
sections; do node audit/$f.js || break; done` is the whole suite.

`playwright-core` is the browserless package: it drives the Chrome on the
machine rather than downloading its own, so this adds one small dev dependency
and no 150 MB of browser. There is no `package.json` here on purpose — nothing
in this repository should imply a node toolchain is required to build the site.

## What each one is for

**`audit.js`** walks every page at four widths in both themes and reports:
horizontal overflow, colour contrast against the actual computed background,
heading order, accessible names, tap-target size, and clipped text. It knows
the two exceptions that matter — WCAG 2.2's inline exception for targets sitting
in a sentence, and that the terminal frame's own palette is measured by the
tool's own CI rather than here.

It found the things a person does not: `--ink-faint` sitting at 3.08:1 against
the surface it was drawn on, and every section rule looking like a heading
without being one, so the document outline went h1 straight to h3.

**`interact.js`** drives the scrubbable frame the way a visitor does —
keyboard, drag, buttons — and checks the theme toggle, the copy button, the
skip link, the focus ring and the on-page contents. It found that the frame's
spoken readout was querying for an element that is a sibling rather than a
child, so it was never populated, and that the contents marked nothing at all
through the long stretches between headings.

**`headings.js`** fails if any heading renders no larger and no heavier than
body text. It exists because a scripted edit once deleted the selector off
`.display` and the landing headline shipped at 17px — valid CSS, valid HTML, and
wrong in a way no test then covered. Some headings here are deliberately small,
so the check is "neither larger nor heavier", not "larger".

**`sections.js`** covers the landing page's nine sections: that every still
actually drew braille rather than an empty box, that mean and peak draw
genuinely different pictures, that the colour-vision control applies each state
and marks the current one by more than hue, that monochrome leaves no hue at
all and withholds a separation figure rather than reporting a failure, and that
the keymap's caps drive the frame above them. One still is asserted to be
*blank* — the closing frame, which is what poptop looks like a second after you
start it.

**`perf.js`** loads the site over a throttled connection and reports layout
shift, paint timing and transferred weight. It is the check on the font work:
the metric-matched fallback in `src/fonts.rs` is only worth its comment if CLS
stays at zero when the real face swaps in.

## A note on writing checks like these

Both failures the first run of `interact.js` reported were the test's fault, not
the site's: it read values before the next animation frame, and it assumed the
frame was live when it was paused. A browser check that has not been made to
fail on purpose is not yet a check. Confirm the failure is real before changing
the thing under test.
