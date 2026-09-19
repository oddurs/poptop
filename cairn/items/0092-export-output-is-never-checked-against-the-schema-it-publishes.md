---
id: 92
title: Export output is never checked against the schema it publishes
type: chore
status: done
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: s
area: export
---

## Problem

0074 added JSON and line-format export "with its schema". Consumers will write parsers against that schema. Nothing checks that what poptop emits conforms to what it says it emits, or that the schema does not change without the change being noticed.

## Proposal

Validate every exported JSON record from a live sample and a recorded store against the published schema in a test. Keep a golden copy of the schema so any change to it appears as a diff in review. Decide and document what counts as a breaking change.

## Acceptance criteria

- [x] A test validates export output from a real sample against the published schema
- [x] The schema is snapshot-tested; changing it requires updating the golden file
- [x] The line format has a stated grammar and a test that parses its output back
- [x] The compatibility promise for the schema is written in the docs

## How it was resolved

`tests/export.rs` holds the export to `--schema`. It needs no new dependency: a JSON reader of about a hundred lines is enough for poptop's own output, and it rejects raw control bytes, non-finite numbers and trailing bytes.

- **Validation.** The schema is read back from the binary itself, and every exported sample is walked against it: the live `--export=json` with every source collected, and a day recorded with `--once --log=on`. Every field must be present, in the declared order, with none extra. Each value must be of its type, where `integer` means no fraction and `[T; n]` means exactly n items. `null` is allowed only where the field is optional. A second test changes one thing at a time in a real sample (`null` where required, a string for a number, a number for a record, `load` with two items, a missing field, an extra one) and requires the validator to catch each. It passes on macOS and Linux. Linux fills in far more of the schema, and the schema is the same on both.
- **Golden file.** `tests/golden/schema.json` must equal `--schema` byte for byte. `POPTOP_BLESS=1 cargo test --test export the_schema` regenerates it, so every schema change is a diff in review.
- **Line format.** The grammar is written in the README and in the reader the test uses: headers once per label and before their rows, every row as wide as its header, and each sample's `sample` row after its children. The test reads a recorded day's line export and checks every value against the JSON export of the same day. Absent must be `-`, booleans `1`/`0`, and numbers must be equal as numbers.
- **Compatibility**, written under "Stability" in the README: which changes are compatible (a new field or record), which are breaking (removal, rename, a change of type or unit, list↔record, a field that was never `null` becoming one), and that consumers of the line format should find columns by name.

**Found and fixed:** the line format replaced TAB and LF inside text, but not CR, which is a line ending to any reader that splits on universal newlines. **Documented rather than fixed:** an optional text field whose value is exactly `-` is indistinguishable from absent in the line format. Escaping it would change values consumers already read. JSON is the lossless format, and the README says so.

Shared harness: `tests/common/mod.rs` now holds the hermetic home and the run assertions for both test files.

