//! Where the flyout opens.
//!
//! Everything needed is in the click event: `tray-icon` hands over the icon's
//! screen rectangle, and winit knows the monitor. So the taskbar edge is
//! *inferred* from where the icon sits rather than asked for with
//! `SHAppBarMessage` — which is also how this stays a pure function, and how a
//! four-edge taskbar with a second monitor becomes a test rather than a bug
//! report.

/// A rectangle in physical screen pixels. Coordinates may be negative: a
/// monitor placed left of or above the primary one starts below zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub const fn right(&self) -> i32 {
        self.x + self.width
    }
    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }
    fn center_x(&self) -> i32 {
        self.x + self.width / 2
    }
    fn center_y(&self) -> i32 {
        self.y + self.height / 2
    }
}

/// Which screen edge the taskbar is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

/// Gap between the flyout and the taskbar.
const MARGIN: i32 = 12;
/// Gap the flyout keeps from the screen edges.
const INSET: i32 = 8;

/// Everything the placement depends on.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    /// The tray icon's rectangle, from the click event.
    pub tray: Rect,
    /// The monitor the icon is on.
    pub monitor: Rect,
    /// How big the flyout wants to be.
    pub size: (i32, i32),
}

/// Infer the taskbar edge from where the icon sits in its monitor.
///
/// Whichever edge the icon is nearest to is the one the taskbar is on: a tray
/// icon is always inside the taskbar.
pub fn edge_of(tray: Rect, monitor: Rect) -> Edge {
    let from_bottom = monitor.bottom() - tray.center_y();
    let from_top = tray.center_y() - monitor.y;
    let from_right = monitor.right() - tray.center_x();
    let from_left = tray.center_x() - monitor.x;

    let mut best = (from_bottom, Edge::Bottom);
    // Ties resolve towards the bottom, which is where Windows puts it unless
    // told otherwise.
    for candidate in [
        (from_top, Edge::Top),
        (from_right, Edge::Right),
        (from_left, Edge::Left),
    ] {
        if candidate.0 < best.0 {
            best = candidate;
        }
    }
    best.1
}

/// Top-left corner for the flyout, in physical screen pixels.
///
/// The near edge is aligned to the icon's near edge — right-aligned for the
/// usual bottom-right taskbar — and then both axes are clamped into the
/// monitor so a tall panel or an icon near a corner cannot push it off screen.
pub fn place(anchor: &Anchor) -> (i32, i32) {
    let (width, height) = anchor.size;
    let tray = anchor.tray;
    let monitor = anchor.monitor;

    let (x, y) = match edge_of(tray, monitor) {
        Edge::Bottom => (tray.right() - width, tray.y - MARGIN - height),
        Edge::Top => (tray.right() - width, tray.bottom() + MARGIN),
        Edge::Right => (tray.x - MARGIN - width, tray.bottom() - height),
        Edge::Left => (tray.right() + MARGIN, tray.bottom() - height),
    };

    (
        clamp(x, monitor.x + INSET, monitor.right() - INSET - width),
        clamp(y, monitor.y + INSET, monitor.bottom() - INSET - height),
    )
}

/// Bridge physical tray coordinates and egui's logical viewport commands.
pub fn place_scaled(tray: Rect, monitor: Rect, size: (f32, f32), scale: f32) -> (f32, f32) {
    place_for_viewport(tray, monitor, size, scale, scale)
}

/// Size belongs to the destination monitor, but egui converts position using
/// the current viewport's scale, which can differ before a monitor move.
pub fn place_for_viewport(
    tray: Rect,
    monitor: Rect,
    size: (f32, f32),
    scale: f32,
    viewport_scale: f32,
) -> (f32, f32) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let (x, y) = place(&Anchor {
        tray,
        monitor,
        size: (
            (size.0 * scale).round() as i32,
            (size.1 * scale).round() as i32,
        ),
    });
    let viewport_scale = if viewport_scale.is_finite() && viewport_scale > 0.0 {
        viewport_scale
    } else {
        1.0
    };
    (x as f32 / viewport_scale, y as f32 / viewport_scale)
}

/// Clamp that survives a flyout larger than the monitor.
///
/// `i32::clamp` panics when `min > max`, which is exactly the case a panel
/// taller than the screen produces — and a panic is not an acceptable answer
/// to a small display.
fn clamp(value: i32, min: i32, max: i32) -> i32 {
    if min >= max {
        return min;
    }
    value.clamp(min, max)
}
