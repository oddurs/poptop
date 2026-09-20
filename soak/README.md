# Soak

The tests run for seconds, and poptop is meant to be left running for days.
`soak/run` leaves it running for a day and watches it:

```sh
cargo build --release
soak/run target/release/poptop /tmp/soak 24     # hours
soak/analyse target/release/poptop /tmp/soak
```

It runs the monitor on a terminal (`script`) at a 200ms interval, logging,
in an empty home of its own, next to a process churn generator. Every minute
it records poptop's resident memory, open descriptors, CPU time and log size
in `metrics.tsv`. Partway through, it does what real use does to a monitor:

- at 1/12 of the run, the **wall clock steps back five minutes**. This uses
  libfaketime, preloaded into poptop alone, where `SOAK_FAKETIME_LIB` names
  it. Only the wall clock moves, as with a step from NTP: the monotonic clock
  that schedules samples does not. This needs no root and changes nothing
  else on the machine. Without it, no step is made and the analysis says so.
- at 1/8, the process is **stopped for three minutes and resumed**, which is
  how a laptop's sleep looks to a process that was running when it slept. A
  real sleep during the run is a real one.
- it runs across at least one **local midnight**, so the day log rolls over.

At the end it types `q`, so the store is written as on any clean exit.

`soak/analyse` checks five things and exits with the number that failed:

- **memory:** resident growth after warm-up (the first sixth of the run) stays
  within 16 MiB or a tenth of the warm figure, whichever is larger
- **descriptors:** flat after warm-up, a spread of at most 2
- **the log:** every logged sample is read back with `--export json`, in
  order, none twice, each in its own local date's file, and the only gaps are
  the stop and the only step back is the clock step. So nothing was lost or
  doubled across midnight, and the stop left a gap, which the timeline marks
  as a seam (0012)

Run it from a copy: a shell reads a script as it runs it, and editing
`soak/run` under a running soak broke the first rehearsal. Log sizes: at 10s
logging, a small container's day is a few megabytes, and a desktop's 700
processes a day is about 800 MB. On a desktop, `SOAK_LOG_INTERVAL=60s`.

`results/` holds the runs worth keeping: `metrics.tsv`, `summary` and
`analysis`. Not the home directory, which holds the machine's process tables.
