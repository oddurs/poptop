---
id: 83
title: Make the colour-vision work something you can operate
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p0
effort: l
area: web
---

## Problem

The strongest thing about this project is that its palette is measured and the
measurement is enforced in CI: green-and-yellow separates by ΔE 3.7 under
protanopia, poptop's worst meaning-bearing pair by 10.3, and a test fails the
build if that ever drops below 8.

On the landing page this arrives as two numbers in two boxes. A reader with
normal colour vision has no way to feel what 3.7 means, and a reader with a
colour vision deficiency is being told about their own experience by a number.

`src/cvd.rs` already implements Machado 2009 simulation and the OKLab
conversion. The site quotes its output and shows none of it.

## Proposal

A control with four states — **normal · protanopia · deuteranopia · monochrome**
— that redraws a live specimen through the same simulation the tool's CI uses.

What it redraws:

1. **Two palette rows**, side by side and always both visible: the convention
   (green / yellow / red) and poptop's default (cyan / amber / red). Under
   protanopia the first row collapses into one colour in front of you. The
   second does not.
2. **A small frame**, drawn from the same buffer as everything else, so the
   effect is shown on the thing it actually matters to rather than on swatches.
3. **The measured separation**, updating with the state — the ΔE for the worst
   pair in each row, so the number and the picture move together.

The monochrome state is the important one and is not a colour-blindness
simulation: it is the proof. With every hue gone the frame still reads, because
the thresholds are dashed rules and the statuses carry glyphs. That is the
project's claim that no meaning rests on colour, made visible in one click.

Where the simulated colours come from: precompute them in Rust and embed them
with the buffer. `src/cvd.rs` is the reference implementation and porting the
matrices to JavaScript would mean two implementations that can disagree — the
exact failure the CVD tests exist to prevent. The site does not need to compute
it live; it needs to display what the tool already computes.

That means a small amount of the tool's colour code becoming reachable from the
site's build. Copy the matrices into the web crate with a comment pointing at
`src/cvd.rs` and a test asserting the two agree on the shipped palettes, or
expose them from the tool — decide when implementing, but do not leave two
uncompared copies.

## Acceptance criteria

- [ ] Four states, switchable, with the current one marked by more than colour
- [ ] Both palette rows visible in every state, so the collapse is a comparison
- [ ] A real frame redraws, not only swatches
- [ ] The ΔE figures shown are computed, not typed into the markup
- [ ] Monochrome demonstrates the thresholds and glyphs still carrying meaning
- [ ] A test asserts the site's simulation agrees with `src/cvd.rs`
- [ ] Operable by keyboard, and the state is announced
