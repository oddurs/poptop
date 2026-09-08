//! Static export.
//!
//! The same handlers that serve the site write it to disk, so there is no
//! second implementation to keep in step and no class of bug where the
//! deployed site differs from the one you developed against.
//!
//! Output is plain directories of `index.html`, which every static host serves
//! without configuration. The 404 page is written to `404.html`, which is the
//! filename GitHub Pages, Netlify, Cloudflare Pages and S3 all look for.

use crate::routes::{self, Kind};
use crate::views::{community, design, docs, home, not_found, og};
use crate::{app, assets, content, site};
use std::fs;
use std::path::{Path, PathBuf};

pub fn build(out: &Path, base_url: &str) -> std::io::Result<usize> {
    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;

    let mut written = 0;
    for route in routes::all() {
        let target = match route.kind {
            Kind::Page => out
                .join(route.path.trim_start_matches('/'))
                .join("index.html"),
            Kind::File | Kind::Binary => out.join(route.path.trim_start_matches('/')),
        };
        match route.kind {
            // A font is not text and must not be routed through a `String`.
            Kind::Binary => {
                let face =
                    crate::fonts::find(&route.path).expect("the route table lists a real face");
                write_bytes(&target, face.bytes)?;
            }
            _ => write(&target, &render(&route.path, base_url))?,
        }
        written += 1;
    }

    // The catch-all, under the name static hosts look for.
    write(&out.join("404.html"), &render("/404", base_url))?;
    written += 1;

    // Headers for hosts that read this file — Netlify, Cloudflare Pages. Hosts
    // that do not will ignore it rather than fail on it. The list is the one
    // the server applies, so a deploy cannot be weaker than a local run.
    let mut headers = String::from("/*\n");
    for (name, value) in site::HEADERS {
        headers.push_str(&format!("  {name}: {value}\n"));
    }
    // Documents are not content-hashed, so they revalidate; the hashed assets
    // under /assets are kept forever. Same split the server applies.
    headers.push_str("  Cache-Control: public, max-age=0, must-revalidate\n");
    headers.push_str("\n/assets/*\n  Cache-Control: public, max-age=31536000, immutable\n");
    write(&out.join("_headers"), &headers)?;

    // GitHub Pages otherwise runs the output through Jekyll, which silently
    // drops files and directories beginning with an underscore.
    write(&out.join(".nojekyll"), "")?;

    Ok(written)
}

/// One route, rendered. Deliberately a `match` over paths rather than a call
/// into the axum router: the router is async and needs a request, and the two
/// things a static export must never do are start a server and guess.
fn render(path: &str, base_url: &str) -> String {
    let shell = |meta, body| app::page(base_url, meta, body).0;

    match path {
        "/" => shell(home::meta(), home::render()),
        "/docs" => shell(docs::index_meta(), docs::index()),
        "/roadmap" => shell(docs::roadmap_meta(), docs::roadmap()),
        "/design" => shell(design::meta(), design::render()),
        "/community" => shell(community::index_meta(), community::index()),
        "/404" => shell(not_found::meta(), not_found::render()),
        "/sitemap.xml" => routes::sitemap(base_url),
        "/robots.txt" => routes::robots(base_url),
        "/favicon.svg" => og::favicon().into_string(),
        "/og.svg" => og::card(site::TAGLINE).into_string(),
        _ if path == *assets::CSS_PATH => assets::CSS.clone(),
        _ if path == *assets::JS_PATH => assets::JS.clone(),
        _ => {
            if let Some(slug) = path.strip_prefix("/community/") {
                let doc = content::community(slug).expect("route table lists a real page");
                shell(community::doc_meta(doc), community::doc(doc))
            } else {
                let slug = path
                    .strip_prefix("/docs/")
                    .expect("route table is exhaustive");
                let doc = content::find(slug).expect("route table lists a real page");
                shell(docs::doc_meta(doc), docs::doc(doc))
            }
        }
    }
}

fn write(target: &PathBuf, body: &str) -> std::io::Result<()> {
    write_bytes(target, body.as_bytes())
}

fn write_bytes(target: &PathBuf, body: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route renders without panicking, and produces something. This is
    /// the whole-site smoke test: it walks the same list the server routes and
    /// the export writes, so a page added to one is exercised here too.
    #[test]
    fn every_route_renders() {
        for route in routes::all() {
            if route.kind == Kind::Binary {
                continue;
            }
            let body = render(&route.path, "https://poptop.dev");
            // robots.txt is legitimately three lines; everything else is a
            // document or a stylesheet.
            let floor = if route.path == "/robots.txt" { 40 } else { 500 };
            assert!(
                body.len() > floor,
                "{} rendered {} bytes",
                route.path,
                body.len()
            );
        }
    }

    /// The deployed artifact, not the code that writes it. A static host reads
    /// `_headers`; if the policy the server applies never reaches that file,
    /// the deployed site is the weaker of the two and nothing says so.
    #[test]
    fn the_exported_headers_file_carries_the_whole_policy() {
        let dir = std::env::temp_dir().join(format!("poptop-web-headers-{}", std::process::id()));
        build(&dir, "https://poptop.dev").expect("export");
        let headers = fs::read_to_string(dir.join("_headers")).expect("_headers");
        for (name, value) in site::HEADERS {
            assert!(
                headers.contains(&format!("{name}: {value}")),
                "missing {name}"
            );
        }
        assert!(
            headers.contains("max-age=31536000, immutable"),
            "assets are not cached"
        );
        assert!(
            headers.contains("max-age=0, must-revalidate"),
            "documents never revalidate"
        );
        assert!(dir.join(".nojekyll").exists(), "Pages would eat /_headers");
        assert!(
            dir.join("404.html").exists(),
            "no 404 page for the host to serve"
        );
        fs::remove_dir_all(&dir).ok();
    }

    /// The provenance rule, enforced. The landing page's frames are drawn from
    /// a buffer generated by `demo.rs`, not captured from a machine, and the
    /// page has to say so where the claim is made.
    ///
    /// This exists because it once stopped saying so. A rewrite of the hero
    /// replaced the caption that carried the disclosure and the page spent a
    /// working session calling seeded data a "recorded poptop session" — on a
    /// site whose whole argument is that it tells you what it cannot do.
    #[test]
    fn the_landing_page_says_where_its_data_came_from() {
        let page = render("/", "https://poptop.dev");
        assert!(
            page.contains("generated from a fixed seed"),
            "the landing page no longer says its buffer is generated"
        );
        assert!(
            page.contains("not recorded from a real"),
            "the landing page no longer distinguishes generated from recorded"
        );
        // And it must not claim to be a recording anywhere, including in the
        // accessible name of the frame, which is where it went wrong last time.
        assert!(
            !page.contains("A recorded poptop session"),
            "the frame is described as a recording again"
        );
    }

    #[test]
    fn every_page_carries_its_canonical_url() {
        for path in routes::pages() {
            let body = render(&path, "https://poptop.dev");
            assert!(
                body.contains(&format!(
                    "<link rel=\"canonical\" href=\"https://poptop.dev{path}\">"
                )),
                "{path} has no canonical link"
            );
        }
    }

    /// Nothing on the site may point at a page the site does not serve. This
    /// catches a renamed slug in a hand-written link, which is the single most
    /// common way a documentation site rots.
    #[test]
    fn no_internal_link_is_broken() {
        let known: std::collections::HashSet<String> = routes::all()
            .into_iter()
            .map(|r| r.path)
            .chain(["/healthz".to_string(), "/404".to_string()])
            .collect();

        for path in routes::pages() {
            let body = render(&path, "https://poptop.dev");
            for href in hrefs(&body) {
                if !href.starts_with('/') || href.starts_with("//") {
                    continue;
                }
                let target = href.split(['#', '?']).next().unwrap_or(&href).to_string();
                let target = target.trim_end_matches('/').to_string();
                let target = if target.is_empty() {
                    "/".to_string()
                } else {
                    target
                };
                assert!(
                    known.contains(&target),
                    "{path} links to {target}, which the site does not serve"
                );
            }
        }
    }

    fn hrefs(html: &str) -> Vec<String> {
        html.match_indices("href=\"")
            .filter_map(|(at, _)| {
                let rest = &html[at + 6..];
                rest.find('"').map(|end| rest[..end].to_string())
            })
            .collect()
    }
}
