# Instrument Sans

Copyright 2022 The Instrument Sans Project Authors, under the SIL Open Font
License 1.1 — see `OFL.txt`, which travels with the files as the licence
requires. Upstream: <https://github.com/Instrument/instrument-sans>.

Four variable subsets, taken from Google Fonts' own served woff2. Each covers a
`unicode-range` recorded in the matching `.range` file; `src/fonts.rs` reads
those at compile time and writes the `@font-face` rules, so the ranges here and
the rules the browser sees cannot drift apart.

They are `include_bytes!`d into the binary rather than fetched from a CDN. Three
reasons, in the order they matter:

1. The site is one file. A font hosted somewhere else is a second thing to
   deploy and a second thing that can be down.
2. No visitor to a page about a system monitor should be announced to a third
   party in order to read it.
3. The weight is 84 KB for the whole family, served once, cached forever under
   a URL containing a hash of the bytes.

The mono is deliberately *not* a webfont. See `assets/css/10-tokens.css`.
