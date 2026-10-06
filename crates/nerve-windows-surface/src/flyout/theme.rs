//! The panel's colours, from the system's.

use egui::{Color32, Visuals};
use nerve_surface_core::palette::Rgb;

use crate::tray::icon::Theme;

/// egui's own colour type, from ours.
pub fn color(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Start from egui's own light or dark and adjust only what the product has an
/// opinion about.
///
/// Deliberately little: a status panel that invents its own chrome stops
/// looking like it belongs to the desktop it sits on, and the six status hues
/// are the only colours here that carry meaning.
pub fn visuals(theme: Theme) -> Visuals {
    let mut visuals = match theme {
        Theme::Light => Visuals::light(),
        Theme::Dark => Visuals::dark(),
    };
    // The flyout has no title bar, so its own background is the whole frame.
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.window_stroke = egui::Stroke::new(
        1.0_f32,
        match theme {
            Theme::Light => Color32::from_gray(0xD0),
            Theme::Dark => Color32::from_gray(0x3A),
        },
    );
    visuals.panel_fill = match theme {
        Theme::Light => Color32::from_rgb(247, 247, 249),
        Theme::Dark => Color32::from_rgb(30, 30, 33),
    };
    visuals.override_text_color = Some(match theme {
        Theme::Light => Color32::from_rgb(32, 32, 36),
        Theme::Dark => Color32::from_rgb(235, 235, 240),
    });
    visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals
}

/// Shared panel rhythm: 13pt body, 11pt metadata, 28pt chrome hit targets.
pub fn install(ctx: &egui::Context, theme: Theme) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals(theme);
    style.spacing.item_spacing = egui::vec2(8.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
    style.spacing.interact_size = egui::vec2(28.0, 28.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    ctx.set_style(style);
}

pub fn surface(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().panel_fill)
        .stroke(egui::Stroke::new(0.5_f32, ui.visuals().window_stroke.color))
        .corner_radius(12)
        .shadow(egui::epaint::Shadow {
            offset: [0, 2],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(35),
        })
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            content(ui);
        });
}

/// The muted colour for text that is context rather than content.
pub fn secondary(theme: Theme) -> Color32 {
    match theme {
        Theme::Light => Color32::from_gray(0x6A),
        Theme::Dark => Color32::from_gray(0x9A),
    }
}
