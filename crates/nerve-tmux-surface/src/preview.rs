//! Mirror the selected job's tmux pane into the sibling content pane, and jump
//! to it.
//!
//! Uses `swap-pane -d` so the sidebar keeps focus while the right-hand pane
//! shows the agent terminal for the selected job. One monitor window can flip
//! through every job without leaving the session.
//!
//! Which pane a job belongs to is [`crate::panes`]; this module is what the
//! sidebar *does* with that answer — swap it in, put it back, land the human on
//! it, and (for a job on another machine) ask that machine's tmux to turn to it
//! (`crate::remote`).

use std::time::{Duration, Instant};

use crate::panes::{self, JobTarget, SidebarPane};
use crate::remote;
use crate::tmux;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::jobpath::{is_ide_deep_link, job_path};
use nerve_surface_core::machine;

/// Outcome of [`PanePreview::jump_to_job`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpResult {
    /// Focus moved to a tmux pane (no terminal suspend needed).
    TmuxPane,
    /// Job `openURL` is an IDE deep link (`cursor://` / `vscode://`) — suspend TUI
    /// before handing the URL to the OS opener.
    NeedsIde,
    /// No tmux pane and no IDE deep link (or switch failed).
    Failed,
}

/// How often a sidebar in a background window asks whether it is on screen
/// again.
///
/// It has nothing to paint there and the question is a tmux round trip, so it
/// is worth a second of staleness rather than five spawns a second.
const HIDDEN_RECHECK: Duration = Duration::from_secs(1);

/// How often the sidebar asks which pane the human is standing in.
///
/// The cursor follows the focus, so this is the latency of that; a tmux round
/// trip is cheap but not free, and nothing here needs to be instant.
const FOCUS_POLL: Duration = Duration::from_millis(400);

/// What the preview last acted on: a job, as it looked when it was matched.
///
/// The pid is part of the key because it is what the match is made of. A job
/// reported before its producer published one matches on its workspace path
/// alone — which is ambiguous between two agents in the same project — and the
/// frame that finally carries the pid has to re-run the scan rather than be
/// recognised as "already showing".
#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviewKey {
    id: String,
    pid: Option<u32>,
}

impl PreviewKey {
    fn of(job: &JobView) -> Self {
        Self {
            id: job.id.clone(),
            pid: job.extensions.pid,
        }
    }
}

/// Tracks which foreign pane was swapped into the preview slot.
pub struct PanePreview {
    sidebar: SidebarPane,
    preview_slot: Option<String>,
    swapped_with: Option<String>,
    last_shown: Option<PreviewKey>,
    /// When to ask again whether this window is back on screen.
    hidden_until: Option<Instant>,
    /// Pane the human was last seen standing in, and when that was asked.
    focused_pane: Option<String>,
    focus_checked: Option<Instant>,
    /// Serialises the "show that job" calls to other machines, newest wins.
    remote: remote::RemoteSelector,
}

impl PanePreview {
    pub fn new() -> Self {
        let sidebar = SidebarPane::detect();
        // A sidebar that was killed rather than closed left its swap in place.
        // Undo it before this one starts moving panes of its own.
        restore_stranded_swap(&sidebar.window);
        Self {
            sidebar,
            preview_slot: None,
            swapped_with: None,
            last_shown: None,
            hidden_until: None,
            focused_pane: None,
            focus_checked: None,
            remote: remote::RemoteSelector::new(),
        }
    }

    /// A preview bound to no pane: it matches nothing and moves nothing.
    ///
    /// What unit tests build their state with, so a `cargo test` run started
    /// from inside tmux cannot rearrange the window it was started in.
    pub fn detached() -> Self {
        Self {
            sidebar: SidebarPane::none(),
            preview_slot: None,
            swapped_with: None,
            last_shown: None,
            hidden_until: None,
            focused_pane: None,
            focus_checked: None,
            remote: remote::RemoteSelector::new(),
        }
    }

    /// Whether the preview slot is already the answer for `job`.
    pub fn is_showing(&self, job: &JobView) -> bool {
        self.last_shown.as_ref() == Some(&PreviewKey::of(job))
    }

    /// Whether anything at all is mirrored into the preview slot.
    pub fn is_idle(&self) -> bool {
        self.last_shown.is_none()
    }

    /// Show `job` in the sibling pane, restoring any prior swap first.
    ///
    /// The highlight moves and the pane follows it — there is no separate
    /// "preview" state to get out of step with the cursor. That only holds if
    /// the memo is written *after* the decision: recording the job first meant
    /// a selection that changed while this window was off screen (the list
    /// re-sorts on every frame) was remembered as shown, and coming back left
    /// the pane on the previous job forever.
    pub fn show(&mut self, job: &JobView) {
        if self.is_showing(job) {
            return;
        }
        if self.sidebar.id.is_empty() || !self.on_screen() {
            return;
        }
        let Some(slot) = self.slot() else {
            return;
        };
        // Match against the unswapped layout: pane ids move with their
        // contents, so a stale swap would skew every locality comparison.
        self.unswap();
        self.last_shown = Some(PreviewKey::of(job));

        let Some(pane) = self.pane_to_show(job) else {
            let _ = tmux::run_tmux(&[
                "display-message",
                "-d",
                "2000",
                &format!("Nerve: {}", nothing_to_show(job)),
            ]);
            return;
        };

        // Already visible in this window — a job does not have to be moved to
        // be looked at, and moving it would shuffle the layout under whoever is
        // standing in it.
        if pane == slot || tmux::display_message(&pane, "#{window_id}") == self.sidebar.window {
            return;
        }
        if tmux::run_tmux(&["swap-pane", "-d", "-s", &pane, "-t", &slot]).is_some() {
            tmux::set_window_option(
                &self.sidebar.window,
                tmux::PREVIEW_SWAP,
                &format!("{pane} {slot}"),
            );
            // Claim it, so the sidebar in the next window looks elsewhere.
            tmux::set_pane_option(&pane, tmux::PREVIEW_OWNER, &self.sidebar.id);
            self.swapped_with = Some(pane);
        }
    }

    /// The pane this job is showing on, wherever it lives.
    ///
    /// A job on another machine has no pane of its own here, so the ssh session
    /// that reaches it is what gets shown — and that machine's tmux is asked to
    /// turn to this job's window, which is what makes one shared session show
    /// the row under the cursor rather than whatever it was left on.
    fn pane_to_show(&mut self, job: &JobView) -> Option<String> {
        let target = JobTarget::of(job)?;
        match &target {
            JobTarget::Local { .. } => panes::find_pane_for_target(&target, job, &self.sidebar),
            JobTarget::Remote { host } => {
                let route = panes::find_ssh_route(host, &self.sidebar)?;
                if let Some(pid) = job.extensions.pid {
                    self.remote.request(&route.destination, pid);
                }
                Some(route.pane)
            }
        }
    }

    /// Restore the preview swap, then focus the job.
    ///
    /// Prefer the matched tmux pane (session/window/pane). Only when the hook
    /// reported an IDE deep link (`cursor://` / `vscode://` — agent ran inside
    /// that editor's integrated terminal) fall back to opening that URL.
    /// Plain `file://` workspace URIs are never opened as apps from here.
    pub fn jump_to_job(&mut self, job: &JobView) -> JumpResult {
        if let Some(host) = machine::ssh_host(job) {
            return self.jump_to_remote_job(job, host);
        }
        let target =
            panes::find_pane_for_job(job, &self.sidebar).or_else(|| self.swapped_with.clone());
        self.settle(job);
        if let Some(pane) = target.filter(|p| tmux::pane_exists(p)) {
            if switch_to_pane(&pane) {
                return JumpResult::TmuxPane;
            }
            let _ = tmux::run_tmux(&[
                "display-message",
                "-d",
                "2000",
                "Nerve: could not switch to job pane",
            ]);
            return JumpResult::Failed;
        }
        if job_has_ide_link(job) {
            return JumpResult::NeedsIde;
        }
        let _ = tmux::run_tmux(&[
            "display-message",
            "-d",
            "3500",
            &format!("Nerve: {}", nothing_to_show(job)),
        ]);
        JumpResult::Failed
    }

    /// Land on a job running on another machine.
    ///
    /// Two steps, and the second one is the point: focus the ssh pane that
    /// reaches that machine, then ask *its* tmux to select the window the agent
    /// is actually on ([`crate::remote`]). Without the second step every job on
    /// a machine lands on the same view — which is what the ssh pane happens to
    /// be showing, not what was picked.
    ///
    /// The remote call runs on a thread of its own: the local jump has already
    /// happened, and a sidebar must not freeze on someone else's network.
    fn jump_to_remote_job(&mut self, job: &JobView, host: &str) -> JumpResult {
        let route = panes::find_ssh_route(host, &self.sidebar);
        self.settle(job);

        let Some(route) = route.filter(|route| tmux::pane_exists(&route.pane)) else {
            if job_has_ide_link(job) {
                return JumpResult::NeedsIde;
            }
            let _ = tmux::run_tmux(&[
                "display-message",
                "-d",
                "3500",
                &format!("Nerve: {}", nothing_to_show(job)),
            ]);
            return JumpResult::Failed;
        };

        if !switch_to_pane(&route.pane) {
            let _ = tmux::run_tmux(&[
                "display-message",
                "-d",
                "2000",
                "Nerve: could not switch to the ssh pane",
            ]);
            return JumpResult::Failed;
        }
        if let Some(pid) = job.extensions.pid {
            // Through the same queue the preview uses, so a select that is
            // still on the wire cannot land after this one.
            self.remote.request(&route.destination, pid);
        }
        JumpResult::TmuxPane
    }

    /// Open IDE deep link from `job.location.openURL` (suspend TUI first).
    pub fn open_ide_for_job(&self, job: &JobView) -> bool {
        focus_ide(job)
    }

    /// Put the preview slot back how it was, and forget what was shown.
    pub fn restore(&mut self) {
        self.last_shown = None;
        self.unswap();
    }

    /// Put the slot back, but keep remembering the job that was shown.
    ///
    /// What `Enter` leaves behind. The human is now standing in the job's own
    /// pane; coming back to the sidebar must not drag that pane into the
    /// preview slot again — the selection has not changed, so there is nothing
    /// new to show. Moving the cursor is what starts previewing again.
    fn settle(&mut self, job: &JobView) {
        self.last_shown = Some(PreviewKey::of(job));
        self.unswap();
    }

    /// Pids on the pane the human just moved to, when that changed.
    ///
    /// The other half of "highlight moves, pane follows": walk into a pane
    /// yourself and the cursor should come to you. `None` while the sidebar
    /// itself has the keyboard — the cursor is the human's then — and while
    /// nothing has moved since the last look.
    pub fn newly_focused_pids(&mut self) -> Option<Vec<u32>> {
        if self.sidebar.id.is_empty() {
            return None;
        }
        if self
            .focus_checked
            .is_some_and(|at| at.elapsed() < FOCUS_POLL)
        {
            return None;
        }
        self.focus_checked = Some(Instant::now());

        let active = tmux::display_message(&self.sidebar.window, "#{pane_id}\t#{pane_tty}");
        let (pane, tty) = active.split_once('\t')?;
        if pane.is_empty() || self.focused_pane.as_deref() == Some(pane) {
            return None;
        }
        self.focused_pane = Some(pane.to_string());
        if pane == self.sidebar.id {
            return None;
        }
        Some(panes::pids_on_tty(tty))
    }

    /// Whether this sidebar's window is on screen, asked at most once a second
    /// while it is not.
    ///
    /// Nothing is remembered about a job decided while hidden — the selection
    /// keeps moving there (the list re-sorts on every frame) and a memo written
    /// then would leave the pane on the wrong job for good.
    fn on_screen(&mut self) -> bool {
        if self
            .hidden_until
            .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }
        if self.window_is_current() {
            self.hidden_until = None;
            return true;
        }
        self.hidden_until = Some(Instant::now() + HIDDEN_RECHECK);
        false
    }

    /// Whether anyone is looking at this sidebar's window.
    ///
    /// One sidebar per window is a supported layout, and a swap moves a real
    /// pane: a sidebar in a window nobody is watching must not reach across
    /// the server and rearrange panes out from under the one that is.
    fn window_is_current(&self) -> bool {
        tmux::display_message(&self.sidebar.id, "#{window_active}") == "1"
    }

    /// The pane the preview paints into, re-detected if it went away.
    ///
    /// Cached: after a swap the pane *beside* the sidebar is the foreign one,
    /// so re-detecting per call would mistake it for the slot and lose the
    /// way back.
    fn slot(&mut self) -> Option<String> {
        if let Some(slot) = self.preview_slot.clone() {
            if tmux::pane_exists(&slot) {
                return Some(slot);
            }
            self.preview_slot = None;
            self.swapped_with = None;
        }
        self.preview_slot = tmux::sibling_content_pane(&self.sidebar.window, &self.sidebar.id);
        self.preview_slot.clone()
    }

    /// Undo the outstanding swap, keeping [`Self::showing`] as it was.
    fn unswap(&mut self) {
        let Some(slot) = self.preview_slot.clone() else {
            self.swapped_with = None;
            return;
        };
        let Some(foreign) = self.swapped_with.take() else {
            return;
        };
        let _ = tmux::run_tmux(&["swap-pane", "-d", "-s", &foreign, "-t", &slot]);
        tmux::unset_pane_option(&foreign, tmux::PREVIEW_OWNER);
        tmux::unset_window_option(&self.sidebar.window, tmux::PREVIEW_SWAP);
    }
}

/// Put back a preview swap that outlived the sidebar that made it.
///
/// Reads the window's [`tmux::PREVIEW_SWAP`] record, so it works from any
/// process: the next sidebar to start, or `close` on its way to `kill-pane`.
/// Panes that no longer exist are dropped along with the record.
pub fn restore_stranded_swap(window: &str) {
    if window.is_empty() {
        return;
    }
    let Some(record) = tmux::window_option(window, tmux::PREVIEW_SWAP) else {
        return;
    };
    if let Some((foreign, slot)) = parse_swap_record(&record) {
        if tmux::pane_exists(foreign) && tmux::pane_exists(slot) {
            let _ = tmux::run_tmux(&["swap-pane", "-d", "-s", foreign, "-t", slot]);
        }
        // Whoever owned it is gone; leaving the claim would hide the pane from
        // every sidebar that comes after.
        tmux::unset_pane_option(foreign, tmux::PREVIEW_OWNER);
    }
    tmux::unset_window_option(window, tmux::PREVIEW_SWAP);
}

/// `"%33 %19"` → the pane that was swapped in, and the slot it landed in.
fn parse_swap_record(record: &str) -> Option<(&str, &str)> {
    let (foreign, slot) = record.trim().split_once(char::is_whitespace)?;
    let (foreign, slot) = (foreign.trim(), slot.trim());
    (!foreign.is_empty() && !slot.is_empty() && foreign != slot).then_some((foreign, slot))
}

fn switch_to_pane(pane: &str) -> bool {
    let _ = tmux::run_tmux(&["switch-client", "-t", pane]);
    let window = tmux::display_message(pane, "#{window_id}");
    if !window.is_empty() {
        let _ = tmux::run_tmux(&["select-window", "-t", &window]);
    }
    tmux::run_tmux(&["select-pane", "-t", pane]).is_some()
}

fn job_has_ide_link(job: &JobView) -> bool {
    job.location
        .as_ref()
        .and_then(|loc| loc.open_url.as_deref())
        .is_some_and(is_ide_deep_link)
}

fn open_url(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = url;
        false
    }
}

/// Open Cursor/VS Code deep link only — never `file://` folders.
fn focus_ide(job: &JobView) -> bool {
    let Some(url) = job
        .location
        .as_ref()
        .and_then(|loc| loc.open_url.as_deref())
        .filter(|u| is_ide_deep_link(u))
    else {
        return false;
    };
    open_url(url)
}

impl Default for PanePreview {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PanePreview {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Why this tmux has nothing to show for `job`.
///
/// A job on another machine is not a failed match — there is nothing here to
/// match — so it is told apart from a local job whose pane could not be found.
/// Naming the machine is the actionable half: ssh there (or open the sidebar
/// there, which is free — CLAUDE.md invariant 5) and the job is one pane away.
fn nothing_to_show(job: &JobView) -> String {
    if let Some(host) = machine::ssh_host(job) {
        return format!("{host} is another machine — no ssh pane here");
    }
    let path = job_path(job)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "?".into());
    format!("no local pane for {path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str) -> JobView {
        serde_json::from_value(serde_json::json!({ "id": id, "name": "work" }))
            .expect("job fixture")
    }

    /// The wart this fixes: `Enter` used to forget what it had shown, so the
    /// moment the human came back to the sidebar the tick re-previewed the same
    /// job — dragging the pane they had just walked into out of its own window.
    #[test]
    fn a_jump_leaves_the_preview_remembering_the_job_it_landed_on() {
        let mut preview = PanePreview::detached();
        let landed = job("codex:s1");
        assert!(preview.is_idle());

        preview.settle(&landed);
        assert!(
            preview.is_showing(&landed),
            "coming back to the sidebar must not re-swap the job's own pane"
        );

        // Moving the cursor is what starts previewing again.
        assert!(!preview.is_showing(&job("codex:s2")));
        preview.restore();
        assert!(preview.is_idle());
    }

    #[test]
    fn swap_records_round_trip() {
        assert_eq!(parse_swap_record("%33 %19"), Some(("%33", "%19")));
        assert_eq!(parse_swap_record("  %33   %19  "), Some(("%33", "%19")));
        assert_eq!(parse_swap_record("%33"), None);
        assert_eq!(parse_swap_record(""), None);
        // A record that swaps a pane with itself would be a no-op at best.
        assert_eq!(parse_swap_record("%33 %33"), None);
    }
}
