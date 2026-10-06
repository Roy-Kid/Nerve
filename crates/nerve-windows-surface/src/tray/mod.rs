//! The notification-area icon: what it shows and when it is redrawn.

pub mod bands;
pub mod dpi;
#[cfg(feature = "legacy-ui")]
pub mod events;
pub mod icon;
pub mod signature;
pub mod tooltip;
pub mod view;
