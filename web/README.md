# The poptop website

Landing page, documentation, and the community pages, as one Rust binary.

```sh
cargo dev      # serve on http://127.0.0.1:3000 with browser reload
cargo freeze   # render every route to ./dist as static files
cargo test     # renders every page and checks every internal link
```

`audit/` holds three browser checks for the things a compiler cannot answer —
contrast, heading order, tap targets, layout shift, and whether the scrubbable
frame actually scrubs. See [audit/README.md](audit/README.md).

## What is unusual about it

**No content lives here.** Every documentation page is a section of a file that
already exists in the repository — the README, `ROADMAP.md`, `CONTRIBUTING.md`,
or one of the design notes under `docs/roadmaps/`. Pages name a heading;
`src/content.rs` cuts the file at that heading. A docs site that keeps its own
copy of the README is a docs site that is wrong six weeks later, and the failure
is silent, because nobody diffs prose.

Rename a heading in the README and `cargo test` fails. That is the only kind of
link check worth having.

**There is no build step.** Content, stylesheets and fonts are `include_str!`d
and `include_bytes!`d at compile time. No bundler, no asset pipeline, no npm
anywhere in the deployed path, and an export that is byte-identical between
builds because everything it draws from is compiled in.

Running it as a server is a single static binary with no runtime dependencies
and a `/healthz`, which is an unusual thing for a documentation site to be. What
this is *not* is poptop's "nothing to set up first" — that principle is about a
stranger's machine at three in the morning, and reusing the phrase for a build
directory spends the project's credibility on a property nothing depends on.

**The hero is the product, not a picture of it.** `assets/js/demo.js` draws a
real poptop frame in braille from a buffer generated on the server, and you can
scrub it. Everything a screenshot could say about a system monitor has already
been said by every other system monitor's screenshot. What poptop does that they
do not is an interaction, and there is no honest way to show an interaction with
an image.

**Serving and exporting are the same code.** `export.rs` calls the same view
functions the router does, so the deployed site is the one you developed against
rather than a second implementation waiting to disagree with the first.

## Layout

```
assets/css/     seven layers, concatenated in cascade order at compile time
assets/fonts/   Instrument Sans, four variable subsets, embedded in the binary
assets/js/      site chrome, and the scrubbable frame
src/fonts.rs    generates the @font-face rules from the subsets' own ranges
src/content.rs  which repository files become which pages
src/markdown.rs comrak, plus heading promotion and link rewriting
src/routes.rs   every URL, walked by both the router and the export
src/views/      maud templates; ui.rs is the component library
```

## The design system

`/design` renders it, live, from the same functions the rest of the site calls —
so it cannot document a component the site no longer has.
`assets/css/10-tokens.css` is the source of every colour, size and duration, and
carries the two rules that matter.

**Colour.** Status hues mean a state, identity hues mean which series, and
reusing one for the other destroys the meaning of the first everywhere else. The
hues are the terminal interface's own, from `themes/safe.theme`.

**Type.** Sans is the human voice; mono is the machine voice. If a person wrote
it, it is Instrument Sans. If the machine printed it, or you would type it, it
is monospace — the terminal frame, code, commands, file paths, keycaps, token
names. That is why `$ poptop` in the masthead is still mono: it is a shell
prompt, not a wordmark.

The sans is one self-hosted variable file, embedded in the binary, 84 KB for the
whole family. The mono is deliberately *not* a webfont: the frame draws braille,
which almost no webfont covers, and a plot whose glyphs fall back to a different
face than the text around them stops lining up with its own gutter.

Every type token is a `font` shorthand, so a size can never be used without the
line height it was drawn for. Tracking rides alongside in a matching
`--track-*` token, because the shorthand cannot carry it.

**Provenance.** A picture of poptop says where its data came from and who drew
it, in the same breath as the claim it supports.

This is the third rule and the one that had to be learnt. The landing page shows
a *simulation* of the product — fabricated data, drawn by a reimplementation of
the renderer — and the tool never had to have a rule for that, so the site did
not write one. A rewrite of the hero then deleted the caption saying the buffer
was generated, and the page described seeded data as a "recorded poptop session"
for a whole working session before anyone noticed.

It survived because nothing said the caption was load-bearing.
`the_landing_page_says_where_its_data_came_from` now fails the build if it goes
missing again, which is the same treatment the colour rule gets in the tool.

## What it does about the boring parts

**Security headers** are one list in `src/site.rs`, applied by the server and
written into `_headers` for static hosts, with a test that reads the exported
file back — so a deploy cannot end up weaker than a local run. The content
security policy allows inline script because the theme has to be applied before
first paint and a static export has no responses to hang a nonce on; `object-src
'none'` and `base-uri 'none'` close the holes that actually matter with inline
script allowed.

**Caching** splits on whether a URL contains a hash of its own contents. The
stylesheet, the script and the fonts do, and are immutable for a year.
Documents do not, and revalidate.

**Layout shift** is zero. The webfont carries `font-display: swap`, and a
metric-matched `@font-face` for the local fallback means the swap does not move
the line. Without it, every paragraph on the page reflows when the font arrives.

**Colour** clears 4.5:1 for every piece of text against the surface it is
actually drawn on, in both themes, measured rather than assumed — see
`audit/audit.js`.

## Deploying

`cargo freeze` writes `dist/` — plain directories of `index.html`, a `404.html`,
a `sitemap.xml`, and hashed immutable assets. Any static host serves it without
configuration. `.github/workflows/site.yml` builds and publishes it to GitHub
Pages on every push to `master`.

Set the origin so canonical URLs and the sitemap are right:

```sh
cargo run --release -- build --out dist --base-url https://poptop.dev
```

To run it as a server instead — behind a reverse proxy, on a container host —
`poptop-web serve --host 0.0.0.0 --port 8080` is a single static binary with no
runtime dependencies, and `/healthz` answers for a health check.

## Its own workspace

Deliberately. The site must never widen the dependency tree of the thing it
documents: `cargo install poptop` should not resolve axum, and keeping the
lockfiles apart is the only way to guarantee that.
