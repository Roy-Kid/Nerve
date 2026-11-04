//! Row words: a busy turn keeps its own summary; the status word follows the reason.

use nerve_surface_core::display::activity_text;
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
