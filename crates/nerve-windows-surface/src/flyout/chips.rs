//! The small count labels in the panel header.

use egui::{RichText, Ui};
use nerve_surface_core::palette;
use nerve_surface_core::tally::Tally;

use super::theme::{color, secondary};
use crate::tray::icon::Theme;

/// Running first, then what needs a person. The second only when it is not
/// zero: a permanent "0 needs you" is a thing people learn to stop reading.
pub fn counts(ui: &mut Ui, tally: &Tally, theme: Theme) {
    let busy = tally.running + tally.monitor;
    metric(
        ui,
        busy,
        color(palette::RUNNING),
        &format!("{busy} running"),
    );

    let needs = tally.problem + tally.attention + tally.waiting;
    if needs > 0 {
        let hue = if tally.problem > 0 {
            palette::PROBLEM
        } else {
            palette::ATTENTION
        };
        metric(ui, needs, color(hue), &format!("{needs} need attention"));
    }
    let _ = secondary(theme);
}

fn metric(ui: &mut Ui, count: usize, ink: egui::Color32, help: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(14.0, 20.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, ink);
        response.on_hover_text(help);
        ui.label(
            RichText::new(count.to_string())
                .size(13.0)
                .strong()
                .color(ink),
        )
        .on_hover_text(help);
    });
}

/// Said plainly, because a stale list that does not admit it is a lie.
pub fn offline(ui: &mut Ui, theme: Theme) {
    ui.add_space(8.0);
    ui.label(
        RichText::new("offline")
            .color(secondary(theme))
            .small()
            .italics(),
    );
}
