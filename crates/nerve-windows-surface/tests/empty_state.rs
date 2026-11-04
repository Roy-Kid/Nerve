use nerve_windows_surface::flyout::view::empty_message;

#[test]
fn disconnected_and_missing_hubs_do_not_claim_nothing_is_running() {
    assert_eq!(empty_message(true, true).0, "Connecting to Nerve");
    assert_eq!(empty_message(true, false).0, "Nerve hub is missing");
}
