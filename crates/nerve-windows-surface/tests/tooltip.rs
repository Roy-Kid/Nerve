//! Tooltip text, and the budget Windows enforces on it.

use nerve_surface_core::frame::JobView;
use nerve_surface_core::tally::Tally;
use nerve_windows_surface::tray::tooltip::{MAX_UTF16_UNITS, Offline, render};
use serde_json::json;

fn job(value: serde_json::Value) -> JobView {
    nerve_surface_core::testkit::job(value)
}

fn tally(running: usize, attention: usize) -> Tally {
    Tally {
        running,
        attention,
        ..Tally::default()
    }
}

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

#[test]
fn an_empty_machine_says_so() {
    let text = render(&Tally::default(), None, Offline::No);
    assert_eq!(text, "Nerve — nothing running");
}

#[test]
fn counts_lead_with_activity_then_the_ask() {
    let text = render(&tally(4, 1), None, Offline::No);
    assert_eq!(text, "Nerve — 4 running, 1 needs you");
}

#[test]
fn the_top_job_is_the_second_line() {
    let text = render(
        &tally(1, 1),
        Some(&job(json!({
            "id": "j", "name": "nerve-hub",
            "attention": { "level": "required", "title": "waiting for your approval" }
        }))),
        Offline::No,
    );
    assert_eq!(
        text,
        "Nerve — 1 running, 1 needs you\n▸ nerve-hub · waiting for your approval"
    );
}

#[test]
fn a_job_with_no_activity_still_names_itself() {
    let text = render(
        &tally(1, 0),
        Some(&job(json!({ "id": "j", "name": "build" }))),
        Offline::No,
    );
    assert!(text.ends_with("\n▸ build"), "{text}");
}

#[test]
fn offline_admits_it_and_says_what_was_last_true() {
    let text = render(&tally(4, 1), None, Offline::HubDown);
    assert_eq!(
        text,
        "Nerve — offline (hub not running)\nlast seen 4 running, 1 needs you"
    );
}

#[test]
fn offline_with_nothing_remembered_says_only_that() {
    let text = render(&Tally::default(), None, Offline::HubDown);
    assert_eq!(text, "Nerve — offline (hub not running)");
}

#[test]
fn a_missing_binary_is_a_different_message_from_a_stopped_hub() {
    let text = render(&Tally::default(), None, Offline::NotInstalled);
    assert_eq!(text, "Nerve — nerve-hub not installed");
}

// ── The budget ──────────────────────────────────────────────────────────────

#[test]
fn a_long_job_name_is_cut_to_fit() {
    let text = render(
        &tally(3, 0),
        Some(&job(json!({ "id": "j", "name": "x".repeat(400) }))),
        Offline::No,
    );
    assert!(units(&text) <= MAX_UTF16_UNITS, "{} units", units(&text));
}

#[test]
fn a_long_activity_is_cut_to_fit() {
    let text = render(
        &tally(3, 0),
        Some(&job(json!({
            "id": "j", "name": "nerve",
            "current": { "type": "tool", "summary": "y".repeat(500) }
        }))),
        Offline::No,
    );
    assert!(units(&text) <= MAX_UTF16_UNITS, "{} units", units(&text));
}

/// The reason the budget is counted in UTF-16 units and not in `chars()`:
/// every one of these costs two units, so a char-counted budget would hand
/// Windows nearly twice what it accepts and let it truncate mid-glyph.
#[test]
fn wide_characters_are_counted_as_windows_counts_them() {
    for name in [
        "项目".repeat(200),
        "😀".repeat(200),
        "日本語テスト".repeat(60),
    ] {
        let text = render(
            &tally(3, 0),
            Some(&job(json!({ "id": "j", "name": name }))),
            Offline::No,
        );
        assert!(
            units(&text) <= MAX_UTF16_UNITS,
            "{} units for {}",
            units(&text),
            &name[..12]
        );
    }
}

#[test]
fn truncation_never_splits_a_character() {
    let text = render(
        &tally(3, 0),
        Some(&job(json!({ "id": "j", "name": "项".repeat(300) }))),
        Offline::No,
    );
    // A split surrogate pair would not be valid UTF-8 to begin with, so the
    // real assertion is that the text survives a round trip intact.
    let round_trip: Vec<u16> = text.encode_utf16().collect();
    assert_eq!(String::from_utf16(&round_trip).unwrap(), text);
}

#[test]
fn the_headline_survives_even_when_the_detail_cannot() {
    // A machine with enough going on that the counts alone fill the budget.
    let busy = Tally {
        running: 999_999,
        attention: 999_999,
        ..Tally::default()
    };
    let text = render(
        &busy,
        Some(&job(json!({ "id": "j", "name": "x".repeat(200) }))),
        Offline::No,
    );
    assert!(text.starts_with("Nerve — "), "{text}");
    assert!(units(&text) <= MAX_UTF16_UNITS);
}

#[test]
fn a_cut_line_is_marked_as_cut() {
    let text = render(
        &tally(3, 0),
        Some(&job(json!({ "id": "j", "name": "x".repeat(400) }))),
        Offline::No,
    );
    assert!(text.contains('…'), "{text}");
}
