//! Status colours as the nearest xterm-256 index of `nerve_surface_core::palette`.
//!
//! Chrome (borders, text, git, selection) stays on a handwritten gray/slate
//! scale. It must not grow its own rainbow.

use nerve_surface_core::palette::{self, Rgb};
use nerve_surface_core::status::StatusClass;

/// Problem — nearest index of `#FF3B30`.
pub const PROBLEM: u8 = nearest(palette::PROBLEM);
/// Human attention — nearest index of `#FF9F0A`.
pub const ATTENTION: u8 = nearest(palette::ATTENTION);
/// Automatic waiting is the same gray as idle.
pub const WAITING: u8 = IDLE;
/// Running — nearest index of `#0A84FF`.
pub const RUNNING: u8 = nearest(palette::RUNNING);
/// Monitor — nearest index of `#BF5AF2`.
pub const MONITOR: u8 = nearest(palette::MONITOR);
/// Success — nearest index of `#30D158`.
pub const SUCCESS: u8 = nearest(palette::SUCCESS);
/// Inactive / idle — nearest index of `#8E8E93`.
pub const IDLE: u8 = nearest(palette::INACTIVE);

/// The index a status paints. Waiting follows idle, the same as `palette::color_of`.
pub fn of(class: StatusClass) -> u8 {
    nearest(palette::color_of(class))
}

/// Alias used by filter/error paths.
pub const ERROR: u8 = PROBLEM;

/// “All jobs” filter chip.
pub const ALL: u8 = 255;

/// Dim chrome for a filter chip that has nothing to count. Not a status hue.
pub const FILTER_INACTIVE: u8 = 245;
pub const BORDER: u8 = 240;
pub const ACCENT: u8 = 153;
pub const SELECTION: u8 = 239;
pub const TEXT_ACTIVE: u8 = 255;
pub const TEXT_MUTED: u8 = 252;
pub const TEXT_INACTIVE: u8 = 244;
pub const ACTIVITY_TS: u8 = 109;
pub const BRANCH: u8 = 109;

const fn channel(level: u8) -> u8 {
    if level == 0 { 0 } else { 55 + 40 * level }
}

/// RGB of one xterm-256 index in 16..=255. Cube 16–231, gray ramp 232–255.
const fn xterm_rgb(index: u8) -> (u8, u8, u8) {
    if index >= 232 {
        let value = 8 + 10 * (index - 232);
        return (value, value, value);
    }
    let cube = index - 16;
    let red = cube / 36;
    let green = (cube % 36) / 6;
    let blue = cube % 6;
    (channel(red), channel(green), channel(blue))
}

const fn delta(a: u8, b: u8) -> u32 {
    a.abs_diff(b) as u32
}

/// Nearest index among the 6×6×6 cube and the gray ramp. Ties keep the lower index.
const fn nearest(color: Rgb) -> u8 {
    let mut best = 16u8;
    let mut best_distance = u32::MAX;
    let mut index = 16u16;
    while index <= 255 {
        let (red, green, blue) = xterm_rgb(index as u8);
        let distance = delta(color.r, red) * delta(color.r, red)
            + delta(color.g, green) * delta(color.g, green)
            + delta(color.b, blue) * delta(color.b, blue);
        if distance < best_distance {
            best_distance = distance;
            best = index as u8;
        }
        index += 1;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_hex_lands_on_the_nearest_index() {
        // Pure red is 196 (`#FF0000`). `#FF3B30` is the cube at 203.
        assert_eq!(PROBLEM, 203);
        assert_eq!(ATTENTION, 214);
        assert_eq!(RUNNING, 33);
        assert_eq!(MONITOR, 135);
        assert_eq!(SUCCESS, 77);
        assert_eq!(IDLE, 246);
        assert_eq!(WAITING, IDLE);
        assert_eq!(of(StatusClass::Problem), PROBLEM);
        assert_eq!(of(StatusClass::Waiting), IDLE);
        assert_eq!(of(StatusClass::Inactive), IDLE);
    }
}
