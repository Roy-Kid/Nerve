//! Row words: a busy turn keeps its own summary; the status word follows the reason.

use nerve_surface_core::display::{activity_text, status_label};
use nerve_surface_core::frame::JobView;
use serde_json::json;

fn job(value: serde_json::Value) -> JobView {
    nerve_surface_core::testkit::job(value)
}

#[test]
fn a_busy_turn_shows_its_summary_not_the_ask_title() {
    let row = job(json!({
        "id": "j",
        "current": { "type": " tool ", "summary": "cargo test" },
        "attention": { "level": "required", "title": "Approve", "reason": "approval" }
    }));
    assert_eq!(activity_text(&row), "cargo test");
}

#[test]
fn an_idle_ask_shows_the_attention_title() {
    let row = job(json!({
        "id": "j",
        "current": { "type": "idle", "summary": "waiting" },
        "attention": { "level": "required", "title": "Approve" }
    }));
    assert_eq!(activity_text(&row), "Approve");
}

#[test]
fn status_words_follow_the_reason() {
    let approval = job(json!({
        "id": "j",
        "attention": { "level": "required", "reason": "approval" }
    }));
    assert_eq!(status_label(&approval), "Approval needed");

    let done = job(json!({
        "id": "j",
        "lifecycle": "active",
        "outcome": "success",
        "current": { "type": "completed" }
    }));
    assert_eq!(status_label(&done), "Turn complete");
}
