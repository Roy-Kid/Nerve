//! `popup.rs` — the read-only job list behind `prefix + N`.
//!
//! ─────────────────────────────────────────────────────────────────────────
//! Tests are never edited to fit an implementation.
//! ─────────────────────────────────────────────────────────────────────────
//!
//! ```ignore
//! // nerve_tmux_surface::popup
//!
//! /// What a popup with nothing to show prints.
//! pub const EMPTY: &str = "nerve: no jobs";
//!
//! pub struct PopupRenderer { /* now */ }
//!
//! impl PopupRenderer {
//!     /// The instant ages are measured against.
//!     ///
//!     /// A frozen reading rather than a `Clock` trait: `display-popup -E`
//!     /// runs a one-shot process that renders once and exits, so "now" is
//!     /// read exactly once, at the composition root. That makes this the
//!     /// injection point *and* removes a port nobody would implement twice.
//!     pub fn at(now: time::OffsetDateTime) -> Self;
//!
//!     /// One line per job, in the order given.
//!     pub fn render(&self, jobs: &[frame::JobView]) -> String;
//! }
//! ```
//!
//! ─────────────────────────────────────────────────────────────────────────
//! ROW FORMAT (pinned here so the implementer has no latitude)
//! ─────────────────────────────────────────────────────────────────────────
//!
//! `{icon}  {name}  {activity}  {age}`, two spaces between the padded name and
//! activity columns, age unpadded. Status is the same icon glyph the sidebar
//! paints (no ANSI here). Every line ends with `\n`, including the last.
//!
//! | column | value |
//! |--------|-------|
//! | icon | `icons::status_icon(StatusClass::of(job))` |
//! | name | `name`, else `-` |
//! | activity | `columns::activity_text(job)`, else `-` |
//! | age | `now - updatedAt` as `<60s`→`Ns`, `<60m`→`Nm`, `<24h`→`Nh`, else `Nd`; `-` when the job carries no `updatedAt` |
//!
//! No `#` escaping: a popup is plain stdout, not a tmux option value, so
//! `summary::escape_dynamic` must **not** be applied here.
//!
//! Read-only is structural: `render` takes job facts and answers a string.
//! There is no action, no key, no callback to add one to (CLAUDE.md
//! invariant 6).
//!
//! Determinism: injected instant, compile-time fixture, literal jobs. No
//! clock, no socket, no filesystem at run time, no hub.

use serde_json::json;
use time::OffsetDateTime;
use time::macros::datetime;

use nerve_surface_core::frame::JobView;
use nerve_tmux_surface::popup::PopupRenderer;

use nerve_surface_core::testkit::job;

/// Five minutes after `fixtures/demo_snapshot.json` last updated its rows.
const NOW: OffsetDateTime = datetime!(2026-07-19 08:15:00 UTC);

fn render(jobs: &[JobView]) -> String {
    PopupRenderer::at(NOW).render(jobs)
}

/// A row with a fixed session name and no activity, so age assertions can be golden.
fn aged(updated_at: &str) -> String {
    render(&[job(json!({
        "id": "demo:j",
        "name": "j",
        "lifecycle": "active",
        "producer": { "id": "demo" },
        "updatedAt": updated_at
    }))])
}

// ── The fixture, golden ─────────────────────────────────────────────────────

/// `fixtures/demo_snapshot.json` rendered at a fixed instant, hard-coded.
#[test]
fn test_the_demo_fixture_renders_the_golden_popup() {
    let rendered = render(&nerve_surface_core::testkit::demo_jobs());

    assert_eq!(
        rendered,
        "● nerve             Writing continuous ribbon view 5m\n\
         ● xcodebuild Nerve  Compiling 42 files             5m\n"
    );
}

// ── One row, column by column ───────────────────────────────────────────────

#[test]
fn test_an_anonymous_row_still_renders_every_column() {
    let rendered = render(&[job(json!({ "id": "orphan" }))]);

    assert_eq!(rendered, "● -  - -\n");
}

// ── Ages ────────────────────────────────────────────────────────────────────

/// A producer whose clock runs ahead must not print a negative age.
#[test]
fn test_a_future_timestamp_reads_as_zero_seconds() {
    assert_eq!(aged("2026-07-19T08:20:00Z"), "● j  - 0s\n");
}

// ── Edge ────────────────────────────────────────────────────────────────────

/// Ordering belongs to the hub; the popup shows what it was handed.
#[test]
fn test_rows_keep_the_order_they_were_given() {
    let jobs = vec![
        job(json!({ "id": "b", "name": "b", "lifecycle": "active", "producer": { "id": "p" } })),
        job(
            json!({ "id": "a", "name": "a", "lifecycle": "active", "outcome": "failure",
                    "producer": { "id": "p" } }),
        ),
    ];

    let rendered = render(&jobs);

    let names: Vec<&str> = rendered
        .lines()
        .map(|line| {
            line.split_whitespace()
                .nth(1)
                .expect("every row has a name")
        })
        .collect();
    assert_eq!(names, vec!["b", "a"]);
}
