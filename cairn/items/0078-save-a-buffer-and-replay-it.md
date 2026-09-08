---
id: 78
title: Save a buffer and replay it
type: feature
status: backlog
milestone: v1.0
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: l
area: cli
---

## Problem

poptop keeps every sample it takes and throws all of it away on exit. You can
rewind, but you cannot show anyone.

"Send me your buffer" is the natural next request from anyone debugging someone
else's machine, and it is currently impossible. The tool whose premise is *keep
the evidence* cannot hand the evidence over.

This surfaced from an unexpected direction. The website needed a recorded
session for its landing page and could not get one, because nothing can produce
one — `--once` prints a single sample and `--bench` times collection. So the
site fabricates a buffer from a seeded generator, discloses that it is
fabricated, and every honesty risk on that page descends from the gap.

## Proposal

    poptop --record incident.ptop     write the buffer as it fills
    poptop --replay incident.ptop     open it, scrub it, no collector running

Replay is most of the value and the smaller half of the work: the UI already
renders from a buffer and does not care where it came from. The collector simply
does not run.

Things to decide rather than assume:

- **The format is a compatibility surface** the project would then owe people.
  Worth a version byte at the front and an explicit statement about what
  compatibility is promised — probably "same minor version", stated rather than
  implied.
- **Writing to disk cuts against "nothing written to disk unless you ask for
  it".** A flag is exactly what asking looks like, so the principle is intact,
  but the README says that sentence and should say it next to this.
- **Size.** The full process table at one sample a second is not small.
  Whether to store deltas, and whether `--record` should cap or roll, is the
  interesting design question.
- **What replay cannot claim.** A replayed buffer has the same blind spots the
  live one had — short-lived processes are still only counted. Replay must not
  look more authoritative than the recording was.

The website becomes its first consumer, which is a good way to find out whether
the format is any use. The landing page's demo would then be real data, and its
provenance line would name a machine instead of a seed.

## Acceptance criteria

- [ ] `--record` writes a buffer that `--replay` opens
- [ ] Replay scrubs, zooms and sorts with no collector running
- [ ] The format carries a version and the compatibility promise is written down
- [ ] The README states the disk-write next to the claim it qualifies
- [ ] Round-trip test: record, replay, assert the frames match
