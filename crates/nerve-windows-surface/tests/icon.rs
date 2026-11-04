//! What the tray icon's pixels actually say.
//!
//! No Windows required: the renderer is a pure function from a tally to RGBA,
//! which is the whole reason the geometry is decided here rather than in a
//! paint handler.

use nerve_surface_core::palette::Rgb;
use nerve_surface_core::tally::Tally;
use nerve_windows_surface::tray::bands::stack;
use nerve_windows_surface::tray::icon::{IconSpec, Theme, render};

fn tally(problem: usize, running: usize, success: usize) -> Tally {
    Tally {
        problem,
        running,
        success,
        ..Tally::default()
    }
}

fn spec(tally: &Tally, size: u32) -> IconSpec {
    IconSpec {
        size,
        bands: stack(tally),
        offline: false,
        theme: Theme::Dark,
    }
}

fn pixel(rgba: &[u8], size: u32, x: u32, y: u32) -> (Rgb, u8) {
    let index = ((y * size + x) * 4) as usize;
    (
        Rgb::new(rgba[index], rgba[index + 1], rgba[index + 2]),
        rgba[index + 3],
    )
}

/// How far across the icon the painted pixels reach on `row`.
fn painted_width(rgba: &[u8], size: u32, row: u32) -> u32 {
    (0..size)
        .filter(|x| pixel(rgba, size, *x, row).1 > 0)
        .count() as u32
}

#[test]
fn an_idle_icon_still_shows_something() {
    // A tray icon cannot be zero-width the way the menu-bar ribbon can, and an
    // invisible one leaves the user no way back to the flyout.
    let rgba = render(&spec(&Tally::default(), 16));
    let painted = rgba.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
    assert!(painted > 0, "idle drew nothing");
}

/// The failure the first render had: at a corner radius of half the bar height
/// a single full-height band stops being a bar and becomes a circle.
#[test]
fn a_single_band_is_a_bar_and_not_a_dot() {
    let size = 32;
    let rgba = render(&spec(&tally(0, 40, 0), size));
    let top = painted_width(&rgba, size, 1);
    let middle = painted_width(&rgba, size, size / 2);
    assert!(top > 0, "a circle would leave the top row empty");
    assert!(
        top * 2 > middle,
        "top row {top} against middle {middle} — that is a dot, not a bar"
    );
}

/// The other first-render failure: one running job looked exactly like forty.
#[test]
fn more_work_makes_a_longer_bar() {
    let size = 32;
    let one = render(&spec(&tally(0, 1, 0), size));
    let many = render(&spec(&tally(0, 40, 0), size));
    let narrow = painted_width(&one, size, size / 2);
    let wide = painted_width(&many, size, size / 2);
    assert!(
        wide > narrow + 4,
        "one job spans {narrow}px, forty spans {wide}px — indistinguishable"
    );
}
