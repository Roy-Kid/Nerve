//! `prefix + e` — open (no focus steal) or toggle focus.
//! `prefix + q` — close when the sidebar pane is focused (`close` subcommand).
//!
//! 1. **closed** → open sidebar, focus stays on content pane
//! 2. **open, sidebar focused** → unfocus (back to content)
//! 3. **open, sidebar unfocused** → focus sidebar
//!
//! Auto-create (`--create-only`) is the same open-without-focus path.

use std::env;

use crate::tmux::{self, SIDEBAR_ROLE};

/// Window option: pane to restore when unfocusing the sidebar.
const RETURN_PANE: &str = "@nerve_sidebar_return";

pub(crate) fn cmd_toggle(args: &[String]) -> i32 {
    let mut create_only = false;
    let mut positional = Vec::new();
    for arg in args {
        if arg == "--create-only" {
            create_only = true;
        } else {
            positional.push(arg.as_str());
        }
    }
    let window_id = match positional.first() {
        Some(id) => *id,
        None => return 0,
    };
    let pane_path = positional.get(1).copied().unwrap_or("~");
    let format = tmux::pane_id_role_format();
    let panes_output =
        tmux::run_tmux(&["list-panes", "-t", window_id, "-F", &format]).unwrap_or_default();

    if let Some(sidebar_pane) = find_sidebar(&panes_output) {
        if create_only {
            return 0;
        }
        let current_pane = tmux::display_message(window_id, "#{pane_id}");
        let current_role =
            tmux::display_message(&current_pane, &format!("#{{{}}}", tmux::PANE_ROLE));
        if current_role == SIDEBAR_ROLE {
            unfocus_sidebar(window_id, &sidebar_pane);
            flash("Nerve sidebar — unfocused (prefix+e to focus)");
        } else {
            let _ = tmux::run_tmux(&["select-pane", "-t", &sidebar_pane]);
            flash("Nerve sidebar — focused");
        }
        return 0;
    }

    // Auto-create keeps the human where they were; `prefix + e` does not.
    open_sidebar_in(window_id, pane_path, !create_only);
    0
}

pub(crate) fn cmd_close(args: &[String]) -> i32 {
    let window_id = match args.first() {
        Some(id) => id.as_str(),
        None => return 0,
    };
    let format = tmux::pane_id_role_format();
    let panes_output =
        tmux::run_tmux(&["list-panes", "-t", window_id, "-F", &format]).unwrap_or_default();
    let Some(sidebar_pane) = find_sidebar(&panes_output) else {
        return 0;
    };
    let current_pane = tmux::display_message(window_id, "#{pane_id}");
    let current_role = tmux::display_message(&current_pane, &format!("#{{{}}}", tmux::PANE_ROLE));
    if current_role != SIDEBAR_ROLE {
        flash("Nerve: focus sidebar (prefix+e), then prefix+q to close");
        return 0;
    }
    // The pane is about to be killed, so its own restore will never run.
    crate::preview::restore_stranded_swap(window_id);
    // Where the human was before the sidebar opened. Read before the kill: the
    // option outlives the pane, but the answer must not depend on that.
    let returning = return_pane(window_id).filter(|pane| pane != &sidebar_pane);
    let _ = tmux::run_tmux(&["kill-pane", "-t", &sidebar_pane]);
    // Closing is a return, not a jump: tmux would otherwise hand the focus to
    // whichever pane it likes, which in a three-pane window is rarely the one
    // that was interrupted.
    if let Some(pane) = returning.filter(|pane| tmux::pane_exists(pane)) {
        let _ = tmux::run_tmux(&["select-pane", "-t", &pane]);
    }
    clear_return_pane(window_id);
    flash("Nerve sidebar closed");
    0
}

/// Unfocus when the sidebar TUI receives `q` / `Esc` (same as prefix+e while focused).
pub fn unfocus_from_sidebar_tui() {
    let sidebar_pane = env::var("TMUX_PANE").unwrap_or_default();
    if sidebar_pane.is_empty() {
        return;
    }
    let role = tmux::display_message(&sidebar_pane, &format!("#{{{}}}", tmux::PANE_ROLE));
    if role != SIDEBAR_ROLE {
        return;
    }
    let window_id = tmux::display_message(&sidebar_pane, "#{window_id}");
    unfocus_sidebar(&window_id, &sidebar_pane);
}

/// Open the sidebar, and decide who keeps the keyboard.
///
/// `prefix + e` is a person asking for the sidebar, so it lands there — one
/// keystroke, one result. Auto-create is not: a window opening its own sidebar
/// must leave the human where they were typing.
fn open_sidebar_in(window_id: &str, pane_path: &str, focus: bool) {
    let prior_pane = tmux::display_message(window_id, "#{pane_id}");
    let sidebar_pane = open_sidebar(window_id, pane_path);
    if sidebar_pane.is_empty() {
        return;
    }
    // Where `prefix + q` and unfocus put the human back, either way.
    if !prior_pane.is_empty() && prior_pane != sidebar_pane {
        set_return_pane(window_id, &prior_pane);
    }
    if focus {
        let _ = tmux::run_tmux(&["select-pane", "-t", &sidebar_pane]);
        flash("Nerve sidebar — focused (prefix+q closes)");
        return;
    }
    if !prior_pane.is_empty() {
        let _ = tmux::run_tmux(&["select-pane", "-t", &prior_pane]);
    }
    flash("Nerve sidebar open — prefix+e focus, prefix+q close");
}

pub(crate) fn cmd_auto_close(args: &[String]) -> i32 {
    let window_id = match args.first() {
        Some(id) => id.as_str(),
        None => return 0,
    };
    let format = tmux::pane_id_role_format();
    let Some(roles) = tmux::run_tmux(&["list-panes", "-t", window_id, "-F", &format]) else {
        return 0;
    };
    if only_sidebar_remains(&roles) {
        let _ = tmux::run_tmux(&["kill-window", "-t", window_id]);
    }
    0
}

/// Whether a window has nothing left in it but the sidebar.
///
/// Every pane must *be* the sidebar. A pane with no role is somebody's shell —
/// the common case, since only the sidebar ever sets one — and a window still
/// holding one is not empty, however the pane-exited hook found it.
fn only_sidebar_remains(panes: &str) -> bool {
    let mut sidebar_seen = false;
    for line in panes.lines() {
        // A line we cannot read is a pane we cannot claim is the sidebar.
        let Some((_, role)) = line.split_once('|') else {
            return false;
        };
        if role != SIDEBAR_ROLE {
            return false;
        }
        sidebar_seen = true;
    }
    sidebar_seen
}

fn open_sidebar(window_id: &str, pane_path: &str) -> String {
    let sidebar_width = resolve_width(window_id);
    let sidebar_position = SidebarPosition::from_setting(&tmux::display_message(
        window_id,
        &format!("#{{{}}}", tmux::SIDEBAR_POSITION),
    ));
    let geometry = tmux::run_tmux(&[
        "list-panes",
        "-t",
        window_id,
        "-F",
        "#{pane_left} #{pane_width} #{pane_id}",
    ])
    .unwrap_or_default();
    let target_pane = target_pane_for_position(&geometry, sidebar_position)
        .unwrap_or_else(|| window_id.to_string());
    let self_bin = std::env::current_exe()
        .ok()
        .map(|path| path.canonicalize().unwrap_or(path))
        .and_then(|p| p.to_str().map(str::to_string))
        .unwrap_or_else(|| "nerve-tmux-surface".to_string());
    // The binary itself is the pane command. tmux still wraps it in
    // `$SHELL -c`, but that is not a login shell — the previous `sh -lc`
    // loaded the user's profile on every prefix+e.
    let sidebar_pane = tmux::run_tmux(&[
        "split-window",
        split_window_flags(sidebar_position),
        "-l",
        &sidebar_width,
        "-t",
        &target_pane,
        "-c",
        pane_path,
        "-P",
        "-F",
        "#{pane_id}",
        &self_bin,
    ])
    .map(|s| s.trim().to_string())
    .unwrap_or_default();
    if !sidebar_pane.is_empty() {
        tmux::set_pane_option(&sidebar_pane, tmux::PANE_ROLE, SIDEBAR_ROLE);
    }
    sidebar_pane
}

fn unfocus_sidebar(window_id: &str, sidebar_pane: &str) {
    let returned = return_pane(window_id);
    let target = returned
        .filter(|pane| pane != sidebar_pane && tmux::pane_exists(pane))
        .or_else(|| tmux::sibling_content_pane(window_id, sidebar_pane));
    if let Some(pane) = target {
        let _ = tmux::run_tmux(&["select-pane", "-t", &pane]);
    }
}

fn set_return_pane(window_id: &str, pane: &str) {
    let _ = tmux::run_tmux(&["set", "-w", "-t", window_id, RETURN_PANE, pane]);
}

fn clear_return_pane(window_id: &str) {
    let _ = tmux::run_tmux(&["set", "-w", "-u", "-t", window_id, RETURN_PANE]);
}

fn return_pane(window_id: &str) -> Option<String> {
    let value = tmux::display_message(window_id, &format!("#{{{RETURN_PANE}}}"));
    (!value.is_empty()).then_some(value)
}

fn flash(message: &str) {
    let _ = tmux::run_tmux(&["display-message", "-d", "2000", message]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarPosition {
    Left,
    Right,
}

impl SidebarPosition {
    fn from_setting(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "right" => Self::Right,
            _ => Self::Left,
        }
    }
}

fn resolve_width(window_id: &str) -> String {
    let setting = tmux::display_message(window_id, &format!("#{{{}}}", tmux::SIDEBAR_WIDTH));
    let setting = if setting.is_empty() {
        "15%".to_string()
    } else {
        setting
    };
    if !setting.ends_with('%') {
        return setting;
    }
    let window_width: u32 = tmux::display_message(window_id, "#{window_width}")
        .parse()
        .unwrap_or(0);
    let pct: u32 = setting.trim_end_matches('%').parse().unwrap_or(15);
    if window_width > 0 && pct > 0 {
        ((window_width * pct / 100).max(1)).to_string()
    } else {
        setting
    }
}

fn find_sidebar(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() >= 2 && parts[1] == SIDEBAR_ROLE {
            Some(parts[0].to_string())
        } else {
            None
        }
    })
}

fn split_window_flags(position: SidebarPosition) -> &'static str {
    match position {
        SidebarPosition::Left => "-hfb",
        SidebarPosition::Right => "-hf",
    }
}

fn target_pane_for_position(output: &str, position: SidebarPosition) -> Option<String> {
    let mut best: Option<(i32, String)> = None;
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let left: i32 = parts.next()?.parse().ok()?;
        let width: i32 = parts.next()?.parse().ok()?;
        let pane = parts.next()?.to_string();
        let edge = left + width;
        let score = match position {
            SidebarPosition::Left => -left,
            SidebarPosition::Right => edge,
        };
        match &best {
            None => best = Some((score, pane)),
            Some((prev, _)) if score > *prev => best = Some((score, pane)),
            _ => {}
        }
    }
    best.map(|(_, pane)| pane)
}

#[cfg(test)]
mod auto_close_tests {
    use super::only_sidebar_remains;

    #[test]
    fn a_lone_sidebar_closes_its_window() {
        assert!(only_sidebar_remains("%1|nerve-sidebar"));
    }

    #[test]
    fn a_live_shell_keeps_the_window() {
        // Regression: only the sidebar ever sets a role, so an empty role is a
        // real pane — treating it as "nothing left" killed windows with work
        // still in them.
        assert!(!only_sidebar_remains("%0|\n%1|nerve-sidebar"));
        assert!(!only_sidebar_remains("%1|nerve-sidebar\n%2|"));
    }

    #[test]
    fn an_unreadable_row_never_closes_anything() {
        assert!(!only_sidebar_remains("%1|nerve-sidebar\ngarbage"));
    }
}
