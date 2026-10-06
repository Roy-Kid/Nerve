//! Drawing the tray icon.
//!
//! Straight (un-premultiplied) RGBA, which is what `tray-icon` wants, at
//! whatever square size the current DPI asks for.
//!
//! Three states, deliberately distinguishable at 16 px without reading colour:
//! **work** is a stack of one to three full-height bars; **idle** is one short
//! gray bar, centred, so the icon is visibly resting rather than gone; and
//! **offline** is the last stack faded and struck through, so "nothing is
//! running" never looks like "I cannot see what is running".

use nerve_surface_core::palette::{self, Rgb};
use nerve_surface_core::ribbon::length_factor;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

use super::bands::Band;

/// One transparent pixel row between bars, so two colours never touch.
const GUTTER: u32 = 1;
/// The narrowest a bar may be, as a share of the space the stack is using —
/// a lone status is still a bar and not a speck.
const MIN_SHARE: f32 = 0.45;
/// The shortest the whole stack may be, as a share of the icon.
///
/// The count ladder scales the stack, and one job must still be clearly a bar
/// rather than a dot.
const MIN_SPAN: f32 = 0.55;
/// Corner radius, as a share of the icon.
///
/// Capped well below half the bar height on purpose: at `h/2` a single
/// full-height band stops being a bar and becomes a circle, which reads as a
/// dot and loses the whole stacked-bar metaphor.
const RADIUS: f32 = 1.0 / 5.0;
/// The idle mark's length, as a fixed share of the icon.
///
/// Unlike the working stack, whose length scales with how much is running, the
/// resting mark must not move: a constant length is what reads as "idle" rather
/// than "a small job". Kept at `MIN_SPAN` so it matches the shortest possible
/// working stack and never grows or shrinks with DPI rung.
const IDLE_WIDTH: f32 = MIN_SPAN;
/// How much of an offline stack survives.
const OFFLINE_ALPHA: f32 = 0.40;

/// Whether the taskbar this icon sits on is light or dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    /// Keep a colour legible against the taskbar without giving it a new hue.
    ///
    /// The same small lift the macOS ribbon applies (`RibbonPalette.nsColor`).
    fn lift(self, color: Rgb) -> Rgb {
        match self {
            Theme::Dark => color.blend(palette::WHITE, 0.04),
            Theme::Light => color.blend(palette::BLACK, 0.06),
        }
    }
}

/// Everything the renderer needs. No hub, no clock, no window.
#[derive(Clone, Debug, PartialEq)]
pub struct IconSpec {
    /// Edge length in pixels; the icon is always square.
    pub size: u32,
    /// Top to bottom. Empty draws the idle mark.
    pub bands: Vec<Band>,
    /// The hub is unreachable: fade what is left and strike it through.
    pub offline: bool,
    pub theme: Theme,
}

/// Straight RGBA, `4 * size * size` bytes, row-major from the top left.
pub fn render(spec: &IconSpec) -> Vec<u8> {
    let size = spec.size.max(1);
    let mut pixmap = Pixmap::new(size, size).expect("a non-zero square is a valid pixmap");

    if spec.bands.is_empty() {
        draw_idle(&mut pixmap, size, spec);
    } else {
        draw_bands(&mut pixmap, size, spec);
    }
    if spec.offline {
        draw_slash(&mut pixmap, size);
    }

    // tiny-skia stores premultiplied alpha; tray icons take straight.
    pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect()
}

fn alpha_for(spec: &IconSpec) -> f32 {
    if spec.offline {
        OFFLINE_ALPHA
    } else {
        1.0
    }
}

fn draw_bands(pixmap: &mut Pixmap, size: u32, spec: &IconSpec) {
    let count = spec.bands.len() as u32;
    let gutters = GUTTER * count.saturating_sub(1);
    let height = (size.saturating_sub(gutters)) / count.max(1);
    if height == 0 {
        // More bands than pixels: better one honest bar than three empty rows.
        return;
    }
    // Rounding leftovers go to the top band, which is the loudest one.
    let leftover = size.saturating_sub(height * count + gutters);

    // How long the whole stack runs. Without this a single status fills the
    // icon whatever its count, and "one job running" looks like "forty".
    let active: usize = spec.bands.iter().map(|band| band.count).sum();
    let span = MIN_SPAN + (1.0 - MIN_SPAN) * length_factor(active);

    let alpha = alpha_for(spec);
    let mut y = 0_u32;
    for (index, band) in spec.bands.iter().enumerate() {
        let bar_height = height + if index == 0 { leftover } else { 0 };
        let share = MIN_SHARE + (1.0 - MIN_SHARE) * band.weight.clamp(0.0, 1.0);
        let width = share * span * size as f32;
        fill_bar(
            pixmap,
            0.0,
            y as f32,
            width,
            bar_height as f32,
            size,
            spec.theme.lift(band.color),
            alpha,
        );
        y += bar_height + GUTTER;
    }
}

fn draw_idle(pixmap: &mut Pixmap, size: u32, spec: &IconSpec) {
    let height = (size as f32 / 4.0).max(3.0);
    let width = size as f32 * IDLE_WIDTH;
    let x = (size as f32 - width) / 2.0;
    let y = (size as f32 - height) / 2.0;
    fill_bar(
        pixmap,
        x,
        y,
        width,
        height,
        size,
        // Idle must remain an obvious click target on both taskbar themes.
        // A translucent mid-gray dash disappears at notification-area sizes.
        match spec.theme {
            Theme::Light => Rgb::new(65, 65, 65),
            Theme::Dark => Rgb::new(225, 225, 225),
        },
        alpha_for(spec),
    );
}

/// Bottom-left to top-right, the shape every OS uses for "no signal".
fn draw_slash(pixmap: &mut Pixmap, size: u32) {
    let edge = size as f32;
    let thickness = (edge / 16.0).max(1.0);
    let mut builder = PathBuilder::new();
    builder.move_to(0.0, edge);
    builder.line_to(edge, 0.0);
    let Some(path) = builder.finish() else {
        return;
    };
    let mut paint = Paint::default();
    let color = palette::INACTIVE;
    paint.set_color_rgba8(color.r, color.g, color.b, 0xFF);
    paint.anti_alias = true;
    let stroke = tiny_skia::Stroke {
        width: thickness,
        ..tiny_skia::Stroke::default()
    };
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

#[allow(clippy::too_many_arguments)]
fn fill_bar(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    size: u32,
    color: Rgb,
    alpha: f32,
) {
    let radius = (size as f32 * RADIUS).min(height / 2.0).min(width / 2.0);
    let Some(rect) = Rect::from_xywh(x, y, width.max(1.0), height.max(1.0)) else {
        return;
    };
    let mut builder = PathBuilder::new();
    push_round_rect(&mut builder, rect, radius);
    let Some(path) = builder.finish() else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(
        color.r,
        color.g,
        color.b,
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    );
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// A rectangle with both ends rounded, drawn as four arcs and two lines.
fn push_round_rect(builder: &mut PathBuilder, rect: Rect, radius: f32) {
    let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let r = radius
        .max(0.0)
        .min((right - left) / 2.0)
        .min((bottom - top) / 2.0);
    if r <= 0.0 {
        builder.push_rect(rect);
        return;
    }
    // Circular arcs as cubics: 0.5523 is the standard control-point ratio.
    let k = r * 0.552_285;
    builder.move_to(left + r, top);
    builder.line_to(right - r, top);
    builder.cubic_to(right - r + k, top, right, top + r - k, right, top + r);
    builder.line_to(right, bottom - r);
    builder.cubic_to(
        right,
        bottom - r + k,
        right - r + k,
        bottom,
        right - r,
        bottom,
    );
    builder.line_to(left + r, bottom);
    builder.cubic_to(left + r - k, bottom, left, bottom - r + k, left, bottom - r);
    builder.line_to(left, top + r);
    builder.cubic_to(left, top + r - k, left + r - k, top, left + r, top);
    builder.close();
}
