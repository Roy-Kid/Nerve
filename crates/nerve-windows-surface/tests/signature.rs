//! The redraw gate.

use nerve_surface_core::palette;
use nerve_surface_core::tally::Tally;
use nerve_windows_surface::tray::bands::{Band, stack};
use nerve_windows_surface::tray::icon::Theme;
use nerve_windows_surface::tray::signature::of;

fn tally(running: usize, problem: usize) -> Tally {
    Tally {
        running,
        problem,
        ..Tally::default()
    }
}

fn sig(tally: &Tally) -> nerve_windows_surface::tray::signature::Signature {
    of(&stack(tally), false, Theme::Dark, 16)
}

#[test]
fn a_changed_count_redraws() {
    assert_ne!(sig(&tally(3, 0)), sig(&tally(4, 0)));
}

/// The gate's whole purpose. A job's `revision` bumps on every tool call, and
/// the icon it paints does not change — redrawing there would have the tray
/// blinking through any long piece of work.
#[test]
fn churn_that_changes_no_pixel_does_not_redraw() {
    let steady = stack(&tally(3, 0));
    let before = of(&steady, false, Theme::Dark, 16);
    // The same tally arriving again in a new frame: different `JobView`s,
    // different revisions, identical bands.
    let after = of(&stack(&tally(3, 0)), false, Theme::Dark, 16);
    assert_eq!(before, after);
}

#[test]
fn going_offline_redraws() {
    let bands = stack(&tally(3, 0));
    assert_ne!(
        of(&bands, false, Theme::Dark, 16),
        of(&bands, true, Theme::Dark, 16)
    );
}

#[test]
fn weights_below_a_pixel_of_difference_do_not_redraw() {
    let a = vec![Band {
        color: palette::RUNNING,
        weight: 0.500_0,
        count: 3,
    }];
    let b = vec![Band {
        color: palette::RUNNING,
        weight: 0.500_4,
        count: 3,
    }];
    assert_eq!(
        of(&a, false, Theme::Dark, 16),
        of(&b, false, Theme::Dark, 16)
    );
}
