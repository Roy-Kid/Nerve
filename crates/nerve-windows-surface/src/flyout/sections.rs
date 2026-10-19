//! Bucketing rows the way the user asked.
//!
//! The macOS panel offers three groupings and the tmux sidebar offers one; the
//! flyout offers all three, because the grouping is also what orders the ribbon
//! above it — switching it is meant to visibly reorder the strip.
//!
//! Machine grouping is [`nerve_surface_core::group`] verbatim. The other two
//! are the same jobs under a different title, so they reuse the one comparator
//! rather than inventing an order.

use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::group::group_by_machine;
use nerve_surface_core::sort::compare_jobs;
use nerve_surface_core::status::StatusClass;
use time::OffsetDateTime;

use crate::settings::GroupMode;

/// One bucket of rows, as positions in the snapshot they came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub jobs: Vec<usize>,
}

/// Bucket `jobs` under `mode`.
pub fn of(
    jobs: &[JobView],
    mode: GroupMode,
    filter: StatusFilter,
    now: OffsetDateTime,
) -> Vec<Section> {
    match mode {
        GroupMode::Machine => group_by_machine(jobs, now, filter)
            .into_iter()
            .map(|section| Section {
                title: section.title,
                jobs: section.jobs,
            })
            .collect(),
        GroupMode::Priority => priority(jobs, filter, now),
        GroupMode::Status => by_status(jobs, filter, now),
    }
}

/// One flat list, loudest first. No headings: the point of this mode is that
/// the order *is* the information.
fn priority(jobs: &[JobView], filter: StatusFilter, now: OffsetDateTime) -> Vec<Section> {
    let mut indices: Vec<usize> = jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| filter.classified(job).is_some())
        .map(|(index, _)| index)
        .collect();
    if indices.is_empty() {
        return Vec::new();
    }
    indices.sort_unstable_by(|a, b| compare_jobs(now, &jobs[*a], &jobs[*b]));
    vec![Section {
        title: String::new(),
        jobs: indices,
    }]
}

/// One bucket per derived status, in the order the statuses themselves rank.
fn by_status(jobs: &[JobView], filter: StatusFilter, now: OffsetDateTime) -> Vec<Section> {
    let classes: Vec<Option<StatusClass>> = jobs.iter().map(|job| filter.classified(job)).collect();
    let mut sections = Vec::new();
    for class in StatusClass::ALL {
        let mut indices: Vec<usize> = classes
            .iter()
            .enumerate()
            .filter(|(_, seen)| **seen == Some(class))
            .map(|(index, _)| index)
            .collect();
        if indices.is_empty() {
            continue;
        }
        indices.sort_unstable_by(|a, b| compare_jobs(now, &jobs[*a], &jobs[*b]));
        sections.push(Section {
            title: class.title().to_string(),
            jobs: indices,
        });
    }
    sections
}

/// The status of every row, in the order the panel will draw them.
///
/// This is what the ribbon above is built from, which is why it is derived from
/// the sections rather than from the snapshot: the strip has to agree with the
/// list under it.
pub fn painted_order(jobs: &[JobView], sections: &[Section]) -> Vec<StatusClass> {
    sections
        .iter()
        .flat_map(|section| section.jobs.iter())
        .filter_map(|index| jobs.get(*index))
        .map(StatusClass::of)
        .collect()
}
