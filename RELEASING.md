# Releasing

Before tagging a version:

1. `./check --linux --fuzz` passes: fmt, clippy, every test on both platforms,
   and every fuzz target for a minute.
2. **Stores and logs.** If this release changed what a sample holds,
   `tests/corpus` has a file written by it — `tests/corpus/write HEAD 15-prNN`.
   CI fails without one; this is the reminder to write it from the release
   commit itself.
3. **The schema.** If `tests/golden/schema.json` changed since the last
   release, the change is listed in `CHANGELOG.md` as compatible or breaking,
   by the rules under **Stability** in
   [docs/design/recording-and-output.md](docs/design/recording-and-output.md).
4. **The numbers.** `validate/run` agrees on every metric, on Linux and on
   macOS: run it on each, or run the `validate` workflow. Keep the output in
   `validate/results/` when it says something new, and fix or file every
   disagreement before tagging.
