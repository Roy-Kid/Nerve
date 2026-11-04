//! Starting with Windows.
//!
//! `HKCU\…\CurrentVersion\Run`, which is per-user, needs no elevation, and —
//! the reason it wins over the Startup folder — shows up in Task Manager's
//! Startup tab, which is where a person looks when they want it gone. A
//! Startup-folder shortcut would need a shell link, and the only reason to
//! want one of those is the AppUserModelID, which [`super::aumid`] handles
//! separately.
//!
//! Default off, and deliberately so: the hub is resident once started
//! (CLAUDE.md invariant 4), so autostart means `nerve-hub.exe` runs from every
//! login on. That is a legitimate thing to want and not something an installer
//! should decide.
//!
//! Every function is total. Failing to read or write a registry value costs
//! the feature, never the surface.

/// The value name under `Run`.
pub const ENTRY: &str = "Nerve";

/// The flag the autostarted copy is launched with.
///
/// It exists so the surface can tell a login from a double-click and stay out
/// of the way during the login storm.
pub const AUTOSTART_FLAG: &str = "--autostart";

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Whether Windows will start this surface at login.
#[cfg(windows)]
pub fn is_enabled() -> bool {
    windows_registry::CURRENT_USER
        .open(RUN_KEY)
        .and_then(|key| key.get_string(ENTRY))
        .is_ok()
}

/// Turn autostart on or off.
///
/// Returns whether the registry actually agreed, so a menu can show the real
/// state rather than the one it asked for.
#[cfg(windows)]
pub fn set(enabled: bool) -> bool {
    let Ok(key) = windows_registry::CURRENT_USER.create(RUN_KEY) else {
        return is_enabled();
    };
    if enabled {
        let Some(exe) = super::paths::exe_path() else {
            return false;
        };
        // Quoted: a path under `C:\Program Files` has a space in it, and an
        // unquoted Run value is split on the first one.
        let command = format!("\"{}\" {AUTOSTART_FLAG}", exe.display());
        let _ = key.set_string(ENTRY, &command);
    } else {
        let _ = key.remove_value(ENTRY);
    }
    is_enabled()
}

/// Nothing to read off Windows; the menu item is hidden there.
#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}

/// Off Windows this is not offered, so asking for it changes nothing.
#[cfg(not(windows))]
pub fn set(_enabled: bool) -> bool {
    false
}
