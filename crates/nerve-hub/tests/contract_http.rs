//! HTTP contract tests (spec `nerve-hub` · T4 · acceptance A1 / A2 / A3 / A5).
//!
//! RED on purpose: `nerve_hub::http` does not exist yet. T5 implements it
//! against the contract in this header; this file is the API definition.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! MINIMAL API CONTRACT (what T5 must expose)
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```text
//! nerve_hub::http
//!   #[derive(Clone)]
//!   pub struct HubState;                       // Arc<Mutex<JobStore>> inside
//!   impl HubState {
//!       pub fn new(store: JobStore) -> Self;   // the store carries clock,
//!                                              // machine alias and PidProbe
//!   }
//!
//!   pub fn router(state: HubState) -> axum::Router;   // Router<()>: ready for
//!                                                     // `axum::serve` and for
//!                                                     // `ServiceExt::oneshot`
//! ```
//!
//! * `GET /v1/stream` (SSE) is **not** in this file — it lands with T6/T7.
//! * The loopback guard reads the peer from `ConnectInfo<SocketAddr>`, so the
//!   binary must serve with
//!   `router.into_make_service_with_connect_info::<SocketAddr>()`. These tests
//!   inject the extension directly and never bind a port, so they cannot
//!   collide with a running Nerve.app on 17890.
//! * Handlers return `(StatusCode, Json<_>)`: no input may panic, and nothing
//!   may answer 5xx.
//!
//! Response bodies are asserted against the literals in
//! `Nerve/Nerve/Ingest/IngestServer.swift` **as parsed JSON**, not as bytes:
//! Swift emits hand-written string literals while `serde_json` sorts object
//! keys, and key order was never part of the contract.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! FIDELITY QUIRKS THIS FILE PINS (read before "fixing" a failing assert)
//! ─────────────────────────────────────────────────────────────────────────
//!
//! 1. **The all-optional envelope swallows single objects.**
//!    `decodeEnvelope` (`IngestServer.swift:295`) and `decodeSnapshot` (`:309`)
//!    try `IngestEnvelope` *first*, and every field of it is optional, so a
//!    single event object or a single job object decodes as an envelope with no
//!    `jobs` and no `events` — the payload is silently dropped and the answer is
//!    `{"applied":0}`, never `1`. If that object carries an `alias` key (both
//!    `NerveEvent` and `Job` have one) it even passes the alias guard.
//!
//! 2. **The alias guard runs before the payload is looked at.**
//!    `validateAndNormalize` (`:263`) is called at `:202` / `:211`, before
//!    `store.apply(envelope:)`, and it consults `envelope.alias` and
//!    `envelope.jobs` **only** — never `envelope.events`. Consequences:
//!    * a bare event array (no place to put an alias) → 400 `alias required`,
//!      even though the *shape* decoded fine;
//!    * an empty `{}` body → 400 `alias required`, not `{"applied":0}`.
//!
//!    Acceptance A2's "three shapes all 200" and "empty body → 200" therefore
//!    hold only for bodies that carry an alias; the tests below encode the
//!    Swift behaviour and say so at each site.
//!
//! 3. **Events do not inherit the envelope alias.** `apply(envelope:)`
//!    (`SubjectStore.swift:406`) pushes the envelope alias into `jobs` only; an
//!    event that creates a job resolves `event.alias ?? LocalMachine.alias`
//!    (`:432`). Envelope-level normalisation first, machine alias last.
//!
//! Determinism: fake clock, fake `PidProbe`, injected machine alias, hard-coded
//! goldens. No wall clock, no socket, no filesystem, no third-party oracle.

use std::net::{Ipv6Addr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use time::macros::datetime;
use tower::ServiceExt;

use nerve_hub::clock::FakeClock;
use nerve_hub::http::{HubState, router};
use nerve_hub::state::{JobStore, PidProbe, PidState};

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Wire instant every fixture is anchored to (same value `./scripts/nerve.sh --verify-loop` uses).
const T0: &str = "2026-07-19T00:00:00Z";

/// The alias injected into `JobStore::new` — "this machine" for the hub.
const MACHINE_ALIAS: &str = "test-mac";

/// A hub whose store reads a frozen clock and calls every pid alive.
fn hub() -> Router {
    let clock = Arc::new(FakeClock::new(datetime!(2026-07-19 00:00:00 UTC)));
    let store = JobStore::new(clock, MACHINE_ALIAS.to_string(), Arc::new(EveryPidAlive));
    router(HubState::new(store))
}

/// Liveness oracle that never reaps: these tests are about HTTP, not the reaper.
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

// ── Driving the router ──────────────────────────────────────────────────────

/// Peer address a producer on this machine connects from.
fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 54_321))
}

fn request(method: &str, uri: &str, body: Body, peer: SocketAddr) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body)
        .expect("test request must build");
    // What `into_make_service_with_connect_info::<SocketAddr>()` installs in
    // production; injected here so no socket is ever bound.
    request.extensions_mut().insert(ConnectInfo(peer));
    request
}

fn get(uri: &str) -> Request<Body> {
    request("GET", uri, Body::empty(), loopback())
}

fn post(uri: &str, body: &str) -> Request<Body> {
    request("POST", uri, Body::from(body.to_string()), loopback())
}

/// One request, one response, fully read.
async fn call(router: &Router, request: Request<Body>) -> Reply {
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("router must answer every request");
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body must be readable")
        .to_bytes();
    Reply {
        status,
        headers,
        body,
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|err| {
            panic!(
                "response body is not JSON ({err}): {}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    fn cors(&self) -> String {
        self.headers
            .get("access-control-allow-origin")
            .unwrap_or_else(|| panic!("response is missing the CORS header: {:?}", self.headers))
            .to_str()
            .expect("CORS header must be ASCII")
            .to_string()
    }
}

// ── Assertion helpers ───────────────────────────────────────────────────────

/// `GET /v1/jobs`, parsed.
async fn jobs_of(router: &Router) -> Value {
    call(router, get("/v1/jobs")).await.json()
}

fn array(value: &Value) -> &Vec<Value> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("expected a bare JSON array, got {value}"))
}

fn at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value
        .pointer(pointer)
        .unwrap_or_else(|| panic!("missing `{pointer}` in {value}"))
}

fn text(value: &Value, pointer: &str) -> String {
    at(value, pointer)
        .as_str()
        .unwrap_or_else(|| panic!("`{pointer}` is not a string in {value}"))
        .to_string()
}

fn job_in(jobs: &Value, id: &str) -> Value {
    array(jobs)
        .iter()
        .find(|job| job.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
        .unwrap_or_else(|| panic!("job `{id}` missing from {jobs}"))
}

// ════════════════════════════════════════════════════════════════════════════
// 1. Route table parity (acceptance A1 · `IngestServer.swift:166`)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_health_returns_the_service_literal() {
    let reply = call(&hub(), get("/v1/health")).await;

    assert_eq!(reply.status, StatusCode::OK);
    // `IngestServer.swift:168` — literal, including the pinned version string.
    assert_eq!(
        reply.json(),
        json!({
            "ok": true,
            "service": "nerve",
            "protocolVersion": 2,
            "version": "0.1.0",
            "watchers": 0,
            "notify": {
                "policy": "single",
                "owner": null,
                "surfaces": [],
                "watchers": 0
            }
        })
    );
}

#[tokio::test]
async fn test_health_legacy_alias_matches_the_v1_route() {
    let hub = hub();

    let legacy = call(&hub, get("/health")).await;
    let versioned = call(&hub, get("/v1/health")).await;

    assert_eq!(legacy.status, StatusCode::OK);
    assert_eq!(legacy.json(), versioned.json());
}

#[tokio::test]
async fn test_jobs_is_a_bare_array_when_empty() {
    let reply = call(&hub(), get("/v1/jobs")).await;

    assert_eq!(reply.status, StatusCode::OK);
    // Top level is an array, never an object wrapper.
    assert_eq!(reply.json(), json!([]));
}

#[tokio::test]
async fn test_subjects_legacy_alias_returns_the_same_array() {
    let hub = hub();
    let body = json!({ "alias": MACHINE_ALIAS, "jobs": [job("a1", json!({}))] }).to_string();
    call(&hub, post("/v1/snapshot", &body)).await;

    let reply = call(&hub, get("/v1/subjects")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), jobs_of(&hub).await);
}

#[tokio::test]
async fn test_snapshot_reports_how_many_jobs_it_read() {
    let body = json!({
        "alias": MACHINE_ALIAS,
        "machineKind": "darwin",
        "jobs": [job("a1", json!({})), job("a2", json!({}))]
    })
    .to_string();

    let reply = call(&hub(), post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 2 }));
}

#[tokio::test]
async fn test_snapshot_makes_the_job_readable_on_get_jobs() {
    let hub = hub();
    let body = json!({
        "alias": MACHINE_ALIAS,
        "jobs": [job("a2", json!({ "kind": "custom.foo", "name": "A2", "version": 7 }))]
    })
    .to_string();

    call(&hub, post("/v1/snapshot", &body)).await;

    let stored = job_in(&jobs_of(&hub).await, "a2");
    assert_eq!(text(&stored, "/kind"), "custom.foo");
    assert_eq!(text(&stored, "/name"), "A2");
    assert_eq!(at(&stored, "/version"), &json!(7));
}

#[tokio::test]
async fn test_events_reports_how_many_events_applied() {
    let body = json!({
        "alias": MACHINE_ALIAS,
        "events": [event("ev1", "claude-code:s1", "job.created", json!({}))]
    })
    .to_string();

    let reply = call(&hub(), post("/v1/events", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 1 }));
}

#[tokio::test]
async fn test_pending_with_producer_id_is_an_empty_array() {
    // The pending subsystem is ported but dormant (spec D6): nothing enqueues,
    // so the contract is an empty array rather than a 404.
    let reply = call(&hub(), get("/v1/actions/pending?producerId=t")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!([]));
}

#[tokio::test]
async fn test_pending_with_legacy_source_id_is_an_empty_array() {
    // `IngestServer.swift:176` — `producerId` falls back to legacy `sourceId`.
    let reply = call(&hub(), get("/v1/actions/pending?sourceId=t")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!([]));
}

#[tokio::test]
async fn test_action_result_miss_returns_the_404_literal() {
    let body = json!({ "id": "nope", "state": "succeeded", "message": "x" }).to_string();

    let reply = call(&hub(), post("/v1/actions/result?producerId=missing", &body)).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    // `IngestServer.swift:198` — literal, matched by `./scripts/nerve.sh --verify-loop`.
    assert_eq!(
        reply.json(),
        json!({ "error": "pending action not found or producer mismatch" })
    );
}

#[tokio::test]
async fn test_demo_returns_the_demo_literal() {
    let reply = call(&hub(), post("/v1/demo", "")).await;

    assert_eq!(reply.status, StatusCode::OK);
    // `IngestServer.swift:224`.
    assert_eq!(reply.json(), json!({ "ok": true, "demo": true }));
}

#[tokio::test]
async fn test_demo_loads_four_jobs() {
    // `SubjectStore.swift:1039` seeds exactly four open jobs, all from producer
    // `demo`, all on this machine. Ended rows are deliberately not demo'd.
    let hub = hub();

    call(&hub, post("/v1/demo", "")).await;

    let jobs = jobs_of(&hub).await;
    assert_eq!(array(&jobs).len(), 4);
    for demo in array(&jobs) {
        assert_eq!(text(demo, "/producer/id"), "demo");
        assert_eq!(text(demo, "/alias"), MACHINE_ALIAS);
    }
}

#[tokio::test]
async fn test_clear_returns_the_ok_literal() {
    let reply = call(&hub(), post("/v1/clear", "")).await;

    assert_eq!(reply.status, StatusCode::OK);
    // `IngestServer.swift:230`.
    assert_eq!(reply.json(), json!({ "ok": true }));
}

#[tokio::test]
async fn test_clear_empties_the_job_list() {
    let hub = hub();
    call(&hub, post("/v1/demo", "")).await;

    call(&hub, post("/v1/clear", "")).await;

    assert_eq!(jobs_of(&hub).await, json!([]));
}

#[tokio::test]
async fn test_refresh_returns_the_job_list() {
    let hub = hub();
    call(&hub, post("/v1/demo", "")).await;

    let reply = call(&hub, post("/v1/refresh", "")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(array(&reply.json()).len(), 4);
    assert_eq!(jobs_of(&hub).await, reply.json());
}

#[tokio::test]
async fn test_actions_invoke_is_not_ported() {
    // Deliberately dropped (spec "刻意分歧"): invoking an action is an
    // NSWorkspace side effect and belongs to a surface. The body is whatever
    // the generic fallback says; only the status is contract.
    let body = json!({ "jobId": "a1", "actionId": "open" }).to_string();

    let reply = call(&hub(), post("/v1/actions/invoke", &body)).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_unknown_path_returns_the_not_found_literal() {
    let reply = call(&hub(), get("/v1/nope")).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    // `IngestServer.swift:258`.
    assert_eq!(reply.json(), json!({ "error": "not found" }));
}

#[tokio::test]
async fn test_known_path_with_wrong_method_falls_back_to_not_found() {
    // Swift routes on the `(method, path)` tuple (`IngestServer.swift:166`), so
    // a wrong method lands in the same default case as an unknown path — 404
    // with the literal, not 405.
    let reply = call(&hub(), get("/v1/snapshot")).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.json(), json!({ "error": "not found" }));
}

#[tokio::test]
async fn test_every_response_carries_the_cors_header() {
    // `IngestServer.swift:333` writes the header on every reply, whatever the
    // status: success, guard rejection, decode failure and fallback alike.
    let hub = hub();
    let oversized = json!({
        "alias": MACHINE_ALIAS,
        "jobs": [],
        "pad": "A".repeat(1024 * 1024)
    })
    .to_string();

    let replies = [
        call(&hub, get("/v1/health")).await,
        call(&hub, post("/v1/snapshot", "{}")).await,
        call(
            &hub,
            request(
                "GET",
                "/v1/health",
                Body::empty(),
                SocketAddr::from(([10, 0, 0, 5], 1_234)),
            ),
        )
        .await,
        call(&hub, get("/v1/nope")).await,
        call(&hub, post("/v1/snapshot", &oversized)).await,
    ];

    for reply in &replies {
        assert_eq!(reply.cors(), "*", "status {} lost CORS", reply.status);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 2. Envelope shape asymmetry (acceptance A2 · `IngestServer.swift:295` / `:309`)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_events_accepts_an_envelope_body() {
    let body = json!({
        "alias": MACHINE_ALIAS,
        "events": [event("ev1", "claude-code:s1", "job.created", json!({}))]
    })
    .to_string();

    let reply = call(&hub(), post("/v1/events", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 1 }));
}

#[tokio::test]
async fn test_events_bare_array_decodes_but_still_needs_an_alias() {
    // The *shape* is accepted (`decodeEnvelope` second branch, `:300`) — the
    // rejection comes from the alias guard, which never looks at events
    // (`validateAndNormalize:263`). A bare array has nowhere to carry an alias,
    // so this form can only ever be 400. The literal below is the proof that
    // decoding succeeded: a decode failure returns a different, unstable error.
    let body = json!([event("ev1", "claude-code:s1", "job.created", json!({}))]).to_string();

    let reply = call(&hub(), post("/v1/events", &body)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.json(), json!({ "error": "alias required" }));
}

#[tokio::test]
async fn test_events_single_event_object_with_alias_is_swallowed_as_an_envelope() {
    // Fidelity quirk #1: `decodeEnvelope` tries the all-optional envelope first,
    // so a single event object decodes as an envelope whose `alias` happens to
    // be the event's own alias and whose `events` is nil. Nothing is applied —
    // the answer is `{"applied":0}`, **not** `1` — and no job appears.
    let hub = hub();
    let body = event(
        "ev1",
        "claude-code:s1",
        "job.created",
        json!({ "alias": MACHINE_ALIAS }),
    )
    .to_string();

    let reply = call(&hub, post("/v1/events", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 0 }));
    assert_eq!(jobs_of(&hub).await, json!([]));
}

#[tokio::test]
async fn test_events_single_event_object_without_alias_is_rejected() {
    // Same swallow as above, but the resulting envelope has no alias at all, so
    // the guard answers before anything is applied.
    let body = event("ev1", "claude-code:s1", "job.created", json!({})).to_string();

    let reply = call(&hub(), post("/v1/events", &body)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.json(), json!({ "error": "alias required" }));
}

#[tokio::test]
async fn test_events_with_an_alias_and_no_events_applies_nothing() {
    let reply = call(
        &hub(),
        post("/v1/events", &json!({ "alias": MACHINE_ALIAS }).to_string()),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 0 }));
}

#[tokio::test]
async fn test_snapshot_rejects_a_bare_job_array() {
    // `decodeSnapshot` (`:309`) has no array fallback: envelope or nothing.
    // The error body is unstable, so only the status is contract.
    let body = json!([job("a1", json!({}))]).to_string();

    let reply = call(&hub(), post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_snapshot_single_job_object_without_alias_is_rejected() {
    let body = job("a1", json!({ "alias": "" })).to_string();

    let reply = call(&hub(), post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.json(), json!({ "error": "alias required" }));
}

#[tokio::test]
async fn test_snapshot_single_job_object_with_alias_is_swallowed_as_an_envelope() {
    // Fidelity quirk #1 again, on the snapshot side: `Job` carries an `alias`
    // key, so a lone job decodes as an aliased envelope with no `jobs`. It
    // passes the guard, applies nothing, and the job is silently dropped.
    let hub = hub();
    let body = job("a1", json!({})).to_string();

    let reply = call(&hub, post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 0 }));
    assert_eq!(jobs_of(&hub).await, json!([]));
}

#[tokio::test]
async fn test_snapshot_with_an_alias_and_no_jobs_applies_nothing() {
    // `./scripts/nerve.sh --verify-loop` posts exactly this body.
    let body = json!({ "alias": "__not_configured__", "jobs": [] }).to_string();

    let reply = call(&hub(), post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 0 }));
}

// ════════════════════════════════════════════════════════════════════════════
// 3. Two-stage alias fill (acceptance A3 · `IngestServer.swift:263`)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_envelope_borrows_alias_from_a_job_and_backfills_the_rest() {
    // No envelope alias: the first job with a non-empty one lends it, and every
    // empty job alias is filled from it — not from the machine alias.
    let hub = hub();
    let body = json!({
        "jobs": [
            job("a1", json!({ "alias": "" })),
            job("a2", json!({ "alias": "borrowed-box" })),
            job("a3", json!({ "alias": "" }))
        ]
    })
    .to_string();

    let reply = call(&hub, post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 3 }));
    let jobs = jobs_of(&hub).await;
    assert_eq!(text(&job_in(&jobs, "a1"), "/alias"), "borrowed-box");
    assert_eq!(text(&job_in(&jobs, "a2"), "/alias"), "borrowed-box");
    assert_eq!(text(&job_in(&jobs, "a3"), "/alias"), "borrowed-box");
}

#[tokio::test]
async fn test_snapshot_without_any_alias_is_rejected() {
    // Nothing to borrow anywhere — including the degenerate `{}` body, which
    // decodes to an empty envelope (`:296`) and then fails the guard.
    let hub = hub();

    let empty = call(&hub, post("/v1/snapshot", "{}")).await;
    let aliasless = call(
        &hub,
        post(
            "/v1/snapshot",
            &json!({ "jobs": [job("a1", json!({ "alias": "" }))] }).to_string(),
        ),
    )
    .await;

    for reply in [empty, aliasless] {
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        // `IngestServer.swift:269`.
        assert_eq!(reply.json(), json!({ "error": "alias required" }));
    }
}

#[tokio::test]
async fn test_any_non_empty_alias_is_accepted() {
    // Open ingest: no allow-list, no rename, not even for the sentinel string
    // `./scripts/nerve.sh --verify-loop` expected to be rejected (zombie assertion, fixed in T8).
    let hub = hub();
    let body = json!({
        "alias": "__not_configured__",
        "jobs": [job("a1", json!({ "alias": "" }))]
    })
    .to_string();

    let reply = call(&hub, post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 1 }));
    assert_eq!(
        text(&job_in(&jobs_of(&hub).await, "a1"), "/alias"),
        "__not_configured__"
    );
}

#[tokio::test]
async fn test_envelope_alias_fills_an_empty_job_alias() {
    // Order proof, snapshot side: the machine-alias fallback
    // (`SubjectStore.swift:414`) only runs after envelope normalisation, so an
    // envelope alias always wins over `MACHINE_ALIAS`.
    let hub = hub();
    let body = json!({
        "alias": "env-box",
        "jobs": [job("a1", json!({ "alias": "" }))]
    })
    .to_string();

    call(&hub, post("/v1/snapshot", &body)).await;

    assert_eq!(
        text(&job_in(&jobs_of(&hub).await, "a1"), "/alias"),
        "env-box"
    );
}

#[tokio::test]
async fn test_event_created_job_falls_back_to_the_machine_alias() {
    // Order proof, events side (fidelity quirk #3): the envelope alias satisfies
    // the guard but is never pushed into events, so a job created by an
    // aliasless event lands on the machine alias — not `env-box`.
    let hub = hub();
    let body = json!({
        "alias": "env-box",
        "events": [event("ev1", "claude-code:s9", "job.created", json!({}))]
    })
    .to_string();

    let reply = call(&hub, post("/v1/events", &body)).await;

    assert_eq!(reply.json(), json!({ "applied": 1 }));
    assert_eq!(
        text(&job_in(&jobs_of(&hub).await, "claude-code:s9"), "/alias"),
        MACHINE_ALIAS
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 4. Cross-cutting guards (acceptance A5 · `IngestServer.swift:129` / `:100`)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_non_loopback_peer_is_forbidden() {
    // The guard runs before routing, so even the harmless health route is shut.
    let reply = call(
        &hub(),
        request(
            "GET",
            "/v1/health",
            Body::empty(),
            SocketAddr::from(([10, 0, 0, 5], 1_234)),
        ),
    )
    .await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    // `IngestServer.swift:134`.
    assert_eq!(reply.json(), json!({ "error": "loopback only" }));
}

#[tokio::test]
async fn test_loopback_subnet_peer_is_allowed() {
    // Swift accepts the whole `127.` prefix (`:133`), not just `127.0.0.1`.
    let reply = call(
        &hub(),
        request(
            "GET",
            "/v1/health",
            Body::empty(),
            SocketAddr::from(([127, 0, 0, 53], 9)),
        ),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn test_ipv6_loopback_peer_is_allowed() {
    // `::1` (`:139`).
    let reply = call(
        &hub(),
        request(
            "GET",
            "/v1/health",
            Body::empty(),
            SocketAddr::from((Ipv6Addr::LOCALHOST, 54_321)),
        ),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn test_body_over_one_megabyte_is_rejected() {
    // `IngestServer.swift:100` — 1 MiB ceiling with its own literal, so the
    // limit cannot be delegated to axum's silent `DefaultBodyLimit`.
    let body = json!({
        "alias": MACHINE_ALIAS,
        "jobs": [],
        "pad": "A".repeat(1024 * 1024)
    })
    .to_string();
    assert!(body.len() > 1024 * 1024, "fixture must exceed the limit");

    let reply = call(&hub(), post("/v1/snapshot", &body)).await;

    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.json(), json!({ "error": "too large" }));
}

#[tokio::test]
async fn test_malformed_json_is_rejected_with_status_only() {
    // The error text is Swift's `"\(error)"` and is not part of the contract.
    let reply = call(&hub(), post("/v1/snapshot", "{\"alias\":")).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_no_input_shape_makes_a_handler_fail() {
    // Handlers return `(StatusCode, Json)` for everything: a panic would abort
    // the `oneshot` future, and a 5xx would mean an unhandled path. Neither is
    // allowed, whatever a producer sends.
    let hub = hub();
    let cases: Vec<(&str, &str, String)> = vec![
        ("POST", "/v1/snapshot", String::new()),
        ("POST", "/v1/snapshot", "null".to_string()),
        ("POST", "/v1/snapshot", "[]".to_string()),
        ("POST", "/v1/snapshot", "\"just a string\"".to_string()),
        (
            "POST",
            "/v1/snapshot",
            json!({ "alias": MACHINE_ALIAS, "jobs": [{ "id": "a1" }] }).to_string(),
        ),
        (
            "POST",
            "/v1/snapshot",
            json!({ "alias": MACHINE_ALIAS, "jobs": "not-an-array" }).to_string(),
        ),
        ("POST", "/v1/events", "null".to_string()),
        (
            "POST",
            "/v1/events",
            json!({ "alias": MACHINE_ALIAS, "events": [{ "id": "ev1" }] }).to_string(),
        ),
        ("POST", "/v1/actions/result", "[]".to_string()),
        ("POST", "/v1/actions/result", "{}".to_string()),
        ("POST", "/v1/demo", "not json at all".to_string()),
        ("POST", "/v1/clear", "[1,2,3]".to_string()),
        ("POST", "/v1/refresh", String::new()),
        ("GET", "/v1/actions/pending?producerId=a%20b", String::new()),
        ("GET", "/v1/jobs?producerId=&sourceId=", String::new()),
    ];

    for (method, uri, body) in cases {
        let reply = call(
            &hub,
            request(method, uri, Body::from(body.clone()), loopback()),
        )
        .await;
        assert!(
            reply.status.as_u16() < 500,
            "{method} {uri} with body {body:?} answered {}",
            reply.status
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 5. Job serialization: the embedded timeline (spec "刻意分歧")
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_jobs_embed_the_hub_maintained_timeline() {
    // New over Swift: the timeline used to live only inside the app process.
    // It is always present, even when empty, so surfaces never branch on it.
    let hub = hub();
    let body = json!({ "alias": MACHINE_ALIAS, "jobs": [job("a1", json!({}))] }).to_string();

    call(&hub, post("/v1/snapshot", &body)).await;

    let stored = job_in(&jobs_of(&hub).await, "a1");
    assert_eq!(at(&stored, "/timeline"), &json!([]));
}

#[tokio::test]
async fn test_timeline_records_an_applied_event() {
    let hub = hub();
    let snapshot = json!({ "alias": MACHINE_ALIAS, "jobs": [job("a1", json!({}))] }).to_string();
    call(&hub, post("/v1/snapshot", &snapshot)).await;
    let patch = json!({
        "alias": MACHINE_ALIAS,
        "events": [event(
            "ev1",
            "a1",
            "attention.changed",
            json!({
                "version": 2,
                "attention": { "level": "urgent", "reason": "input", "title": "Need input" }
            })
        )]
    })
    .to_string();

    call(&hub, post("/v1/events", &patch)).await;

    let timeline = at(&job_in(&jobs_of(&hub).await, "a1"), "/timeline").clone();
    assert_eq!(array(&timeline).len(), 1);
    assert_eq!(text(&timeline[0], "/kind"), "attention.changed");
    assert_eq!(text(&timeline[0], "/title"), "Attention → urgent");
    assert_eq!(text(&timeline[0], "/jobId"), "a1");
}
