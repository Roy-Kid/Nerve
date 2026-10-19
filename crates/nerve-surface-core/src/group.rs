//! Jobs, bucketed by the machine that reported them.
//!
//! Buckets appear in first-seen order and are then titled case-insensitively,
//! so the list is stable across frames rather than reshuffling as the hub's
//! order changes. Within a bucket the order is [`crate::sort::compare_jobs`].
//!
//! Sections hold *indices* into the slice they were built from, not clones —
//! a surface already owns the snapshot and re-cloning every job per frame is
//! the kind of work a 150 ms frame pump cannot afford.

use std::cmp::Ordering;

use time::OffsetDateTime;

use crate::filter::StatusFilter;
use crate::frame::JobView;
use crate::sort::compare_jobs;

/// One machine's jobs, as positions in the snapshot they came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSection {
    pub title: String,
    pub jobs: Vec<usize>,
}

pub fn machine_label(job: &JobView) -> &str {
    job.extensions.hub.machine_label.trim()
}

/// Section title for a job whose producer named no machine.
const UNKNOWN_MACHINE: &str = "unknown";

/// Order two section titles the way `to_lowercase()` would, without building
/// the two lower-cased copies it takes to answer.
fn case_insensitive(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

/// Group `jobs` by machine, keeping only what `filter` admits.
///
/// `now` is passed in rather than sampled so the whole result is a pure
/// function of its inputs — and so the fallback in
/// [`crate::sort::compare_jobs`] stays consistent across the sort.
pub fn group_by_machine(
    jobs: &[JobView],
    now: OffsetDateTime,
    filter: StatusFilter,
) -> Vec<MachineSection> {
    let mut buckets: Vec<(String, String, Vec<usize>)> = Vec::new();
    for (index, job) in jobs.iter().enumerate() {
        if !filter.matches(job) {
            continue;
        }
        let title = match machine_label(job) {
            "" => UNKNOWN_MACHINE,
            label => label,
        };
        let key = &job.extensions.hub.machine_group;
        if let Some(bucket) = buckets.iter_mut().find(|(group, _, _)| group == key) {
            bucket.2.push(index);
        } else {
            buckets.push((key.to_string(), title.to_string(), vec![index]));
        }
    }
    buckets.sort_unstable_by(|(_, a, _), (_, b, _)| case_insensitive(a, b));
    buckets
        .into_iter()
        .map(|(_, title, mut bucket)| {
            bucket.sort_unstable_by(|a, b| compare_jobs(now, &jobs[*a], &jobs[*b]));
            MachineSection {
                title,
                jobs: bucket,
            }
        })
        .collect()
}
