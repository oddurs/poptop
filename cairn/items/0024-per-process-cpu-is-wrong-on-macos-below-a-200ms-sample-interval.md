---
id: 24
title: Per-process CPU is wrong on macOS below a 200ms sample interval
type: bug
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
area: collect
---

## What happens

Item 0013 made the sample interval configurable with a floor of 50ms, justified
by the cost of a `/proc` pass. That floor is a Linux argument applied to both
backends, and sysinfo documents a 200ms minimum between CPU refreshes on macOS.

Measured on this machine, sampling the same idle-ish system at each interval:

    interval   50ms -> cpu_total   16.1%  max core   33.4%  max proc     3.5%
    interval  100ms -> cpu_total   62.7%  max core   80.8%  max proc   262.9%
    interval  200ms -> cpu_total   45.5%  max core   60.7%  max proc   230.4%
    interval 1000ms -> cpu_total   50.1%  max core   65.7%  max proc   323.8%

At 50ms the busiest process reports 3.5% where it is really using two and a
half cores. The figure is not noisy, it is wrong — and nothing on screen says
so. `interval = 50ms` is a documented, reachable setting.

## What should happen

The floor is a property of the backend, not of the tool. Either raise it on
macOS to what sysinfo can actually deliver, or refuse the setting there with
the reason.

Refusing is more in keeping with the rest of this project than quietly
substituting a different interval from the one the user configured: everywhere
else, a value poptop cannot honour is reported rather than adjusted.

Whichever way it goes, the number is the argument — the 200ms figure should be
in the message, not just in sysinfo's documentation.

## Acceptance criteria

- [ ] The interval floor is per-backend rather than one constant
- [ ] macOS rejects or raises a sub-200ms interval, and says why
- [ ] Linux keeps its 50ms floor, which is justified by its own measurement
- [ ] A test pins each backend's floor
