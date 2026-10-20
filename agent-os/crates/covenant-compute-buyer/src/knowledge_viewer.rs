//! A private, project-bound viewer. No filesystem paths or upstream URLs are
//! accepted by HTTP. All static content is embedded in the released binary.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use super::{validate_query, QueryContext};

#[derive(Clone)]
struct ViewerState {
    context: QueryContext,
    authority: String,
    origin: String,
    permits: Arc<Semaphore>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryRequest {
    operation: String,
    #[serde(default = "empty_args")]
    arguments: Value,
}
fn empty_args() -> Value {
    json!({})
}

fn headers_allowed(headers: &HeaderMap, authority: &str, origin: &str) -> bool {
    // Do not derive trust from proxy-supplied Host/Origin or client addresses.
    // Refuse common forwarding signals even when a local proxy reaches us.
    if headers.keys().any(|name| {
        let name = name.as_str();
        name == "forwarded"
            || name.starts_with("x-forwarded-")
            || name == "x-real-ip"
            || name == "via"
    }) {
        return false;
    }
    if headers.get_all(header::HOST).iter().count() != 1
        || headers.get(header::HOST).and_then(|v| v.to_str().ok()) != Some(authority)
    {
        return false;
    }
    if headers.get_all(header::ORIGIN).iter().count() > 1 {
        return false;
    }
    if let Some(value) = headers.get(header::ORIGIN) {
        if value.to_str().ok() != Some(origin) {
            return false;
        }
    }
    if let Some(value) = headers.get("sec-fetch-site") {
        if !matches!(value.to_str().ok(), Some("same-origin" | "none")) {
            return false;
        }
    }
    true
}
fn secure_headers(response: &mut Response) {
    for (name,value) in [
        ("cache-control","no-store, private"),
        ("content-security-policy","default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"),
        ("referrer-policy","no-referrer"),
        ("x-content-type-options","nosniff"),
        ("x-frame-options","DENY"),
        ("cross-origin-resource-policy","same-origin"),
        ("cross-origin-opener-policy","same-origin"),
    ] {response.headers_mut().insert(name,HeaderValue::from_static(value));}
}
async fn guard(State(state): State<Arc<ViewerState>>, request: Request, next: Next) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|v| v.0.ip());
    let allowed = peer.is_some_and(|ip| ip.is_loopback())
        && headers_allowed(request.headers(), &state.authority, &state.origin);
    let mut response = if allowed {
        next.run(request).await
    } else {
        (StatusCode::FORBIDDEN, "private local knowledge session").into_response()
    };
    secure_headers(&mut response);
    response
}
async fn query(
    State(state): State<Arc<ViewerState>>,
    Json(request): Json<QueryRequest>,
) -> Response {
    if let Err(error) = validate_query(&request.operation, &request.arguments, true) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":error.to_string()})),
        )
            .into_response();
    }
    let Ok(permit) = Arc::clone(&state.permits).try_acquire_owned() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"another query is running; try again"})),
        )
            .into_response();
    };
    let context = state.context.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        context.query(&request.operation, &request.arguments)
    })
    .await
    {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":error.to_string()})),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"knowledge query could not complete"})),
        )
            .into_response(),
    }
}
fn asset(bytes: &'static [u8], content_type: &'static str) -> Response {
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}
fn router(state: Arc<ViewerState>, base: &str) -> Router {
    Router::new()
        .route(
            &format!("{base}/"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/index.html"),
                    "text/html; charset=utf-8",
                )
            }),
        )
        .route(
            &format!("{base}/style.css"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/style.css"),
                    "text/css; charset=utf-8",
                )
            }),
        )
        .route(
            &format!("{base}/app.js"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/app.js"),
                    "text/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            &format!("{base}/archivo.ttf"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/archivo-expanded-600.ttf"),
                    "font/ttf",
                )
            }),
        )
        .route(
            &format!("{base}/archivo-body.woff2"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/archivo-body.woff2"),
                    "font/woff2",
                )
            }),
        )
        .route(
            &format!("{base}/mono.ttf"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/jetbrains-mono-500.ttf"),
                    "font/ttf",
                )
            }),
        )
        .route(
            &format!("{base}/archivo-license.txt"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/OFL-Archivo.txt"),
                    "text/plain; charset=utf-8",
                )
            }),
        )
        .route(
            &format!("{base}/mono-license.txt"),
            get(|| async {
                asset(
                    include_bytes!("knowledge_viewer/OFL-JetBrainsMono.txt"),
                    "text/plain; charset=utf-8",
                )
            }),
        )
        .route(&format!("{base}/api/query"), post(query))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn_with_state(Arc::clone(&state), guard))
        .with_state(state)
}

pub(super) async fn serve(context: QueryContext, port: u16, json_out: bool) -> Result<()> {
    // Resolve once before announcing readiness. Server never receives selectors.
    context.query("status", &json!({}))?;
    let listener =
        tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
            .await
            .context("bind local knowledge viewer")?;
    let authority = listener.local_addr()?.to_string();
    let origin = format!("http://{authority}");
    let session = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let base = format!("/session/{session}");
    let url = format!("{origin}{base}/");
    let state = Arc::new(ViewerState {
        context: context.clone(),
        authority,
        origin,
        permits: Arc::new(Semaphore::new(2)),
    });
    let app = router(state, &base);
    if json_out {
        println!(
            "{}",
            json!({"url":url,"project":context.project,"snapshot":context.snapshot,"read_only":true,"loopback_only":true})
        );
    } else {
        println!("Covenant Compute · Knowledge\n{url}\n\nPrivate, read-only local session. Press Ctrl-C to close.");
    }
    use std::io::Write;
    std::io::stdout().flush()?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .context("serve local knowledge viewer")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_origin_and_proxy_headers_are_exact() {
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("127.0.0.1:1234"));
        assert!(headers_allowed(
            &headers,
            "127.0.0.1:1234",
            "http://127.0.0.1:1234"
        ));
        for value in [
            "http://evil.test",
            "null",
            "http://localhost:1234",
            "https://127.0.0.1:1234",
        ] {
            headers.insert("origin", HeaderValue::from_str(value).unwrap());
            assert!(!headers_allowed(
                &headers,
                "127.0.0.1:1234",
                "http://127.0.0.1:1234"
            ));
        }
        headers.insert("origin", HeaderValue::from_static("http://127.0.0.1:1234"));
        assert!(headers_allowed(
            &headers,
            "127.0.0.1:1234",
            "http://127.0.0.1:1234"
        ));
        for name in [
            "forwarded",
            "x-forwarded-host",
            "x-forwarded-for",
            "x-forwarded-proto",
            "x-real-ip",
            "via",
        ] {
            headers.insert(name, HeaderValue::from_static("anything"));
            assert!(!headers_allowed(
                &headers,
                "127.0.0.1:1234",
                "http://127.0.0.1:1234"
            ));
            headers.remove(name);
        }
        headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!headers_allowed(
            &headers,
            "127.0.0.1:1234",
            "http://127.0.0.1:1234"
        ));
        headers.remove("sec-fetch-site");
        headers.insert("host", HeaderValue::from_static("knowledge.example"));
        assert!(!headers_allowed(
            &headers,
            "127.0.0.1:1234",
            "http://127.0.0.1:1234"
        ));
    }
}
