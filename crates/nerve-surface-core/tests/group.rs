//! Grouping and ordering, as a pure function.
//!
//! The same shape `nerve-tmux-surface`'s panel test pinned before this code
//! was extracted — asserted here against the function itself, with no surface,
//! no viewport and no clock of its own.

use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::group::group_by_machine;
use serde_json::json;
use time::OffsetDateTime;
use time::macros::datetime;

const NOW: OffsetDateTime = datetime!(2026-07-19 12:00:00 UTC);

fn jobs(value: serde_json::Value) -> Vec<JobView> {
    nerve_surface_core::testkit::jobs(value)
}

/// `(section title, [job id])` — the whole observable result in one literal.
fn shape(jobs: &[JobView], filter: StatusFilter) -> Vec<(String, Vec<String>)> {
    group_by_machine(jobs, NOW, filter)
        .into_iter()
        .map(|section| {
            let ids = section
                .jobs
                .iter()
                .map(|index| jobs[*index].id.clone())
                .collect();
            (section.title, ids)
        })
        .collect()
}

/// Order inside a machine is status, then createdAt descending. A job with no
/// createdAt falls back to updatedAt, then to `now`, so a missing timestamp
/// does not reshuffle when `now` changes. `a-stale` has only an old updatedAt,
/// which is why it sits after the two dated rows.
#[test]
fn sections_group_by_machine_and_order_within_each() {
    let fixtures = jobs(json!([
        { "id": "b-old", "name": "b-old", "alias": "Beta",
          "createdAt": "2026-07-19T08:00:00Z" },
        { "id": "a-stale", "name": "a-stale", "alias": "alpha",
          "updatedAt": "2020-01-01T00:00:00Z" },
        { "id": "a-new", "name": "a-new", "alias": "alpha",
          "createdAt": "2026-07-19T09:00:00Z" },
        { "id": "a-old", "name": "a-old", "alias": "alpha",
          "createdAt": "2026-07-19T08:00:00Z" },
        { "id": "b-loud", "name": "b-loud", "alias": "Beta",
          "attention": { "level": "required" },
          "createdAt": "2020-01-01T00:00:00Z" },
    ]));

    assert_eq!(
        shape(&fixtures, StatusFilter::default()),
        vec![
            // Titles sort case-insensitively, so `alpha` precedes `Beta`.
            (
                "alpha".to_string(),
                vec![
                    "a-new".to_string(),
                    "a-old".to_string(),
                    "a-stale".to_string()
                ]
            ),
            // Attention outranks every time field.
            (
                "Beta".to_string(),
                vec!["b-loud".to_string(), "b-old".to_string()]
            ),
        ]
    );
}

/// The grouping is a pure function of `now`, so the same input at a different
/// instant must give the same answer. This is the drift the tmux panel test
/// could only pin indirectly.
#[test]
fn a_later_now_does_not_reorder_jobs_that_report_no_update() {
    let fixtures = jobs(json!([
        { "id": "old", "name": "old", "alias": "m", "createdAt": "2026-07-19T08:00:00Z" },
        { "id": "new", "name": "new", "alias": "m", "createdAt": "2026-07-19T09:00:00Z" },
    ]));

    let early = group_by_machine(
        &fixtures,
        datetime!(2026-07-19 10:00:00 UTC),
        StatusFilter::default(),
    );
    let late = group_by_machine(
        &fixtures,
        datetime!(2031-01-01 00:00:00 UTC),
        StatusFilter::default(),
    );

    assert_eq!(early, late);
    assert_eq!(early[0].jobs, vec![1, 0], "newer createdAt first");
}

#[test]
fn a_producer_that_named_no_machine_lands_under_unknown() {
    let fixtures = jobs(json!([{ "id": "j", "name": "j", "alias": "  " }]));

    assert_eq!(
        shape(&fixtures, StatusFilter::default()),
        vec![("unknown".to_string(), vec!["j".to_string()])]
    );
}

#[test]
fn an_empty_snapshot_has_no_sections() {
    assert!(shape(&[], StatusFilter::default()).is_empty());
}
