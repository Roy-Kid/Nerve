//! Ribbon weights — the reason one problem among forty is still visible.

use nerve_surface_core::ribbon::{Run, weighted_runs};
use nerve_surface_core::status::StatusClass::{Problem, Running};

fn weights(runs: &[(nerve_surface_core::status::StatusClass, usize)]) -> Vec<f32> {
    weighted_runs(runs)
        .iter()
        .map(|run: &Run| run.weight)
        .collect()
}

fn sums_to_one(runs: &[(nerve_surface_core::status::StatusClass, usize)]) {
    let total: f32 = weights(runs).iter().sum();
    assert!((total - 1.0).abs() < 1e-5, "weights summed to {total}");
}

/// The case the floors exist for: at a true 1/41 this band would be too small
/// to see, and the one thing the ribbon is for would be invisible.
#[test]
fn a_single_problem_among_forty_running_stays_visible() {
    let runs = [(Problem, 1), (Running, 40)];
    let w = weights(&runs);
    assert!(w[0] > 0.10, "problem got {:.3} of the strip", w[0]);
    sums_to_one(&runs);
}

// ── Length ──────────────────────────────────────────────────────────────────
