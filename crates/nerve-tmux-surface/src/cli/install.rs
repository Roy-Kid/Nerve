//! `nerve-tmux-surface install` — wire this tmux server (TPM / run-shell entry).
//!
//! Replaces `surfaces/tmux/nerve.conf` and the bash that used to grep `list-keys`
//! and `show-hooks`. Every tmux call is an arg array; the command always exits 0.

use crate::tmux::{self, SIDEBAR_BOTTOM_HEIGHT, SIDEBAR_POSITION, SIDEBAR_WIDTH};

const AUTO_CREATE: &str = "@nerve_sidebar_auto_create";
const TOGGLE_KEY: &str = "@nerve_sidebar_key";
const CLOSE_KEY: &str = "@nerve_sidebar_close_key";
const SIDEBAR_BIN: &str = "@nerve_sidebar_bin";

const DEFAULTS: [(&str, &str); 6] = [
    (SIDEBAR_WIDTH, "15%"),
    (SIDEBAR_POSITION, "left"),
    (SIDEBAR_BOTTOM_HEIGHT, "20"),
    (AUTO_CREATE, "on"),
    (TOGGLE_KEY, "e"),
    (CLOSE_KEY, "q"),
];

/// Fail-open: a missing helper or a tmux hiccup leaves the server as it was.
pub(crate) fn cmd_install(_args: &[String]) -> i32 {
    let Some(bin) = helper_bin() else {
        return 0;
    };
    let _ = tmux::run_tmux(&["set", "-g", SIDEBAR_BIN, &bin]);
    for (name, value) in DEFAULTS {
        set_default(name, value);
    }

    let key = option(TOGGLE_KEY).unwrap_or_else(|| "e".to_string());
    let close_key = option(CLOSE_KEY).unwrap_or_else(|| "q".to_string());
    bind_run(&key, &toggle_payload(&bin));
    bind_run(&close_key, &close_payload(&bin));

    drop_legacy_bindings();
    drop_legacy_hooks();

    if option(AUTO_CREATE).as_deref() != Some("off") {
        let _ = tmux::run_tmux(&["set-hook", "-g", "after-new-window", AFTER_NEW_WINDOW]);
    }
    let _ = tmux::run_tmux(&["set-hook", "-g", "pane-exited", PANE_EXITED]);
    0
}

/// Expand at hook time, not at install time: the helper may move.
const AFTER_NEW_WINDOW: &str = r##"run-shell '"#{@nerve_sidebar_bin}" toggle --create-only "#{window_id}" "#{pane_current_path}"'"##;
const PANE_EXITED: &str = r##"run-shell '"#{@nerve_sidebar_bin}" auto-close "#{window_id}"'"##;

fn helper_bin() -> Option<String> {
    let path = std::env::current_exe().ok()?;
    let path = path.canonicalize().unwrap_or(path);
    if !path.is_file() {
        return None;
    }
    let path = path.to_str()?.to_string();
    (!path.is_empty()).then_some(path)
}

fn option(name: &str) -> Option<String> {
    let value = tmux::run_tmux(&["show-options", "-gqv", name])?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn set_default(name: &str, value: &str) {
    if option(name).is_none() {
        let _ = tmux::run_tmux(&["set", "-g", name, value]);
    }
}

fn bind_run(key: &str, payload: &str) {
    let _ = tmux::run_tmux(&["bind", key, "run-shell", payload]);
}

fn toggle_payload(bin: &str) -> String {
    format!(r##""{bin}" toggle "#{{window_id}}" "#{{pane_current_path}}""##)
}

fn close_payload(bin: &str) -> String {
    format!(r##""{bin}" close "#{{window_id}}""##)
}

fn drop_legacy_bindings() {
    let Some(listing) = tmux::run_tmux(&["list-keys", "-T", "prefix"]) else {
        return;
    };
    for line in listing.lines() {
        if !is_legacy_prefix_binding(line) {
            continue;
        }
        if let Some(key) = prefix_key(line) {
            let _ = tmux::run_tmux(&["unbind-key", "-T", "prefix", key]);
        }
    }
}

fn drop_legacy_hooks() {
    for hook in ["after-new-window", "pane-exited"] {
        let Some(body) = tmux::run_tmux(&["show-hooks", "-g", hook]) else {
            continue;
        };
        if body.contains("agent_sidebar") {
            let _ = tmux::run_tmux(&["set-hook", "-gu", hook]);
        }
    }
}

/// Bindings older Nerve / tmux-agent-sidebar installs left on prefix+j / E.
fn is_legacy_prefix_binding(line: &str) -> bool {
    line.contains("nerve-tmux-surface") && (line.contains("popup") || line.contains("toggle-all"))
}

/// `bind-key -T prefix e run-shell "..."` → `e`.
fn prefix_key(line: &str) -> Option<&str> {
    let rest = line.split_once("-T prefix")?.1.trim();
    rest.split_whitespace().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_payload_expands_window_and_path_at_keypress() {
        let payload = toggle_payload("/opt/homebrew/bin/nerve-tmux-surface");
        assert!(payload.contains("nerve-tmux-surface"));
        assert!(payload.contains("toggle "));
        assert!(payload.contains("#{window_id}"));
        assert!(payload.contains("#{pane_current_path}"));
        assert!(!payload.contains("##{"));
    }

    #[test]
    fn legacy_popup_and_toggle_all_are_the_ones_unbound() {
        assert!(is_legacy_prefix_binding(
            r#"bind-key -T prefix j run-shell "nerve-tmux-surface popup""#
        ));
        assert!(is_legacy_prefix_binding(
            r#"bind-key -T prefix E run-shell "nerve-tmux-surface toggle-all""#
        ));
        assert!(!is_legacy_prefix_binding(
            r#"bind-key -T prefix e run-shell "nerve-tmux-surface toggle""#
        ));
        assert!(!is_legacy_prefix_binding(
            r#"bind-key -T prefix q run-shell "nerve-tmux-surface close""#
        ));
    }
}
