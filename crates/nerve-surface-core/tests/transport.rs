//! The client against a real hub.
//!
//! Every other test in this crate drives a fake `FrameSource`, which proves the
//! reconnect and status rules but never touches a socket. This one binds a real
//! `nerve-hub` on an ephemeral port and talks to it with the real client, so
//! chunked framing, the event-stream parser and the status handling are
//! exercised against the server that produces them.
//!
//! Port `0` throughout: 17890 belongs to the hub a developer is actually
//! running.

use std::sync::Arc;

use axum::extract::ConnectInfo;
use axum::http::Request;
use nerve_hub::HubRuntime;
use nerve_hub::clock::FakeClock;
use nerve_hub::state::{JobStore, PidProbe, PidState};
use nerve_surface_core::frame::Frame;
use nerve_surface_core::hub::{Hub, HubError};
use serde_json::{Value, json};
use std::net::SocketAddr;
use time::macros::datetime;
use tokio::net::TcpListener;
use tower::ServiceExt;

/// Never reaps: these tests are about the wire, not the reaper.
struct EveryPidAlive;

impl PidProbe for EveryPidAlive {
    fn state(&self, _pid: i32) -> PidState {
        PidState::Alive
    }
}

fn runtime() -> HubRuntime {
    let clock = Arc::new(FakeClock::new(datetime!(2026-07-19 00:00:00 UTC)));
    HubRuntime::new(JobStore::new(
        clock,
        "test-mac".to_string(),
        Arc::new(EveryPidAlive),
    ))
}

/// Serve on an ephemeral loopback port.
///
/// `into_make_service_with_connect_info` installs the peer address the loopback
/// guard reads; without it every request is refused.
async fn serve(runtime: &HubRuntime) -> SocketAddr {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("an ephemeral loopback port must be bindable");
    let address = listener
        .local_addr()
        .expect("a bound listener knows itself");
    assert_ne!(address.port(), 17_890, "never take the hub's real port");
    let service = runtime
        .router()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        axum::serve(listener, service).await.expect("serve");
    });
    address
}

/// A producer POST through the same runtime, so writes need no second socket.
async fn post(runtime: &HubRuntime, uri: &str, body: Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .expect("test request builds");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50_000))));
    let response = runtime.router().oneshot(request).await.expect("post");
    assert!(
        response.status().is_success(),
        "producer POST failed: {}",
        response.status()
    );
}

/// A full wire job — the ingest contract rejects a partial one, which is the
/// point: these tests must post what a real producer posts.
fn snapshot(name: &str) -> Value {
    const T0: &str = "2026-07-19T00:00:00Z";
    json!({
        "alias": "test-mac",
        "machineKind": "darwin",
        "jobs": [{
            "id": format!("demo:{name}"),
            "kind": "session",
            "name": name,
            "alias": "test-mac",
            "lifecycle": "active",
            "current": { "type": "tool", "summary": "Using Bash" },
            "attention": { "level": "none" },
            "health": "ok",
            "progress": { "kind": "none" },
            "producer": { "id": "t" },
            "capabilities": [],
            "actions": [],
            "createdAt": T0,
            "startedAt": T0,
            "updatedAt": T0,
            "version": 1,
            "extensions": {}
        }]
    })
}

/// The stream, end to end. This is the path a hand-rolled reader got quietly
/// wrong: the hub's body is chunked, and a chunk boundary may fall anywhere —
/// including inside a `data:` line.
#[tokio::test(flavor = "multi_thread")]
async fn the_event_stream_delivers_a_decodable_frame() {
    let hub_runtime = runtime();
    let address = serve(&hub_runtime).await;
    post(&hub_runtime, "/v1/snapshot", snapshot("nerve")).await;

    let base = format!("http://{address}");
    let frame = tokio::task::spawn_blocking(move || {
        let hub = Hub::at(base);
        let mut events = hub.open("/v1/stream?surface=test").expect("stream opens");
        events.next_payload()
    })
    .await
    .expect("join")
    .expect("the stream yields")
    .expect("a connect frame arrives");

    let frame = Frame::decode(&frame).expect("the payload is a frame");
    assert_eq!(frame.jobs.len(), 1);
    assert_eq!(frame.jobs[0].name, "nerve");
}

/// Nothing listening is a different state from a connection that broke: one
/// paints "offline", the other reconnects.
#[tokio::test(flavor = "multi_thread")]
async fn a_closed_port_is_unreachable_rather_than_broken() {
    // Bind and drop, so the port is almost certainly free and nothing answers.
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let address = listener.local_addr().expect("addr");
    drop(listener);

    let base = format!("http://{address}");
    let error = tokio::task::spawn_blocking(move || Hub::at(base).get("/v1/jobs"))
        .await
        .expect("join")
        .expect_err("a closed port has no body");

    assert!(
        matches!(error, HubError::Unreachable(_)),
        "expected unreachable, got {error:?}"
    );
}
