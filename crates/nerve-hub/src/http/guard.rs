//! Cross-cutting request rules: who may talk to the hub, how much they may
//! send, and the header every answer carries.
//!
//! These run as layers rather than inside handlers so that a route added later
//! cannot forget one, and so that they apply to the fallbacks too — Swift
//! checks the peer in `respond`, before it has
//! looked at the request line at all.

use std::net::SocketAddr;

use axum::Json;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// The largest body the hub will read.
///
/// A body of exactly this size is accepted; one byte more is refused.
pub(super) const MAX_BODY_BYTES: usize = 1024 * 1024;

/// `Access-Control-Allow-Origin: *` on every response, whatever its status.
///
/// The hub is loopback-only, so this opens nothing a browser could not already
/// reach; it exists so a local page can read the job list without a proxy.
pub(super) async fn allow_any_origin(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

/// Refuse anyone who is not on this machine.
///
/// `IpAddr::is_loopback` is exactly Swift's rule: the whole `127.0.0.0/8` block
/// for IPv4 and `::1` alone for IPv6.
///
/// A request with no peer is refused rather than waved through: the hub cannot
/// prove such a request is local. In production it cannot happen — the listener
/// binds loopback and the service installs `ConnectInfo` — so refusing costs
/// nothing and keeps the guard honest if the assembly ever drifts.
pub(super) async fn loopback_only(request: Request, next: Next) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(peer)| *peer);

    match peer {
        Some(peer) if peer.ip().is_loopback() => next.run(request).await,
        _ => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "loopback only" })),
        )
            .into_response(),
    }
}

/// Read the body once, under the ceiling, and hand the collected bytes on.
///
/// Buffering here rather than in each handler keeps the limit in one place and
/// gives every handler a `Bytes` body it can decode more than once.
///
/// Reading can also fail because a producer died mid-body. That case is not
/// told apart from the ceiling: only one of the two has a reader left to
/// receive the answer, so both get the ceiling's literal.
pub(super) async fn limit_body(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let Ok(body) = axum::body::to_bytes(body, MAX_BODY_BYTES).await else {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({ "error": "too large" })),
        )
            .into_response();
    };
    next.run(Request::from_parts(parts, Body::from(body))).await
}
