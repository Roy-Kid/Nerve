//! Shared, hard-coded frame fixtures for every surface's test suite.
//!
//! Behind the `testkit` feature so it never ships in a surface binary. It lives
//! in `src/` rather than each crate's `tests/` because two surfaces need the
//! same frames, and a fixture duplicated per crate is a fixture that drifts.
//!
//! Everything here is a literal or a compile-time include. No clock, no socket,
//! no filesystem read at run time, no live hub.

#![allow(dead_code)]

use serde_json::Value;

use crate::frame::{Frame, JobView};

/// One hub frame carrying exactly one job per derived status, plus a departed
/// row (acceptance A1).
///
/// Shaped like what `GET /v1/stream` actually publishes
/// (`crates/nerve-hub/src/sse/frame.rs` → `{"jobs":[…],"departed":[…]}`, each
/// job carrying the hub-only `timeline` key from
/// `crates/nerve-hub/src/state/store.rs:457`), so the surface is proved to
/// tolerate fields it does not model.
///
/// The expected class of each row, and the `Nerve/Nerve/Models/Subject.swift`
/// line that decides it:
///
/// | id | facets | class | Swift |
/// |----|--------|-------|-------|
/// | `claude-code:problem` | `outcome=failure` | problem | `Subject.swift:39` |
/// | `claude-code:attention` | `attention{required,input}` | attention | `Subject.swift:43-52` |
/// | `claude-code:waiting` | `attention{suggested,resource}` | attention | `Subject.swift:43` |
/// | `claude-code:running` | `current.type=thinking`, poisoned summary | running | `Subject.swift:81-85` |
/// | `claude-code:success` | `active` + `outcome=partial` + `monitor` | monitor | `Subject.swift:66-77` |
/// | `claude-code:inactive` | `current.type=starting` | inactive | `Subject.swift:92-93` |
/// | `claude-code:departed` | `ended` + `outcome=failure` | (never counted) | — |
///
/// `claude-code:running` carries `current.summary = "build failed!!"` and a
/// matching `current.name`: free text that would flip a naive classifier to
/// `problem`. Structured facets say the job is healthy, so it stays `running`
/// (CLAUDE.md invariant 3).
pub const SIX_STATE_FRAME: &str = r#"{
  "jobs": [
    {
      "id": "claude-code:problem",
      "kind": "session",
      "name": "nerve",
      "alias": "local",
      "lifecycle": "active",
      "current": { "type": "tool", "name": "Bash", "summary": "Running tests" },
      "attention": { "level": "none" },
      "health": "ok",
      "outcome": "failure",
      "progress": { "kind": "none" },
      "producer": { "id": "claude-code", "name": "Claude Code", "kind": "agent" },
      "capabilities": [],
      "actions": [],
      "createdAt": "2026-07-19T07:00:00Z",
      "startedAt": "2026-07-19T07:00:00Z",
      "updatedAt": "2026-07-19T08:10:00Z",
      "version": 7,
      "extensions": { "pid": 4242 },
      "timeline": [
        { "kind": "attention.changed", "title": "Tool failed", "at": "2026-07-19T08:09:00Z" }
      ]
    },
    {
      "id": "claude-code:attention",
      "kind": "session",
      "name": "index-page",
      "alias": "local",
      "lifecycle": "active",
      "current": { "type": "idle" },
      "attention": { "level": "required", "reason": "input", "title": "Your turn" },
      "health": "ok",
      "producer": { "id": "claude-code", "name": "Claude Code" },
      "createdAt": "2026-07-19T07:30:00Z",
      "updatedAt": "2026-07-19T08:11:00Z",
      "version": 4,
      "timeline": []
    },
    {
      "id": "claude-code:waiting",
      "kind": "build",
      "name": "xcodebuild Nerve",
      "alias": "local",
      "lifecycle": "active",
      "attention": { "level": "suggested", "reason": "resource" },
      "health": "ok",
      "producer": { "id": "xcode" },
      "createdAt": "2026-07-19T08:00:00Z",
      "updatedAt": "2026-07-19T08:12:00Z",
      "version": 2,
      "timeline": []
    },
    {
      "id": "claude-code:running",
      "kind": "session",
      "name": "hub",
      "alias": "local",
      "lifecycle": "active",
      "current": {
        "type": "thinking",
        "name": "build failed!!",
        "summary": "build failed!!",
        "detail": "error: cannot continue"
      },
      "attention": { "level": "none", "title": "build failed!!" },
      "health": "ok",
      "producer": { "id": "claude-code", "name": "Claude Code" },
      "createdAt": "2026-07-19T08:01:00Z",
      "updatedAt": "2026-07-19T08:13:00Z",
      "version": 9,
      "timeline": [],
      "futureField": { "invented": ["by", "a", "later", "hub"] }
    },
    {
      "id": "claude-code:success",
      "kind": "session",
      "name": "docs",
      "alias": "local",
      "lifecycle": "active",
      "current": { "type": "monitor", "summary": "Waiting for stream feedback" },
      "attention": { "level": "none" },
      "health": "ok",
      "outcome": "partial",
      "producer": { "id": "claude-code", "name": "Claude Code" },
      "createdAt": "2026-07-19T07:45:00Z",
      "updatedAt": "2026-07-19T08:14:00Z",
      "version": 3,
      "timeline": []
    },
    {
      "id": "claude-code:inactive",
      "kind": "session",
      "name": "scratch",
      "alias": "local",
      "lifecycle": "active",
      "current": { "type": "starting", "summary": "Ready" },
      "attention": { "level": "none" },
      "health": "ok",
      "producer": { "id": "claude-code", "name": "Claude Code" },
      "createdAt": "2026-07-19T08:14:30Z",
      "updatedAt": "2026-07-19T08:14:30Z",
      "version": 1,
      "timeline": []
    }
  ],
  "departed": [
    {
      "id": "claude-code:departed",
      "kind": "session",
      "name": "gone",
      "alias": "local",
      "lifecycle": "ended",
      "outcome": "failure",
      "attention": { "level": "urgent", "reason": "failure" },
      "health": "unresponsive",
      "producer": { "id": "claude-code", "name": "Claude Code" },
      "createdAt": "2026-07-19T06:00:00Z",
      "endedAt": "2026-07-19T08:09:00Z",
      "updatedAt": "2026-07-19T08:09:00Z",
      "version": 12,
      "timeline": []
    }
  ]
}"#;

/// `fixtures/demo_snapshot.json`, embedded at compile time.
///
/// Path is relative to this file: `crates/nerve-tmux-surface/tests/common/` →
/// repo root. Compile-time inclusion means the popup golden breaks loudly if
/// the fixture drifts, and the test still reads no filesystem when it runs.
pub const DEMO_SNAPSHOT: &str = include_str!("../../../fixtures/demo_snapshot.json");

/// Decode a frame, failing the test with the raw text on any error.
pub fn frame(raw: &str) -> Frame {
    let mut value: Value = serde_json::from_str(raw).expect("fixture JSON");
    value["protocolVersion"] = serde_json::json!(2);
    value["hubInstanceId"] = serde_json::json!("fixture");
    value["revision"] = serde_json::json!(1);
    for key in ["jobs", "departed"] {
        if value[key].is_null() {
            value[key] = serde_json::json!([]);
        }
        if let Some(rows) = value[key].as_array_mut() {
            for row in rows {
                *row = nerve_hub::projection::project_json(row.clone());
            }
        }
    }
    value["summary"] = nerve_hub::projection::summarize(&value["jobs"]);
    Frame::decode(&value.to_string()).expect("projected hub fixture")
}

/// Build one `JobView` from a JSON literal.
pub fn job(value: Value) -> JobView {
    serde_json::from_value(nerve_hub::projection::project_json(value.clone()))
        .unwrap_or_else(|err| panic!("job must decode ({err}): {value}"))
}

/// The two jobs of `fixtures/demo_snapshot.json`, in file order.
///
/// The fixture is a *snapshot* envelope (`{"alias":…,"jobs":[…]}`), not a
/// frame, so its `jobs` array is lifted out here — the same array the hub
/// republishes inside every frame.
pub fn demo_jobs() -> Vec<JobView> {
    let snapshot: Value =
        serde_json::from_str(DEMO_SNAPSHOT).expect("fixtures/demo_snapshot.json must be JSON");
    snapshot
        .get("jobs")
        .and_then(Value::as_array)
        .expect("fixtures/demo_snapshot.json must carry a `jobs` array")
        .iter()
        .cloned()
        .map(job)
        .collect()
}

pub fn jobs(value: Value) -> Vec<JobView> {
    value
        .as_array()
        .expect("fixture job array")
        .iter()
        .cloned()
        .map(job)
        .collect()
}

pub fn summary(jobs: &[JobView]) -> crate::frame::HubSummary {
    let values=serde_json::json!(jobs.iter().map(|j|serde_json::json!({"id":j.id,"display":{"status":j.extensions.hub.status,"ask":j.extensions.hub.ask,"statusRank":j.extensions.hub.status_rank},"attention":{"level":j.attention.level.wire()}})).collect::<Vec<_>>());
    serde_json::from_value(nerve_hub::projection::summarize(&values)).unwrap()
}
