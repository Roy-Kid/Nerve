//! The panel behind the tray icon.

pub mod anchor;
#[cfg(feature = "legacy-ui")]
pub mod chips;
#[cfg(feature = "legacy-ui")]
pub mod chrome;
#[cfg(feature = "legacy-ui")]
pub mod fonts;
#[cfg(feature = "legacy-ui")]
pub mod ribbon;
#[cfg(feature = "legacy-ui")]
pub mod row;
pub mod sections;
#[cfg(feature = "legacy-ui")]
pub mod theme;
#[cfg(feature = "legacy-ui")]
pub mod view;
pub mod visibility;
