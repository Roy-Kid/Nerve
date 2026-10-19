//! Subscription counts and notification routing. Counts never stop the daemon.
pub mod notify;
pub mod refcount;
pub mod roster;

pub use notify::{NotifyLease, NotifyPolicy};
pub use refcount::{Presence, Subscription, Watchers};
pub use roster::{RosterGuard, SurfaceRoster};
