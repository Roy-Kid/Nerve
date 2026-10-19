//! Turning writes into frames, once per window instead of once per write.
//!
//! An agent hook can push a dozen snapshots in a few milliseconds (a tool
//! finishing, a permission prompt, a status line). Rendering a frame for each
//! would spend more time serialising than the surface spends drawing, so a
//! change only *schedules* a frame: the pump waits out a short window, then
//! publishes one frame for everything that landed in it. Frames are full sets,
//! so coalescing loses nothing.
//!
//! Two seams, deliberately separate: [`ChangeSignal`] is the write edge (the
//! HTTP handlers raise it and know nothing else about frames), [`Broadcaster`]
//! is the read edge (it renders and fans out).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::{Notify, broadcast};

use crate::lifecycle::SurfaceRoster;
use crate::state::JobStore;

use super::frame::Frame;

/// How many rendered frames a slow subscriber may fall behind before it is
/// resynchronised instead of caught up.
const FRAME_BACKLOG: usize = 16;

/// "Something changed" — the only thing a write path has to say.
///
/// Cloneable and cheap: every handler holds one. A signal nobody pumps (the
/// contract tests, or any embedding that serves REST only) simply goes unread.
#[derive(Clone)]
pub struct ChangeSignal {
    inner: Arc<Changes>,
}

#[derive(Default)]
struct Changes {
    pending: AtomicBool,
    wake: Notify,
}

impl ChangeSignal {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Changes::default()),
        }
    }

    /// A write landed. Never blocks — a producer's POST must not wait on a
    /// surface's frame.
    pub fn raise(&self) {
        self.inner.pending.store(true, Ordering::Release);
        self.inner.wake.notify_one();
    }

    /// Wait until [`raise`](Self::raise) is called, or return at once if it was
    /// called while nobody was waiting.
    pub(crate) async fn woken(&self) {
        self.inner.wake.notified().await;
    }

    /// Claim whatever has been raised so far; `false` when nothing had been.
    pub(crate) fn take(&self) -> bool {
        self.inner.pending.swap(false, Ordering::AcqRel)
    }
}

impl Default for ChangeSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders frames from the store and fans them out to the live streams.
pub struct Broadcaster {
    store: Arc<Mutex<JobStore>>,
    changes: ChangeSignal,
    window: Duration,
    frames: broadcast::Sender<Arc<str>>,
    roster: SurfaceRoster,
    notifications: Mutex<crate::notification::Notifications>,
}

impl Broadcaster {
    /// The coalescing window. Long enough to fold a hook's burst into one
    /// frame, short enough that a surface still feels immediate.
    pub const WINDOW: Duration = Duration::from_millis(150);

    pub fn new(
        store: Arc<Mutex<JobStore>>,
        changes: ChangeSignal,
        window: Duration,
        roster: SurfaceRoster,
    ) -> Self {
        let (frames, _) = broadcast::channel(FRAME_BACKLOG);
        Self {
            store,
            changes,
            window,
            frames,
            roster,
            notifications: Mutex::new(crate::notification::Notifications::default()),
        }
    }

    /// Join the stream: the receiver first, then the frame that goes in front
    /// of it.
    ///
    /// Both are taken under one lock, and [`publish`](Self::publish) sends under
    /// that same lock, so no frame rendered *before* this one can arrive
    /// *after* it and roll a surface back to an older state.
    pub fn join(&self) -> (broadcast::Receiver<Arc<str>>, Arc<str>) {
        let store = self.store();
        let receiver = self.frames.subscribe();
        let notify = self.roster.lease();
        (receiver, Arc::from(Frame::connect(&store, notify).render()))
    }

    /// The authoritative set, taking nothing from the departed buffer — what a
    /// subscriber that fell too far behind is resynchronised with.
    pub fn resync(&self) -> Arc<str> {
        let notify = self.roster.lease();
        Arc::from(Frame::connect(&self.store(), notify).render())
    }

    /// Coalesce changes into frames until the runtime drops this task.
    pub async fn pump(self: Arc<Self>) {
        loop {
            self.changes.woken().await;
            if !self.changes.take() {
                // A leftover notification from a change an earlier frame
                // already carried.
                continue;
            }
            tokio::time::sleep(self.window).await;
            self.publish();
        }
    }

    /// Render one frame for everything raised so far and hand it to every live
    /// stream.
    fn publish(&self) {
        let mut store = self.store();
        // Claimed before the render, so a write that lands while it runs
        // schedules the next frame instead of being folded into a frame that
        // may already have passed it.
        self.changes.take();
        let notify = self.roster.lease();
        let mut frame = Frame::published(&mut store, notify).value();
        let notifications = self.notifications.lock().unwrap().evaluate_frame(
            &frame["jobs"],
            &frame["departed"],
            &self.roster,
        );
        frame["notifications"] = serde_json::json!(notifications);
        let frame: Arc<str> = Arc::from(frame.to_string());
        // No subscriber is not an error: a hub with no surface attached still
        // keeps its state, it simply has nobody to tell.
        let _ = self.frames.send(frame);
    }

    fn store(&self) -> MutexGuard<'_, JobStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
