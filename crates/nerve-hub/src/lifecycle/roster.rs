//! Hub-owned client registrations, notification capabilities and policy.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::sse::ChangeSignal;

use super::notify::{NotifyLease, NotifyPolicy, surface_label};
use super::refcount::{Presence, Subscription, Watchers};

/// Live surface connections plus the notify policy.
#[derive(Clone)]
pub struct SurfaceRoster {
    presence: Presence,
    inner: Arc<Mutex<Inner>>,
    next_id: Arc<AtomicU64>,
    changes: ChangeSignal,
}

struct Inner {
    names: HashMap<u64, String>,
    policy: NotifyPolicy,
    clients: HashMap<u64, Client>,
    preferences: HashMap<String, ClientPreferences>,
}

impl SurfaceRoster {
    /// Nobody watching, default [`NotifyPolicy::Single`].
    pub fn new(changes: ChangeSignal) -> Self {
        Self {
            presence: Presence::new(),
            inner: Arc::new(Mutex::new(Inner {
                names: HashMap::new(),
                policy: NotifyPolicy::Single,
                clients: HashMap::new(),
                preferences: HashMap::new(),
            })),
            next_id: Arc::new(AtomicU64::new(1)),
            changes,
        }
    }

    /// The subscription count used for observation and routing.
    pub fn presence(&self) -> Presence {
        self.presence.clone()
    }

    /// Follow the count — same channel [`Presence::watch`] publishes.
    pub fn watch(&self) -> tokio::sync::watch::Receiver<Watchers> {
        self.presence.watch()
    }

    /// One surface arrives, labelled with `?surface=`.
    pub fn subscribe(&self, surface: impl AsRef<str>) -> RosterGuard {
        self.subscribe_client(surface.as_ref(), "", false)
    }

    pub fn subscribe_client(&self, surface: &str, instance: &str, capable: bool) -> RosterGuard {
        let label = surface_label(Some(surface));
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let others = {
            let mut inner = self.lock();
            let others = inner.names.len();
            inner.names.insert(id, label.clone());
            let instance = if instance.is_empty() {
                format!("connection-{id}")
            } else {
                instance.to_owned()
            };
            inner.clients.insert(
                id,
                Client {
                    surface: label.clone(),
                    instance,
                    capable,
                },
            );
            others
        };
        let subscription = self.presence.subscribe();
        // 0→1 has nobody else to tell; the connect frame already carries
        // this surface. Raising here would republish ~150 ms later and
        // steal the next `frame()` from tests (and from surfaces) that
        // expect the next event to be a job write.
        if others > 0 {
            self.changes.raise();
        }
        RosterGuard {
            subscription: Some(subscription),
            id,
            label,
            inner: Arc::clone(&self.inner),
            changes: self.changes.clone(),
        }
    }

    /// The lease the next frame (and `/v1/health`) should carry.
    pub fn lease(&self) -> NotifyLease {
        let inner = self.lock();
        let watchers = inner.names.len();
        let surfaces: Vec<String> = inner.names.values().cloned().collect();
        NotifyLease::from_connections(inner.policy, watchers, surfaces)
    }

    /// Replace the policy. Returns `true` when it actually changed, so the
    /// caller can decide whether to publish a frame.
    pub fn set_policy(&self, policy: NotifyPolicy) -> bool {
        let mut inner = self.lock();
        if inner.policy == policy {
            return false;
        }
        inner.policy = policy;
        drop(inner);
        self.changes.raise();
        true
    }

    pub fn policy(&self) -> NotifyPolicy {
        self.lock().policy
    }

    pub fn configure_client(&self, id: String, preferences: ClientPreferences) {
        self.lock().preferences.insert(id, preferences);
    }
    pub fn targets(
        &self,
        job: &serde_json::Value,
        level: u64,
        kind: &str,
    ) -> Vec<(String, u64, bool)> {
        let inner = self.lock();
        let mut clients: Vec<_> = inner
            .clients
            .iter()
            .filter(|(_, client)| {
                if !client.capable {
                    return false;
                }
                let Some(p) = inner.preferences.get(&client.instance) else {
                    return false;
                };
                if !p.enabled {
                    return false;
                }
                match kind {
                    "ask" => {
                        if level < p.floor || (level < 4 && !p.ask) || (level == 4 && !p.urgent) {
                            return false;
                        }
                    }
                    "failure" => {
                        if !p.failure {
                            return false;
                        }
                    }
                    "unresponsive" => {
                        if !p.unresponsive {
                            return false;
                        }
                    }
                    "longSuccess" => {
                        if !p.long_success {
                            return false;
                        }
                        let parse = |v: &serde_json::Value| {
                            v.as_str().and_then(|s| {
                                time::OffsetDateTime::parse(
                                    s,
                                    &time::format_description::well_known::Rfc3339,
                                )
                                .ok()
                            })
                        };
                        let start = parse(&job["startedAt"]).or_else(|| parse(&job["createdAt"]));
                        let end = parse(&job["endedAt"]);
                        if !start.zip(end).is_some_and(|(start, end)| {
                            (end - start).as_seconds_f64() >= p.long_success_seconds
                        }) {
                            return false;
                        }
                    }
                    _ => return false,
                }
                if p.dnd_enabled {
                    let utc = time::OffsetDateTime::now_utc();
                    let minutes = (i64::from(utc.hour()) * 60
                        + i64::from(utc.minute())
                        + p.utc_offset_minutes)
                        .rem_euclid(1440);
                    let quiet = if p.dnd_start == p.dnd_end {
                        true
                    } else if p.dnd_start < p.dnd_end {
                        minutes >= p.dnd_start && minutes < p.dnd_end
                    } else {
                        minutes >= p.dnd_start || minutes < p.dnd_end
                    };
                    if quiet {
                        return false;
                    }
                }
                let producer = job["producer"]["id"].as_str().unwrap_or("");
                let project = job["context"]["project"].as_str().unwrap_or("");
                let workspace = job["context"]["workspace"].as_str().unwrap_or("");
                !p.muted_sources.iter().any(|s| s == producer)
                    && !p
                        .muted_projects
                        .iter()
                        .any(|s| s == project || s == workspace)
            })
            .collect();
        clients.sort_by_key(|(id, client)| {
            (
                ["macos", "windows", "vscode", "tmux"]
                    .iter()
                    .position(|s| *s == client.surface)
                    .unwrap_or(4),
                &client.instance,
                **id,
            )
        });
        clients.dedup_by(|a, b| a.1.instance == b.1.instance);
        if inner.policy == NotifyPolicy::Single {
            clients.truncate(1);
        }
        clients
            .into_iter()
            .map(|(id, c)| {
                (
                    c.instance.clone(),
                    *id,
                    inner.preferences[&c.instance].sound && level >= 3,
                )
            })
            .collect()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One surface's presence, with its label.
///
/// Dropping it *is* the disconnect: the SSE response body owns one.
pub struct RosterGuard {
    subscription: Option<Subscription>,
    id: u64,
    label: String,
    inner: Arc<Mutex<Inner>>,
    changes: ChangeSignal,
}

impl RosterGuard {
    pub fn connection_id(&self) -> u64 {
        self.id
    }
}
impl Drop for RosterGuard {
    fn drop(&mut self) {
        let remaining = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            inner.names.remove(&self.id);
            inner.clients.remove(&self.id);
            inner.names.len()
        };
        tracing::info!(surface = self.label.as_str(), remaining, "surface detached");
        // Count first, then the frame: a surface that just left must not
        // still be the notify owner on the frame this raise schedules.
        drop(self.subscription.take());
        if remaining > 0 {
            self.changes.raise();
        }
    }
}

struct Client {
    surface: String,
    instance: String,
    capable: bool,
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientPreferences {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub failure: bool,
    #[serde(default)]
    pub unresponsive: bool,
    #[serde(default)]
    pub long_success: bool,
    #[serde(default = "success_threshold")]
    pub long_success_seconds: f64,
    #[serde(default = "yes")]
    pub ask: bool,
    #[serde(default)]
    pub dnd_enabled: bool,
    #[serde(default)]
    pub dnd_start: i64,
    #[serde(default)]
    pub dnd_end: i64,
    #[serde(default)]
    pub utc_offset_minutes: i64,
    #[serde(default)]
    pub sound: bool,
    #[serde(default = "default_floor")]
    pub floor: u64,
    #[serde(default = "yes")]
    pub urgent: bool,
    #[serde(default)]
    pub muted_sources: Vec<String>,
    #[serde(default)]
    pub muted_projects: Vec<String>,
}
fn default_floor() -> u64 {
    2
}
fn yes() -> bool {
    true
}

fn success_threshold() -> f64 {
    300.0
}
