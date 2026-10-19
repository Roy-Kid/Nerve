//! The tray icon's current appearance, derived from a snapshot.
//!
//! Pure: a snapshot and the display's state in, the icon and its tooltip out.
//! The shell call that follows is the only part that needs Windows, which is
//! what lets the whole of "what the tray says" be tested anywhere.

use nerve_surface_core::sort::compare_jobs;
use nerve_surface_core::store::JobsSnapshot;
use time::OffsetDateTime;

use super::bands::stack;
use super::icon::{IconSpec, Theme};
use super::signature::{self, Signature};
use super::tooltip::{self, Offline};

/// Everything the shell needs to be told, plus the gate on telling it.
pub struct View {
    pub icon: IconSpec,
    pub tooltip: String,
    pub signature: Signature,
}

/// Build the view for `snapshot`.
///
/// `hub_installed` separates "the hub is not running" from "there is no hub to
/// run", because the two want different words: one is a state that will fix
/// itself, the other is an installation the user has not finished.
pub fn of(
    snapshot: &JobsSnapshot,
    hub_installed: bool,
    theme: Theme,
    size: u32,
    now: OffsetDateTime,
) -> View {
    let tally = snapshot.summary.tally();
    let bands = stack(&tally);

    let offline = match (snapshot.offline, hub_installed) {
        (false, _) => Offline::No,
        (true, true) => Offline::HubDown,
        (true, false) => Offline::NotInstalled,
    };

    // The most urgent row is the one worth naming in a two-line tooltip.
    let top = snapshot.jobs.iter().min_by(|a, b| compare_jobs(now, a, b));

    View {
        signature: signature::of(&bands, snapshot.offline, theme, size),
        tooltip: snapshot
            .error
            .clone()
            .unwrap_or_else(|| tooltip::render(&tally, top, offline)),
        icon: IconSpec {
            size,
            bands,
            offline: snapshot.offline,
            theme,
        },
    }
}
