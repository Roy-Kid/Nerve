//! Where Nerve writes debug logs on this machine.
//!
//! Jobs stay in hub RAM (CLAUDE.md invariant 4). These files are only the
//! daemon and surface traces, so a spawn that discards stderr still leaves
//! something to read.

use std::path::PathBuf;

/// Directory for Nerve log files. Created by the process that first writes.
pub fn dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home().join("Library/Logs/Nerve")
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(home)
            .join("Nerve")
            .join("Logs")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local/state"))
            .join("nerve")
            .join("logs")
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
