//! In-memory state: the jobs the hub knows, and how reports change them.
//!
//! [`JobStore`] is the single write point. Time and process liveness are
//! injected traits, so every rule in here is testable with fakes alone.

pub mod dedupe;
mod demo;
pub mod machine;
mod patch;
pub mod pending;
pub mod reaper;
pub mod store;
pub mod timeline;

// Only the store and the seams callers must inject are lifted to this level;
// everything else stays addressed by the module that owns it.
pub use dedupe::{MAX_SEEN_EVENTS, SEEN_EVENT_EVICT_BATCH};
pub use machine::{LocalAlias, MachineName};
pub use reaper::{PidProbe, PidState, SignalProbe};
pub use store::{JobStore, JobView, JobsSeq};
