//! Protocol-v2 SSE contract: full snapshots, terminal hints and recovery.
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use time::macros::datetime;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tower::ServiceExt;

use nerve_hub::HubRuntime;
use nerve_hub::clock::FakeClock;
use nerve_hub::state::{JobStore, PidProbe, PidState};

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Wire instant every fixture is anchored to, and therefore the `endedAt` the
/// hub fills in from the frozen clock.
const T0: &str = "2026-07-19T00:00:00Z";

/// The alias injected into `JobStore::new` — "this machine" for the hub.
const MACHINE_ALIAS: &str = "test-mac";

/// How long a test waits for a frame it expects. Generous: it only has to
/// exceed the coalesce window, never to measure it.
const FRAME_WAIT: Duration = Duration::from_secs(2);

/// Silence that means "the burst is over". Comfortably past the ~150 ms window.
const QUIET: Duration = Duration::from_millis(400);

fn store() -> JobStore {
    let clock = Arc::new(FakeClock::new(datetime!(2026-07-19 00:00:00 UTC)));
    JobStore::new(clock, MACHINE_ALIAS.to_string(), Arc::new(EveryPidAlive))
}

/// A resident runtime for frame tests.
fn runtime() -> HubRuntime {
    HubRuntime::new(store())
}

/// Liveness oracle that never reaps: these tests are about frames, not the reaper.
struct EveryPidAlive;

impl PidProbe for EveryPidAlive {
    fn state(&self, _pid: i32) -> PidState {
        PidState::Alive
    }
}

/// Full wire job with `overrides` applied on top, so each test states only the
/// fields it is about.
fn job(id: &str, overrides: Value) -> Value {
    let base = json!({
        "id": id,
        "kind": "session",
        "name": id,
        "alias": MACHINE_ALIAS,
        "lifecycle": "active",
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
    });
    merged(base, overrides)
}

fn event(id: &str, job_id: &str, kind: &str, overrides: Value) -> Value {
    let base = json!({
        "id": id,
        "jobId": job_id,
        "kind": kind,
        "timestamp": T0,
        "producerId": "t"
    });
    merged(base, overrides)
}

/// Shallow key overwrite — fixtures are flat enough that nested merging would
/// only hide which object a test actually asserts on.
fn merged(base: Value, overrides: Value) -> Value {
    let (Value::Object(mut base), Value::Object(overrides)) = (base, overrides) else {
        panic!("fixture and overrides must both be JSON objects");
    };
    for (key, value) in overrides {
        base.insert(key, value);
    }
    Value::Object(base)
}

// ── Driving the runtime ─────────────────────────────────────────────────────

/// Serve this runtime on an ephemeral loopback port.
///
/// `into_make_service_with_connect_info` is what installs the peer address the
/// loopback guard reads; without it every request would be refused. Port `0`
/// keeps the test off 17890, which belongs to the real hub.
async fn serve(runtime: &HubRuntime) -> SocketAddr {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("an ephemeral loopback port must be bindable");
    let address = listener
        .local_addr()
        .expect("a bound listener must know its address");
    assert_ne!(
        address.port(),
        17_890,
        "tests must never take the hub's single-instance port"
    );
    let service = runtime
        .router()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        axum::serve(listener, service)
            .await
            .expect("the test server must serve until the test ends");
    });
    address
}

/// A producer POST through the same runtime the stream is served from.
async fn post(runtime: &HubRuntime, uri: &str, body: Value) -> Value {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("test request must build");
    // What `into_make_service_with_connect_info::<SocketAddr>()` installs in
    // production; injected here so writes need no second socket.
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 54_321))));

    let response = runtime
        .router()
        .oneshot(request)
        .await
        .expect("router must answer every request");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body must be readable")
        .to_bytes();
    assert_eq!(status, StatusCode::OK, "{uri} answered {status}");
    serde_json::from_slice(&body).unwrap_or_else(|err| {
        panic!(
            "response body is not JSON ({err}): {}",
            String::from_utf8_lossy(&body)
        )
    })
}

/// POST a snapshot and insist the hub read all of it, so a frame that never
/// arrives is never blamed on a rejected write.
async fn snapshot(runtime: &HubRuntime, jobs: Vec<Value>) {
    let count = jobs.len();
    let reply = post(
        runtime,
        "/v1/snapshot",
        json!({ "alias": MACHINE_ALIAS, "jobs": jobs }),
    )
    .await;
    assert_eq!(reply, json!({ "applied": count }));
}

async fn events(runtime: &HubRuntime, events: Vec<Value>) {
    let count = events.len();
    let reply = post(
        runtime,
        "/v1/events",
        json!({ "alias": MACHINE_ALIAS, "events": events }),
    )
    .await;
    assert_eq!(reply, json!({ "applied": count }));
}

/// `GET /v1/jobs`, parsed — the shape a frame's `jobs` must equal.
async fn jobs_of(runtime: &HubRuntime) -> Value {
    let mut request = Request::builder()
        .method("GET")
        .uri("/v1/jobs")
        .body(Body::empty())
        .expect("test request must build");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 54_321))));

    let response = runtime
        .router()
        .oneshot(request)
        .await
        .expect("router must answer every request");
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body must be readable")
        .to_bytes();
    serde_json::from_slice(&body).expect("`GET /v1/jobs` must answer JSON")
}

// ── A hand-written SSE client ───────────────────────────────────────────────

/// One live subscription, held open until it is dropped.
struct SseClient {
    reader: BufReader<TcpStream>,
}

impl SseClient {
    async fn connect(address: SocketAddr, query: &str) -> Self {
        let mut socket = TcpStream::connect(address)
            .await
            .expect("the hub must accept a stream client");
        let request = format!(
            "GET /v1/stream{query} HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\n\r\n"
        );
        socket
            .write_all(request.as_bytes())
            .await
            .expect("the stream request must be writable");

        let mut reader = BufReader::new(socket);
        let status = read_line(&mut reader, FRAME_WAIT)
            .await
            .expect("the hub must answer `GET /v1/stream`");
        assert!(
            status.starts_with("HTTP/1.1 200"),
            "`GET /v1/stream` answered `{}`",
            status.trim_end()
        );

        let mut content_type = String::new();
        loop {
            let header = read_line(&mut reader, FRAME_WAIT)
                .await
                .expect("headers must end with a blank line");
            let header = header.trim_end().to_ascii_lowercase();
            if header.is_empty() {
                break;
            }
            if let Some(value) = header.strip_prefix("content-type:") {
                content_type = value.trim().to_string();
            }
        }
        assert!(
            content_type.starts_with("text/event-stream"),
            "`GET /v1/stream` is not an event stream: `{content_type}`"
        );

        Self { reader }
    }

    /// The next frame, or a failed test.
    async fn frame(&mut self) -> Value {
        self.try_frame(FRAME_WAIT)
            .await
            .expect("the hub must push a frame")
    }

    /// The next frame, or `None` when the stream stays quiet for `within`.
    async fn try_frame(&mut self, within: Duration) -> Option<Value> {
        loop {
            let line = read_line(&mut self.reader, within).await?;
            // Tolerant on purpose: chunked-encoding size lines, `:` comments,
            // `event:` / `id:` lines and the blank line that ends an event are
            // all skipped. Only `data:` carries a frame.
            let Some(payload) = line.trim_end().strip_prefix("data:") else {
                continue;
            };
            let payload = payload.trim();
            return Some(serde_json::from_str(payload).unwrap_or_else(|err| {
                panic!("frame is not compact JSON on one `data:` line ({err}): {payload}")
            }));
        }
    }

    /// Every frame until the stream goes quiet — how a burst is counted.
    async fn frames_until_quiet(&mut self) -> Vec<Value> {
        let mut frames = Vec::new();
        while let Some(frame) = self.try_frame(QUIET).await {
            frames.push(frame);
        }
        frames
    }
}

/// One line, or `None` on quiet/EOF.
///
/// `read_line` is not cancellation safe, so a timeout could in principle drop a
/// half-read line. Every timeout here happens on an idle stream (the hub writes
/// a whole event at once), and no test reads on after a quiet timeout.
async fn read_line(reader: &mut BufReader<TcpStream>, within: Duration) -> Option<String> {
    let mut line = String::new();
    match timeout(within, reader.read_line(&mut line)).await {
        Err(_elapsed) => None,
        Ok(Ok(0)) => None,
        Ok(Ok(_)) => Some(line),
        Ok(Err(err)) => panic!("stream read failed: {err}"),
    }
}

// ── Assertion helpers ───────────────────────────────────────────────────────

fn array<'a>(value: &'a Value, what: &str) -> &'a Vec<Value> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("`{what}` must be a JSON array, got {value}"))
}

fn ids(jobs: &Value, what: &str) -> Vec<String> {
    array(jobs, what)
        .iter()
        .map(|job| {
            job.get("id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("job without an id in `{what}`: {job}"))
                .to_string()
        })
        .collect()
}

/// Every `departed` entry for `id` across a burst of frames.
fn departed_for<'a>(frames: &'a [Value], id: &str) -> Vec<&'a Value> {
    frames
        .iter()
        .flat_map(|frame| array(&frame["departed"], "departed"))
        .filter(|job| job.get("id").and_then(Value::as_str) == Some(id))
        .collect()
}

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .unwrap_or_else(|| panic!("missing `{pointer}` in {value}"))
        .as_str()
        .unwrap_or_else(|| panic!("`{pointer}` is not a string in {value}"))
        .to_string()
}

// ════════════════════════════════════════════════════════════════════════════
// 1. The first frame is a full resync (acceptance A6)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_connecting_pushes_the_full_job_list_at_once() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({})), job("a2", json!({}))]).await;
    let address = serve(&runtime).await;

    let mut stream = SseClient::connect(address, "").await;
    let frame = stream.frame().await;

    // Full set, not a delta: a surface that just connected knows everything.
    assert_eq!(ids(&frame["jobs"], "jobs"), vec!["a1", "a2"]);
    assert_eq!(frame["departed"], json!([]));
}

#[tokio::test]
async fn test_a_frame_carries_exactly_the_two_contract_keys() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    let address = serve(&runtime).await;

    let frame = SseClient::connect(address, "").await.frame().await;

    let object = frame
        .as_object()
        .unwrap_or_else(|| panic!("a frame must be a JSON object, got {frame}"));
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    // No delta protocol, no event-type tag, no cursor: jobs, departed, notify.
    assert_eq!(
        keys,
        [
            "departed",
            "hubInstanceId",
            "jobs",
            "machineId",
            "machines",
            "notifications",
            "notify",
            "order",
            "protocolVersion",
            "revision",
            "summary",
            "tabIndicators"
        ]
    );
    assert_eq!(frame["notify"]["policy"], "single");
}

#[tokio::test]
async fn test_frame_jobs_match_get_jobs_and_embed_the_timeline() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    events(
        &runtime,
        vec![event(
            "ev1",
            "a1",
            "attention.changed",
            json!({
                "version": 2,
                "attention": { "level": "urgent", "reason": "input", "title": "Need input" }
            }),
        )],
    )
    .await;
    let address = serve(&runtime).await;

    let frame = SseClient::connect(address, "").await.frame().await;

    // Same shape as the REST route — surfaces parse one job type, not two.
    assert_eq!(frame["jobs"], jobs_of(&runtime).await);
    let timeline = frame["jobs"][0]["timeline"].clone();
    assert_eq!(array(&timeline, "timeline").len(), 1);
    assert_eq!(text(&timeline[0], "/kind"), "attention.changed");
}

// ════════════════════════════════════════════════════════════════════════════
// 2. Changes reach live subscribers, coalesced (acceptance A6)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_a_snapshot_after_connect_arrives_in_the_next_frame() {
    let runtime = runtime();
    let address = serve(&runtime).await;
    let mut stream = SseClient::connect(address, "").await;
    assert_eq!(stream.frame().await["jobs"], json!([]));

    snapshot(&runtime, vec![job("a1", json!({ "name": "A1" }))]).await;

    let frame = stream.frame().await;
    assert_eq!(ids(&frame["jobs"], "jobs"), vec!["a1"]);
    assert_eq!(text(&frame["jobs"][0], "/name"), "A1");
    // The embedded timeline is always present, even when empty, so a surface
    // never has to branch on its absence.
    assert_eq!(frame["jobs"][0]["timeline"], json!([]));
}

#[tokio::test]
async fn test_a_burst_inside_the_window_coalesces_into_fewer_frames() {
    let runtime = runtime();
    let address = serve(&runtime).await;
    let mut stream = SseClient::connect(address, "").await;
    stream.frame().await;

    // Three in-process writes, microseconds apart: comfortably inside the
    // ~150 ms window whatever machine this runs on.
    for id in ["a1", "a2", "a3"] {
        snapshot(&runtime, vec![job(id, json!({}))]).await;
    }

    let frames = stream.frames_until_quiet().await;
    assert!(
        !frames.is_empty(),
        "three changes must produce at least one frame"
    );
    // A tolerant upper bound, not a frame count: leading-edge plus trailing-edge
    // coalescing is fine, one frame per change is not.
    assert!(
        frames.len() < 3,
        "three changes inside one window produced {} frames — nothing coalesced",
        frames.len()
    );
    let last = frames.last().expect("checked non-empty above");
    assert_eq!(ids(&last["jobs"], "jobs"), vec!["a1", "a2", "a3"]);
}

// ════════════════════════════════════════════════════════════════════════════
// 3. Terminal states leave through `departed` (acceptance A6)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_an_ended_job_moves_from_jobs_to_departed() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    let address = serve(&runtime).await;
    let mut stream = SseClient::connect(address, "").await;
    assert_eq!(ids(&stream.frame().await["jobs"], "jobs"), vec!["a1"]);

    // SessionEnd: the row is evicted on the spot, so only `departed` can carry
    // its terminal state to a surface.
    snapshot(
        &runtime,
        vec![job(
            "a1",
            json!({ "lifecycle": "ended", "outcome": "cancelled", "version": 2 }),
        )],
    )
    .await;

    let frame = stream.frame().await;
    assert_eq!(frame["jobs"], json!([]));
    let departed = array(&frame["departed"], "departed");
    assert_eq!(departed.len(), 1);
    assert_eq!(text(&departed[0], "/id"), "a1");
    assert_eq!(text(&departed[0], "/lifecycle"), "ended");
    assert_eq!(text(&departed[0], "/outcome"), "cancelled");
    // The producer left `endedAt` out, so the hub filled it from its clock —
    // frozen at T0 here, hence a hard-coded golden rather than "non-null".
    assert_eq!(text(&departed[0], "/endedAt"), T0);
}

#[tokio::test]
async fn test_two_terminal_states_for_one_id_collapse_to_the_last() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    let address = serve(&runtime).await;
    let mut stream = SseClient::connect(address, "").await;
    stream.frame().await;

    // A producer that reports the end twice (retry, or SessionEnd racing a final
    // snapshot). Both writes land inside one window.
    for (version, ended_at) in [(2, "2026-07-19T00:00:05Z"), (3, "2026-07-19T00:00:09Z")] {
        snapshot(
            &runtime,
            vec![job(
                "a1",
                json!({
                    "lifecycle": "ended",
                    "outcome": "success",
                    "endedAt": ended_at,
                    "version": version
                }),
            )],
        )
        .await;
    }

    let frames = stream.frames_until_quiet().await;
    let departed = departed_for(&frames, "a1");
    assert_eq!(
        departed.len(),
        1,
        "one id may appear once per window, got {departed:?}"
    );
    assert_eq!(text(departed[0], "/endedAt"), "2026-07-19T00:00:09Z");
}

#[tokio::test]
async fn test_departed_is_drained_by_the_frame_that_carries_it() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    let address = serve(&runtime).await;
    let mut stream = SseClient::connect(address, "").await;
    stream.frame().await;
    snapshot(
        &runtime,
        vec![job("a1", json!({ "lifecycle": "ended", "version": 2 }))],
    )
    .await;
    assert_eq!(departed_for(&[stream.frame().await], "a1").len(), 1);

    snapshot(&runtime, vec![job("b1", json!({}))]).await;

    // `departed` means "since the last frame", so a terminal state is never
    // replayed to a subscriber that already saw it.
    let frame = stream.frame().await;
    assert_eq!(ids(&frame["jobs"], "jobs"), vec!["b1"]);
    assert_eq!(frame["departed"], json!([]));
}

// ════════════════════════════════════════════════════════════════════════════
// 4. Reconnect and surface label (acceptance A6)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reconnecting_replays_the_whole_state() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    let address = serve(&runtime).await;
    let mut first = SseClient::connect(address, "").await;
    first.frame().await;

    drop(first);
    // Changed while nobody was listening: there is no delta protocol to catch
    // up with, so the reconnect frame must simply be everything.
    snapshot(&runtime, vec![job("b1", json!({}))]).await;

    let frame = SseClient::connect(address, "").await.frame().await;
    assert_eq!(ids(&frame["jobs"], "jobs"), vec!["a1", "b1"]);
    assert_eq!(frame["departed"], json!([]));
}

#[tokio::test]
async fn test_the_surface_label_does_not_change_the_frame() {
    let runtime = runtime();
    snapshot(&runtime, vec![job("a1", json!({}))]).await;
    let address = serve(&runtime).await;

    let labelled = SseClient::connect(address, "?surface=tmux")
        .await
        .frame()
        .await;
    let plain = SseClient::connect(address, "").await.frame().await;

    // `?surface=` names the notify lease. It never filters jobs.
    assert_eq!(labelled["jobs"], plain["jobs"]);
    assert_eq!(labelled["departed"], plain["departed"]);
    assert_eq!(labelled["notify"]["surfaces"], json!(["tmux"]));
}

// ════════════════════════════════════════════════════════════════════════════
// 5. A stream is the reference the lifecycle counts (acceptance A6 + A7)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_a_live_stream_holds_the_hub_open_and_releases_it_on_disconnect() {
    // The only real-time lifecycle assertion in the suite: it is what proves the
    // socket is wired to the refcount. `tests/lifecycle.rs` owns the timing
    // rules themselves, on a paused clock and without any socket.
    let grace = Duration::from_secs(1);
    let runtime = HubRuntime::new(store());
    let address = serve(&runtime).await;
    let mut stream = SseClient::connect(address, "").await;
    // Read the first frame: the body is now being polled, so the subscription is
    // unambiguously live rather than merely accepted.
    stream.frame().await;

    assert!(
        timeout(grace * 2, runtime.shutdown()).await.is_err(),
        "subscription changes must never stop the resident hub"
    );

    drop(stream);

    assert!(
        timeout(Duration::from_secs(2), runtime.shutdown())
            .await
            .is_err(),
        "surface disconnection must not stop hub"
    );
}
