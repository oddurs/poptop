# Soak results

One directory a run, each holding what `soak/run` recorded and what
`soak/analyse` made of it:

| file | what it is |
| --- | --- |
| `summary` | the machine, the length, and what was done to poptop while it ran |
| `metrics.tsv` | a row a minute: elapsed, resident KB, open descriptors, CPU seconds, log bytes |
| `analysis` | every check and its verdict |

These are kept because the claim they support — poptop is cheap enough to
leave running for a day — is not one a unit test can make. A run that is not
written down is a run somebody has to do again to believe.

The analysis is run with the same `TZ` the recording used: it reads the log's
timestamps back as local dates, and a different zone would put a sample in
the wrong day and fail a check poptop passed.
