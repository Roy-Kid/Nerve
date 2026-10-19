//! Job → the text a row prints.
//!
//! Derivations, not layout: what a cell *says*, with no opinion about how wide
//! it is or which characters a terminal counts as one column. A tmux row, a
//! tray tooltip and a panel line all want the same answers here; only one of
//! them has cells to pad.

use time::OffsetDateTime;

use crate::frame::JobView;

/// Shown when a cell has nothing to print.
pub const MISSING: &str = "-";

/// Activity / attention title — mirrored from `ExpandableSubjectRow.activityText`.
///
/// Borrowed from the job: every row asks for this on every repaint, and the
/// answer is always a field the job already holds. A busy turn (tool, shell,
/// thinking) keeps its own summary; the ask title waits until that turn ends.
pub fn activity_text(job: &JobView) -> &str {
    &job.extensions.hub.activity
}
pub fn status_label(job: &JobView) -> &str {
    &job.extensions.hub.status_label
}

/// What a cell prints, with [`MISSING`] standing in for nothing at all.
pub fn cell_text(value: &str) -> &str {
    if value.trim().is_empty() {
        MISSING
    } else {
        value
    }
}

/// How long ago the job last said anything.
///
/// A producer whose clock runs ahead reads as `0s` rather than as a negative
/// age. One ladder for the sidebar and the popup, so the two cannot drift.
pub fn age_label(now: OffsetDateTime, job: &JobView) -> String {
    let Some(updated) = job.updated_at else {
        return MISSING.to_string();
    };
    let seconds = (now - updated.instant()).whole_seconds().max(0);
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}
