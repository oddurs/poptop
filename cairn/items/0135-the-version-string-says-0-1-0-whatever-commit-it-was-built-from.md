---
id: 135
title: The version string says 0.1.0 whatever commit it was built from
type: feature
status: backlog
milestone: later
labels:
- repo
created: 2026-09-19
updated: 2026-09-19
priority: p3
---

## Problem

`poptop --version` prints `poptop 0.1.0`. That is the crate version, and it
is the same string for a released binary, a build from master, a build from
a branch, and a build from a working tree with uncommitted changes.

Found by installing from a checkout with `cargo install --path .` and being
unable to answer "is this the latest?" from the binary. The answer took a
`git rev-parse` and a `shasum` of two files.

For a tagged release the version is enough — the tag is the version. For
everything else it is not, and everything else is how the program is used
during development, which is when a stale binary actually misleads.

## Proposal

A build script that records the commit and whether the tree was clean, and a
version line that reports them when the build is not a released one:

```
poptop 0.1.0                       a released build
poptop 0.1.0 (b7c2c12)             built from a commit
poptop 0.1.0 (b7c2c12, modified)   built from a dirty tree
```

Two things to be careful of, both of which are why this is not a one-liner:

- **A build script must not make the build depend on git.** Building from a
  crates.io tarball, or from a checkout with no `.git`, has to keep working
  and produce the bare version.
- **It must not break reproducibility gratuitously.** Two builds of the same
  commit should still be identical, so the string is the commit, not a
  timestamp.

## Acceptance criteria

- [ ] The version line names the commit when one is knowable
- [ ] A tarball with no `.git` builds and prints the bare version
- [ ] A dirty tree says so
- [ ] The release workflow's binaries print what the tag says
