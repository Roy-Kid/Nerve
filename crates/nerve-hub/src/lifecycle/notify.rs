//! Who may raise an OS notification for this hub.
//!
//! Surfaces still fire locally (the hub has no notification identity). What
//! this module owns is the *lease*: given the connected surface labels and a
//! policy, which one of them is allowed to interrupt. `All` is the old
//! behaviour — every surface that has notifications on may fire, and two on
//! one machine both will. `Single` (the default) elects one owner so the
//! macOS menu bar and the VS Code extension do not both banner the same Ask.
//!
//! The preference order is the native interrupt surfaces first, then the
//! in-app ones. A missing label falls through to lexicographic order so a
//! custom surface still gets a deterministic owner.

use serde::{Deserialize, Serialize};

/// How the hub spreads the interrupt channel across connected surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifyPolicy {
    /// Exactly one connected surface may fire. Default, because two Nerve
    /// surfaces on one machine (menu bar + VS Code, tray + VS Code) otherwise
    /// both interrupt for the same Ask.
    #[default]
    Single,
    /// Every connected surface that has notifications on may fire. The
    /// historical peer rule, kept as an explicit setting.
    All,
}

impl NotifyPolicy {
    /// The one spelling this policy has on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::All => "all",
        }
    }

    /// Parse a PUT body. Unknown spellings are a 400, not a silent default.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "single" => Some(Self::Single),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

/// Native interrupt surfaces, most specific first.
///
/// The macOS menu bar speaks UserNotifications; Windows the tray toast. VS Code and tmux interrupt inside their own UI. Prefer the OS
/// banner when one is connected, so an editor toast does not win over the
/// menu bar the user already has.
const PREFER: &[&str] = &["macos", "windows", "vscode", "tmux"];

/// The lease one frame (or `/v1/health`) carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NotifyLease {
    pub policy: NotifyPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub surfaces: Vec<String>,
    pub watchers: usize,
}

impl NotifyLease {
    /// Empty hub: nobody watching, default policy, no owner.
    pub fn empty(policy: NotifyPolicy) -> Self {
        Self {
            policy,
            owner: None,
            surfaces: Vec::new(),
            watchers: 0,
        }
    }

    /// Build the lease from the live connection set.
    pub fn from_connections(
        policy: NotifyPolicy,
        watchers: usize,
        mut surfaces: Vec<String>,
    ) -> Self {
        surfaces.sort_unstable();
        surfaces.dedup();
        let owner = elect(policy, &surfaces);
        Self {
            policy,
            owner,
            surfaces,
            watchers,
        }
    }

    /// JSON object the REST routes return (owner always present, null when
    /// none — more stable than omitting the key).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "policy": self.policy.as_str(),
            "owner": self.owner,
            "surfaces": self.surfaces,
            "watchers": self.watchers,
        })
    }
}

/// Pick the owner, or `None` when the policy says everyone (or no one is
/// watching).
pub fn elect(policy: NotifyPolicy, surfaces: &[String]) -> Option<String> {
    if policy == NotifyPolicy::All || surfaces.is_empty() {
        return None;
    }
    for preferred in PREFER {
        if surfaces.iter().any(|name| name == preferred) {
            return Some((*preferred).to_string());
        }
    }
    surfaces
        .iter()
        .filter(|name| name.as_str() != "anonymous")
        .min()
        .cloned()
        .or_else(|| surfaces.first().cloned())
}

/// Normalise `?surface=` into a roster key.
///
/// Empty, missing, and whitespace-only become `anonymous`. Case is folded so
/// `macOS` and `macos` count as one surface for election.
pub fn surface_label(raw: Option<&str>) -> String {
    let trimmed = raw.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return "anonymous".to_string();
    }
    let mut label = trimmed.to_ascii_lowercase();
    label.truncate(64);
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_prefers_macos_over_vscode() {
        let owner = elect(
            NotifyPolicy::Single,
            &["vscode".into(), "macos".into(), "tmux".into()],
        );
        assert_eq!(owner.as_deref(), Some("macos"));
    }

    #[test]
    fn single_falls_to_vscode_when_macos_is_gone() {
        let owner = elect(NotifyPolicy::Single, &["tmux".into(), "vscode".into()]);
        assert_eq!(owner.as_deref(), Some("vscode"));
    }

    #[test]
    fn all_elects_nobody() {
        assert_eq!(
            elect(NotifyPolicy::All, &["macos".into(), "vscode".into()]),
            None
        );
    }

    #[test]
    fn unknown_label_is_still_a_deterministic_owner() {
        let owner = elect(NotifyPolicy::Single, &["zeta".into(), "alpha".into()]);
        assert_eq!(owner.as_deref(), Some("alpha"));
    }

    #[test]
    fn empty_set_has_no_owner() {
        assert_eq!(elect(NotifyPolicy::Single, &[]), None);
    }

    #[test]
    fn surface_label_folds_case_and_blanks() {
        assert_eq!(surface_label(None), "anonymous");
        assert_eq!(surface_label(Some("  ")), "anonymous");
        assert_eq!(surface_label(Some("macOS")), "macos");
    }
}
