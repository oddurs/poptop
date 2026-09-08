//! Every URL the site serves, in one list.
//!
//! The router and the static export both walk this, so a page that exists in
//! one and not the other is not a thing that can happen. `poptop-web routes`
//! prints it, which is also what the deploy uses to check nothing 404s.

use crate::content;

/// What a route produces. Kept separate from the rendering so the export can
/// choose filenames — `/docs/keys` becomes `docs/keys/index.html`, `/og.svg`
/// stays `og.svg`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An HTML page, exported as `<path>/index.html`.
    Page,
    /// A file with its own extension, exported at exactly its path.
    File,
    /// A file whose contents are not text — exported byte for byte.
    Binary,
}

pub struct Route {
    pub path: String,
    pub kind: Kind,
}

pub fn all() -> Vec<Route> {
    let page = |p: &str| Route {
        path: p.to_string(),
        kind: Kind::Page,
    };
    let file = |p: &str| Route {
        path: p.to_string(),
        kind: Kind::File,
    };

    let mut routes = vec![
        page("/"),
        page("/docs"),
        page("/design"),
        page("/community"),
        page("/roadmap"),
    ];
    routes.extend(content::DOCS.iter().map(|d| page(&d.path())));
    routes.extend(content::COMMUNITY.iter().map(|d| page(&d.path())));
    routes.extend(crate::fonts::FACES.iter().map(|f| Route {
        path: f.path(),
        kind: Kind::Binary,
    }));
    routes.extend([
        file("/sitemap.xml"),
        file("/robots.txt"),
        file("/favicon.svg"),
        file("/og.svg"),
        file(&crate::assets::CSS_PATH),
        file(&crate::assets::JS_PATH),
    ]);
    routes
}

/// The pages that belong in a sitemap: the HTML ones.
pub fn pages() -> impl Iterator<Item = String> {
    all()
        .into_iter()
        .filter(|r| r.kind == Kind::Page)
        .map(|r| r.path)
}

pub fn sitemap(base_url: &str) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for path in pages() {
        // The landing page is the entry point; documentation outranks the
        // roadmap, which changes weekly and is not what anyone is searching for.
        let priority = match path.as_str() {
            "/" => "1.0",
            p if p.starts_with("/docs") => "0.8",
            _ => "0.5",
        };
        out.push_str(&format!(
            "  <url><loc>{base_url}{path}</loc><priority>{priority}</priority></url>\n"
        ));
    }
    out.push_str("</urlset>\n");
    out
}

pub fn robots(base_url: &str) -> String {
    format!("User-agent: *\nAllow: /\n\nSitemap: {base_url}/sitemap.xml\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documentation_page_is_routable() {
        let paths: Vec<String> = all().into_iter().map(|r| r.path).collect();
        for doc in content::DOCS.iter().chain(content::COMMUNITY) {
            assert!(paths.contains(&doc.path()), "{} is not routed", doc.slug);
        }
    }

    #[test]
    fn no_route_is_listed_twice() {
        let mut seen = std::collections::HashSet::new();
        for route in all() {
            assert!(
                seen.insert(route.path.clone()),
                "duplicate route {}",
                route.path
            );
        }
    }

    #[test]
    fn the_sitemap_holds_pages_and_not_assets() {
        let xml = sitemap("https://poptop.dev");
        assert!(xml.contains("<loc>https://poptop.dev/docs/keys</loc>"));
        assert!(!xml.contains(".css"));
        assert!(!xml.contains(".woff2"));
        assert!(!xml.contains("robots"));
    }

    #[test]
    fn every_embedded_face_is_routed() {
        let paths: Vec<String> = all().into_iter().map(|r| r.path).collect();
        for face in crate::fonts::FACES.iter() {
            assert!(paths.contains(&face.path()), "{} is not routed", face.slug);
        }
    }
}
