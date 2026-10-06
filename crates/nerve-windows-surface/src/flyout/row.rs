//! One job, and what it looks like opened up.

use egui::{Align, Color32, Layout, Response, RichText, Sense, Ui, Vec2};
use nerve_surface_core::display::{activity_text, age_label};
use nerve_surface_core::frame::JobView;
use nerve_surface_core::palette;
use nerve_surface_core::status::StatusClass;
use time::OffsetDateTime;

use super::theme::{color, secondary};
use crate::tray::icon::Theme;

/// The status dot's diameter.
const DOT: f32 = 10.0;
/// How many timeline entries the detail block shows.
const RECENT: usize = 5;

/// What the user asked the surface to do about a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    /// Take me there.
    Open,
    /// Put it on the clipboard.
    Copy,
}

/// Draw one row; returns an action if a button in it was pressed.
///
/// `expanded` is owned by the caller so only one row can be open at a time —
/// a panel with three detail blocks unfurled is a scroll bar, not a glance.
pub fn show(
    ui: &mut Ui,
    job: &JobView,
    timeline: &[nerve_surface_core::frame::TimelineEntry],
    expanded: bool,
    theme: Theme,
    now: OffsetDateTime,
) -> (Response, Option<RowAction>) {
    let mut action = None;

    let response = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(6)
        .fill(if expanded {
            ui.visuals().faint_bg_color
        } else {
            Color32::TRANSPARENT
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.horizontal(|ui| {
                dot(ui, StatusClass::of(job));
                ui.add_space(6.0);
                ui.allocate_ui_with_layout(
                    Vec2::new((ui.available_width() - 55.0).max(0.0), 20.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(job.name.trim()).size(13.0)).truncate(),
                        )
                        .on_hover_text(job.name.trim());
                    },
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let age = age_label(now, job);
                    ui.label(
                        RichText::new(if age == "-" { "" } else { &age })
                            .color(secondary(theme))
                            .small(),
                    );
                });
            });
            let activity = activity_text(job).trim();
            if !activity.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(DOT + 6.0 + ui.spacing().item_spacing.x);
                    ui.add(
                        egui::Label::new(RichText::new(activity).color(secondary(theme)).small())
                            .truncate(),
                    )
                    .on_hover_text(activity);
                });
            }
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    let c = egui::pos2(response.rect.right() - 3.0, response.rect.top() + 16.0);
    let points = if expanded {
        [
            c + egui::vec2(-3.0, -1.5),
            c + egui::vec2(0.0, 1.5),
            c + egui::vec2(3.0, -1.5),
        ]
    } else {
        [
            c + egui::vec2(-1.5, -3.0),
            c + egui::vec2(1.5, 0.0),
            c + egui::vec2(-1.5, 3.0),
        ]
    };
    ui.painter().add(egui::Shape::line(
        points.to_vec(),
        egui::Stroke::new(1.0_f32, secondary(theme).gamma_multiply(0.65)),
    ));
    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect,
            6,
            egui::Stroke::new(0.5_f32, ui.visuals().window_stroke.color),
            egui::StrokeKind::Inside,
        );
    }

    if expanded {
        ui.indent(&job.id, |ui| {
            action = detail(ui, job, timeline, theme);
        });
    }

    (response, action)
}

fn dot(ui: &mut Ui, class: StatusClass) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(DOT), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), DOT / 2.0, color(palette::color_of(class)));
}

/// Prompt, context, actions, recent activity.
///
/// The prompt is the job's own `extensions.lastPrompt` and never falls back to
/// the activity summary: showing what the agent is doing in the place a person
/// looks for what they asked is worse than showing nothing.
fn detail(
    ui: &mut Ui,
    job: &JobView,
    timeline: &[nerve_surface_core::frame::TimelineEntry],
    theme: Theme,
) -> Option<RowAction> {
    let mut action = None;
    let muted = secondary(theme);

    if let Some(prompt) = job.extensions.last_prompt.as_deref().map(str::trim) {
        if !prompt.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Prompt").color(muted).small());
            ui.label(RichText::new(truncate(prompt, 240)).small());
        }
    }

    field(ui, "Machine", job.alias.trim(), muted);
    if let Some(workspace) = job.context.workspace.as_deref().map(str::trim) {
        if !workspace.is_empty() && workspace != job.name.trim() {
            field(ui, "Project", workspace, muted);
        }
    }

    if let Some(location) = &job.location {
        if let Some(value) = location
            .focus_hint
            .as_deref()
            .or(location.open_url.as_deref())
        {
            field(ui, "Location", value.trim(), muted);
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.small_button("Open").clicked() {
            action = Some(RowAction::Open);
        }
        if ui.small_button("Copy").clicked() {
            action = Some(RowAction::Copy);
        }
    });

    if !timeline.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Recent").color(muted).small());
        for entry in timeline.iter().rev().take(RECENT) {
            ui.label(
                RichText::new(format!("{}  {}", clock(entry), truncate(&entry.title, 40)))
                    .color(muted)
                    .small(),
            );
        }
    }
    ui.add_space(4.0);

    action
}

fn field(ui: &mut Ui, label: &str, value: &str, muted: Color32) {
    if value.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(muted).small());
        ui.add(egui::Label::new(RichText::new(value).small()).truncate())
            .on_hover_text(value);
    });
}

/// `HH:MM`, or nothing when the entry carries no time.
fn clock(entry: &nerve_surface_core::frame::TimelineEntry) -> String {
    entry
        .at
        .map(|at| {
            let time = at.instant();
            format!("{:02}:{:02}", time.hour(), time.minute())
        })
        .unwrap_or_default()
}

/// Cut on a character boundary, with an ellipsis when something was removed.
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}
