//! The panel: ribbon, counts, rows, and what to do when there are none.

use egui::{Align, Layout, RichText, ScrollArea, Ui};
use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::store::JobsSnapshot;
use nerve_surface_core::tally::Tally;
use time::OffsetDateTime;

use super::row::{self, RowAction};
use super::sections;
use super::theme::secondary;
use super::{chips, chrome};
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
    /// Flip "start at login".
    ToggleAutostart,
    /// Flip "Ask notifications".
    ToggleToasts,
    /// Flip "notification sound".
    ToggleToastSound,
    Quit,
}

/// What the panel remembers between frames.
#[derive(Debug, Default)]
pub struct PanelState {
    /// At most one: three unfurled detail blocks is a scroll bar, not a glance.
    pub expanded: Option<String>,
    /// Keep keyboard selection attached to a job when live frames reorder rows.
    pub selected: Option<String>,
    /// Whether the Settings section is unfurled.
    pub settings_open: bool,
}

/// The toggles the Settings section shows, so the view stays dumb.
#[derive(Clone, Copy, Debug, Default)]
pub struct SettingsView {
    pub autostart: bool,
    pub toasts_enabled: bool,
    pub toast_sound: bool,
}

/// Draw the whole panel. Returns whatever the user asked for.
pub struct Presentation {
    pub group_mode: GroupMode,
    pub settings: SettingsView,
    pub hub_installed: bool,
    pub theme: Theme,
    pub now: OffsetDateTime,
}

pub fn show(
    ui: &mut Ui,
    snapshot: &JobsSnapshot,
    state: &mut PanelState,
    presentation: Presentation,
) -> Option<PanelAction> {
    let Presentation {
        group_mode,
        settings,
        hub_installed,
        theme,
        now,
    } = presentation;
    let mut action = None;
    resize_grip(ui);
    let tally = Tally::of(&snapshot.jobs);
    let sections = sections::of(&snapshot.jobs, group_mode, StatusFilter::default(), now);
    ui.add_space(2.0);
    if let Some(header_action) = header(ui, state, &tally, snapshot.offline, group_mode, theme) {
        action = Some(header_action);
    }
    ui.add_space(4.0);
    chrome::divider(ui);
    if state.settings_open {
        ui.add_space(8.0);
        return settings_section(ui, settings).or(action);
    }
    let order: Vec<&str> = sections
        .iter()
        .flat_map(|section| &section.jobs)
        .filter_map(|index| snapshot.jobs.get(*index))
        .map(|job| job.id.as_str())
        .collect();
    if state
        .selected
        .as_deref()
        .is_some_and(|id| !order.contains(&id))
    {
        state.selected = None;
    }
    if state
        .expanded
        .as_deref()
        .is_some_and(|id| !order.contains(&id))
    {
        state.expanded = None;
    }
    let mut scroll_selected = false;
    // Leave Tab-focused controls their Enter/Space activation. Plain arrows
    // otherwise operate on the displayed order, independent of wire order.
    if !ui.ctx().wants_keyboard_input() && !order.is_empty() {
        let (up, down, toggle) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    | input.consume_key(egui::Modifiers::NONE, egui::Key::Space),
            )
        });
        if up || down {
            let current = state
                .selected
                .as_deref()
                .and_then(|id| order.iter().position(|item| *item == id));
            let next = match current {
                None => 0,
                Some(index) if down => (index + 1).min(order.len() - 1),
                Some(index) => index.saturating_sub(1),
            };
            let id = order[next].to_string();
            state.selected = Some(id.clone());
            state.expanded = Some(id);
            scroll_selected = true;
        }
        if toggle {
            let id = state
                .selected
                .clone()
                .unwrap_or_else(|| order[0].to_string());
            state.expanded = if state.expanded.as_ref() == Some(&id) {
                None
            } else {
                Some(id.clone())
            };
            state.selected = Some(id);
            scroll_selected = true;
        }
    }
    ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .max_height((ui.available_height() - 18.0).max(0.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if snapshot.jobs.is_empty() {
                empty(ui, snapshot.offline, hub_installed, theme);
            }
            for section in &sections {
                if !section.title.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(section.title.to_uppercase())
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
                    let (response, row_action) = ui
                        .push_id(&job.id, |ui| {
                            row::show(ui, job, timeline, expanded, theme, now)
                        })
                        .inner;

                    if response.clicked() {
                        state.selected = Some(job.id.clone());
                        state.expanded = if expanded { None } else { Some(job.id.clone()) };
                    }
                    if state.selected.as_deref() == Some(job.id.as_str()) {
                        ui.painter().rect_stroke(
                            response.rect,
                            6,
                            egui::Stroke::new(1.0_f32, ui.visuals().selection.stroke.color),
                            egui::StrokeKind::Inside,
                        );
                        if scroll_selected {
                            response.scroll_to_me(Some(Align::Center));
                        }
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

/// Borderless panels still need a discoverable resize target. Let Windows
/// perform the drag so the top-left stays fixed and viewport size limits apply.
fn resize_grip(ui: &mut Ui) {
    let rect = egui::Rect::from_min_max(
        ui.max_rect().max - egui::vec2(20.0, 20.0),
        ui.max_rect().max,
    );
    let response = ui
        .interact(rect, egui::Id::new("panel-resize"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeNwSe)
        .on_hover_text("Resize panel");
    for inset in [4.0, 8.0, 12.0] {
        ui.painter().line_segment(
            [
                rect.right_bottom() - egui::vec2(inset, 2.0),
                rect.right_bottom() - egui::vec2(2.0, inset),
            ],
            egui::Stroke::new(1.0_f32, ui.visuals().weak_text_color()),
        );
    }
    if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::BeginResize(
                egui::ResizeDirection::SouthEast,
            ));
    }
}

fn settings_section(ui: &mut Ui, settings: SettingsView) -> Option<PanelAction> {
    let mut action = None;
    for (label, on, requested) in [
        (
            "Start at login",
            settings.autostart,
            PanelAction::ToggleAutostart,
        ),
        (
            "Ask notifications",
            settings.toasts_enabled,
            PanelAction::ToggleToasts,
        ),
        (
            "Notification sound",
            settings.toast_sound,
            PanelAction::ToggleToastSound,
        ),
    ] {
        if chrome::switch(ui, label, on) {
            action = Some(requested);
        }
    }
    ui.add_space(10.0);
    ui.label(
        RichText::new("Notifications appear when an agent needs you.")
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );
    action
}

fn header(
    ui: &mut Ui,
    state: &mut PanelState,
    tally: &Tally,
    offline: bool,
    group_mode: GroupMode,
    theme: Theme,
) -> Option<PanelAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        if state.settings_open {
            ui.label(RichText::new("Settings").size(13.0).strong());
        } else {
            chips::counts(ui, tally, theme);
            if offline {
                chips::offline(ui, theme);
            }
        }
        // A borderless viewport has no native caption to grab. Keep a drag
        // region between the metrics and buttons, without stealing controls.
        let controls_width = if state.settings_open { 58.0 } else { 88.0 };
        let width = (ui.available_width() - controls_width - ui.spacing().item_spacing.x).max(0.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 28.0), egui::Sense::hover());
        let drag = ui
            .interact(rect, egui::Id::new("panel-drag"), egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text("Drag to move panel");
        if drag.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if chrome::button(ui, "panel-quit", chrome::Icon::Power, false, "Quit Nerve").clicked()
            {
                action = Some(PanelAction::Quit);
            }
            if chrome::button(
                ui,
                "panel-settings",
                chrome::Icon::Settings,
                state.settings_open,
                "Settings",
            )
            .clicked()
            {
                state.settings_open = !state.settings_open;
            }
            if !state.settings_open
                && chrome::button(
                    ui,
                    "panel-group",
                    chrome::Icon::Group,
                    false,
                    &format!("Group by {}", group_mode.label()),
                )
                .clicked()
            {
                action = Some(PanelAction::CycleGroup);
            }
        });
    });
    action
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
    ui.add_space((ui.available_height() * 0.30).max(16.0));
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(42.0, 32.0), egui::Sense::hover());
        let ink = secondary(theme).gamma_multiply(0.4);
        for (offset, height) in [(-8.0, 10.0), (0.0, 22.0), (8.0, 15.0)] {
            ui.painter().rect_filled(
                egui::Rect::from_center_size(
                    rect.center() + egui::vec2(offset, 0.0),
                    egui::vec2(4.0, height),
                ),
                2,
                ink,
            );
        }
        ui.add_space(12.0);
        let (title, message) = empty_message(offline, hub_installed);
        ui.label(RichText::new(title).strong());
        ui.add_space(6.0);
        ui.label(RichText::new(message).color(secondary(theme)).small());
    });
}
