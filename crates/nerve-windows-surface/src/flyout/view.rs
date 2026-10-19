//! The panel: ribbon, counts, rows, and what to do when there are none.

use egui::{Align, Layout, RichText, ScrollArea, Ui};
use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::store::JobsSnapshot;
use nerve_surface_core::tally::Tally;
use time::OffsetDateTime;

use super::row::{self, RowAction};
use super::sections;
use super::theme::secondary;
use super::{chips, ribbon};
use crate::settings::GroupMode;
use crate::tray::icon::Theme;

/// What the panel is asking the surface to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelAction {
    /// Take me to this job.
    Open(String),
    /// Copy this job's line.
    Copy(String),
    /// Cycle the grouping.
    CycleGroup,
}

/// What the panel remembers between frames.
#[derive(Debug, Default)]
pub struct PanelState {
    /// At most one: three unfurled detail blocks is a scroll bar, not a glance.
    pub expanded: Option<String>,
}

/// Draw the whole panel. Returns whatever the user asked for.
pub fn show(
    ui: &mut Ui,
    snapshot: &JobsSnapshot,
    state: &mut PanelState,
    group_mode: GroupMode,
    hub_installed: bool,
    theme: Theme,
    now: OffsetDateTime,
) -> Option<PanelAction> {
    let mut action = None;
    let tally = snapshot.summary.tally();
    let sections = sections::of(&snapshot.jobs, group_mode, StatusFilter::default(), now);
    let painted = sections::painted_order(&snapshot.jobs, &sections);

    let runs = ribbon::runs_for(&painted);
    let strip = ribbon::show(ui, &runs, &tally, snapshot.offline, theme);
    if !runs.is_empty() {
        strip.on_hover_text(
            runs.iter()
                .map(|run| format!("{} {}", run.count, run.class.title()))
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }

    ui.add_space(8.0);
    if header(ui, &tally, snapshot.offline, group_mode, theme) {
        action = Some(PanelAction::CycleGroup);
    }
    ui.separator();

    if snapshot.jobs.is_empty() {
        empty(ui, snapshot.offline, hub_installed, theme);
        return action;
    }

    ScrollArea::vertical().show(ui, |ui| {
        for section in &sections {
            if !section.title.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(section.title.as_str())
                        .color(secondary(theme))
                        .small(),
                );
            }
            for index in &section.jobs {
                let Some(job) = snapshot.jobs.get(*index) else {
                    continue;
                };
                let expanded = state.expanded.as_deref() == Some(job.id.as_str());
                let timeline = job.timeline.as_slice();
                let (response, row_action) = row::show(ui, job, timeline, expanded, theme, now);

                if response.clicked() {
                    state.expanded = if expanded { None } else { Some(job.id.clone()) };
                }
                match row_action {
                    Some(RowAction::Open) => action = Some(PanelAction::Open(job.id.clone())),
                    Some(RowAction::Copy) => action = Some(PanelAction::Copy(job.id.clone())),
                    None => {}
                }
            }
        }
    });

    action
}

/// Counts on the left, grouping on the right. Returns whether it was clicked.
fn header(ui: &mut Ui, tally: &Tally, offline: bool, group_mode: GroupMode, theme: Theme) -> bool {
    let mut cycled = false;
    ui.horizontal(|ui| {
        chips::counts(ui, tally, theme);
        if offline {
            chips::offline(ui, theme);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .small_button(group_mode.label())
                .on_hover_text("Group by machine, priority or status")
                .clicked()
            {
                cycled = true;
            }
        });
    });
    cycled
}

/// Nothing to show, and why.
///
/// Two different nothings: a hub that is running and has no jobs wants the
/// ingest address, and a machine with no hub at all wants an install line.
pub fn empty_message(offline: bool, hub_installed: bool) -> (&'static str, &'static str) {
    match (offline, hub_installed) {
        (false, _) => (
            "Nothing running",
            "Start an agent with the Nerve plugin to see its progress here.",
        ),
        (true, true) => (
            "Connecting to Nerve",
            "Reconnecting automatically. Your jobs will appear when the connection returns.",
        ),
        (true, false) => (
            "Nerve hub is missing",
            "Install both Nerve applications, then restart Nerve.",
        ),
    }
}

fn empty(ui: &mut Ui, offline: bool, hub_installed: bool, theme: Theme) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        let (title, message) = empty_message(offline, hub_installed);
        ui.label(RichText::new(title).strong());
        ui.add_space(6.0);
        ui.label(RichText::new(message).color(secondary(theme)).small());
    });
}
