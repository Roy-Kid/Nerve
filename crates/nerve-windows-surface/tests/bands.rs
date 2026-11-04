//! Folding seven statuses into three learnable slots.

use nerve_surface_core::palette;
use nerve_surface_core::tally::Tally;
use nerve_windows_surface::tray::bands::{has_work, stack};

fn tally(
    problem: usize,
    attention: usize,
    waiting: usize,
    running: usize,
    monitor: usize,
    success: usize,
    inactive: usize,
) -> Tally {
    Tally {
        problem,
        attention,
        waiting,
        running,
        monitor,
        success,
        inactive,
        ask: 0,
    }
}

#[test]
fn inactive_alone_is_not_work() {
    // Inactive is chrome. An icon that lit up for it would never be dark.
    let idle = tally(0, 0, 0, 0, 0, 0, 5);
    assert!(stack(&idle).is_empty());
    assert!(!has_work(&idle));
}

#[test]
fn the_slots_keep_their_order_whatever_is_present() {
    // Top is always "needs you", middle always "happening", bottom always
    // "finished" — three positions to learn instead of seven stripes to read.
    let bands = stack(&tally(1, 0, 0, 2, 0, 3, 0));
    let colors: Vec<_> = bands.iter().map(|band| band.color).collect();
    assert_eq!(
        colors,
        vec![palette::PROBLEM, palette::RUNNING, palette::SUCCESS]
    );
}

#[test]
fn one_problem_among_forty_running_keeps_a_visible_share() {
    let bands = stack(&tally(1, 0, 0, 40, 0, 0, 0));
    assert!(bands[0].weight > 0.10, "problem got {:.3}", bands[0].weight);
}
