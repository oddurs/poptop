//! Facts about the site that more than one page needs.

/// Where the site is published. Used for canonical URLs, the sitemap and the
/// social card — the three places a relative URL is not allowed. Overridable
/// with `--base-url`, because a preview deploy that advertises the production
/// domain as canonical is how a preview deploy ends up in a search index.
pub const DEFAULT_BASE_URL: &str = "https://poptop.dev";

pub const NAME: &str = "poptop";
pub const TAGLINE: &str = "A system monitor you can rewind";
pub const REPO: &str = "https://github.com/oddurs/poptop";
pub const CRATE: &str = "https://crates.io/crates/poptop";
pub const ISSUES: &str = "https://github.com/oddurs/poptop/issues";
pub const DISCUSSIONS: &str = "https://github.com/oddurs/poptop/discussions";
pub const INSTALL: &str = "cargo install poptop";
pub const LICENSE: &str = "GPL-3.0-or-later";

/// Response headers every document gets.
///
/// One list, used by the server and written into `_headers` for static hosts,
/// so a deployed site cannot end up weaker than a local run.
///
/// The content security policy is as tight as the site allows. Two `script-src`
/// hashes would be tidier than `'unsafe-inline'`, but the theme boot script has
/// to run before the first paint and the buffer is a JSON island; both are
/// generated, so a nonce would have to change per response and a static export
/// has no responses. `object-src 'none'` and `base-uri 'none'` close the two
/// holes that actually matter with inline script allowed.
pub const CSP: &str = "default-src 'self'; \
    script-src 'self' 'unsafe-inline'; \
    style-src 'self' 'unsafe-inline'; \
    img-src 'self' data:; \
    font-src 'self'; \
    connect-src 'self'; \
    form-action 'none'; \
    frame-ancestors 'none'; \
    object-src 'none'; \
    base-uri 'none'";

pub const HEADERS: &[(&str, &str)] = &[
    ("content-security-policy", CSP),
    // The site serves user-supplied nothing, but a stylesheet mis-sniffed as
    // HTML is a real class of bug and the header costs one line.
    ("x-content-type-options", "nosniff"),
    // Send the origin to other sites, the full path to our own.
    ("referrer-policy", "strict-origin-when-cross-origin"),
    // Nothing here needs a camera, a microphone, or a location.
    (
        "permissions-policy",
        "accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()",
    ),
];

/// One sentence, used as the meta description of the landing page and as the
/// fallback everywhere else.
pub const DESCRIPTION: &str = "poptop keeps every sample it takes, including the \
    full process table, so you can scrub backwards and ask what was eating the \
    machine forty seconds ago. No daemon, no config, nothing that had to be \
    running before you noticed.";
