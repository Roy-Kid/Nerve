//! Windows presentation core for the native C# / WinUI 3 host.
//!
//! `nerve-windows-core` owns hub subscription, status derivation, grouping,
//! notification policy and navigation, exposing versioned NDJSON through
//! [`desktop`]. The native host in `surfaces/windows` owns all application UI.
//! The previous egui host is available only with the opt-in `legacy-ui` feature
//! for regression comparisons; the default build has no GUI toolkit dependency.

pub mod actions;
#[cfg(feature = "legacy-ui")]
pub mod app;
pub mod desktop;
pub mod flyout;
pub mod notify;
pub mod platform;
#[cfg(feature = "legacy-ui")]
pub mod runtime;
pub mod settings;
pub mod tray;
