//! Golden parity tests (spec `nerve-hub` · T8 · acceptance A9 / A10).
//!
//! Two goldens, both transplanted rather than invented:
//!
//! 1. **`./scripts/nerve.sh --verify-loop`** — the closed-loop check that has guarded the
//!    Swift ingest server. Every one of its assertions lands here as its own
//!    test, driving the hub's `Router` in-process instead of `curl`, so the
//!    suite needs no port and cannot collide with a running Nerve.app on 17890.
//!    The request bodies below are the script's, verbatim (the shell here-doc's
//!    `\"` escapes unwound, `$ALIAS` left as the placeholder the script
//!    interpolates).
//!
//! 2. **`fixtures/demo_snapshot.json`** — a file that used to be an orphan and
//!    is now golden input: `./scripts/nerve.sh --demo` POSTs it, and this file
//!    pins what `GET /v1/jobs` must answer afterwards, field by field.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! TWO SCRIPT ASSERTIONS ARE CORRECTED HERE (spec Testing §, acceptance A9)
//! ─────────────────────────────────────────────────────────────────────────
//!
//! Both were zombies — they encoded behaviour the product never had:
//!
//! * `./scripts/nerve.sh --verify-loop` asserted `len(jobs) == 5`. `a5` is
//!   `lifecycle:"ended"`, and an ended row is evicted on arrival
//!   (`SubjectStore.swift:530`, ported at `state/store.rs:133`), so it never
//!   enters the store. The truth is **4** — which the script's own
//!   `active == 4` check three lines later already said.
//! * `./scripts/nerve.sh --verify-loop` asserted an "unknown" alias is rejected with
//!   403/400. There is no allow-list: open alias ingest is a product invariant
//!   (CLAUDE.md #5), and any non-empty alias is accepted with **200**.
//!
//! The script itself is corrected to match in the same task; these tests are
//! what keeps it honest.
//!
//! Determinism: fake clock, fake `PidProbe`, injected machine alias, hard-coded
//! goldens, fixture embedded at compile time. No wall clock, no socket, no
//! filesystem access at run time, no third-party oracle.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use time::macros::datetime;
use tower::ServiceExt;

use nerve_hub::clock::FakeClock;
use nerve_hub::http::{HubState, router};
use nerve_hub::state::{JobStore, PidProbe, PidState};

// ── Fixtures ────────────────────────────────────────────────────────────────

/// The alias `./scripts/nerve.sh --verify-loop` derives from the hostname, pinned to a literal.
///
/// Deliberately **not** equal to [`MACHINE_ALIAS`]: the hub keeps no allow-list,
/// so a literal both keeps the test deterministic and proves the posted alias is
/// echoed rather than replaced by this machine's.
const SCRIPT_ALIAS: &str = "verify-loop-host";

/// The alias injected into `JobStore::new` — "this machine" for the hub.
const MACHINE_ALIAS: &str = "test-mac";

/// `./scripts/nerve.sh --verify-loop` — five jobs, one of them already ended.
const VERIFY_LOOP_SNAPSHOT: &str = r#"{
  "alias": "$ALIAS",
  "machineKind": "darwin",
  "jobs": [
    {"id":"a1","kind":"session","name":"A1","alias":"$ALIAS","lifecycle":"active","attention":{"level":"none"},"health":"ok","progress":{"kind":"none"},"producer":{"id":"t"},"capabilities":[],"actions":[],"createdAt":"2026-07-19T00:00:00Z","updatedAt":"2026-07-19T00:00:00Z","version":1,"extensions":{"x":1}},
    {"id":"a2","kind":"custom.foo","name":"A2","alias":"$ALIAS","lifecycle":"active","attention":{"level":"required","reason":"approval"},"health":"ok","progress":{"kind":"none"},"producer":{"id":"t"},"capabilities":[],"actions":[],"createdAt":"2026-07-19T00:00:00Z","updatedAt":"2026-07-19T00:00:00Z","version":1,"extensions":{}},
    {"id":"a3","kind":"build","name":"A3","alias":"$ALIAS","lifecycle":"active","attention":{"level":"none"},"health":"unresponsive","progress":{"kind":"none"},"producer":{"id":"t"},"capabilities":[],"actions":[],"createdAt":"2026-07-19T00:00:00Z","updatedAt":"2026-07-19T00:00:00Z","version":1,"extensions":{}},
    {"id":"a4","kind":"test","name":"A4","alias":"$ALIAS","lifecycle":"active","attention":{"level":"none"},"health":"ok","progress":{"kind":"none"},"producer":{"id":"t"},"capabilities":[],"actions":[],"createdAt":"2026-07-19T00:00:00Z","updatedAt":"2026-07-19T00:00:00Z","version":1,"extensions":{}},
    {"id":"a5","kind":"workflow","name":"A5","alias":"$ALIAS","lifecycle":"ended","outcome":"failure","attention":{"level":"informational"},"health":"ok","progress":{"kind":"none"},"producer":{"id":"t"},"capabilities":[],"actions":[],"createdAt":"2026-07-19T00:00:00Z","endedAt":"2026-07-19T00:01:00Z","updatedAt":"2026-07-19T00:01:00Z","version":1,"extensions":{}}
  ]
}"#;

/// `./scripts/nerve.sh --verify-loop` — raises `a1` to `urgent` at version 2.
const VERIFY_LOOP_EVENT: &str = r#"{
  "alias": "$ALIAS",
  "events":[{"id":"ev1","jobId":"a1","kind":"attention.changed","timestamp":"2026-07-19T00:02:00Z","producerId":"t","version":2,"attention":{"level":"urgent","reason":"input","title":"Need input"}}]
}"#;

/// `./scripts/nerve.sh --verify-loop` — a *new* event id carrying an *older*
/// version, so only the version rule can reject it.
const VERIFY_LOOP_STALE_EVENT: &str = r#"{
  "alias": "$ALIAS",
  "events":[{"id":"ev2","jobId":"a1","kind":"attention.changed","timestamp":"2026-07-19T00:02:00Z","producerId":"t","version":1,"attention":{"level":"none"}}]
}"#;

/// `./scripts/nerve.sh --verify-loop` — an alias no machine list ever knew.
const VERIFY_LOOP_UNKNOWN_ALIAS: &str = r#"{"alias":"__not_configured__","jobs":[]}"#;

/// `./scripts/nerve.sh --verify-loop` — a result for a request nobody is waiting on.
const VERIFY_LOOP_ACTION_RESULT: &str = r#"{"id":"nope","state":"succeeded","message":"x"}"#;

/// `fixtures/demo_snapshot.json`, embedded at compile time.
///
/// Path is relative to this file: `crates/nerve-hub/tests/` → repo root, i.e.
/// `$CARGO_MANIFEST_DIR/../../fixtures/demo_snapshot.json`. Compile-time
/// inclusion means the assertions below break loudly if the fixture drifts,
/// and the test still reads no filesystem when it runs.
const DEMO_SNAPSHOT: &str = include_str!("../../../fixtures/demo_snapshot.json");

/// A script body with the shell's `$ALIAS` interpolation applied.
fn script_body(template: &str) -> String {
    template.replace("$ALIAS", SCRIPT_ALIAS)
}

// ── Driving the router ──────────────────────────────────────────────────────

/// A hub whose store reads a frozen clock and calls every pid alive.
///
/// The instant is the script's own timeline, one minute past its last event, so
/// any `endedAt` the store fills in is a hard-coded value too.
fn hub() -> Router {
    let clock = Arc::new(FakeClock::new(datetime!(2026-07-19 00:03:00 UTC)));
    let store = JobStore::new(clock, MACHINE_ALIAS.to_string(), Arc::new(EveryPidAlive));
    router(HubState::new(store))
}

/// Liveness oracle that never reaps: these tests are about the wire, not the
/// reaper (which `tests/state_core.rs` drives with its own fakes).
struct EveryPidAlive;

impl PidProbe for EveryPidAlive {
    fn state(&self, _pid: i32) -> PidState {
        PidState::Alive
    }
}

/// Peer address a producer on this machine connects from.
fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 54_321))
}

fn request(method: &str, uri: &str, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body)
        .expect("test request must build");
    // What `into_make_service_with_connect_info::<SocketAddr>()` installs in
    // production; injected here so no socket is ever bound.
    request.extensions_mut().insert(ConnectInfo(loopback()));
    request
}

fn get(uri: &str) -> Request<Body> {
    request("GET", uri, Body::empty())
}

fn post(uri: &str, body: &str) -> Request<Body> {
    request("POST", uri, Body::from(body.to_string()))
}

async fn call(router: &Router, request: Request<Body>) -> Reply {
    let response = router
        .clone()
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
    Reply { status, body }
}

struct Reply {
    status: StatusCode,
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

    /// `curl -sf` fails the script on any non-2xx, so every transplanted step
    /// carries this check even when its own assertion is about the body.
    fn ok(self) -> Self {
        assert_eq!(self.status, StatusCode::OK, "curl -sf would have failed");
        self
    }
}

// ── Script steps, replayed ──────────────────────────────────────────────────

/// `./scripts/nerve.sh --verify-loop`: health, clear, then the five-job snapshot.
async fn seeded() -> Router {
    let hub = hub();
    call(&hub, get("/v1/health")).await.ok();
    call(&hub, post("/v1/clear", "")).await.ok();
    call(
        &hub,
        post("/v1/snapshot", &script_body(VERIFY_LOOP_SNAPSHOT)),
    )
    .await
    .ok();
    hub
}

/// `./scripts/nerve.sh --verify-loop`: the attention event on top of the seeded store.
async fn patched() -> Router {
    let hub = seeded().await;
    call(&hub, post("/v1/events", &script_body(VERIFY_LOOP_EVENT)))
        .await
        .ok();
    hub
}

// ── Assertion helpers ───────────────────────────────────────────────────────

/// `GET /v1/jobs`, parsed — the script's `curl … | json.load`.
async fn jobs_of(router: &Router) -> Value {
    call(router, get("/v1/jobs")).await.ok().json()
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

/// The script's `by_id = {s["id"]: s for s in data}` lookup.
fn job_in(jobs: &Value, id: &str) -> Value {
    array(jobs)
        .iter()
        .find(|job| job.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
        .unwrap_or_else(|| panic!("job `{id}` missing from {jobs}"))
}

/// Whether a published date is exactly `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Second precision with no fraction is the one wire spelling
/// (`plugins/nerve/hooks/nerve_hook.py:103`), so this checks the shape rather
/// than re-parsing: a fractional or offset date must not merely parse, it must
/// not be published.
fn is_wire_date(raw: &str) -> bool {
    let digits_at = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let punctuation_at = [(4, '-'), (7, '-'), (10, 'T'), (13, ':'), (16, ':')];
    let chars: Vec<char> = raw.chars().collect();
    chars.len() == 20
        && chars[19] == 'Z'
        && digits_at.iter().all(|&i| chars[i].is_ascii_digit())
        && punctuation_at.iter().all(|&(i, c)| chars[i] == c)
}

// ════════════════════════════════════════════════════════════════════════════
// 1. ./scripts/nerve.sh --verify-loop, assertion by assertion (acceptance A9)
// ════════════════════════════════════════════════════════════════════════════

/// `./scripts/nerve.sh --verify-loop` — `curl -sf … | grep -q '"ok":true'`.
#[tokio::test]
async fn test_health_answers_the_literal_the_script_greps() {
    let reply = call(&hub(), get("/v1/health")).await.ok();

    let body = reply.json();
    // The script greps `"ok":true`; extra keys (watchers, notify) are fine.
    assert_eq!(body["ok"], true);
    assert_eq!(body["service"], "nerve");
    assert_eq!(body["version"], "0.1.0");
    assert_eq!(body["watchers"], 0);
    assert_eq!(body["notify"]["policy"], "single");
}

/// `./scripts/nerve.sh --verify-loop` — `curl -sf -X POST …/v1/clear`.
#[tokio::test]
async fn test_clear_answers_ok() {
    let reply = call(&hub(), post("/v1/clear", "")).await.ok();

    assert_eq!(reply.json(), json!({ "ok": true }));
}

/// `./scripts/nerve.sh --verify-loop` — `grep -q '"applied":5'`.
///
/// Five is what the producer *sent*, including the ended `a5`: `applied`
/// answers "how much of your batch did I read", not "how many rows do I hold".
#[tokio::test]
async fn test_snapshot_of_five_jobs_reports_applied_five() {
    let hub = hub();
    call(&hub, post("/v1/clear", "")).await.ok();

    let reply = call(
        &hub,
        post("/v1/snapshot", &script_body(VERIFY_LOOP_SNAPSHOT)),
    )
    .await
    .ok();

    assert_eq!(reply.json(), json!({ "applied": 5 }));
}

/// `./scripts/nerve.sh --verify-loop` — `grep -q '"applied":1'`.
#[tokio::test]
async fn test_event_patch_reports_applied_one() {
    let hub = seeded().await;

    let reply = call(&hub, post("/v1/events", &script_body(VERIFY_LOOP_EVENT)))
        .await
        .ok();

    assert_eq!(reply.json(), json!({ "applied": 1 }));
}

/// `./scripts/nerve.sh --verify-loop` — the same event id again, `grep -q '"applied":0'`.
#[tokio::test]
async fn test_resending_the_same_event_id_reports_applied_zero() {
    let hub = patched().await;

    let reply = call(&hub, post("/v1/events", &script_body(VERIFY_LOOP_EVENT)))
        .await
        .ok();

    assert_eq!(reply.json(), json!({ "applied": 0 }));
}

/// `./scripts/nerve.sh --verify-loop` — a fresh event id at an older version,
/// `grep -q '"applied":0'`.
#[tokio::test]
async fn test_stale_version_event_reports_applied_zero() {
    let hub = patched().await;

    let reply = call(
        &hub,
        post("/v1/events", &script_body(VERIFY_LOOP_STALE_EVENT)),
    )
    .await
    .ok();

    assert_eq!(reply.json(), json!({ "applied": 0 }));
}

/// `./scripts/nerve.sh --verify-loop`, **corrected from 5 to 4**.
///
/// `a5` arrives `lifecycle:"ended"` and is evicted on arrival, so the store
/// holds the four open rows. See this file's header for why the 5 was a zombie.
#[tokio::test]
async fn test_jobs_holds_four_rows_because_the_ended_one_never_enters() {
    let jobs = jobs_of(&patched().await).await;

    assert_eq!(array(&jobs).len(), 4);
}

/// The reason the count above is 4 and not 5, stated on its own.
#[tokio::test]
async fn test_the_ended_job_is_absent_from_the_job_list() {
    let jobs = jobs_of(&patched().await).await;

    let mut ids: Vec<String> = array(&jobs).iter().map(|job| text(job, "/id")).collect();
    ids.sort();
    assert_eq!(ids, vec!["a1", "a2", "a3", "a4"]);
}

/// `./scripts/nerve.sh --verify-loop` — `by_id["a1"]["attention"]["level"] == "urgent"`.
#[tokio::test]
async fn test_a1_attention_is_urgent_after_the_event() {
    let jobs = jobs_of(&patched().await).await;

    assert_eq!(text(&job_in(&jobs, "a1"), "/attention/level"), "urgent");
}

/// `./scripts/nerve.sh --verify-loop` — `by_id["a2"]["kind"] == "custom.foo"`.
///
/// An unknown `kind` is carried through untouched: kinds are producer
/// vocabulary, not a hub enum.
#[tokio::test]
async fn test_a2_keeps_its_custom_kind() {
    let jobs = jobs_of(&patched().await).await;

    assert_eq!(text(&job_in(&jobs, "a2"), "/kind"), "custom.foo");
}

/// `./scripts/nerve.sh --verify-loop` — `assert by_id["a1"]["alias"]` (non-empty).
///
/// Stronger here: the posted alias survives verbatim rather than being replaced
/// by this machine's, which the script could not see (its `$ALIAS` and the
/// server's machine alias are the same string).
#[tokio::test]
async fn test_a1_keeps_the_alias_it_was_posted_with() {
    let jobs = jobs_of(&patched().await).await;
    let alias = text(&job_in(&jobs, "a1"), "/alias");

    assert!(!alias.is_empty(), "alias must never come back empty");
    assert_eq!(alias, SCRIPT_ALIAS);
    assert_ne!(alias, MACHINE_ALIAS);
}

/// `./scripts/nerve.sh --verify-loop` — `len([s for s in data if s["lifecycle"] != "ended"]) == 4`.
#[tokio::test]
async fn test_every_published_job_is_still_open() {
    let jobs = jobs_of(&patched().await).await;

    let active: Vec<&Value> = array(&jobs)
        .iter()
        .filter(|job| text(job, "/lifecycle") != "ended")
        .collect();
    assert_eq!(active.len(), 4);
}

/// `./scripts/nerve.sh --verify-loop` — `assert isinstance(d, list)`.
///
/// The queue is ported but dormant (spec D6): the shape is the contract, the
/// emptiness is the current truth.
#[tokio::test]
async fn test_pending_answers_a_bare_array() {
    let reply = call(&patched().await, get("/v1/actions/pending?producerId=t"))
        .await
        .ok();

    assert_eq!(reply.json(), json!([]));
}

/// `./scripts/nerve.sh --verify-loop` — a result for an unknown request is `404`.
#[tokio::test]
async fn test_action_result_for_an_unknown_request_is_404() {
    let reply = call(
        &patched().await,
        post(
            "/v1/actions/result?producerId=missing",
            VERIFY_LOOP_ACTION_RESULT,
        ),
    )
    .await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(
        reply.json(),
        json!({ "error": "pending action not found or producer mismatch" })
    );
}

/// `./scripts/nerve.sh --verify-loop`, **corrected from 403/400 to 200**.
///
/// Open alias ingest is a product invariant (CLAUDE.md #5): there is no
/// allow-list, so `__not_configured__` is as valid as any hostname. Only an
/// entirely absent alias is refused, which `tests/contract_http.rs` pins.
#[tokio::test]
async fn test_any_non_empty_alias_is_accepted() {
    let reply = call(&hub(), post("/v1/snapshot", VERIFY_LOOP_UNKNOWN_ALIAS)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), json!({ "applied": 0 }));
}

// ════════════════════════════════════════════════════════════════════════════
// 2. fixtures/demo_snapshot.json replayed (acceptance A10)
// ════════════════════════════════════════════════════════════════════════════

/// The fixture posted to a fresh hub, as `./scripts/nerve.sh --demo` does.
async fn demo_loaded() -> Router {
    let hub = hub();
    call(&hub, post("/v1/snapshot", DEMO_SNAPSHOT)).await.ok();
    hub
}

#[tokio::test]
async fn test_demo_fixture_applies_both_of_its_jobs() {
    let reply = call(&hub(), post("/v1/snapshot", DEMO_SNAPSHOT)).await.ok();

    assert_eq!(reply.json(), json!({ "applied": 2 }));
}

#[tokio::test]
async fn test_demo_fixture_publishes_two_jobs_in_id_order() {
    let jobs = jobs_of(&demo_loaded().await).await;

    let mut ids: Vec<String> = array(&jobs).iter().map(|job| text(job, "/id")).collect();
    ids.sort();
    assert_eq!(ids, vec!["demo-agent-1", "demo-build-1"]);
}

/// The agent row's identity and current work, hard-coded from the fixture.
#[tokio::test]
async fn test_demo_agent_job_keeps_its_kind_and_current_work() {
    let jobs = jobs_of(&demo_loaded().await).await;
    let agent = job_in(&jobs, "demo-agent-1");

    assert_eq!(text(&agent, "/kind"), "session");
    assert_eq!(text(&agent, "/name"), "nerve");
    assert_eq!(text(&agent, "/lifecycle"), "active");
    assert_eq!(text(&agent, "/current/type"), "editing");
    assert_eq!(text(&agent, "/current/name"), "Implement ribbon");
    assert_eq!(at(&agent, "/version"), &json!(3));
}

/// `ensureLocalActions` is **not** ported (spec "刻意分歧"): the two actions the
/// fixture declares come back byte-for-byte, with nothing added.
#[tokio::test]
async fn test_demo_agent_actions_are_echoed_verbatim() {
    let jobs = jobs_of(&demo_loaded().await).await;
    let agent = job_in(&jobs, "demo-agent-1");

    assert_eq!(
        at(&agent, "/actions"),
        &json!([
            {
                "id": "open",
                "title": "Open",
                "kind": "open",
                "state": "available",
                "destructive": false,
                "confirmationRequired": false
            },
            {
                "id": "copy",
                "title": "Copy",
                "kind": "copy_summary",
                "state": "available",
                "destructive": false,
                "confirmationRequired": false
            }
        ])
    );
}

/// The build row: an indeterminate bar, and an empty action list that stays
/// empty — a surface derives Open/Copy, the hub never does.
#[tokio::test]
async fn test_demo_build_job_keeps_indeterminate_progress_and_display_only_actions() {
    let jobs = jobs_of(&demo_loaded().await).await;
    let build = job_in(&jobs, "demo-build-1");

    assert_eq!(text(&build, "/kind"), "build");
    assert_eq!(text(&build, "/progress/kind"), "indeterminate");
    assert_eq!(text(&build, "/progress/label"), "Building");
    assert_eq!(at(&build, "/version"), &json!(2));
    assert_eq!(at(&build, "/actions/0/kind"), &json!("copy_summary"));
}

/// Both rows carry the fixture's own `local`, not this machine's alias: the
/// job-level fallback fills empties only.
#[tokio::test]
async fn test_demo_jobs_keep_the_alias_the_fixture_declares() {
    let jobs = jobs_of(&demo_loaded().await).await;

    for job in array(&jobs) {
        assert_eq!(text(job, "/alias"), "local", "in {job}");
        assert_ne!(text(job, "/alias"), MACHINE_ALIAS);
    }
}

/// Every published date is second precision, no fraction, `Z`.
#[tokio::test]
async fn test_demo_dates_come_back_at_second_precision() {
    let jobs = jobs_of(&demo_loaded().await).await;
    let agent = job_in(&jobs, "demo-agent-1");

    assert_eq!(text(&agent, "/createdAt"), "2026-07-19T07:00:00Z");
    assert_eq!(text(&agent, "/startedAt"), "2026-07-19T07:00:00Z");
    assert_eq!(text(&agent, "/updatedAt"), "2026-07-19T08:10:00Z");
    assert_eq!(text(&agent, "/current/startedAt"), "2026-07-19T08:00:00Z");

    for job in array(&jobs) {
        for field in ["/createdAt", "/startedAt", "/updatedAt"] {
            let raw = text(job, field);
            assert!(is_wire_date(&raw), "`{field}` is `{raw}` in {job}");
        }
    }
}

/// The one contract the hub adds over the Swift original: a job publishes the
/// timeline the hub keeps for it. A snapshot records no timeline entries, so
/// the key is present and empty rather than missing.
#[tokio::test]
async fn test_demo_jobs_publish_an_embedded_timeline() {
    let jobs = jobs_of(&demo_loaded().await).await;

    for job in array(&jobs) {
        assert_eq!(at(job, "/timeline"), &json!([]), "in {job}");
    }
}

/// The fixture's `location` is passed through for surfaces to act on — the hub
/// stores where the human should be sent back to, and never sends them.
#[tokio::test]
async fn test_demo_agent_location_is_passed_through() {
    let jobs = jobs_of(&demo_loaded().await).await;
    let agent = job_in(&jobs, "demo-agent-1");

    assert_eq!(text(&agent, "/location/openURL"), "file:///tmp");
    assert_eq!(
        text(&agent, "/location/focusHint"),
        "Demo · nerve · session · /tmp"
    );
}
