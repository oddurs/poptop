# Contributing

poptop takes changes from anyone. This page is what to expect, so that a first
patch does not spend two rounds of review learning the house rules.

## Before you start

Run it. Break it. The most useful contributions so far have come from someone
watching the thing get a number wrong on their own machine, and the second most
useful from someone reading a claim in the README and checking it.

If you are planning something larger than a fix, open an issue first. Not for
permission — to find out whether it is already argued about somewhere in
[`docs/roadmaps/`](docs/roadmaps/), which is where the reasoning behind the
current design lives. Several ideas that look obviously right were tried and
rejected for reasons written down there.

## The one command

```sh
./check
```

That is everything CI enforces, in the order CI enforces it: `cargo fmt
--check`, a build of all targets, the test suite, `cargo clippy --all-targets
-- -D warnings`, and two smoke tests against live data.

On a Mac, add `--linux`:

```sh
./check --linux
```

`src/collect/linux.rs` is `cfg`'d out of a macOS build entirely, so on a Mac the
container run is the only thing that compiles the `/proc` backend at all. A
syntax error in it has passed a clean local build before.

A missing tool is a failure in `./check`, never a skip. Every check that does
not run is reported as not having run — a check that silently counts as zero is
worse than no check.

## What a change is expected to carry

**A test, where the thing is testable.** The interesting ones in this project
are not unit tests of arithmetic. `src/cvd.rs` implements Machado 2009 colour
vision simulation so that a test can fail the build when two meaning-bearing
hues drift closer than ΔE 8. `src/ui_tests.rs` asserts that status hues and
identity hues stay in their own panels. If your change makes a claim, prefer a
test that would catch the claim becoming false over a sentence saying it is
true.

**A reason, in the diff.** Comments here explain why, not what. The convention
throughout the codebase is that a comment earns its place by recording
something a future reader could not recover from the code — a measurement, a
rejected alternative, a bug that a line prevents from coming back.

**Prose that survives contact.** The README states numbers. If a change makes
one of them wrong, change the number in the same commit.

## Commits

Small, and each one a thing that works. The commit message says what changed and
why; the body is where the why goes if it does not fit in the subject.

`.githooks/no-attribution` refuses tool attribution in commit messages, file
contents, and pull request text. Commit messages and file contents end up in
public history, and none of them is the place for a note about what wrote them.
Install the hooks once per clone:

```sh
git config core.hooksPath .githooks
```

The same script backs three layers — the hook, `./check`, and CI — so they
cannot disagree, and the CI layer cannot be skipped.

## Colour, and why the palette is not a matter of taste

If you are changing a colour, read
[`docs/roadmaps/01-color-and-accessibility.md`](docs/roadmaps/01-color-and-accessibility.md)
first. The palette is under two enforced constraints: every pair among the five
meaning-bearing hues stays at least ΔE 8 apart under simulated protanopia and
deuteranopia, and every hue clears a contrast floor against the surfaces it is
drawn on. Both have already caught real regressions, including a replacement
palette that was well separated and invisible.

Status hues mean a state. Identity hues mean which series a thing is. Reusing a
status hue for identity destroys the meaning of that status hue everywhere else
in the interface, and a test will tell you so.

## Where work is tracked

[`ROADMAP.md`](ROADMAP.md) is rendered from the item files under `cairn/items/`.
`cairn board` shows the current state and `cairn next` shows what is ready to
pick up. The roadmap documents under `docs/roadmaps/` are the reasoning rather
than the tracker: measurements, prior-art comparisons, and the arguments that
produced each decision.

Issues and discussions are on GitHub. Both are read.

## Licence

poptop is GPL-3.0-or-later. By contributing you agree your work ships under the
same terms, and stays free for the next person the way it was free for you.
