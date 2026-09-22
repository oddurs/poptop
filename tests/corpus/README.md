# Store corpus

0059 made a promise: poptop reads every store and day log from format 15 on,
forever. This directory is what holds it to that. Each subdirectory was written
by the poptop of one commit, by `tests/corpus/write`, and `store::corpus` reads
every one with today's code.

| directory | written by |
| --- | --- |
| `14-before-pr77` | the last format 14 build. Must be **refused**, with a message naming format 14 |
| `15-pr77` … `15-pr91` | each merge that changed what a sample holds, from the schema block's arrival (#77) to NFS (#91) |
| `15-pr92` | the first day log (#92), from before entries carried a checksum |
| `15-pr115` | today's store and day log, with checksums |
| `15-pr159`, `15-pr167` | later additions to a sample: #167 is temperatures and fans |

Each has `history` (a store holding one sample), `log/` where that version had
a day log (two samples), and `manifest`: the commit, the format number, what
was run, and the machine's core count and memory from `/proc`, so the test
checks values poptop did not supply. Every file must decode whole, with no
field dropped, its samples timestamped while it was being written.

## When the schema changes

`the_corpus_has_a_file_with_this_builds_schema` fails until a file written
with the new shape is here. After the change is merged:

```sh
tests/corpus/write HEAD 15-prNN     # NN: the pull request that changed it
```

and commit the directory. It exports that commit, builds it and runs it in an
empty home inside a Linux container (docker), because a store is a whole
process table: written on your own machine it would carry every command line
you were running, and this repository is public. A change to the format number
itself is a change to this promise, and the README's "Stability" section with
it.
