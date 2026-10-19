//! Resident hub assembly: store, projection pump, machine manager and maintenance.
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::Router;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::http::{self, HubState, StreamState};
use crate::lifecycle::{RosterGuard, SurfaceRoster};
use crate::sse::{Broadcaster, ChangeSignal};
use crate::state::JobStore;

/// A hub, running: routes to serve, surfaces to count, frames to push.
///
/// Its background work starts with it, so constructing one requires a tokio
/// runtime, and dropping one stops it.
pub struct HubRuntime {
    store: Arc<Mutex<JobStore>>,
    changes: ChangeSignal,
    frames: Arc<Broadcaster>,
    roster: SurfaceRoster,
    background: Vec<JoinHandle<()>>,
    machines: crate::federation::Machines,
    slots: Arc<Mutex<crate::hook::SlotMap>>,
}

impl HubRuntime {
    /// How often stale requests expire and dead local producers are reaped
    /// (`SubjectStore.swift:1021`).
    const MAINTENANCE_PERIOD: Duration = Duration::from_secs(5);

    /// Assemble a hub around `store` and start its background work.
    ///
    /// The daemon stays resident independent of subscription count.
    pub fn new(store: JobStore) -> Self {
        let store = Arc::new(Mutex::new(store));
        let changes = ChangeSignal::new();
        let roster = SurfaceRoster::new(changes.clone());
        let frames = Arc::new(Broadcaster::new(
            Arc::clone(&store),
            changes.clone(),
            Broadcaster::WINDOW,
            roster.clone(),
        ));

        let background = vec![
            tokio::spawn(Arc::clone(&frames).pump()),
            tokio::spawn(
                Maintenance {
                    store: Arc::clone(&store),
                    changes: changes.clone(),
                    period: Self::MAINTENANCE_PERIOD,
                }
                .run(),
            ),
        ];

        let machines = crate::federation::Machines::new(store.clone(), changes.clone());
        Self {
            slots: Arc::new(Mutex::new(crate::hook::SlotMap::new())),
            machines,
            store,
            changes,
            frames,
            roster,
            background,
        }
    }

    /// The hub's whole HTTP surface: the contract table plus `GET /v1/stream`.
    ///
    /// Callable as often as needed — every router shares this runtime's one
    /// store and one broadcaster, so a write through any of them reaches a
    /// stream served by any other.
    pub fn start_connections(&self) {
        self.machines.start();
    }

    pub fn router(&self) -> Router {
        http::router(
            HubState::from_shared_with_machines(
                Arc::clone(&self.store),
                self.changes.clone(),
                self.roster.clone(),
                self.machines.clone(),
            )
            .with_slots(self.slots.clone()),
        )
        .merge(http::stream_router(StreamState::new(
            Arc::clone(&self.frames),
            self.roster.clone(),
        )))
    }

    /// Count one surface as present until the returned guard drops.
    ///
    /// The SSE route takes one per connection; a caller can take one directly
    /// to register presence for routing. Lifecycle tests use this
    /// rather than opening a socket.
    pub fn subscribe(&self) -> RosterGuard {
        self.roster.subscribe("anonymous")
    }

    /// Resolve when the hub has decided to stop.
    ///
    /// Observation only: asking has no effect on the count or the timer, and
    /// asking after the fact is answered immediately.
    #[expect(
        clippy::manual_async_fn,
        reason = "the shutdown future must not borrow the runtime; callers require a static future"
    )]
    pub fn shutdown(&self) -> impl Future<Output = ()> + Send + 'static {
        async {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

impl Drop for HubRuntime {
    /// A runtime that is gone has nothing left to pump, count or time.
    fn drop(&mut self) {
        for task in &self.background {
            task.abort();
        }
    }
}

/// The upkeep every hub does on a timer: expire stale requests, close rows
/// whose producer process is gone, and publish a frame when either changed
/// something.
struct Maintenance {
    store: Arc<Mutex<JobStore>>,
    changes: ChangeSignal,
    period: Duration,
}

impl Maintenance {
    async fn run(self) {
        let mut tick = tokio::time::interval(self.period);
        // A hub that was suspended (a laptop lid, a paused test clock) owes no
        // catch-up ticks: the next one re-reads the whole world anyway.
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let changed = self
                .store
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .expire_and_reap();
            if changed {
                tracing::debug!("maintenance published");
                self.changes.raise();
            }
        }
    }
}
