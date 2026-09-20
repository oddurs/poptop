# Tests

```sh
./check                        # everything CI enforces, on the host
./check --linux                # the same again inside rust:1-slim
./check --quick                # fmt, build, test; skip clippy and live data

cargo test -- --ignored --nocapture show_frame   # print a rendered frame
```

The last one matters on a Mac: the `/proc` backend is `cfg`'d out of a macOS
build entirely, so it is neither compiled nor tested unless you run it on Linux.

`./check` exists because of one property: **a tool that is missing is a failed
check, never a skipped one.** The `rust:1-slim` image ships without clippy, so
`docker run … cargo clippy` exits with "not installed for the toolchain" — which
looks nothing like a lint warning and is easy to read as a clean run. That is how
a dead field in `collect/linux.rs` reached a pull request: `cargo test` passed on
both platforms, and the clippy step that would have caught it had never once
executed. The script installs the component inside the container, and reports
anything it could not run rather than passing quietly.

UI tests render through ratatui's `TestBackend` and assert on the resulting
buffer, including a 1×1 terminal — a monitor that panics on a small window is
worse than no monitor, and it never shows up in normal use.

## Fuzzing

Every reader of bytes poptop did not write this second has a fuzz target in
`fuzz/`: the restart store, the day logs, every `/proc` parser, the NFS and
taskstats readers, and everything a person types — the filter, the jump box, a
colour, a config or theme file. One of them, `log_torn`, checks an answer
rather than only the absence of a panic: build a day out of whole and
crash-torn entries, and every whole one must come back.

```sh
./check --fuzz                 # every target for a minute, from fuzz/seeds
./check --fuzz=600 --linux     # ten minutes each, and the /proc targets on Linux
cd fuzz && cargo +nightly fuzz run store -- -max_total_time=3600
```

It needs a nightly toolchain and `cargo install cargo-fuzz`. CI runs every
target for a minute on Linux. What a fuzzer finds becomes an ordinary unit test
— the fuzzer is how a bug is found, not how it stays fixed.

`cargo fuzz` can only link a library, and poptop is a binary. `src/lib.rs`
compiles the same modules a second time behind the `fuzzing` feature; without
it the library is empty and the program is exactly what it was.

The ordinary tests carry a cheaper version of the same idea. `src/mangle.rs`
takes a real input and bends it — each line removed or doubled, each number at
the edge of its type, bytes that are not UTF-8, the file cut at every line — and
the parsers' tests assert that none of it panics. That runs on every
`cargo test`, on stable, on both platforms.

## Soaking

The tests run for seconds and poptop is meant to be left running for days. A
leak of a kilobyte a sample is invisible in either, and obvious after a day.

```sh
cargo build --release
soak/run target/release/poptop /tmp/soak 24     # hours
soak/analyse target/release/poptop /tmp/soak
```

It leaves the monitor on a real terminal at a 200 ms interval, logging, in an
empty home of its own with a process-churn generator beside it, and records its
resident memory, open descriptors, CPU time and log size every minute. Partway
through it does what real use does to a monitor: steps the wall clock backwards
(where libfaketime is available), stops and resumes the process as a laptop's
sleep does, and runs across a local midnight so the day log rolls over.
`soak/analyse` reads the `metrics.tsv` afterwards and says whether anything
grew that should not have. See [`soak/README.md`](../../soak/README.md).

Nothing in CI depends on it; it is run by hand before a release, and by anyone
who wants to know what a day of poptop costs.
