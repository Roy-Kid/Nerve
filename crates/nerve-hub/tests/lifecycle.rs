use nerve_hub::{
    HubRuntime,
    clock::SystemClock,
    state::{JobStore, SignalProbe},
};
use std::{sync::Arc, time::Duration};
#[tokio::test(start_paused = true)]
async fn hub_survives_startup_and_all_surface_disconnects() {
    let hub = HubRuntime::new(JobStore::new(
        Arc::new(SystemClock),
        "test".into(),
        Arc::new(SignalProbe),
    ));
    assert!(
        tokio::time::timeout(Duration::from_secs(60), hub.shutdown())
            .await
            .is_err()
    );
    let first = hub.subscribe();
    let second = hub.subscribe();
    drop(first);
    drop(second);
    assert!(
        tokio::time::timeout(Duration::from_secs(60), hub.shutdown())
            .await
            .is_err()
    );
}
