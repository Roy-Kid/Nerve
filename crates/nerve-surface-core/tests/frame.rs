//! `frame.rs` — decoding one SSE frame (spec T2 · acceptance A1).
//!
//! ─────────────────────────────────────────────────────────────────────────
//! API CONTRACT — the implementer fills `src/frame.rs` to satisfy this file.
//! Tests are never edited to fit an implementation.
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```ignore
//! // nerve_surface_core::frame
//!
//! /// One frame from `GET /v1/stream`: two keys, forever
//! /// (`crates/nerve-hub/src/sse/frame.rs`).
//! #[derive(Debug, Deserialize)]
//! pub struct Frame {
//!     pub jobs: Vec<JobView>,      // absent key => empty
//!     pub departed: Vec<JobView>,  // absent key => empty
//! }
//! impl Frame {
//!     pub fn decode(raw: &str) -> Result<Frame, FrameError>;
//! }
//!
//! /// Why a frame could not be read. `Display` carries the serde message.
//! #[derive(Debug)]
//! pub struct FrameError { /* … */ }
//!
//! /// The subset of a hub job this surface paints. Everything but `id`
//! /// defaults: a surface renders what it was sent and never refuses a frame
//! /// over a field it does not model.
//! #[derive(Clone, Debug, Deserialize)]
//! #[serde(rename_all = "camelCase")]
//! pub struct JobView {
//!     pub id: String,
//!     pub kind: String,                    // default "session"
//!     pub name: String,                    // default ""
//!     pub alias: String,                   // default ""
//!     pub lifecycle: Lifecycle,            // default Active
//!     pub current: Option<Current>,
//!     pub attention: Attention,            // default level None
//!     pub health: Health,                  // default Ok
//!     pub outcome: Option<Outcome>,
//!     pub producer: ProducerInfo,          // default empty id
//!     pub created_at: Option<WireTime>,
//!     pub updated_at: Option<WireTime>,
//! }
//!
//! #[derive(Clone, Debug, Deserialize)]
//! #[serde(rename_all = "camelCase")]
//! pub struct Current {
//!     #[serde(rename = "type")] pub kind: String,
//!     pub name: Option<String>,
//!     pub summary: Option<String>,
//!     pub detail: Option<String>,
//! }
//!
//! #[derive(Clone, Debug, Default, Deserialize)]
//! pub struct Attention {
//!     pub level: AttentionLevel,
//!     pub reason: Option<String>,
//!     pub title: Option<String>,
//!     pub summary: Option<String>,
//! }
//!
//! #[derive(Clone, Debug, Default, Deserialize)]
//! pub struct ProducerInfo {
//!     pub id: String,
//!     pub name: Option<String>,
//!     pub kind: Option<String>,
//! }
//!
//! /// Lenient wire enums, ported from `crates/nerve-hub/src/model/facet.rs`:
//! /// input is trimmed and lower-cased, anything unrecognised degrades to the
//! /// fallback rather than failing the frame.
//! pub enum Lifecycle { Created, Pending, Active, Suspended, Ended, Unknown }
//! pub enum AttentionLevel { None, Informational, Suggested, Required, Urgent }
//! pub enum Health { Ok, Degraded, Unresponsive, Unknown }
//! pub enum Outcome { Success, Failure, Cancelled, Partial, Unknown }
//! // `AttentionLevel` is `Ord`, ascending in the order declared above
//! // (`CoreTypes.swift:29-41`) — `Subject.swift:43` compares with `>=`.
//!
//! /// A UTC instant at whole-second precision — the one wire date shape
//! /// (`crates/nerve-hub/src/model/time.rs`).
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! pub struct WireTime(/* … */);
//! impl WireTime {
//!     pub fn instant(self) -> time::OffsetDateTime;
//! }
//! ```
//!
//! Determinism: literal frames, compile-time fixture include, no clock, no
//! socket, no filesystem at run time.

use time::macros::datetime;

use nerve_surface_core::frame::{AttentionLevel, Frame, Health, JobView, Lifecycle, Outcome};

use nerve_surface_core::testkit::{SIX_STATE_FRAME, frame, job};

// ── The published frame shape ───────────────────────────────────────────────

#[test]
fn test_decode_reads_every_job_of_the_frame() {
    let decoded = frame(SIX_STATE_FRAME);

    let ids: Vec<&str> = decoded.jobs.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "claude-code:problem",
            "claude-code:attention",
            "claude-code:waiting",
            "claude-code:running",
            "claude-code:success",
            "claude-code:inactive",
        ]
    );
}

#[test]
fn test_decode_keeps_departed_rows_in_their_own_list() {
    let decoded = frame(SIX_STATE_FRAME);

    let ids: Vec<&str> = decoded.departed.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(ids, vec!["claude-code:departed"]);
}

/// The hub publishes each job with a `timeline` it keeps for it
/// (`crates/nerve-hub/src/state/store.rs:457`). This surface ignores it, and a
/// key it has never heard of must not cost it the frame either.
#[test]
fn test_unknown_job_fields_including_timeline_are_tolerated() {
    let decoded = frame(SIX_STATE_FRAME);

    let running = &decoded.jobs[3];
    assert_eq!(running.id, "claude-code:running");
    assert_eq!(running.name, "hub");
}

#[test]
fn test_unknown_top_level_frame_fields_are_tolerated() {
    let decoded =
        frame(r#"{"jobs":[],"departed":[],"seq":41,"serverTime":"2026-07-19T08:00:00Z"}"#);

    assert!(decoded.jobs.is_empty());
    assert!(decoded.departed.is_empty());
}

#[test]
fn test_absent_frame_keys_decode_as_empty_lists() {
    let decoded = frame("{}");

    assert!(decoded.jobs.is_empty());
    assert!(decoded.departed.is_empty());
}

// ── Job defaults ────────────────────────────────────────────────────────────

/// Only `id` is required. Everything else takes the hub's own default
/// (`crates/nerve-hub/src/model/job.rs:26-69`), so a terse producer still
/// yields a well-formed row.
#[test]
fn test_a_job_with_only_an_id_decodes_with_hub_defaults() {
    let decoded: JobView = job(serde_json::json!({ "id": "claude-code:bare" }));

    assert_eq!(decoded.id, "claude-code:bare");
    assert_eq!(decoded.kind, "session");
    assert_eq!(decoded.name, "");
    assert_eq!(decoded.alias, "");
    assert_eq!(decoded.lifecycle, Lifecycle::Active);
    assert_eq!(decoded.health, Health::Ok);
    assert_eq!(decoded.attention.level, AttentionLevel::None);
    assert!(decoded.current.is_none());
    assert!(decoded.outcome.is_none());
    assert!(decoded.updated_at.is_none());
}

#[test]
fn test_a_job_without_an_id_is_refused() {
    assert!(Frame::decode(r#"{"jobs":[{"name":"nameless"}],"departed":[]}"#).is_err());
}

/// The wire key is `type`; the Rust field cannot be.
#[test]
fn test_current_type_decodes_into_the_kind_field() {
    let decoded = job(serde_json::json!({
        "id": "claude-code:x",
        "current": { "type": "thinking", "summary": "Planning" }
    }));

    let current = decoded.current.expect("current must decode");
    assert_eq!(current.kind, "thinking");
    assert_eq!(current.summary.as_deref(), Some("Planning"));
}

#[test]
fn test_producer_name_and_kind_are_optional() {
    let decoded = job(serde_json::json!({
        "id": "xcode:build",
        "producer": { "id": "xcode" }
    }));

    assert_eq!(decoded.producer.id, "xcode");
    assert!(decoded.producer.name.is_none());
    assert!(decoded.producer.kind.is_none());
}

// ── Lenient enums (`facet.rs` parity) ───────────────────────────────────────

#[test]
fn test_unknown_lifecycle_degrades_to_unknown() {
    let decoded = job(serde_json::json!({ "id": "a", "lifecycle": "hibernating" }));

    assert_eq!(decoded.lifecycle, Lifecycle::Unknown);
}

#[test]
fn test_unknown_attention_level_degrades_to_none() {
    let decoded = job(serde_json::json!({ "id": "a", "attention": { "level": "screaming" } }));

    assert_eq!(decoded.attention.level, AttentionLevel::None);
}

#[test]
fn test_unknown_health_degrades_to_unknown() {
    let decoded = job(serde_json::json!({ "id": "a", "health": "sick" }));

    assert_eq!(decoded.health, Health::Unknown);
}

#[test]
fn test_unknown_outcome_degrades_to_unknown_but_stays_present() {
    let decoded = job(serde_json::json!({ "id": "a", "outcome": "aborted" }));

    assert_eq!(decoded.outcome, Some(Outcome::Unknown));
}

#[test]
fn test_enum_spellings_are_trimmed_and_case_folded() {
    let decoded = job(serde_json::json!({
        "id": "a",
        "lifecycle": " ENDED ",
        "health": "Degraded",
        "outcome": "SUCCESS",
        "attention": { "level": "Required" }
    }));

    assert_eq!(decoded.lifecycle, Lifecycle::Ended);
    assert_eq!(decoded.health, Health::Degraded);
    assert_eq!(decoded.outcome, Some(Outcome::Success));
    assert_eq!(decoded.attention.level, AttentionLevel::Required);
}

/// `Subject.swift:43` and `:56` compare with `>=`, so the ordering is part of
/// the contract, not an implementation detail (`CoreTypes.swift:29-41`).
#[test]
fn test_attention_levels_order_from_none_up_to_urgent() {
    assert!(AttentionLevel::None < AttentionLevel::Informational);
    assert!(AttentionLevel::Informational < AttentionLevel::Suggested);
    assert!(AttentionLevel::Suggested < AttentionLevel::Required);
    assert!(AttentionLevel::Required < AttentionLevel::Urgent);
}

// ── Dates ───────────────────────────────────────────────────────────────────

#[test]
fn test_wire_dates_decode_to_utc_second_precision() {
    let decoded = job(serde_json::json!({
        "id": "a",
        "createdAt": "2026-07-19T07:00:00Z",
        "updatedAt": "2026-07-19T08:10:00Z"
    }));

    assert_eq!(
        decoded.created_at.expect("createdAt must decode").instant(),
        datetime!(2026-07-19 07:00:00 UTC)
    );
    assert_eq!(
        decoded.updated_at.expect("updatedAt must decode").instant(),
        datetime!(2026-07-19 08:10:00 UTC)
    );
}

/// The hub publishes canonical `Z` dates, but a producer talking straight to a
/// surface may not; RFC3339 is read, then normalised (`model/time.rs:70-76`).
#[test]
fn test_an_offset_date_is_normalised_to_utc() {
    let decoded = job(serde_json::json!({ "id": "a", "updatedAt": "2026-07-19T10:10:00+02:00" }));

    assert_eq!(
        decoded.updated_at.expect("updatedAt must decode").instant(),
        datetime!(2026-07-19 08:10:00 UTC)
    );
}

// ── Refusals ────────────────────────────────────────────────────────────────

/// A frame that cannot be read is an error, never a silently empty frame: the
/// caller (`stream.rs`) decides whether to keep the last segment or go offline.
#[test]
fn test_text_that_is_not_json_is_an_error() {
    assert!(Frame::decode("no server running").is_err());
}

#[test]
fn test_a_frame_whose_jobs_are_not_a_list_is_an_error() {
    assert!(Frame::decode(r#"{"jobs":{"id":"a"},"departed":[]}"#).is_err());
}

#[test]
fn test_frame_error_explains_itself() {
    let err = Frame::decode("{").expect_err("truncated JSON must fail");

    assert!(
        !err.to_string().is_empty(),
        "FrameError must Display the reason"
    );
}

#[test]
fn job_extensions_keep_role_group_and_ribbon() {
    let decoded = job(serde_json::json!({
        "id": "j",
        "extensions": {
            "role": "member",
            "groupId": "g1",
            "parentJobId": "parent",
            "paintRibbon": false,
            "mystery": 1
        }
    }));

    assert_eq!(decoded.extensions.role.as_deref(), Some("member"));
    assert_eq!(decoded.extensions.group_id.as_deref(), Some("g1"));
    assert_eq!(decoded.extensions.parent_job_id.as_deref(), Some("parent"));
    assert_eq!(decoded.extensions.paint_ribbon, Some(false));
}

#[test]
fn a_missing_notify_object_is_observation_only() {
    let decoded = frame(r#"{"jobs":[]}"#);
    assert_eq!(
        decoded.notify.policy,
        nerve_surface_core::notify::NotifyPolicy::Single
    );
}

#[test]
fn a_present_notify_object_without_a_policy_is_single() {
    let decoded = frame(r#"{"jobs":[],"notify":{"owner":"macos"}}"#);
    assert_eq!(decoded.notify.owner.as_deref(), Some("macos"));
}

#[test]
fn an_unknown_notify_policy_on_a_frame_is_single() {
    let decoded = frame(r#"{"notify":{"policy":"nope","owner":"windows"}}"#);
    assert_eq!(
        decoded.notify.policy,
        nerve_surface_core::notify::NotifyPolicy::Single
    );
}

#[test]
fn incompatible_protocol_and_unprojected_jobs_require_upgrade() {
    assert!(
        Frame::decode(r#"{"jobs":[]}"#)
            .unwrap_err()
            .to_string()
            .contains("Upgrade")
    );
    assert!(
        Frame::decode_jobs_list(r#"[{"id":"legacy"}]"#)
            .unwrap_err()
            .to_string()
            .contains("Upgrade")
    );
}
