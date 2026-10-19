use nerve_surface_core::testkit;
#[test]
fn counts_are_read_from_the_hub_projection() {
    let frame = testkit::frame(testkit::SIX_STATE_FRAME);
    let tally = frame.summary.tally();
    assert_eq!(frame.summary.total, 6);
    assert_eq!(tally.total(), 6);
    assert_eq!(tally.problem, 1);
    assert_eq!(tally.ask, 1);
    assert_eq!(tally.waiting, 1);
}
