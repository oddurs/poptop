//! The router.
//!
//! One handler per page, a catch-all that renders a real 404 rather than a
//! line of plain text, and three layers: compression, immutable caching for
//! the hashed assets, and request tracing.

use crate::views::layout::Meta;
use crate::views::{community, design, docs, home, layout, not_found, og};
use crate::{assets, content, routes, site};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use maud::Markup;
use std::sync::Arc;

#[derive(Clone)]
pub struct Site {
    pub base_url: Arc<str>,
}

pub fn router(base_url: &str) -> Router {
    use tower_http::CompressionLevel;
    use tower_http::compression::CompressionLayer;
    use tower_http::set_header::SetResponseHeaderLayer;
    use tower_http::trace::TraceLayer;

    let state = Site {
        base_url: Arc::from(base_url),
    };

    // The asset URLs contain a hash of their own contents, so they can never
    // be stale and never need revalidating.
    let mut immutable = Router::new()
        .route(&assets::CSS_PATH, get(css))
        .route(&assets::JS_PATH, get(js));
    for face in crate::fonts::FACES.iter() {
        let bytes = face.bytes;
        immutable = immutable.route(
            &face.path(),
            get(move || async move { ([(header::CONTENT_TYPE, "font/woff2")], bytes) }),
        );
    }
    let immutable = immutable.layer(SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    ));

    let routes = Router::new()
        .route("/", get(index))
        .route("/docs", get(docs_index))
        .route("/docs/{*slug}", get(docs_page))
        .route("/roadmap", get(roadmap))
        .route("/design", get(design_page))
        .route("/community", get(community_index))
        .route("/community/{slug}", get(community_page))
        .route("/sitemap.xml", get(sitemap))
        .route("/robots.txt", get(robots))
        .route("/favicon.svg", get(favicon))
        .route("/og.svg", get(og_card))
        .route("/healthz", get(|| async { "ok" }))
        .merge(immutable)
        .fallback(not_found);

    // Documents are not hashed, so they revalidate on every visit — cheap,
    // because a 304 is a header exchange — while the hashed assets above are
    // kept forever. Without this a proxy is free to invent its own freshness
    // and serve yesterday's page after a deploy.
    let routes = routes.layer(SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=0, must-revalidate"),
    ));

    // Applied to every response, documents and assets alike.
    let router = site::HEADERS.iter().fold(routes, |router, (name, value)| {
        router.layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        ))
    });

    router
        // Level 6 rather than the default. Brotli at its default quality
        // loses to gzip on files this size, which makes advertising it a
        // slight pessimisation; at 6 it wins by about a fifth and still costs
        // under a millisecond on a document this small.
        .layer(CompressionLayer::new().quality(CompressionLevel::Precise(6)))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Render a page inside the shell. Every handler goes through here.
pub fn page(base_url: &str, meta: Meta, body: Markup) -> Html<String> {
    Html(layout::page(base_url, &meta, body).into_string())
}

async fn index(State(site): State<Site>) -> impl IntoResponse {
    page(&site.base_url, home::meta(), home::render())
}

async fn docs_index(State(site): State<Site>) -> impl IntoResponse {
    page(&site.base_url, docs::index_meta(), docs::index())
}

async fn docs_page(State(site): State<Site>, Path(slug): Path<String>) -> Response {
    // Trailing slashes arrive from links written by hand and from crawlers.
    match content::find(slug.trim_end_matches('/')) {
        Some(doc) => page(&site.base_url, docs::doc_meta(doc), docs::doc(doc)).into_response(),
        None => not_found(State(site)).await.into_response(),
    }
}

async fn roadmap(State(site): State<Site>) -> impl IntoResponse {
    page(&site.base_url, docs::roadmap_meta(), docs::roadmap())
}

async fn design_page(State(site): State<Site>) -> impl IntoResponse {
    page(&site.base_url, design::meta(), design::render())
}

async fn community_index(State(site): State<Site>) -> impl IntoResponse {
    page(&site.base_url, community::index_meta(), community::index())
}

async fn community_page(State(site): State<Site>, Path(slug): Path<String>) -> Response {
    match content::community(slug.trim_end_matches('/')) {
        Some(doc) => page(
            &site.base_url,
            community::doc_meta(doc),
            community::doc(doc),
        )
        .into_response(),
        None => not_found(State(site)).await.into_response(),
    }
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        assets::CSS.as_str(),
    )
}

async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        assets::JS.as_str(),
    )
}

async fn sitemap(State(site): State<Site>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/xml")],
        routes::sitemap(&site.base_url),
    )
}

async fn robots(State(site): State<Site>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        routes::robots(&site.base_url),
    )
}

async fn favicon() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml")],
        og::favicon().into_string(),
    )
}

async fn og_card() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml")],
        og::card(site::TAGLINE).into_string(),
    )
}

/// The only thing this adds to the page itself is the status code.
pub async fn not_found(State(site): State<Site>) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        page(&site.base_url, not_found::meta(), not_found::render()),
    )
}
