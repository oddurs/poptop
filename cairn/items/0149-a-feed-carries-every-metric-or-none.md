---
id: 149
title: A feed carries every metric or none
type: feature
status: done
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p3
effort: s
area: export
---

## Problem

`--export` writes every field of every record: at four hundred processes a JSON sample is hundreds of kilobytes. That is right for "every metric by name", and wrong for a feed a dashboard reads every second, where the consumer wants four numbers and pays for the process table each time. The only way to narrow it today is to pipe the whole thing through `jq`, which means poptop still built it.

## Proposal

`--fields cpu_total,mem.used,procs.name` — dotted paths into the schema, checked against it, so a wrong name is refused with what it could have been rather than silently dropped. Absent fields stay absent: a filter that turns a missing figure into a zero would undo the one rule the format is built on.

## Acceptance criteria

- [x] A named subset is emitted, in schema order, for both formats
- [x] An unknown field is refused, naming the closest match the schema has
- [x] The absent-versus-zero distinction survives the filter
- [x] The cost of a narrow feed is measured against a whole one

## How it was resolved

`--export=json|line --fields cpu_total,mem.used,procs.name`, dotted paths into the schema.

**A filter between the sample and the writer**, not inside each writer: `export::Only` is a `Visit` that forwards only what was asked for, so both formats narrow the same way and neither had to learn what a field name is. It tracks the logical path as the walk opens records, and skips a subtree nobody asked for rather than emitting it and dropping the pieces.

**A list is transparent.** `procs.name` is that field of every process; the alternative is naming an index, and `procs.17.name` is not a thing anyone asks of a process table. Naming a record takes all of its fields (`--fields mem`), and the spelling the line format prints works too, so `sample.cpu_total` and `cpu_total` are one field.

**Schema order, not the order asked in**, so a consumer builds one column map whatever the caller wrote. `at` is always included: a record that cannot say when it was taken is not a sample of anything, and somebody asking for `cpu_total` meant the series.

**Absence survives the filter.** A field nobody reported is still `null` or `-`, never a missing column — a narrow feed that dropped it would turn "poptop did not ask" into "the kernel does not publish", which is the one distinction this format exists to keep. The line format's header still names exactly the columns its rows carry, narrowed.

**A wrong name is refused with the nearest right one.** Levenshtein against every path the schema has, bounded at a third of the name so a suggestion is usually right; with nothing near enough it points at `--schema`. Silently dropping it would produce a feed missing exactly the figure the caller was watching for, with nothing in it saying so.

**Measured** (`measure_a_narrow_feed`, 400 processes): a whole JSON sample is 86,040 bytes and 150 µs to write; `cpu_total,mem.used,load` is 64 bytes and 31 µs. The saving is in what is written, not in what is collected — the optional sources are still gated by `needs`, and wiring `--fields` into that is a separate question, worth asking if a feed's collection cost ever shows up.

**Tests.** The subset in both formats with the schema's order and the list's own table; the time always present; absence surviving, with the row still matching its header; a whole record by name; every refusal, including the empty list and the nearest-match suggestions; and an end-to-end run in `tests/cli.rs` that holds the narrow JSON to a tenth of the whole and the line header to exactly its two columns.
