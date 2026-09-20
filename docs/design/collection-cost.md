# What it costs to watch

## Sample rate and window

`interval` is the time between samples; `window` is how much history to keep,
**expressed in time rather than a sample count** — the span is what you
actually want, and the buffer size follows from it. Spans are written the way
people write them: `500ms`, `2s`, `10m`, `1h`. A bare number is seconds,
because that is what someone typing `interval = 2` means.

Sub-second sampling narrows the window in which a short-lived process is
invisible; longer intervals stretch the retained span on a quiet box. The ring
buffer already tolerates uneven intervals, because the timestamps are real.

**The two settings cost memory together, not separately.** Every sample retains
its whole process table — that is what makes scrubbing show the real table from
that instant rather than an interpolation — so the buffer is about
`samples × processes × 96 bytes`:

| processes | 10m at 1s | 1h at 1s | 10m at 100ms |
|---|---|---|---|
| 100 | 6 MB | 35 MB | 59 MB |
| 400 | 23 MB | 139 MB | 231 MB |
| 4000 | 231 MB | 1.4 GB | 2.3 GB |

(a buffer holds one more sample than the span needs — `n` samples span `n − 1`
intervals — so the real figures are a few kilobytes above these)

(measured by the `show_sample_footprint` test, so the table can't drift from
the structs)

A day of history at one sample a second and ten minutes at sixty a second are
the same buffer, so poptop bounds the **product** — the sample count — rather
than either setting, and says what the buffer would cost when it refuses:

```
poptop: `window` 86400s at `interval` 500ms is 172801 samples, above the limit
      of 86401; every sample retains a whole process table, which is about
      6663 MB on a 400-process box
```

**The interval floor belongs to the backend**, because the two have different
reasons for having one.

On Linux it is **50 ms**, and the reason is cost: a `/proc` pass is about 1 ms
at 400 processes, so 50 ms already spends 2% of a core and 10 ms would spend
10%. A monitor that is itself the load is not measuring the machine.

On macOS it is **200 ms**, and the reason is correctness: sysinfo needs that
long between CPU refreshes, and below it the per-process figures are not noisy,
they are wrong. Measured on an idle-ish machine, the busiest process reported
`3.5%` at 50 ms and `262.9%` at 100 ms — two and a half cores of work, reported
as three and a half percent, with nothing on screen to say so.

poptop refuses the setting rather than quietly substituting a different one, and
the message carries the reason:

```
poptop: `interval` is 50ms; the fastest this build can sample is 200ms, because
        sysinfo needs 200ms between CPU refreshes on this platform, and below it
        the per-process figures are wrong rather than merely noisy
```
