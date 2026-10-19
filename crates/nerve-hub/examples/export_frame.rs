//! Generate a canonical surface/documentation fixture without opening a socket.
use nerve_hub::{
    clock::FakeClock,
    lifecycle::{NotifyLease, NotifyPolicy},
    sse::Frame,
    state::{JobStore, SignalProbe},
};
use std::sync::Arc;
fn main() {
    let clock = Arc::new(FakeClock::new(time::macros::datetime!(2026-01-01 0:00 UTC)));
    let mut store = JobStore::new(clock, "fixture-machine".into(), Arc::new(SignalProbe));
    store.load_demo();
    let mut frame = Frame::connect(&store, NotifyLease::empty(NotifyPolicy::Single)).value();
    frame["machineId"] = serde_json::json!("fixture-machine-id");
    frame["hubInstanceId"] = serde_json::json!("fixture-hub");
    for job in frame["jobs"].as_array_mut().unwrap() {
        job["machineId"] = serde_json::json!("fixture-machine-id");
        job["display"]["machineGroup"] = serde_json::json!("fixture-machine-id");
        job["extensions"]["hub"] = job["display"].clone();
    }
    frame["summary"] = nerve_hub::projection::summarize(&frame["jobs"]);
    println!("{}", frame);
}
