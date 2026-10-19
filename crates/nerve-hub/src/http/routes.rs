//! The route table, and the one thing its handlers share.
//!
//! One row per `(method, path)` pair of `IngestServer.swift:166`, legacy
//! aliases included. Swift routes on the tuple, so a known path with the wrong
//! method lands in the same `default` case as an unknown path: both fallbacks
//! below are therefore the same handler, and both answer 404 with the literal.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use serde_json::{Value, json};

use crate::hook::SlotMap;
use crate::lifecycle::SurfaceRoster;
use crate::sse::ChangeSignal;
use crate::state::JobStore;

use super::{actions, admin, guard, ingest};

/// Exclusive access to the hub's single write point, and the one thing a
/// handler has to say after it writes.
///
/// Deliberately not a context bag: a handler that needs something other than
/// these takes it as its own argument — the stream route, which needs the frame
/// pump and the surface count, has a state of its own.
#[derive(Clone)]
pub struct HubState {
    store: Arc<Mutex<JobStore>>,
    slots: Arc<Mutex<SlotMap>>,
    changes: ChangeSignal,
    roster: SurfaceRoster,
    pub(super) machines: crate::federation::Machines,
}

impl HubState {
    /// Take ownership of the store the routes will serve. The store already
    /// carries its clock, this machine's alias and the liveness probe.
    ///
    /// The change signal such a state raises has no reader: a store nobody else
    /// holds cannot be rendered into frames by anyone. Serving REST only is a
    /// legitimate way to use these routes. The roster starts empty, so health
    /// reports zero watchers.
    pub fn new(store: JobStore) -> Self {
        let changes = ChangeSignal::new();
        let roster = SurfaceRoster::new(changes.clone());
        Self::from_shared(Arc::new(Mutex::new(store)), changes, roster)
    }

    /// The same routes over a store the runtime also renders frames from.
    pub fn from_shared(
        store: Arc<Mutex<JobStore>>,
        changes: ChangeSignal,
        roster: SurfaceRoster,
    ) -> Self {
        let machines = crate::federation::Machines::new(store.clone(), changes.clone());
        Self::from_shared_with_machines(store, changes, roster, machines)
    }
    pub fn from_shared_with_machines(
        store: Arc<Mutex<JobStore>>,
        changes: ChangeSignal,
        roster: SurfaceRoster,
        machines: crate::federation::Machines,
    ) -> Self {
        Self {
            store,
            changes,
            roster,
            machines,
            slots: Arc::new(Mutex::new(SlotMap::new())),
        }
    }

    pub fn with_slots(mut self, slots: Arc<Mutex<SlotMap>>) -> Self {
        self.slots = slots;
        self
    }

    pub(super) fn roster(&self) -> &SurfaceRoster {
        &self.roster
    }

    /// Lock the store for the length of one handler.
    ///
    /// Poisoning is recovered rather than propagated: one panicking handler
    /// must not turn every later request into a 500.
    pub(super) fn store(&self) -> MutexGuard<'_, JobStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Schedule a frame: this handler changed what surfaces are looking at.
    ///
    /// Called after the store lock is released, and never for a write that
    /// changed nothing.
    pub(super) fn changed(&self) {
        self.changes.raise();
    }

    pub(super) fn slots(&self) -> MutexGuard<'_, SlotMap> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The hub's HTTP contract, ready for `axum::serve` and for `oneshot`.
///
/// The binary must serve this with
/// `into_make_service_with_connect_info::<SocketAddr>()`: the loopback guard
/// reads the peer from that extension and refuses a request it cannot place.
pub fn router(state: HubState) -> Router {
    Router::new()
        .route("/v1/health", get(admin::health))
        // Legacy aliases, kept for producers that predate the `/v1` prefix and
        // the subject → job rename.
        .route("/health", get(admin::health))
        .route("/v1/jobs", get(admin::jobs))
        .route("/v1/frame", get(admin::frame))
        .route("/v1/focus/remote", post(admin::remote_focus))
        .route(
            "/v1/clients/{id}",
            axum::routing::put(admin::configure_client),
        )
        .route("/v1/notifications/result", post(admin::notification_result))
        .route(
            "/v1/machines",
            get(admin::machines).put(admin::configure_machine),
        )
        .route("/v1/machines/discover", post(admin::discover_machines))
        .route("/v1/machines/{id}/connect", post(admin::connect_machine))
        .route(
            "/v1/machines/{id}/disconnect",
            post(admin::disconnect_machine),
        )
        .route("/v1/subjects", get(admin::jobs))
        .route("/v1/snapshot", post(ingest::snapshot))
        .route("/v1/events", post(ingest::events))
        .route("/v1/hook", post(super::hook::ingest))
        .route("/v1/actions/pending", get(actions::pending))
        .route("/v1/actions/result", post(actions::result))
        .route("/v1/refresh", post(admin::refresh))
        .route("/v1/demo", post(admin::demo))
        .route("/v1/clear", post(admin::clear))
        .route("/v1/notify", get(admin::notify).put(admin::put_notify))
        // `POST /v1/actions/invoke` is deliberately absent: invoking an action
        // is an NSWorkspace side effect and belongs to a surface. It falls
        // through here like any other unknown path.
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(state)
        // Applied inside-out: the size ceiling sits closest to the handlers,
        // the peer guard runs before a byte of body is read, and the CORS
        // header wraps every answer — the guards' rejections included.
        .layer(middleware::from_fn(guard::limit_body))
        .layer(middleware::from_fn(guard::loopback_only))
        .layer(middleware::from_fn(guard::allow_any_origin))
}

/// `IngestServer.swift:258` — unknown path and unsupported method alike.
///
/// Shared with the stream router, which owes the same answer: Swift routes on
/// the `(method, path)` tuple, so `POST /v1/stream` is as unknown to it as
/// `POST /v1/nowhere`.
pub(super) async fn not_found() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" })))
}
