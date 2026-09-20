# The graphs

## What the timeline graphs

`CPU` and `WAIT`, and memory only when there is room for it.

Memory used over ten minutes is a flat line or a slow ramp that repeats what
the header already says, and it was occupying half the most valuable space on
screen. Waiting is the row that turns a stalled machine from a mystery into a
shape, so memory is the one that yields — and comes back as a third row on a
terminal tall enough to carry it. Where the platform publishes no iowait, the
row goes back to memory rather than the graph carrying one it cannot fill.

A series is only drawn if the gutter can name it, because identity here rests
on the label rather than the hue — the third series reuses a colour.

That is a cost decision, not a limit. Searching the whole cube for a sixth hue
clearing ΔE 8 against the existing five returns 77 candidates, the best at ΔE
10.5 against a palette whose current worst pair is 10.3. But every one of them
is adjacent to a hue already in use — orange between `warn` and `critical`,
pale cyan beside `ok`, periwinkle beside `series_cpu` — because the safe
palette already avoids green, the pair red-green deficiency destroys. So a
sixth token would mean another key in every user theme and `--check-theme`
going from ten pairs to fifteen, in exchange for a colour that reads as a
near-miss of an existing one, for a row the gutter names outright.

The hues alternate instead, which guarantees the only thing that matters: two
graphs touching each other never share a colour.
