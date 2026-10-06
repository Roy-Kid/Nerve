//! Light or dark, as the taskbar has it.
//!
//! Two separate settings exist: `SystemUsesLightTheme` is the taskbar and the
//! notification area, `AppsUseLightTheme` is application chrome. Respect each
//! independently so a dark taskbar can coexist with a light panel.

use crate::tray::icon::Theme;

#[cfg(windows)]
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// What the taskbar is using right now.
///
/// Read on demand rather than watched: it is a cheap registry read on a path
/// already gated by the redraw signature, and `RegNotifyChangeKeyValue` would
/// need a thread and an `unsafe` call to save nothing.
#[cfg(windows)]
pub fn current() -> Theme {
    read("SystemUsesLightTheme")
}

#[cfg(windows)]
pub fn apps() -> Theme {
    read("AppsUseLightTheme")
}

#[cfg(windows)]
fn read(value_name: &str) -> Theme {
    let light = windows_registry::CURRENT_USER
        .open(PERSONALIZE)
        .and_then(|key| key.get_u32(value_name))
        .map(|value| value != 0)
        // Windows ships dark, and an icon lifted for dark is still readable on
        // light — the other way round is not.
        .unwrap_or(false);
    if light {
        Theme::Light
    } else {
        Theme::Dark
    }
}

/// Off Windows there is no taskbar to follow; dark is what the preview tools
/// and the tests assume.
#[cfg(not(windows))]
pub fn current() -> Theme {
    Theme::Dark
}

#[cfg(not(windows))]
pub fn apps() -> Theme {
    current()
}
