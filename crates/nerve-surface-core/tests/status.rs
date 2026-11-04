//! Structured display contract: human requests, automatic waits, execution and
//! turn completion stay distinct. No free-text inference.

use serde_json::{Value, json};

use nerve_surface_core::status::StatusClass;

use nerve_surface_core::testkit::{SIX_STATE_FRAME, frame, job};

/// The class of one literal job.
fn class(value: Value) -> StatusClass {
    StatusClass::of(&job(value))
}

// ── The enum itself ─────────────────────────────────────────────────────────

// ── :39 / :40 — problem ─────────────────────────────────────────────────────

/// `:39` runs before the lifecycle branches, so an ended failure is a problem
/// rather than an inactive row.
#[test]
fn test_line_39_beats_the_ended_lifecycle_branch() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "ended", "outcome": "failure" })),
        StatusClass::Problem
    );
}

/// `:40` runs before `:68`, so an ended-but-successful row that stopped
/// answering is still a problem.
#[test]
fn test_line_40_beats_an_ended_success() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "ended",
            "outcome": "success",
            "health": "unresponsive"
        })),
        StatusClass::Problem
    );
}

// ── :43-:53 — elevated attention (>= suggested) ─────────────────────────────

#[test]
fn test_busy_current_beats_stale_ask() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "current": { "type": "tool", "summary": "Using Read" },
            "attention": { "level": "required", "reason": "approval", "title": "Approval needed in agent" }
        })),
        StatusClass::Running
    );
}

#[test]
fn test_line_49_required_attention_asking_for_input_is_attention() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "attention": { "level": "required", "reason": "input" }
        })),
        StatusClass::Attention
    );
}

#[test]
fn test_line_53_elevated_attention_without_a_reason_is_attention() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "attention": { "level": "suggested" }
        })),
        StatusClass::Attention
    );
}

#[test]
fn test_line_43_every_wait_reason_at_required_is_waiting() {
    for reason in [
        "resource",
        "dependency",
        "queue",
        "system",
        "lock",
        "throttle",
        "rate",
        "capacity",
        "failure",
    ] {
        assert_eq!(
            class(json!({
                "id": "a",
                "lifecycle": "active",
                "attention": { "level": "required", "reason": reason }
            })),
            StatusClass::Waiting,
            "attention.reason `{reason}` at required must be attention"
        );
    }
}

// ── :56-:66 — informational attention ───────────────────────────────────────

#[test]
fn test_line_58_every_ask_reason_at_informational_is_attention() {
    for reason in [
        "input",
        "approval",
        "auth",
        "permission",
        "decision",
        "elicitation",
        "review",
    ] {
        assert_eq!(
            class(json!({
                "id": "a",
                "lifecycle": "active",
                "attention": { "level": "informational", "reason": reason }
            })),
            StatusClass::Attention,
            "attention.reason `{reason}` at informational must ask"
        );
    }
}

#[test]
fn test_line_48_a_wait_reason_at_informational_is_waiting() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "attention": { "level": "informational", "reason": "queue" }
        })),
        StatusClass::Waiting
    );
}

/// `:64` breaks out of the switch instead of returning, so an informational
/// note nobody classified leaves the job exactly as busy as it was.
#[test]
fn test_line_64_an_unrecognised_informational_reason_falls_through_to_running() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "attention": { "level": "informational", "reason": "chatter" }
        })),
        StatusClass::Running
    );
}

// ── :68-:74 — lifecycle ─────────────────────────────────────────────────────

#[test]
fn test_line_69_an_ended_success_is_success() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "ended", "outcome": "success" })),
        StatusClass::Success
    );
}

#[test]
fn test_line_70_an_ended_job_without_an_outcome_is_inactive() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "ended" })),
        StatusClass::Inactive
    );
}

#[test]
fn test_line_72_a_suspended_job_is_inactive() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "suspended" })),
        StatusClass::Inactive
    );
}

#[test]
fn test_line_73_a_pending_job_is_waiting() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "pending" })),
        StatusClass::Waiting
    );
}

#[test]
fn test_line_74_a_degraded_job_is_problem() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "active", "health": "degraded" })),
        StatusClass::Problem
    );
}

/// An open session reporting a partial outcome is a monitor holding the
/// stream: purple, not green (ended success) and not blue (still executing).
#[test]
fn test_line_77_an_open_job_with_a_partial_outcome_is_monitor() {
    assert_eq!(
        class(json!({ "id": "a", "lifecycle": "active", "outcome": "partial" })),
        StatusClass::Monitor
    );
}

// ── :81-:96 — current activity ──────────────────────────────────────────────

#[test]
fn test_line_84_shell_and_agent_activity_is_running() {
    for kind in ["subagent", "tool", "thinking", "info"] {
        assert_eq!(
            class(json!({
                "id": "a",
                "lifecycle": "active",
                "current": { "type": kind }
            })),
            StatusClass::Running,
            "current.type `{kind}` must run"
        );
    }
}

#[test]
fn test_line_86_an_open_monitor_is_monitor() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "current": { "type": "monitor" }
        })),
        StatusClass::Monitor
    );
}

#[test]
fn test_line_88_a_waiting_activity_is_waiting() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "current": { "type": "waiting" }
        })),
        StatusClass::Waiting
    );
}

/// A session that is open but has not taken a turn yet is Ready, never Running
/// (`.claude/notes/notes.md` 2026-07-22 "SessionStart is starting/Ready").
#[test]
fn test_line_92_idle_starting_and_booting_are_inactive() {
    for kind in ["idle", "starting", "booting"] {
        assert_eq!(
            class(json!({
                "id": "a",
                "lifecycle": "active",
                "current": { "type": kind }
            })),
            StatusClass::Inactive,
            "current.type `{kind}` must stay inactive"
        );
    }
}

/// `current.type` is producer vocabulary, not an enum: a spelling this table
/// never heard of leaves an open job running (`:94` breaks, `:98` answers).
#[test]
fn test_line_94_an_unmodelled_current_type_leaves_an_open_job_running() {
    assert_eq!(
        class(json!({
            "id": "a",
            "lifecycle": "active",
            "current": { "type": "editing" }
        })),
        StatusClass::Running
    );
}

// ── Free text never classifies (CLAUDE.md invariant 3) ──────────────────────

#[test]
fn test_no_free_text_field_can_move_a_running_job() {
    let poison = "FAILED — needs approval — error — waiting on you!!";

    assert_eq!(
        class(json!({
            "id": "a",
            "name": poison,
            "lifecycle": "active",
            "health": "ok",
            "current": {
                "type": "thinking",
                "name": poison,
                "summary": poison,
                "detail": poison
            },
            "attention": { "level": "none", "title": poison, "summary": poison }
        })),
        StatusClass::Running
    );
}

/// The mirror image: structured facets classify even when the prose is calm.
#[test]
fn test_calm_prose_does_not_hide_a_structured_problem() {
    assert_eq!(
        class(json!({
            "id": "a",
            "name": "all good",
            "lifecycle": "active",
            "outcome": "failure",
            "current": { "type": "thinking", "summary": "everything is fine" }
        })),
        StatusClass::Problem
    );
}

// ── The published frame, class by class ─────────────────────────────────────

/// Each live row of `nerve_surface_core::testkit::SIX_STATE_FRAME` lands on a painted class.
/// The `waiting` id is neutral Waiting, not human Attention.
#[test]
fn test_the_six_state_frame_separates_waiting_from_attention() {
    let decoded = frame(SIX_STATE_FRAME);

    let classes: Vec<(&str, StatusClass)> = decoded
        .jobs
        .iter()
        .map(|job| (job.id.as_str(), StatusClass::of(job)))
        .collect();

    assert_eq!(
        classes,
        vec![
            ("claude-code:problem", StatusClass::Problem),
            ("claude-code:attention", StatusClass::Attention),
            ("claude-code:waiting", StatusClass::Waiting),
            ("claude-code:running", StatusClass::Running),
            ("claude-code:success", StatusClass::Monitor),
            ("claude-code:inactive", StatusClass::Inactive),
        ]
    );
}

#[test]
fn completed_turn_and_real_ask_remain_distinct() {
    for (facets, expected) in [
        (
            json!({"current": {"type": "completed"}}),
            StatusClass::Success,
        ),
        (
            json!({"current": {"type": "completed"}, "attention": {"level": "required", "reason": "input"}}),
            StatusClass::Attention,
        ),
        (
            json!({"current": {"type": "thinking"}, "attention": {"level": "suggested", "reason": "input"}}),
            StatusClass::Running,
        ),
        (
            json!({"lifecycle": "ended", "outcome": "success", "attention": {"level": "required", "reason": "approval"}}),
            StatusClass::Success,
        ),
    ] {
        let mut value = json!({"id": "a", "lifecycle": "active"});
        value
            .as_object_mut()
            .unwrap()
            .extend(facets.as_object().unwrap().clone());
        assert_eq!(class(value), expected);
    }
}
