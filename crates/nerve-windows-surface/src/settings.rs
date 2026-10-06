//! What the user chose, and where it is kept.
//!
//! The only thing this surface writes to disk. Runtime job state stays in the
//! hub's memory (CLAUDE.md invariant 4) — what lives here is preferences, and
//! losing the file costs a user their window size, not their work.
//!
//! Every read is total: an unreadable file, an unknown key or an absurd number
//! all resolve to the default rather than to an error. A surface that refused
//! to start because its settings were malformed would be worse than one that
//! forgot them.

use std::path::PathBuf;

use nerve_surface_core::frame::AttentionLevel;
use serde::{Deserialize, Serialize};

/// Panel size limits, mirroring `SettingsStore.clampPanelWidth/Height`.
pub const WIDTH: (f32, f32) = (320.0, 560.0);
pub const HEIGHT: (f32, f32) = (240.0, 800.0);
const DEFAULT_SIZE: (f32, f32) = (340.0, 360.0);

/// How rows are bucketed in the panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupMode {
    /// Which machine reported it.
    #[default]
    Machine,
    /// Loudest first, ungrouped.
    Priority,
    /// One bucket per derived status.
    Status,
}

impl GroupMode {
    /// The order the context menu and the header button cycle through.
    pub fn next(self) -> Self {
        match self {
            Self::Machine => Self::Priority,
            Self::Priority => Self::Status,
            Self::Status => Self::Machine,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Machine => "Machine",
            Self::Priority => "Priority",
            Self::Status => "Status",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub panel_width: f32,
    pub panel_height: f32,
    pub group_mode: GroupMode,
    /// OS toasts for the Ask channel. Off until asked for: a Windows user very
    /// plausibly runs the VS Code extension too, and peers cannot know about
    /// each other (invariant 7), so two toasts for one Ask is a worse first
    /// impression than none.
    pub toasts_enabled: bool,
    pub toast_sound: bool,
    pub toast_floor: AttentionLevel,
    /// Start with Windows. Off by default: autostart plus invariant 4 means
    /// `nerve-hub.exe` becomes permanently resident, which is the user's call
    /// rather than an installer's.
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            panel_width: DEFAULT_SIZE.0,
            panel_height: DEFAULT_SIZE.1,
            group_mode: GroupMode::default(),
            toasts_enabled: false,
            toast_sound: false,
            toast_floor: AttentionLevel::Suggested,
            autostart: false,
        }
    }
}

impl Settings {
    /// Remember a user resize using the same bounds as the native window.
    pub fn set_panel_size(&mut self, width: f32, height: f32) {
        self.panel_width = clamp(width, WIDTH, DEFAULT_SIZE.0);
        self.panel_height = clamp(height, HEIGHT, DEFAULT_SIZE.1);
    }

    /// Pull every field back into a range the panel can actually draw.
    ///
    /// Applied on load as well as on save, because the file is editable and a
    /// hand-typed `0` should not produce an invisible window.
    pub fn clamped(mut self) -> Self {
        self.panel_width = clamp(self.panel_width, WIDTH, DEFAULT_SIZE.0);
        self.panel_height = clamp(self.panel_height, HEIGHT, DEFAULT_SIZE.1);
        self
    }

    /// Read from `path`, or the defaults if anything at all goes wrong.
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default()
            .clamped()
    }

    /// Write to `path`, creating its directory. Errors are the caller's to
    /// ignore — a preference that failed to save is not worth interrupting for.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw =
            serde_json::to_string_pretty(&self.clone().clamped()).map_err(std::io::Error::other)?;
        std::fs::write(path, raw)
    }
}

/// `%APPDATA%\Nerve\settings.json`, or the closest thing this OS has.
pub fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default();
    base.join("Nerve").join("settings.json")
}

/// Clamp that survives a non-finite value, which `f32::clamp` would propagate.
fn clamp(value: f32, range: (f32, f32), fallback: f32) -> f32 {
    if !value.is_finite() {
        return fallback;
    }
    value.clamp(range.0, range.1)
}
