//! The three groupings, and the order they put rows in.

use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::frame::JobView;
use nerve_windows_surface::flyout::sections::{Section, of};
use nerve_windows_surface::settings::GroupMode;
use serde_json::json;
use time::OffsetDateTime;
use time::macros::datetime;

const NOW: OffsetDateTime = datetime!(2026-07-19 12:00:00 UTC);

fn jobs() -> Vec<JobView> {
    nerve_surface_core::testkit::jobs(json!([
        { "id": "a-run", "name": "a-run", "alias": "alpha",
          "current": { "type": "tool" }, "createdAt": "2026-07-19T08:00:00Z" },
        { "id": "b-ask", "name": "b-ask", "alias": "Beta",
          "attention": { "level": "required", "reason": "approval" } },
        { "id": "a-fail", "name": "a-fail", "alias": "alpha",
          "lifecycle": "ended", "outcome": "failure" },
    ]))
}

fn shape(sections: &[Section], jobs: &[JobView]) -> Vec<(String, Vec<String>)> {
    sections
        .iter()
        .map(|section| {
            (
                section.title.clone(),
                section
                    .jobs
                    .iter()
                    .map(|index| jobs[*index].id.clone())
                    .collect(),
            )
        })
        .collect()
}

/// The point of this mode is that the order carries the information, so it has
/// one unnamed bucket rather than headings a person has to read past.
#[test]
fn priority_mode_is_one_flat_list_loudest_first() {
    let jobs = jobs();
    let sections = of(&jobs, GroupMode::Priority, StatusFilter::default(), NOW);
    assert_eq!(sections.len(), 1);
    assert!(sections[0].title.is_empty());

    let ids: Vec<&str> = sections[0]
        .jobs
        .iter()
        .map(|index| jobs[*index].id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["a-fail", "b-ask", "a-run"],
        "a problem outranks an ask, and both outrank running"
    );
}

#[test]
fn every_mode_draws_every_job() {
    let jobs = jobs();
    for mode in [GroupMode::Machine, GroupMode::Priority, GroupMode::Status] {
        let sections = of(&jobs, mode, StatusFilter::default(), NOW);
        let total: usize = sections.iter().map(|s| s.jobs.len()).sum();
        assert_eq!(total, jobs.len(), "{mode:?} dropped a job");
        let _ = shape(&sections, &jobs);
    }
}
