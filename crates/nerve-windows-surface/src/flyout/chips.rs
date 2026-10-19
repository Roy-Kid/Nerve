//! The small count labels in the panel header.

use egui::{RichText, Ui};
use nerve_surface_core::palette;
use nerve_surface_core::tally::Tally;

use super::theme::{color, secondary};
use crate::tray::icon::Theme;

/// Running first, then what needs a person. The second only when it is not
/// zero: a permanent "0 needs you" is a thing people learn to stop reading.
pub fn counts(ui: &mut Ui, tally: &Tally, theme: Theme) {
    let busy = tally.running_count();
    ui.label(
        RichText::new(format!("{busy} running"))
            .color(color(palette::RUNNING))
            .small(),
    );

    if tally.problem > 0 {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{} {}",
                tally.problem,
                if tally.problem == 1 {
                    "problem"
                } else {
                    "problems"
                }
            ))
            .color(color(palette::PROBLEM))
            .small(),
        );
    }

    let needs = tally.needs_you();
    if needs > 0 {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("{needs} needs you"))
                .color(color(palette::ATTENTION))
                .small(),
        );
    }
    let _ = secondary(theme);
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
