# poptop documentation

Three kinds of page, and they answer three different questions.

- **[Guide](#guide)** — "how do I do this?" Task first, with real output.
- **[Reference](#reference)** — "what does this mean?" Tables, checked
  against the code where they can be.
- **[Design](#design)** — "why is it like this?" The arguments, kept.

## Guide

| | |
|---|---|
| [First run](guide/first-run.md) | what is on screen, the three regions, the first four keys |
| [Finding what is slow](guide/whats-slow.md) | the header, the constraint, the filter, the views |
| [Scrubbing to a moment](guide/scrubbing.md) | `←`/`→`, `b`, `+`/`-`, and what stays true while scrubbed |
| [One process in depth](guide/one-process.md) | `d`, `t`, `y`, and the selection |
| [Groups and trees](guide/groups.md) | `g`, `t`, `C` — and which one answers your question |
| [Signals](guide/signals.md) | `x`, `X`, and the two rules other monitors cannot offer |
| [Recording and replay](guide/recording.md) | `--store`, `--log`, `--read`, `--days`, `--report` |
| [Troubleshooting](guide/troubleshooting.md) | every message poptop prints, and what to do |

## Reference

| | |
|---|---|
| [Keys](reference/keys.md) | every key, held against `ui::HELP` by a test |
| [Columns and marks](reference/columns.md) | every column, every mark, and why a column disappears |
| [Filter](reference/filter.md) | the query grammar behind `/` |
| [Views and modes](reference/views.md) | `v`, `t`, `g`, `y`, `d`, `C`, `K`, `i`, and sorting |
| [Configuration](reference/configuration.md) | every key, its flag, its default, its unit |
| [Themes](reference/themes.md) | the file format, the tokens, the tiers, `NO_COLOR` |
| [Output](reference/output.md) | `--once`, `--export`, `--schema`, `--report`, `--days` |
| [Platforms](reference/platforms.md) | what each metric needs, on Linux and macOS |

## Design

Why poptop is shaped the way it is. These moved out of the README intact;
several are the only record of an argument that was had.

| | |
|---|---|
| [Prior art](design/prior-art.md) | where poptop and atop, htop, btop, bottom, zenith actually differ |
| [The keys, and the questions they answer](design/answering-questions.md) | each key, and the question it exists for |
| [Reading the screen](design/reading-the-screen.md) | the table and the header, figure by figure |
| [Configuration](design/settings.md) | the settings, signals, history across restarts |
| [Recording, replay and output](design/recording-and-output.md) | the log, the export, opening yesterday |
| [What it measures, and what it will not](design/what-it-measures.md) | storage, filesystems, network, NFS, throttling, stall, and the line poptop will not cross |
| [Colour](design/colour.md) | themes, tiers, and measuring a theme against colour-vision deficiency |
| [The graphs](design/graphs.md) | what the timeline draws, and why those series |
| [What it costs to watch](design/collection-cost.md) | sample rate, window, and the price of a process table |
| [How it works](design/how-it-works.md) | the collection path |
| [Notes from reading the others](design/notes-from-the-others.md) | lessons taken from htop, btop and bottom, with measurements |
| [Tests](design/tests.md) | what the suite is for, including the fuzzers |

## Roadmaps

[`roadmaps/`](roadmaps/) holds the per-release reasoning: what was audited,
what was decided, and what was declined. Open work itself is in
[`ROADMAP.md`](../ROADMAP.md), rendered from
[cairn](https://github.com/oddurs/cairn) items.
