//! Where "Open" goes — the whole decision table, on any OS.

use nerve_surface_core::frame::JobView;
use nerve_windows_surface::actions::open::{OpenPlan, plan};
use serde_json::json;

fn job(value: serde_json::Value) -> JobView {
    nerve_surface_core::testkit::job(value)
}

fn local(location: serde_json::Value) -> JobView {
    job(json!({ "id": "j", "name": "nerve", "alias": "thinkpad", "location": location }))
}

fn remote(location: serde_json::Value) -> JobView {
    job(json!({
        "id": "j", "name": "nerve", "alias": "buildbox",
        "extensions": { "sshHost": "buildbox" }, "location": location
    }))
}

// ── Local ───────────────────────────────────────────────────────────────────

#[test]
fn a_local_file_url_is_revealed() {
    assert_eq!(
        plan(&local(json!({ "openURL": "file:///C:/work/nerve" }))),
        OpenPlan::Reveal("C:/work/nerve".into())
    );
}

/// The producer's URL is the hook's contract. A surface that rewrites it is
/// guessing at what the editor wanted.
#[test]
fn an_ide_deep_link_is_handed_over_untouched() {
    for url in [
        "vscode://file/C:/work/nerve",
        "cursor://file/Users/me/nerve",
        "vscode-insiders://file/C:/work",
    ] {
        assert_eq!(
            plan(&local(json!({ "openURL": url }))),
            OpenPlan::Shell(url.into()),
            "{url}"
        );
    }
}

#[test]
fn the_breadcrumb_tail_stands_in_when_there_is_neither() {
    let row = local(json!({ "focusHint": "Claude · nerve · session · C:\\work\\nerve" }));
    assert_eq!(plan(&row), OpenPlan::Reveal("C:\\work\\nerve".into()));
}

// ── Another machine ─────────────────────────────────────────────────────────

/// The rule that matters most. `C:\work\nerve` on the build box is very likely
/// also a directory here, and opening the wrong one silently is worse than
/// opening nothing.
#[test]
fn a_remote_path_is_never_opened_locally() {
    for location in [
        json!({ "openURL": "file:///C:/work/nerve" }),
        json!({ "focusHint": "Claude · nerve · session · C:\\work\\nerve" }),
        json!({ "openURL": "file:///Users/me/nerve" }),
    ] {
        match plan(&remote(location.clone())) {
            OpenPlan::Copy { reason, .. } => {
                assert!(reason.contains("another machine"), "{reason}");
            }
            other => panic!("remote {location} planned {other:?}"),
        }
    }
}

/// An IDE deep link is the exception: it carries its own authority, so the
/// editor decides where it lands rather than this surface.
#[test]
fn a_remote_ide_deep_link_still_routes_itself() {
    assert_eq!(
        plan(&remote(json!({ "openURL": "vscode://file/C:/work/nerve" }))),
        OpenPlan::Shell("vscode://file/C:/work/nerve".into())
    );
}

/// Only the hub's `sshHost` makes a job remote. An alias that is not this
/// machine's — a raw hostname, another spelling, none at all — says nothing.
#[test]
fn a_job_the_hub_did_not_reach_over_ssh_is_mine_whatever_its_alias() {
    for alias in ["ThinkPad", "", "buildbox"] {
        let row = job(json!({
            "id": "j", "name": "nerve", "alias": alias,
            "location": { "openURL": "file:///C:/work/nerve" }
        }));
        assert_eq!(
            plan(&row),
            OpenPlan::Reveal("C:/work/nerve".into()),
            "{alias:?}"
        );
    }
}

// ── Invariant 6 ─────────────────────────────────────────────────────────────

/// Nerve never reverse-controls an agent. A job may advertise `approve`,
/// `cancel`, `submit_input` — this surface classifies them and performs none.
#[test]
fn a_job_offering_remote_actions_still_only_gets_opened() {
    let row = job(json!({
        "id": "j", "name": "nerve", "alias": "thinkpad",
        "location": { "openURL": "file:///C:/work/nerve" },
        "actions": [
            { "id": "approve", "title": "Approve", "kind": "approve", "state": "available" },
            { "id": "cancel", "title": "Cancel", "kind": "cancel", "state": "available" }
        ]
    }));
    assert_eq!(plan(&row), OpenPlan::Reveal("C:/work/nerve".into()));
}
