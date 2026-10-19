//! Reading the hub and resetting it: health, the job list, refresh, demo, clear.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::lifecycle::NotifyPolicy;

use super::HubState;

/// Pinned by the contract, not by the crate version: producers, the site
/// handbook and `./scripts/nerve.sh --verify-loop` all match this exact literal
/// (`IngestServer.swift:168`).
const SERVICE_VERSION: &str = "0.1.0";

pub(super) async fn health(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    let lease = state.roster().lease();
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "service": "nerve",
            "protocolVersion": 2,
            "version": SERVICE_VERSION,
            "watchers": lease.watchers,
            "notify": lease.to_json(),
        })),
    )
}

/// Every stored job as a **bare array** — never an object wrapper — each with
/// the timeline the hub maintains for it.
pub(super) async fn jobs(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    let jobs = state.store().jobs_json();
    (StatusCode::OK, Json(jobs))
}

/// Surface Refresh: run the maintenance tick now, always republish a frame,
/// and answer the same bare job array as `GET /v1/jobs`.
///
/// The panel's refresh is this write, not a local re-read. Reaping dead local
/// PIDs and expiring pending requests is hub work; a GET would just echo
/// whatever was already in RAM.
pub(super) async fn refresh(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    let changed = state.store().expire_and_reap();
    state.changed();
    tracing::info!(changed, "refresh");
    (StatusCode::OK, Json(state.store().jobs_json()))
}

/// Seed the four demo rows (`SubjectStore.swift:1039`). The body is ignored:
/// this route is a button, not a report.
pub(super) async fn demo(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    state.store().load_demo();
    state.changed();
    tracing::info!("demo loaded");
    (StatusCode::OK, Json(json!({ "ok": true, "demo": true })))
}

/// Drop every job, timeline, pending request and seen event id.
pub(super) async fn clear(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    state.store().clear();
    state.changed();
    tracing::info!("cleared");
    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// The live notify lease: policy, elected owner, connected surface labels.
pub(super) async fn notify(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(state.roster().lease().to_json()))
}

#[derive(Deserialize)]
pub(super) struct NotifyBody {
    policy: String,
}

/// Replace the notify policy. Surfaces learn the new owner on the next frame.
pub(super) async fn put_notify(
    State(state): State<HubState>,
    Json(body): Json<NotifyBody>,
) -> (StatusCode, Json<Value>) {
    let Some(policy) = NotifyPolicy::parse(&body.policy) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "unknown notify policy" })),
        );
    };
    tracing::info!(policy = body.policy, "notify policy");
    state.roster().set_policy(policy);
    (StatusCode::OK, Json(state.roster().lease().to_json()))
}

pub(super) async fn frame(State(state): State<HubState>) -> Json<Value> {
    let raw = crate::sse::Frame::connect(&state.store(), state.roster().lease()).render();
    Json(serde_json::from_str(&raw).expect("hub frame is valid JSON"))
}

pub(super) async fn machines(State(state): State<HubState>) -> Json<Value> {
    Json(state.machines.list())
}
pub(super) async fn configure_machine(
    State(state): State<HubState>,
    Json(config): Json<crate::federation::MachineConfig>,
) -> (StatusCode, Json<Value>) {
    match state.machines.configure(config) {
        Ok(()) => (StatusCode::OK, Json(state.machines.list())),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))),
    }
}
pub(super) async fn connect_machine(
    State(state): State<HubState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.machines.connect(&id) {
        Ok(()) => (StatusCode::ACCEPTED, Json(json!({"ok":true}))),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))),
    }
}
pub(super) async fn disconnect_machine(
    State(state): State<HubState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<Value> {
    state.machines.disconnect(&id);
    Json(json!({"ok":true}))
}

pub(super) async fn configure_client(
    State(state): State<HubState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(preferences): Json<crate::lifecycle::roster::ClientPreferences>,
) -> Json<Value> {
    state.roster().configure_client(id, preferences);
    Json(json!({"ok":true}))
}
pub(super) async fn notification_result(Json(result): Json<Value>) -> Json<Value> {
    tracing::info!(notification_id = ?result["id"], delivered = ?result["delivered"], "notification execution result");
    Json(json!({"ok":true}))
}

pub(super) async fn discover_machines(State(state): State<HubState>) -> (StatusCode, Json<Value>) {
    match state.machines.discover() {
        Ok(()) => (StatusCode::OK, Json(state.machines.list())),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))),
    }
}

#[derive(Deserialize)]
pub(super) struct RemoteFocus {
    host: String,
    pid: u32,
}
pub(super) async fn remote_focus(
    State(state): State<HubState>,
    Json(request): Json<RemoteFocus>,
) -> (StatusCode, Json<Value>) {
    let jobs = state.store().jobs_json();
    let authorized = jobs.as_array().is_some_and(|rows| {
        rows.iter().any(|job| {
            job["extensions"]["sshHost"].as_str() == Some(&request.host)
                && job["extensions"]["pid"].as_u64() == Some(u64::from(request.pid))
        })
    });
    if !authorized {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"remote job not found"})),
        );
    }
    let result = tokio::task::spawn_blocking(move || {
        crate::remote_focus::select_job(&request.host, request.pid)
    })
    .await;
    (
        StatusCode::OK,
        Json(json!({"result":format!("{result:?}")})),
    )
}
