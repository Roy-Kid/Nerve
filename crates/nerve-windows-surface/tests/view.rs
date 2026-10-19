//! What the tray says about a snapshot.

use nerve_surface_core::frame::JobView;
use nerve_surface_core::store::JobsSnapshot;
use nerve_windows_surface::tray::icon::Theme;
use nerve_windows_surface::tray::view;
use serde_json::json;
use time::OffsetDateTime;
use time::macros::datetime;

const NOW: OffsetDateTime = datetime!(2026-07-19 12:00:00 UTC);

fn jobs(value: serde_json::Value) -> Vec<JobView> {
    nerve_surface_core::testkit::jobs(value)
}

fn snapshot(jobs: Vec<JobView>, offline: bool) -> JobsSnapshot {
    JobsSnapshot {
        summary: nerve_surface_core::testkit::summary(&jobs),
        jobs,
        offline,
        notify: Default::default(),
        ..Default::default()
    }
}

fn view(snapshot: &JobsSnapshot, hub_installed: bool) -> view::View {
    view::of(snapshot, hub_installed, Theme::Dark, 16, NOW)
}

#[test]
fn an_empty_machine_draws_the_idle_mark() {
    let view = view(&snapshot(Vec::new(), false), true);
    assert!(view.icon.bands.is_empty());
    assert_eq!(view.tooltip, "Nerve — nothing running");
}

#[test]
fn a_busy_machine_names_its_loudest_job() {
    let view = view(
        &snapshot(
            jobs(json!([
                { "id": "quiet", "name": "quiet", "current": { "type": "tool" } },
                { "id": "loud", "name": "deploy",
                  "attention": { "level": "required", "title": "waiting for your approval" } }
            ])),
            false,
        ),
        true,
    );
    assert!(
        view.tooltip.contains("deploy"),
        "the tooltip named the wrong job: {}",
        view.tooltip
    );
    assert!(view.tooltip.contains("waiting for your approval"));
}

/// Two states that look the same to the hub and must not look the same to the
/// user: one fixes itself, the other is an install the user has not finished.
#[test]
fn a_stopped_hub_and_a_missing_one_read_differently() {
    let rows = jobs(json!([{ "id": "j", "name": "nerve" }]));
    let stopped = view(&snapshot(rows.clone(), true), true);
    let missing = view(&snapshot(rows, true), false);

    assert!(stopped.tooltip.contains("offline"), "{}", stopped.tooltip);
    assert!(
        missing.tooltip.contains("not installed"),
        "{}",
        missing.tooltip
    );
}

/// Going offline never blanks the rows: the hub publishes full frames, so the
/// next successful attach restores everything. Forgetting would be a lie.
#[test]
fn going_offline_keeps_the_bands_and_marks_them() {
    let rows = jobs(json!([{ "id": "j", "name": "nerve", "current": { "type": "tool" } }]));
    let live = view(&snapshot(rows.clone(), false), true);
    let gone = view(&snapshot(rows, true), true);

    assert_eq!(gone.icon.bands.len(), live.icon.bands.len());
    assert!(gone.icon.offline);
    assert_ne!(gone.signature, live.signature, "the icon must be redrawn");
}

#[test]
fn an_unchanged_snapshot_keeps_its_signature() {
    let rows = jobs(json!([{ "id": "j", "name": "nerve", "current": { "type": "tool" } }]));
    let first = view(&snapshot(rows.clone(), false), true);
    let again = view(&snapshot(rows, false), true);
    assert_eq!(first.signature, again.signature);
}

#[test]
fn the_tooltip_always_fits_what_windows_accepts() {
    let rows = jobs(json!([{
        "id": "j", "name": "项目".repeat(200),
        "current": { "type": "tool", "summary": "x".repeat(400) }
    }]));
    let view = view(&snapshot(rows, false), true);
    assert!(view.tooltip.encode_utf16().count() <= 127);
}

#[test]
fn new_activity_updates_the_tooltip_without_redrawing_identical_bands() {
    let before = view(
        &snapshot(
            jobs(json!([
                { "id": "j", "name": "project", "current": { "type": "tool", "summary": "Reading files" } }
            ])),
            false,
        ),
        true,
    );
    let after = view(
        &snapshot(
            jobs(json!([
                { "id": "j", "name": "project", "current": { "type": "tool", "summary": "Running tests" } }
            ])),
            false,
        ),
        true,
    );
    assert_eq!(before.signature, after.signature);
    assert_ne!(before.tooltip, after.tooltip);
}
