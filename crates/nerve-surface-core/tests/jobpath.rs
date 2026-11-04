//! Turning a producer's reported location into a path.
//!
//! Every input here is a string from *some* machine, so the suite deliberately
//! mixes POSIX and Windows shapes and runs identically on both.

use std::path::PathBuf;

use nerve_surface_core::jobpath::{
    is_ide_deep_link, path_from_focus_hint, workspace_path_from_url,
};

fn path(s: &str) -> Option<PathBuf> {
    Some(PathBuf::from(s))
}

// ── openURL ─────────────────────────────────────────────────────────────────

#[test]
fn parses_cursor_deep_link() {
    assert_eq!(
        workspace_path_from_url("cursor://file/Users/me/proj"),
        path("/Users/me/proj")
    );
}

#[test]
fn ide_deep_link_is_not_file_uri() {
    assert!(is_ide_deep_link("cursor://file/Users/me"));
    assert!(!is_ide_deep_link("file:///Users/me"));
}

// ── openURL, reported by a Windows producer ─────────────────────────────────

#[test]
fn parses_a_windows_deep_link() {
    // The slash after `file` is the URL's, not the path's.
    assert_eq!(
        workspace_path_from_url("vscode://file/C:/work/nerve"),
        path("C:/work/nerve")
    );
}

// ── focus breadcrumb ────────────────────────────────────────────────────────

/// The hub builds this breadcrumb as `{producer} · {project} · {host} · {cwd}`
/// (`crates/nerve-hub/src/hook/build.rs`). Finding the path by searching for
/// the literal `" · /"` meant a Windows producer's cwd was never found at all.
#[test]
fn a_windows_cwd_is_found_in_the_breadcrumb() {
    assert_eq!(
        path_from_focus_hint(Some("Claude · nerve · session · C:\\work\\nerve")),
        path("C:\\work\\nerve")
    );
}

#[test]
fn the_last_absolute_segment_wins() {
    assert_eq!(
        path_from_focus_hint(Some("Claude · /opt/tool · session · /Users/me/nerve")),
        path("/Users/me/nerve")
    );
}
