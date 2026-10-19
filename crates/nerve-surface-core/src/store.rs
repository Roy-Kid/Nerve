//! Hub job store — one SSE reader thread, many TUI readers.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use crate::frame::{Frame, JobView};
use crate::hub::Hub;
use crate::launch::{DetachedSpawner, HttpHealth, HubLauncher};
use crate::locate::{FileProbe, HubLocator};
use crate::notify::NotifyLease;
use crate::stream::{Backoff, FrameSource, HubFrameSource};

#[derive(Clone, Debug, Default)]
pub struct JobsSnapshot {
    pub jobs: Vec<JobView>,
    pub summary: crate::frame::HubSummary,
    pub hub_instance_id: String,
    pub revision: u64,
    pub error: Option<String>,
    pub machines: Vec<serde_json::Value>,
    pub offline: bool,
    pub notify: NotifyLease,
}

#[derive(Clone)]
pub struct JobsStore {
    inner: Arc<RwLock<Arc<JobsSnapshot>>>,
    instance_id: Arc<String>,
    notifications: Arc<Mutex<Vec<crate::frame::HubNotification>>>,
    preferences: Arc<Mutex<serde_json::Value>>,
}

impl JobsStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Arc::new(JobsSnapshot::default()))),
            instance_id: Arc::new(format!(
                "client-{}-{}",
                std::process::id(),
                time::OffsetDateTime::now_utc().unix_timestamp_nanos()
            )),
            notifications: Arc::default(),
            preferences: Arc::new(Mutex::new(serde_json::json!({"enabled":false}))),
        }
    }

    /// Share the current snapshot. Readers get an `Arc`, not a deep copy of
    /// every job's timeline — the TUI reads this on every loop wake.
    pub fn snapshot(&self) -> Arc<JobsSnapshot> {
        self.inner
            .read()
            .map(|guard| Arc::clone(&guard))
            .unwrap_or_default()
    }

    /// Pull the hub's current job list before subscribing to SSE.
    ///
    /// Fail-open: a bad read leaves whatever the store already held.
    pub fn fetch_from_hub(&self) {
        let Ok(raw) = Hub::loopback().get("/v1/frame") else {
            return;
        };
        let frame = match Frame::decode(&raw) {
            Ok(frame) => frame,
            Err(error) => {
                self.set_error(error.to_string());
                return;
            }
        };
        self.apply_frame(frame);
    }

    /// Replace the whole set from one SSE frame, lease included.
    pub fn apply_frame(&self, frame: Frame) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.hub_instance_id == frame.hub_instance_id && frame.revision < guard.revision {
                return;
            }
            self.notifications.lock().unwrap().extend(
                frame
                    .notifications
                    .iter()
                    .filter(|e| e.target == *self.instance_id)
                    .cloned(),
            );
            *guard = Arc::new(JobsSnapshot {
                jobs: frame.jobs,
                summary: frame.summary,
                hub_instance_id: frame.hub_instance_id,
                revision: frame.revision,
                error: None,
                machines: frame.machines,
                notify: frame.notify,
                offline: false,
            });
        }
    }

    /// Replace the whole set, wholesale.
    ///
    /// Never merged: a frame is the authoritative full list, which is exactly
    /// what makes a dropped connection cost nothing. Also how a surface's tests
    /// seed a store without a hub. The notify lease is left as it was, so a
    /// test that only cares about jobs does not have to invent one.
    #[cfg(feature = "testkit")]
    pub fn set_jobs(&self, jobs: Vec<JobView>) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = Arc::new(JobsSnapshot {
                jobs,
                summary: guard.summary.clone(),
                hub_instance_id: guard.hub_instance_id.clone(),
                revision: guard.revision,
                error: guard.error.clone(),
                machines: guard.machines.clone(),
                notify: guard.notify.clone(),
                offline: false,
            });
        }
    }

    pub fn drain_notifications(&self) -> Vec<crate::frame::HubNotification> {
        std::mem::take(&mut *self.notifications.lock().unwrap())
    }
    pub fn configure_notifications(&self, value: serde_json::Value) {
        let mut preferences = self.preferences.lock().unwrap();
        if *preferences == value {
            return;
        }
        *preferences = value.clone();
        let path = format!("/v1/clients/{}", self.instance_id);
        std::thread::spawn(move || {
            let _ = Hub::loopback().send_json(&path, reqwest::Method::PUT, &value);
        });
    }
    pub fn notification_result(&self, id: String, delivered: bool) {
        std::thread::spawn(move || {
            let _ = Hub::loopback().send_json(
                "/v1/notifications/result",
                reqwest::Method::POST,
                &serde_json::json!({"id":id,"delivered":delivered}),
            );
        });
    }
    fn set_error(&self, error: String) {
        if let Ok(mut guard) = self.inner.write() {
            let mut next = (**guard).clone();
            next.error = Some(error);
            next.offline = true;
            *guard = Arc::new(next);
        }
    }
    fn set_offline(&self) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.offline {
                return;
            }
            *guard = Arc::new(JobsSnapshot {
                jobs: guard.jobs.clone(),
                summary: guard.summary.clone(),
                hub_instance_id: guard.hub_instance_id.clone(),
                revision: guard.revision,
                error: guard.error.clone(),
                machines: guard.machines.clone(),
                notify: guard.notify.clone(),
                offline: true,
            });
        }
    }

    /// Attach to the hub forever, on a thread of its own.
    ///
    /// `stream_path` names which surface is asking; the hub treats it as a log
    /// tag (`crates/nerve-hub/src/http/stream.rs`). Holding this stream open is
    /// what keeps the hub alive, so the thread outlives any window.
    pub fn spawn_reader(
        self,
        home: PathBuf,
        stream_path: impl Into<String>,
    ) -> thread::JoinHandle<()> {
        let stream_path = format!(
            "{}&instanceId={}&notifications=true",
            stream_path.into(),
            self.instance_id
        );
        thread::spawn(move || {
            let locator = HubLocator::new(FileProbe, home);
            let mut launcher =
                HubLauncher::new(DetachedSpawner::new(), HttpHealth::new(Hub::loopback()));
            let mut source = HubFrameSource::new(Hub::loopback(), stream_path.clone());
            let mut backoff = Backoff::new();
            loop {
                let launch = launcher.ensure(locator.locate().as_deref());
                if let Some(error) = &launcher.last_error {
                    tracing::error!(%error,"hub ensure failed");
                    self.set_error(error.clone());
                }
                tracing::debug!(?launch, path = stream_path.as_str(), "hub launch");
                let _ = Hub::loopback().send_json(
                    &format!("/v1/clients/{}", self.instance_id),
                    reqwest::Method::PUT,
                    &self.preferences.lock().unwrap(),
                );
                self.fetch_from_hub();
                if let Err(error) = source.open() {
                    tracing::warn!(?error, path = stream_path.as_str(), "stream open failed");
                    self.set_offline();
                    thread::sleep(backoff.fail());
                    continue;
                }
                tracing::info!(path = stream_path.as_str(), "stream attached");
                let mut frames = 0usize;
                loop {
                    match source.next_frame() {
                        Ok(Some(frame)) => {
                            frames += 1;
                            tracing::debug!(jobs = frame.jobs.len(), "frame");
                            self.apply_frame(frame);
                        }
                        Ok(None) => {
                            tracing::info!(path = stream_path.as_str(), frames, "stream closed");
                            break;
                        }
                        Err(error) => {
                            self.set_error(format!("{error:?}"));
                            tracing::warn!(
                                ?error,
                                path = stream_path.as_str(),
                                frames,
                                "stream ended"
                            );
                            break;
                        }
                    }
                }
                self.set_offline();
                if frames > 0 {
                    backoff.reset();
                }
                let delay = backoff.fail();
                tracing::debug!(?delay, "reconnect");
                thread::sleep(delay);
            }
        })
    }
}

impl Default for JobsStore {
    fn default() -> Self {
        Self::new()
    }
}
