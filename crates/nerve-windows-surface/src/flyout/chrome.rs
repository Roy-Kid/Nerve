//! Small vector controls with the same 14pt glyph / 28pt target as macOS.
use egui::{pos2, vec2, Color32, Response, Sense, Stroke, Ui};

#[derive(Clone, Copy)]
pub enum Icon {
    Group,
    Settings,
    Power,
}

pub fn button(ui: &mut Ui, id: &'static str, icon: Icon, selected: bool, label: &str) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
    let response = ui.interact(rect, egui::Id::new(id), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let painter = ui.painter();
    if response.hovered() || selected {
        painter.rect_filled(rect, 6, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let c = rect.center();
    let ink = if response.hovered() || selected {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let stroke = Stroke::new(1.35_f32, ink);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([c + vec2(a.0, a.1), c + vec2(b.0, b.1)], stroke);
    };
    match icon {
        Icon::Group => {
            line((-3.0, 6.0), (-3.0, -6.0));
            line((-6.0, -3.0), (-3.0, -6.0));
            line((-3.0, -6.0), (0.0, -3.0));
            line((3.0, -6.0), (3.0, 6.0));
            line((0.0, 3.0), (3.0, 6.0));
            line((3.0, 6.0), (6.0, 3.0));
        }
        Icon::Settings => {
            painter.circle_stroke(c, 4.7, stroke);
            painter.circle_stroke(c, 1.8, stroke);
            for n in 0..8 {
                let angle = n as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(angle.cos(), angle.sin());
                painter.line_segment([c + d * 4.7, c + d * 6.5], stroke);
            }
        }
        Icon::Power => {
            let points = (0..25)
                .map(|n| {
                    let a = -std::f32::consts::FRAC_PI_2
                        + 0.65
                        + n as f32 / 24.0 * (std::f32::consts::TAU - 1.3);
                    c + vec2(a.cos(), a.sin()) * 6.0
                })
                .collect();
            painter.add(egui::Shape::line(points, stroke));
            line((0.0, -7.0), (0.0, 0.0));
        }
    }
    response
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(0.5_f32, ui.visuals().window_stroke.color),
    );
}

pub fn switch(ui: &mut Ui, label: &str, on: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label));
    ui.painter().text(
        pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    let track =
        egui::Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(30.0, 18.0));
    ui.painter().rect_filled(
        track,
        9,
        if on {
            Color32::from_rgb(52, 120, 246)
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        },
    );
    ui.painter().circle_filled(
        pos2(
            if on {
                track.right() - 9.0
            } else {
                track.left() + 9.0
            },
            track.center().y,
        ),
        7.0,
        Color32::WHITE,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
