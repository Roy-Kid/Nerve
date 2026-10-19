use nerve_hub::{
    lifecycle::roster::ClientPreferences,
    lifecycle::{NotifyPolicy, SurfaceRoster},
    notification::Notifications,
    projection::project_json,
    sse::ChangeSignal,
};
use serde_json::json;
fn preferences(enabled: bool) -> ClientPreferences {
    serde_json::from_value(json!({"enabled":enabled})).unwrap()
}
fn ask(level: &str) -> serde_json::Value {
    json!([project_json(
        json!({"id":"codex:s","attention":{"level":level,"reason":"approval"}})
    )])
}
#[test]
fn one_instance_wins_and_reconnect_does_not_replay() {
    let roster = SurfaceRoster::new(ChangeSignal::new());
    roster.configure_client("first".into(), preferences(true));
    roster.configure_client("second".into(), preferences(true));
    let first = roster.subscribe_client("macos", "first", true);
    let _second = roster.subscribe_client("macos", "second", true);
    let mut policy = Notifications::default();
    let events = policy.evaluate(&ask("suggested"), &roster);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["target"], "first");
    assert!(policy.evaluate(&ask("suggested"), &roster).is_empty());
    drop(first);
    let _first = roster.subscribe_client("macos", "first", true);
    assert!(policy.evaluate(&ask("suggested"), &roster).is_empty());
    assert_eq!(policy.evaluate(&ask("required"), &roster).len(), 1);
    assert!(policy.evaluate(&ask("suggested"), &roster).is_empty());
    assert!(policy.evaluate(&ask("required"), &roster).is_empty());
}
#[test]
fn all_targets_capable_enabled_instances_only_and_wait_is_silent() {
    let roster = SurfaceRoster::new(ChangeSignal::new());
    roster.set_policy(NotifyPolicy::All);
    roster.configure_client("a".into(), preferences(true));
    roster.configure_client("b".into(), preferences(false));
    roster.configure_client("c".into(), preferences(true));
    let _a = roster.subscribe_client("windows", "a", true);
    let _b = roster.subscribe_client("macos", "b", true);
    let _c = roster.subscribe_client("vscode", "c", false);
    let mut policy = Notifications::default();
    assert_eq!(policy.evaluate(&ask("required"), &roster).len(), 1);
    let wait = json!([project_json(
        json!({"id":"codex:wait","attention":{"level":"required","reason":"resource"}})
    )]);
    assert!(policy.evaluate(&wait, &roster).is_empty());
}

#[test]
fn repeated_connections_share_one_executor_and_dedupe_expires_only_after_a_new_transition() {
    let roster = SurfaceRoster::new(ChangeSignal::new());
    roster.set_policy(NotifyPolicy::All);
    roster.configure_client("same".into(), preferences(true));
    let first = roster.subscribe_client("vscode", "same", true);
    let _overlap = roster.subscribe_client("vscode", "same", true);
    let mut policy = Notifications::default();
    let now = std::time::Instant::now();
    let events = policy.evaluate_frame_at(&ask("required"), &json!([]), &roster, now);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["connectionId"], first.connection_id());
    policy.evaluate_frame_at(&json!([]), &json!([]), &roster, now);
    assert!(
        policy
            .evaluate_frame_at(
                &ask("required"),
                &json!([]),
                &roster,
                now + std::time::Duration::from_secs(119)
            )
            .is_empty()
    );
    policy.evaluate_frame_at(
        &json!([]),
        &json!([]),
        &roster,
        now + std::time::Duration::from_secs(120),
    );
    assert_eq!(
        policy
            .evaluate_frame_at(
                &ask("required"),
                &json!([]),
                &roster,
                now + std::time::Duration::from_secs(121)
            )
            .len(),
        1
    );
}
#[test]
fn all_delivers_to_distinct_enabled_instances_and_terminal_success_is_hub_filtered() {
    let roster = SurfaceRoster::new(ChangeSignal::new());
    roster.set_policy(NotifyPolicy::All);
    for id in ["a", "b"] {
        roster.configure_client(id.into(), preferences(true));
    }
    let _a = roster.subscribe_client("macos", "a", true);
    let _b = roster.subscribe_client("macos", "b", true);
    let mut policy = Notifications::default();
    assert_eq!(policy.evaluate(&ask("suggested"), &roster).len(), 2);
    roster.configure_client(
        "a".into(),
        serde_json::from_value(json!({"enabled":true,"longSuccess":true,"longSuccessSeconds":300}))
            .unwrap(),
    );
    let ended = json!([project_json(
        json!({"id":"codex:end","lifecycle":"ended","outcome":"success","createdAt":"2026-01-01T00:00:00Z","endedAt":"2026-01-01T00:06:00Z"})
    )]);
    let events = policy.evaluate_frame(&json!([]), &ended, &roster);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["kind"], "longSuccess");
    assert_eq!(events[0]["target"], "a");
}
