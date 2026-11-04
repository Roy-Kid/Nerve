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

// ── The budget ──────────────────────────────────────────────────────────────

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
