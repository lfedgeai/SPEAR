//! SPEAR Console handlers / SPEAR Console 前端处理器
//!
//! This module serves the user-facing web UI under /console.
//! 本模块用于在 /console 下提供面向用户的 Web UI。

use axum::extract::Path;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};

pub async fn console_index() -> Html<&'static str> {
    Html(include_str!("../../../assets/console/index.html"))
}

pub async fn console_static(headers: HeaderMap, Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    let enc = headers
        .get(axum::http::header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let (bytes, mime, content_encoding) = match (path, enc.as_str()) {
        ("main.js", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/console/main.js.br")).as_ref(),
            "application/javascript",
            Some("br"),
        ),
        ("main.js", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/console/main.js.gz")).as_ref(),
            "application/javascript",
            Some("gzip"),
        ),
        ("main.css", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/console/main.css.br")).as_ref(),
            "text/css",
            Some("br"),
        ),
        ("main.css", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/console/main.css.gz")).as_ref(),
            "text/css",
            Some("gzip"),
        ),
        ("index.html", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/console/index.html.br")).as_ref(),
            "text/html",
            Some("br"),
        ),
        ("index.html", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/console/index.html.gz")).as_ref(),
            "text/html",
            Some("gzip"),
        ),
        ("main.js", _) => (
            include_bytes!("../../../assets/console/main.js").as_ref(),
            "application/javascript",
            None,
        ),
        ("main.css", _) => (
            include_bytes!("../../../assets/console/main.css").as_ref(),
            "text/css",
            None,
        ),
        ("index.html", _) => (
            include_bytes!("../../../assets/console/index.html").as_ref(),
            "text/html",
            None,
        ),
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };

    let mut resp = axum::response::Response::new(bytes.into());
    resp.headers_mut()
        .insert(CONTENT_TYPE, mime.parse().unwrap());
    let cache = match path {
        "index.html" | "main.js" | "main.css" => "no-cache, no-store, must-revalidate",
        _ => "public, max-age=31536000",
    };
    resp.headers_mut()
        .insert(CACHE_CONTROL, cache.parse().unwrap());
    resp.headers_mut()
        .insert(axum::http::header::VARY, "Accept-Encoding".parse().unwrap());
    if let Some(ce) = content_encoding {
        resp.headers_mut()
            .insert(axum::http::header::CONTENT_ENCODING, ce.parse().unwrap());
    }
    resp
}
