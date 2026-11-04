//! Preferences: the only thing this surface writes to disk.

use nerve_surface_core::frame::AttentionLevel;
use nerve_windows_surface::settings::{GroupMode, Settings};

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nerve-settings-{name}-{}", std::process::id()));
    dir.join("settings.json")
}

#[test]
fn settings_round_trip_through_a_file() {
    let path = temp_path("roundtrip");
    let settings = Settings {
        panel_width: 420.0,
        group_mode: GroupMode::Status,
        toasts_enabled: true,
        toast_floor: AttentionLevel::Required,
        ..Settings::default()
    };
    settings.save(&path).expect("save");

    let loaded = Settings::load(&path);
    assert_eq!(loaded, settings);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// The file is editable, so it will be edited. A surface that refused to start
/// over a stray comma would be worse than one that forgets a window size.
#[test]
fn unreadable_content_falls_back_rather_than_failing() {
    let path = temp_path("garbage");
    std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    std::fs::write(&path, "{ not json at all").expect("write");

    assert_eq!(Settings::load(&path), Settings::default());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// `f32::clamp` propagates NaN rather than replacing it, and a NaN window size
/// is a window that never appears.
#[test]
fn a_non_finite_size_falls_back_to_the_default() {
    let broken = Settings {
        panel_width: f32::NAN,
        panel_height: f32::INFINITY,
        ..Settings::default()
    }
    .clamped();
    assert!(broken.panel_width.is_finite());
    assert!(broken.panel_height.is_finite());
}

#[test]
fn resizing_is_saved_and_keeps_window_bounds() {
    let path = temp_path("resize");
    let mut settings = Settings::default();
    settings.set_panel_size(510.0, 700.0);
    settings.save(&path).expect("save resize");
    let loaded = Settings::load(&path);
    assert_eq!((loaded.panel_width, loaded.panel_height), (510.0, 700.0));
    settings.set_panel_size(1.0, 10000.0);
    assert_eq!(
        (settings.panel_width, settings.panel_height),
        (320.0, 800.0)
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
