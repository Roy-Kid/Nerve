//! Carrying out a plan, with the OS replaced by a notebook.

use nerve_surface_core::frame::JobView;
use nerve_windows_surface::actions::clipboard::RecordingClipboard;
use nerve_windows_surface::actions::perform;
use nerve_windows_surface::actions::shell::RecordingOpener;
use serde_json::json;

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

    let outcome = perform(&row, &opener, &mut clipboard);

    assert!(outcome.succeeded, "{}", outcome.message);
    assert!(outcome.opened);
    assert_eq!(opener.opened.borrow().as_slice(), ["path:C:/work/nerve"]);
    assert!(clipboard.copied.is_empty(), "it copied instead of opening");
}

/// The rule that matters most: a path on another machine is very likely also a
/// directory here, and opening the wrong one silently is worse than opening
/// nothing.
#[test]
fn another_machines_path_is_copied_and_never_opened() {
    let row = job(json!({
        "id": "j", "name": "nerve", "alias": "buildbox", "extensions": { "sshHost": "buildbox" },
        "location": { "openURL": "file:///C:/work/nerve" }
    }));
    let opener = RecordingOpener::default();
    let mut clipboard = RecordingClipboard::default();

    let outcome = perform(&row, &opener, &mut clipboard);

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
