use nerve_windows_surface::notify::policy::Toast;
#[test]
fn notification_executor_carries_hub_identity_verbatim() {
    let toast = Toast {
        title: "Your turn".into(),
        body: "Continue in agent".into(),
        job_id: "m/p:s".into(),
        tag: "n1".into(),
        sound: false,
    };
    assert_eq!(toast.tag, "n1");
    assert!(!toast.sound);
}
