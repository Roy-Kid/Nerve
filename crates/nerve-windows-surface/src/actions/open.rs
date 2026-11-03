//! Deciding where "Open" goes, before anything is opened.
//!
//! A pure function, so the whole decision table is a test rather than a thing
//! discovered by clicking. The two rules worth stating out loud:
//!
//! - **A path belonging to another machine is never handed to the local
//!   shell.** `C:\work\nerve` on a colleague's laptop is very likely also a
//!   directory here, and opening the wrong one silently is worse than opening
//!   nothing. An IDE deep link is the exception because it routes itself.
//! - **The URL a producer emitted is never rewritten.** `vscode://file/C:/…`
//!   is the hook's contract; a surface that "fixes" it is guessing.

use nerve_platform::path;
use nerve_surface_core::display::activity_text;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::jobpath::{
    is_ide_deep_link, path_from_focus_hint, workspace_path_from_url,
};
use nerve_surface_core::machine::ssh_host;

/// What the surface should do when the human asks to be taken to a job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenPlan {
    /// Hand this to the shell as-is — an IDE deep link, or a URL whose app
    /// resolves it. Never a `file://` URL for another machine.
    Shell(String),
    /// Show this directory in Explorer.
    Reveal(String),
    /// Nothing local can get there; put it on the clipboard and say why.
    Copy { text: String, reason: &'static str },
    /// The job named no location at all.
    None(&'static str),
}

/// Decide, given the job. Which machine it is on is the hub's word
/// ([`ssh_host`]), so every row of the table below is a unit test and not a
/// hostname.
pub fn plan(job: &JobView) -> OpenPlan {
    let open_url = job
        .location
        .as_ref()
        .and_then(|location| location.open_url.as_deref())
        .map(str::trim)
        .filter(|url| !url.is_empty());
    let hint = job
        .location
        .as_ref()
        .and_then(|location| location.focus_hint.as_deref())
        .map(str::trim)
        .filter(|hint| !hint.is_empty());

    if ssh_host(job).is_some() {
        // An IDE deep link carries its own remote authority, so the editor
        // decides where it lands. Anything else is a path here that means
        // something there.
        if let Some(url) = open_url.filter(|url| is_ide_deep_link(url)) {
            return OpenPlan::Shell(url.to_string());
        }
        return match hint.or(open_url) {
            Some(text) => OpenPlan::Copy {
                text: text.to_string(),
                reason: "that job is on another machine",
            },
            None => OpenPlan::None("no location, and the job is on another machine"),
        };
    }

    if let Some(url) = open_url {
        if is_ide_deep_link(url) {
            return OpenPlan::Shell(url.to_string());
        }
        if let Some(local) = workspace_path_from_url(url) {
            return OpenPlan::Reveal(local.to_string_lossy().into_owned());
        }
        // A scheme we do not model — let the shell try rather than refuse.
        if !url.starts_with("file://") {
            return OpenPlan::Shell(url.to_string());
        }
    }

    if let Some(workspace) = job
        .context
        .workspace
        .as_deref()
        .map(str::trim)
        .filter(|workspace| path::is_absolute(workspace))
    {
        return OpenPlan::Reveal(workspace.to_string());
    }

    if let Some(hint) = hint {
        if let Some(from_hint) = path_from_focus_hint(Some(hint)) {
            return OpenPlan::Reveal(from_hint.to_string_lossy().into_owned());
        }
        return OpenPlan::Copy {
            text: hint.to_string(),
            reason: "the job named no path to open",
        };
    }

    OpenPlan::None("no open target")
}

/// `"{name} — {summary}"`, the one thing Copy puts on the clipboard.
pub fn copy_text(job: &JobView) -> String {
    let summary = activity_text(job).trim();
    let name = job.name.trim();
    if summary.is_empty() {
        name.to_string()
    } else {
        format!("{name} — {summary}")
    }
}
