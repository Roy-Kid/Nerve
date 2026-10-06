use std::time::{Duration, Instant};

use nerve_windows_surface::flyout::visibility::Visibility;

#[test]
fn explicit_open_of_visible_panel_ignores_stale_focus_loss() {
    let mut panel = Visibility::default();
    panel.show();
    panel.lost_focus(Some(true));
    panel.show();
    assert!(!panel.lost_focus(Some(false)));
    assert!(!panel.lost_focus(Some(true)));
    assert!(panel.lost_focus(Some(false)));
}

#[test]
fn opening_waits_for_focus_before_dismissing() {
    let mut panel = Visibility::default();
    panel.show();
    assert!(!panel.lost_focus(Some(false)));
    assert!(!panel.lost_focus(None));
    assert!(!panel.lost_focus(Some(true)));
    assert!(panel.lost_focus(Some(false)));
}

#[test]
fn reopening_does_not_reuse_the_previous_focus() {
    let mut panel = Visibility::default();
    panel.show();
    panel.lost_focus(Some(true));
    panel.hide(Instant::now());
    panel.show();
    assert!(!panel.lost_focus(Some(false)));
}

#[test]
fn clicking_the_icon_after_focus_loss_does_not_reopen_the_panel() {
    let mut panel = Visibility::default();
    let now = Instant::now();
    assert!(panel.can_reopen(now));
    panel.show();
    panel.hide(now);
    assert!(!panel.visible);
    assert!(!panel.can_reopen(now + Duration::from_millis(100)));
    assert!(panel.can_reopen(now + Duration::from_millis(201)));
}

#[test]
fn unknown_focus_does_not_dismiss_an_open_panel() {
    let mut panel = Visibility::default();
    panel.show();
    panel.lost_focus(Some(true));
    assert!(!panel.lost_focus(None));
    assert!(panel.lost_focus(Some(false)));
}
