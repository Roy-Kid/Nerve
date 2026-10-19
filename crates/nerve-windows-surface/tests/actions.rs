//! Carrying out a plan, with the OS replaced by a notebook.

use nerve_surface_core::frame::JobView;
use nerve_windows_surface::actions::clipboard::RecordingClipboard;
use nerve_windows_surface::actions::shell::RecordingOpener;
use nerve_windows_surface::actions::{copy, perform};
use serde_json::json;

const LOCAL: Option<&str> = Some("thinkpad");

fn job(value: serde_json::Value) -> JobView {
    nerve_surface_core::testkit::job(value)
}

#[test]
fn a_local_folder_is_opened() {
    let row = job(json!({
        "id": "j", "name": "nerve", "alias": "thinkpad",
        "location": { "openURL": "file:///C:/work/nerve" }
    }));
    let opener = RecordingOpener::default();
    let mut clipboard = RecordingClipboard::default();

    let outcome = perform(&row, LOCAL, &opener, &mut clipboard);

    assert!(outcome.succeeded, "{}", outcome.message);
    assert!(outcome.opened);
    assert_eq!(opener.opened.borrow().as_slice(), ["path:C:/work/nerve"]);
    assert!(clipboard.copied.is_empty(), "it copied instead of opening");
}

#[test]
fn an_ide_deep_link_goes_to_the_shell_untouched() {
    let row = job(json!({
        "id": "j", "name": "nerve", "alias": "thinkpad",
        "location": { "openURL": "vscode://file/C:/work/nerve" }
    }));
    let opener = RecordingOpener::default();
    let mut clipboard = RecordingClipboard::default();

    perform(&row, LOCAL, &opener, &mut clipboard);

    assert_eq!(
        opener.opened.borrow().as_slice(),
        ["url:vscode://file/C:/work/nerve"]
    );
}

/// The rule that matters most: a path on another machine is very likely also a
/// directory here, and opening the wrong one silently is worse than opening
/// nothing.
#[test]
fn another_machines_path_is_copied_and_never_opened() {
    let row = job(json!({
        "id": "j", "name": "nerve", "alias": "buildbox",
        "location": { "openURL": "file:///C:/work/nerve" }
    }));
    let opener = RecordingOpener::default();
    let mut clipboard = RecordingClipboard::default();

    let outcome = perform(&row, LOCAL, &opener, &mut clipboard);

    assert!(outcome.succeeded);
    assert!(!outcome.opened, "keep the copy explanation visible");
    assert!(opener.opened.borrow().is_empty(), "it opened a remote path");
    assert_eq!(clipboard.copied.len(), 1);
    assert!(
        outcome.message.contains("another machine"),
        "the user was not told why: {}",
        outcome.message
    );
}

#[test]
fn a_job_with_nowhere_to_go_says_so_without_copying() {
    let row = job(json!({ "id": "j", "name": "nerve", "alias": "thinkpad" }));
    let opener = RecordingOpener::default();
    let mut clipboard = RecordingClipboard::default();

    let outcome = perform(&row, LOCAL, &opener, &mut clipboard);

    assert!(!outcome.succeeded);
    assert!(opener.opened.borrow().is_empty());
    assert!(clipboard.copied.is_empty());
}

#[test]
fn copy_puts_the_name_and_the_activity_on_the_clipboard() {
    let row = job(json!({
        "id": "j", "name": "nerve",
        "current": { "type": "tool", "summary": "Using Bash" }
    }));
    let mut clipboard = RecordingClipboard::default();

    let outcome = copy(&row, &mut clipboard);

    assert!(outcome.succeeded);
    assert_eq!(clipboard.copied, vec!["nerve — Using Bash"]);
}

/// Invariant 6: a job may advertise `approve` or `cancel`; this surface
/// classifies them and performs none.
#[test]
fn a_job_offering_remote_actions_is_still_only_opened() {
    let row = job(json!({
        "id": "j", "name": "nerve", "alias": "thinkpad",
        "location": { "openURL": "file:///C:/work/nerve" },
        "actions": [
            { "id": "approve", "title": "Approve", "kind": "approve", "state": "available" }
        ]
    }));
    let opener = RecordingOpener::default();
    let mut clipboard = RecordingClipboard::default();

    perform(&row, LOCAL, &opener, &mut clipboard);

    let opened = opener.opened.borrow();
    assert_eq!(opened.len(), 1);
    assert!(opened[0].starts_with("path:"), "{opened:?}");
}
