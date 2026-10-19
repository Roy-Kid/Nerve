//! Preserve the order supplied by the authoritative hub.
use crate::frame::JobView;
use std::cmp::Ordering;
use time::OffsetDateTime;
pub fn compare_jobs(_now: OffsetDateTime, a: &JobView, b: &JobView) -> Ordering {
    a.extensions.hub.order_key.cmp(&b.extensions.hub.order_key)
}
