//! The hover text behind the tray icon.
//!
//! `Shell_NotifyIcon` caps `szTip` at **128 UTF-16 code units including the
//! terminating NUL**, so the budget is 127 — counted in code units, not
//! characters and not bytes. A job named `项目` costs 2 units per character and
//! an emoji costs 2 for one character, so anything that counts `chars()` or
//! `len()` will silently hand Windows a string it truncates itself, mid-glyph.
//!
//! Two lines: what the machine is doing, and the one job most worth returning
//! to. The second line is dropped before the first is ever shortened.

use nerve_surface_core::display::activity_text;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::tally::Tally;

/// Usable code units, after reserving one for the NUL.
pub const MAX_UTF16_UNITS: usize = 127;

/// Why the surface has nothing live to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offline {
    /// The hub is running and answering; this is the truth.
    No,
    /// Nothing answered on the port, but a hub binary exists to start.
    HubDown,
    /// No hub binary anywhere — the user has not finished installing.
    NotInstalled,
}

/// Build the tooltip, guaranteed to fit.
pub fn render(tally: &Tally, top: Option<&JobView>, offline: Offline) -> String {
    let headline = match offline {
        Offline::NotInstalled => "Nerve — nerve-hub not installed".to_string(),
        Offline::HubDown => "Nerve — offline (hub not running)".to_string(),
        Offline::No => format!("Nerve — {}", counts(tally)),
    };

    let detail = match offline {
        Offline::NotInstalled => None,
        // Say what was last true rather than nothing: an offline surface that
        // forgets is worse than one that admits it is looking at a stale list.
        Offline::HubDown if !tally.is_empty() => Some(format!("last seen {}", counts(tally))),
        Offline::HubDown => None,
        Offline::No => top.map(job_line),
    };

    assemble(headline, detail)
}

/// `4 running, 1 needs you` — activity first, then the ask.
fn counts(tally: &Tally) -> String {
    if tally.is_empty() {
        return "nothing running".to_string();
    }
    let mut parts = Vec::new();
    let busy = tally.running_count();
    if busy > 0 {
        parts.push(format!("{busy} running"));
    }
    if tally.problem > 0 {
        let word = if tally.problem == 1 {
            "problem"
        } else {
            "problems"
        };
        parts.push(format!("{} {word}", tally.problem));
    }
    let needs = tally.needs_you();
    if needs > 0 {
        parts.push(format!("{needs} needs you"));
    }
    if parts.is_empty() {
        // Only finished or inactive rows left; still say how many.
        parts.push(format!("{} idle", tally.total()));
    }
    parts.join(", ")
}

fn job_line(job: &JobView) -> String {
    let activity = activity_text(job).trim();
    if activity.is_empty() {
        format!("▸ {}", job.name.trim())
    } else {
        format!("▸ {} · {activity}", job.name.trim())
    }
}

/// Fit both lines into the budget, dropping and then trimming the second.
fn assemble(headline: String, detail: Option<String>) -> String {
    let headline = truncate_utf16(&headline, MAX_UTF16_UNITS);
    let Some(detail) = detail else {
        return headline;
    };
    // One unit for the newline.
    let remaining = MAX_UTF16_UNITS.saturating_sub(utf16_len(&headline) + 1);
    // Below this there is no room for anything worth reading.
    if remaining < 8 {
        return headline;
    }
    format!("{headline}\n{}", truncate_utf16(&detail, remaining))
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Cut to `limit` UTF-16 units, never through a character.
///
/// An ellipsis costs one unit and is only added when something was actually
/// removed.
fn truncate_utf16(text: &str, limit: usize) -> String {
    if utf16_len(text) <= limit {
        return text.to_string();
    }
    if limit == 0 {
        return String::new();
    }
    let budget = limit - 1;
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let cost = ch.len_utf16();
        if used + cost > budget {
            break;
        }
        out.push(ch);
        used += cost;
    }
    out.push('…');
    out
}
