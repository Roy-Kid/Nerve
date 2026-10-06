//! What the tray icon's pixels actually say.
//!
//! No Windows required: the renderer is a pure function from a tally to RGBA,
//! which is the whole reason the geometry is decided here rather than in a
//! paint handler.

use nerve_surface_core::palette::{self, Rgb};
use nerve_surface_core::tally::Tally;
use nerve_windows_surface::tray::bands::stack;
use nerve_windows_surface::tray::icon::{render, IconSpec, Theme};

/// Every rung Windows asks for: 100%, 125%, 150%, 175%, 200%.
const RUNGS: [u32; 5] = [16, 20, 24, 28, 32];

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
fn every_dpi_rung_produces_a_square_rgba_buffer() {
    for size in RUNGS {
        let rgba = render(&spec(&tally(0, 3, 0), size));
        assert_eq!(rgba.len(), (4 * size * size) as usize, "at {size}px");
    }
}

#[test]
fn an_idle_icon_still_shows_something() {
    // A tray icon cannot be zero-width the way the menu-bar ribbon can, and an
    // invisible one leaves the user no way back to the flyout.
    let rgba = render(&spec(&Tally::default(), 16));
    let painted = rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(painted > 0, "idle drew nothing");
}

#[test]
fn an_idle_icon_is_gray_and_not_a_status_colour() {
    let rgba = render(&spec(&Tally::default(), 32));
    let (color, alpha) = pixel(&rgba, 32, 16, 16);
    assert!(alpha > 0, "idle bar missing at its own centre");
    assert_eq!(color.r, color.g);
    assert_eq!(color.g, color.b);
    assert_eq!(alpha, 255, "idle target must be opaque");
}

#[test]
fn idle_is_centered_and_high_contrast_at_every_dpi_rung() {
    for size in RUNGS {
        for theme in [Theme::Light, Theme::Dark] {
            let rgba = render(&IconSpec {
                theme,
                ..spec(&Tally::default(), size)
            });
            let (ink, alpha) = pixel(&rgba, size, size / 2, size / 2);
            assert_eq!(alpha, 255);
            match theme {
                Theme::Light => assert!(ink.r < 80),
                Theme::Dark => assert!(ink.r > 210),
            }
            let first = (0..size)
                .find(|x| pixel(&rgba, size, *x, size / 2).1 > 0)
                .unwrap();
            let last = (0..size)
                .rfind(|x| pixel(&rgba, size, *x, size / 2).1 > 0)
                .unwrap();
            assert!((first as i32 - (size - 1 - last) as i32).abs() <= 1);
        }
    }
}

#[test]
fn an_idle_icon_is_visibly_shorter_than_working_one() {
    let size = 32;
    let idle = render(&spec(&Tally::default(), size));
    let working = render(&spec(&tally(0, 12, 0), size));
    let idle_rows = (0..size)
        .filter(|y| painted_width(&idle, size, *y) > 0)
        .count();
    let working_rows = (0..size)
        .filter(|y| painted_width(&working, size, *y) > 0)
        .count();
    assert!(
        working_rows > idle_rows * 2,
        "idle {idle_rows}, working {working_rows}"
    );
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

#[test]
fn each_slot_paints_its_own_colour() {
    let size = 32;
    let rgba = render(&spec(&tally(1, 3, 2), size));
    let sample = |y: u32| pixel(&rgba, size, 1, y).0;
    let near = |a: Rgb, b: Rgb| {
        (i32::from(a.r) - i32::from(b.r)).abs()
            + (i32::from(a.g) - i32::from(b.g)).abs()
            + (i32::from(a.b) - i32::from(b.b)).abs()
            < 40
    };
    assert!(near(sample(4), palette::PROBLEM), "top was {:?}", sample(4));
    assert!(
        near(sample(16), palette::RUNNING),
        "middle was {:?}",
        sample(16)
    );
    assert!(
        near(sample(27), palette::SUCCESS),
        "bottom was {:?}",
        sample(27)
    );
}

#[test]
fn bands_never_touch() {
    // A transparent gutter, so two status colours cannot be read as one band.
    let size = 32;
    let rgba = render(&spec(&tally(1, 3, 2), size));
    let clear_rows = (0..size)
        .filter(|y| painted_width(&rgba, size, *y) == 0)
        .count();
    assert!(
        clear_rows >= 2,
        "only {clear_rows} clear rows between 3 bands"
    );
}

#[test]
fn offline_fades_what_is_left_and_strikes_it_through() {
    let size = 32;
    let live = render(&spec(&tally(0, 3, 0), size));
    let offline = render(&IconSpec {
        offline: true,
        ..spec(&tally(0, 3, 0), size)
    });

    let alpha_at = |rgba: &[u8], x: u32, y: u32| pixel(rgba, size, x, y).1;
    assert!(
        alpha_at(&offline, 1, 16) < alpha_at(&live, 1, 16),
        "offline is not faded"
    );
    // The slash runs bottom-left to top-right through otherwise empty corners.
    assert!(
        alpha_at(&offline, size - 2, 1) > 0,
        "no slash at the top right"
    );
}

#[test]
fn a_light_taskbar_darkens_and_a_dark_one_lifts() {
    let size = 32;
    let on_dark = render(&spec(&tally(0, 3, 0), size));
    let on_light = render(&IconSpec {
        theme: Theme::Light,
        ..spec(&tally(0, 3, 0), size)
    });
    let dark_pixel = pixel(&on_dark, size, 1, 16).0;
    let light_pixel = pixel(&on_light, size, 1, 16).0;
    assert_ne!(dark_pixel, light_pixel, "theme made no difference");
    let sum = |c: Rgb| u32::from(c.r) + u32::from(c.g) + u32::from(c.b);
    assert!(
        sum(dark_pixel) > sum(light_pixel),
        "the lift went the wrong way"
    );
}

#[test]
fn three_bands_still_fit_at_the_smallest_rung() {
    let rgba = render(&spec(&tally(1, 3, 2), 16));
    let painted = rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(painted > 16, "only {painted} pixels survived 16px");
}
