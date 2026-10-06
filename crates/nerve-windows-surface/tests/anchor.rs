//! Where the flyout opens, for every taskbar a person might have.

use nerve_windows_surface::flyout::anchor::{edge_of, place, Anchor, Edge, Rect};

/// A 1920×1080 primary monitor.
const SCREEN: Rect = Rect::new(0, 0, 1920, 1080);
const SIZE: (i32, i32) = (380, 460);

/// The usual place: bottom-right, in a 48 px taskbar.
fn bottom_right_icon() -> Rect {
    Rect::new(1800, 1040, 24, 24)
}

fn anchor(tray: Rect, monitor: Rect) -> Anchor {
    Anchor {
        tray,
        monitor,
        size: SIZE,
    }
}

// ── Which edge ──────────────────────────────────────────────────────────────

#[test]
fn an_icon_near_the_bottom_means_a_bottom_taskbar() {
    assert_eq!(edge_of(bottom_right_icon(), SCREEN), Edge::Bottom);
}

#[test]
fn an_icon_near_the_top_means_a_top_taskbar() {
    assert_eq!(edge_of(Rect::new(1800, 12, 24, 24), SCREEN), Edge::Top);
}

#[test]
fn an_icon_near_the_right_means_a_right_taskbar() {
    assert_eq!(edge_of(Rect::new(1890, 540, 24, 24), SCREEN), Edge::Right);
}

#[test]
fn an_icon_near_the_left_means_a_left_taskbar() {
    assert_eq!(edge_of(Rect::new(6, 540, 24, 24), SCREEN), Edge::Left);
}

// ── Where it lands ──────────────────────────────────────────────────────────

#[test]
fn a_bottom_taskbar_opens_the_flyout_above_it() {
    let (x, y) = place(&anchor(bottom_right_icon(), SCREEN));
    assert!(y + SIZE.1 < 1040, "flyout overlapped the taskbar at y={y}");
    // Right edge aligned to the icon's right edge.
    assert_eq!(x + SIZE.0, 1824);
}

#[test]
fn a_top_taskbar_opens_the_flyout_below_it() {
    let tray = Rect::new(1800, 12, 24, 24);
    let (_, y) = place(&anchor(tray, SCREEN));
    assert!(y > tray.bottom(), "flyout overlapped the taskbar at y={y}");
}

#[test]
fn a_left_taskbar_opens_the_flyout_beside_it() {
    let tray = Rect::new(6, 540, 24, 24);
    let (x, _) = place(&anchor(tray, SCREEN));
    assert!(x > tray.right(), "flyout overlapped the taskbar at x={x}");
}

#[test]
fn a_right_taskbar_opens_the_flyout_beside_it() {
    let tray = Rect::new(1890, 540, 24, 24);
    let (x, _) = place(&anchor(tray, SCREEN));
    assert!(
        x + SIZE.0 < tray.x,
        "flyout overlapped the taskbar at x={x}"
    );
}

// ── Staying on screen ───────────────────────────────────────────────────────

#[test]
fn the_flyout_never_leaves_the_monitor() {
    for tray in [
        bottom_right_icon(),
        Rect::new(0, 1040, 24, 24),    // bottom-left corner
        Rect::new(1896, 1040, 24, 24), // hard against the corner
        Rect::new(1800, 0, 24, 24),    // top-right
        Rect::new(0, 0, 24, 24),       // top-left
    ] {
        let (x, y) = place(&anchor(tray, SCREEN));
        assert!(x >= SCREEN.x, "x={x} off the left for {tray:?}");
        assert!(y >= SCREEN.y, "y={y} off the top for {tray:?}");
        assert!(x + SIZE.0 <= SCREEN.right(), "off the right for {tray:?}");
        assert!(y + SIZE.1 <= SCREEN.bottom(), "off the bottom for {tray:?}");
    }
}

/// A monitor placed left of or above the primary one has negative
/// coordinates, which is where naive clamping puts the flyout on the wrong
/// screen.
#[test]
fn a_secondary_monitor_at_negative_coordinates_works() {
    let secondary = Rect::new(-1920, -200, 1920, 1080);
    let tray = Rect::new(-200, 800, 24, 24);
    let (x, y) = place(&anchor(tray, secondary));
    assert!(x >= secondary.x && x + SIZE.0 <= secondary.right(), "x={x}");
    assert!(
        y >= secondary.y && y + SIZE.1 <= secondary.bottom(),
        "y={y}"
    );
}

/// A panel resized past the screen height. `i32::clamp` panics when its own
/// bounds cross, which is exactly what this produces — the answer has to be a
/// position, not a crash.
#[test]
fn a_flyout_taller_than_the_screen_still_gets_a_position() {
    let small = Rect::new(0, 0, 1024, 600);
    let tray = Rect::new(980, 570, 24, 24);
    let (x, y) = place(&Anchor {
        tray,
        monitor: small,
        size: (380, 900),
    });

    // The axis that still fits keeps its alignment to the icon.
    assert_eq!(x + 380, tray.right());
    // The one that cannot falls back to the top of the screen rather than
    // hanging off the bottom, so the rows nearest the taskbar stay reachable.
    assert_eq!(y, small.y + 8);
}

#[test]
fn a_hidpi_monitor_is_just_a_bigger_rectangle() {
    let retina = Rect::new(0, 0, 3840, 2160);
    let (x, y) = place(&anchor(Rect::new(3700, 2100, 32, 32), retina));
    assert!(x + SIZE.0 <= retina.right());
    assert!(y + SIZE.1 <= retina.bottom());
}

#[test]
fn viewport_commands_convert_physical_pixels_to_logical_points() {
    use nerve_windows_surface::flyout::anchor::place_scaled;
    for scale in [1.0, 1.5, 2.0] {
        let screen = Rect::new(0, 0, (1920.0 * scale) as i32, (1080.0 * scale) as i32);
        let tray = Rect::new(
            (1800.0 * scale) as i32,
            (1040.0 * scale) as i32,
            (24.0 * scale) as i32,
            (24.0 * scale) as i32,
        );
        let (x, y) = place_scaled(tray, screen, (380.0, 460.0), scale);
        assert!((x + 380.0 - 1824.0).abs() < 1.0);
        assert!(y + 460.0 < 1040.0);
    }
}

#[test]
fn moving_between_different_dpi_monitors_preserves_physical_position() {
    use nerve_windows_surface::flyout::anchor::place_for_viewport;
    let screen = Rect::new(-3840, 0, 3840, 2160);
    let tray = Rect::new(-200, 2100, 32, 32);
    let (x, y) = place_for_viewport(tray, screen, (380.0, 460.0), 2.0, 1.5);
    assert!((x * 1.5 + 760.0 - tray.right() as f32).abs() < 1.0);
    assert!(y * 1.5 + 920.0 < tray.y as f32);
}
